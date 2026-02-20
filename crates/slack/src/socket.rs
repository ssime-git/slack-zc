use crate::api::SlackApi;
use crate::types::Message;
use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use tracing::{debug, error, info, warn};

const USER_CACHE_TTL: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub enum SlackEvent {
    Message { channel: String, message: Message },
    UserTyping { channel: String, user: String },
    ChannelJoined { channel: String },
    ChannelLeft { channel: String },
    Connected,
    Disconnected,
}

pub struct SocketModeClient {
    api: SlackApi,
    xapp_token: String,
    xoxp_token: String,
    event_tx: mpsc::UnboundedSender<SlackEvent>,
    user_display_names: RwLock<HashMap<String, String>>,
    user_cache_updated_at: RwLock<Option<Instant>>,
}

impl SocketModeClient {
    pub fn new(
        xapp_token: String,
        xoxp_token: String,
        event_tx: mpsc::UnboundedSender<SlackEvent>,
    ) -> Self {
        Self {
            api: SlackApi::new(),
            xapp_token,
            xoxp_token,
            event_tx,
            user_display_names: RwLock::new(HashMap::new()),
            user_cache_updated_at: RwLock::new(None),
        }
    }

    pub async fn run(self) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);

        loop {
            match self.connect_and_listen().await {
                Ok(()) => {
                    info!("Socket mode connection closed gracefully");
                    backoff = Duration::from_secs(1);
                }
                Err(e) => {
                    error!("Socket mode error: {}. Reconnecting in {:?}", e, backoff);
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, max_backoff);
                }
            }
        }
    }

    async fn connect_and_listen(&self) -> Result<()> {
        let url = self.api.get_socket_mode_url(&self.xapp_token).await?;
        info!(
            "Connecting to Socket Mode at {}",
            Self::redact_socket_url(&url)
        );

        let (ws_stream, _) = connect_async(&url).await?;
        info!("WebSocket connected");

        let _ = self.event_tx.send(SlackEvent::Connected);

        let (mut write, mut read) = ws_stream.split();

        loop {
            match timeout(Duration::from_secs(60), read.next()).await {
                Ok(Some(Ok(WsMessage::Text(text)))) => {
                    debug!("Received websocket frame ({} bytes)", text.len());

                    if let Err(e) = self.handle_message(&text).await {
                        warn!("Error handling message: {}", e);
                    }

                    if let Ok(data) = serde_json::from_str::<Value>(&text) {
                        if let Some(envelope_id) = data.get("envelope_id").and_then(|v| v.as_str())
                        {
                            let ack = serde_json::json!({
                                "envelope_id": envelope_id,
                            });
                            write.send(WsMessage::Text(ack.to_string().into())).await?;
                        }
                    }
                }
                Ok(Some(Ok(WsMessage::Close(_)))) => {
                    info!("WebSocket closed by server");
                    break;
                }
                Ok(Some(Err(e))) => {
                    return Err(anyhow!("WebSocket error: {}", e));
                }
                Ok(None) => {
                    info!("WebSocket stream ended");
                    break;
                }
                Err(_) => {
                    debug!("Ping timeout");
                }
                _ => {}
            }
        }

        let _ = self.event_tx.send(SlackEvent::Disconnected);
        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        let data: Value = serde_json::from_str(text)?;

        if data.get("type").and_then(|v| v.as_str()) == Some("hello") {
            info!("Socket mode handshake successful");
            return Ok(());
        }

        if data.get("type").and_then(|v| v.as_str()) == Some("disconnect") {
            return Err(anyhow!("Server requested disconnect"));
        }

        let payload = data.get("payload").ok_or_else(|| anyhow!("No payload"))?;
        let event = payload.get("event").ok_or_else(|| anyhow!("No event"))?;
        let event_type = event.get("type").and_then(|v| v.as_str());

        match event_type {
            Some("message") => {
                if event.get("subtype").is_none() {
                    if let Some((channel, message)) = self.parse_message(event).await {
                        let _ = self.event_tx.send(SlackEvent::Message { channel, message });
                    }
                }
            }
            Some("user_typing") => {
                let channel = event.get("channel").and_then(|v| v.as_str());
                let user = event.get("user").and_then(|v| v.as_str());
                if let (Some(ch), Some(u)) = (channel, user) {
                    let _ = self.event_tx.send(SlackEvent::UserTyping {
                        channel: ch.to_string(),
                        user: u.to_string(),
                    });
                }
            }
            Some("member_joined_channel") => {
                let channel = event.get("channel").and_then(|v| v.as_str());
                if let Some(ch) = channel {
                    let _ = self.event_tx.send(SlackEvent::ChannelJoined {
                        channel: ch.to_string(),
                    });
                }
            }
            Some("member_left_channel") => {
                let channel = event.get("channel").and_then(|v| v.as_str());
                if let Some(ch) = channel {
                    let _ = self.event_tx.send(SlackEvent::ChannelLeft {
                        channel: ch.to_string(),
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn parse_message(&self, event: &Value) -> Option<(String, Message)> {
        let ts = event.get("ts")?.as_str()?.to_string();
        let user_id = event.get("user")?.as_str()?.to_string();
        let text = event.get("text")?.as_str()?.to_string();
        let channel = event.get("channel")?.as_str()?.to_string();

        let username = self.resolve_username(&user_id).await;

        let message = Message {
            ts,
            user_id,
            username,
            text,
            thread_ts: event
                .get("thread_ts")
                .and_then(|t| t.as_str())
                .map(String::from),
            timestamp: chrono::DateTime::from_timestamp(
                event
                    .get("ts")?
                    .as_str()?
                    .split('.')
                    .next()?
                    .parse::<i64>()
                    .ok()?,
                0,
            )?,
            is_agent: false,
            reactions: Vec::new(),
            is_edited: false,
            is_deleted: false,
            files: Vec::new(),
            reply_count: None,
            last_read: None,
        };

        Some((channel, message))
    }

    fn redact_socket_url(url: &str) -> String {
        url.split('?')
            .next()
            .unwrap_or("wss://slack-gateway")
            .to_string()
    }

    async fn resolve_username(&self, user_id: &str) -> String {
        {
            let cache = self.user_display_names.read().await;
            if let Some(name) = cache.get(user_id) {
                return name.clone();
            }
        }

        if self.should_refresh_user_cache().await {
            if let Err(e) = self.refresh_user_cache().await {
                debug!("Failed to refresh user cache: {}", e);
            }
        }

        let cache = self.user_display_names.read().await;
        cache
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| user_id.to_string())
    }

    async fn should_refresh_user_cache(&self) -> bool {
        let updated_at = *self.user_cache_updated_at.read().await;
        match updated_at {
            Some(ts) => ts.elapsed() >= USER_CACHE_TTL,
            None => true,
        }
    }

    async fn refresh_user_cache(&self) -> Result<()> {
        let users = self.api.list_users(&self.xoxp_token).await?;
        let next_cache: HashMap<String, String> = users
            .into_iter()
            .map(|u| {
                let display_name = u.display_name();
                (u.id, display_name)
            })
            .collect();

        {
            let mut cache = self.user_display_names.write().await;
            *cache = next_cache;
        }

        let mut updated_at = self.user_cache_updated_at.write().await;
        *updated_at = Some(Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_redact_socket_url_removes_query_params() {
        let url = "wss://example.com?token=abc123&team=T123";
        let redacted = SocketModeClient::redact_socket_url(url);
        assert!(!redacted.contains("token"));
        assert!(!redacted.contains("abc123"));
    }

    #[test]
    fn test_slack_event_enum_variants() {
        let _event1 = SlackEvent::Connected;
        let _event2 = SlackEvent::Disconnected;
        let _event3 = SlackEvent::Message {
            channel: "C123".to_string(),
            message: Message {
                ts: "123.456".to_string(),
                user_id: "U123".to_string(),
                username: "test".to_string(),
                text: "Hello".to_string(),
                thread_ts: None,
                timestamp: chrono::Utc::now(),
                is_agent: false,
                reactions: Vec::new(),
                is_edited: false,
                is_deleted: false,
                files: Vec::new(),
                reply_count: None,
                last_read: None,
            },
        };
    }

    #[tokio::test]
    async fn test_channel_event_variants() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = SocketModeClient::new(
            "xapp-test".to_string(),
            "xoxp-test".to_string(),
            tx,
        );
        
        let event = SlackEvent::ChannelJoined {
            channel: "C123".to_string(),
        };
        
        match event {
            SlackEvent::ChannelJoined { .. } => {}
            _ => panic!("Expected ChannelJoined variant"),
        }
    }
}
