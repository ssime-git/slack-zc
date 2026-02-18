use crate::api::SlackApi;
use crate::types::Message;
use anyhow::{anyhow, Result};
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use tracing::{debug, error, info, warn};

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
        info!("Connecting to Socket Mode: {}", url);

        let (ws_stream, _) = connect_async(&url).await?;
        info!("WebSocket connected");

        let _ = self.event_tx.send(SlackEvent::Connected);

        let (mut write, mut read) = ws_stream.split();

        loop {
            match timeout(Duration::from_secs(60), read.next()).await {
                Ok(Some(Ok(WsMessage::Text(text)))) => {
                    debug!("Received: {}", text);

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

        let users = self
            .api
            .list_users(&self.xoxp_token)
            .await
            .unwrap_or_default();
        let users_map: HashMap<String, crate::types::User> =
            users.into_iter().map(|u| (u.id.clone(), u)).collect();

        let username = users_map
            .get(&user_id)
            .map(|u| u.display_name())
            .unwrap_or_else(|| user_id.clone());

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
        };

        Some((channel, message))
    }
}
