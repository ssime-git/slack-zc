use crate::types::{Channel, FileInfo, Message, User};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use rand::Rng;

const SLACK_API_BASE: &str = "https://slack.com/api";
const USER_CACHE_TTL: Duration = Duration::from_secs(600);
const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 30_000;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_after_rate_limit() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: Result<&str, _> = with_retry(move || {
            let attempt_count = attempt_count_clone.clone();
            async move {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(anyhow!("429 retry_after:0"))
                } else {
                    Ok("success")
                }
            }
        }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: Result<&str, _> = with_retry(move || {
            let attempt_count = attempt_count_clone.clone();
            async move {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("429 retry_after:0"))
            }
        }).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_does_not_retry_non_rate_limit_errors() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: Result<&str, _> = with_retry(move || {
            let attempt_count = attempt_count_clone.clone();
            async move {
                attempt_count.fetch_add(1, Ordering::SeqCst);
                Err(anyhow!("some other error"))
            }
        }).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_calculate_backoff_increases_with_attempts() {
        let delay_0 = calculate_backoff(0);
        let delay_4 = calculate_backoff(4);
        assert!(delay_4 > delay_0);
    }

    #[test]
    fn test_calculate_backoff_is_capped() {
        let delay = calculate_backoff(20);
        assert!(delay.as_millis() <= MAX_BACKOFF_MS as u128 + 500);
    }

    #[tokio::test]
    async fn test_retry_on_transient_network_error() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: Result<&str, _> = with_retry(move || {
            let attempt_count = attempt_count_clone.clone();
            async move {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(anyhow!("connection reset by peer"))
                } else {
                    Ok("success")
                }
            }
        }).await;

        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_retry_after_extracts_seconds() {
        assert_eq!(parse_retry_after("rate_limited retry_after:30 more"), Some(30));
        assert_eq!(parse_retry_after("no header here"), None);
    }

    #[tokio::test]
    async fn test_user_cache_returns_cached_users() {
        let api = SlackApi::new();
        
        let users1 = api.get_users_cached("fake_token").await;
        let users2 = api.get_users_cached("fake_token").await;
        
        assert_eq!(users1.len(), users2.len());
    }
}

struct UserCache {
    users: HashMap<String, User>,
    updated_at: Option<Instant>,
}

#[derive(Clone)]
pub struct SlackApi {
    client: Client,
    user_cache: Arc<RwLock<UserCache>>,
}

impl Default for SlackApi {
    fn default() -> Self {
        Self::new()
    }
}

enum RetryDecision {
    Retry(Duration),
    Fail,
}

fn calculate_backoff(attempt: u32) -> Duration {
    let jitter = rand::thread_rng().gen_range(0..500);
    let exponential = BASE_DELAY_MS * 2u64.pow(attempt);
    Duration::from_millis((exponential + jitter).min(MAX_BACKOFF_MS))
}

fn retry_decision(error: &anyhow::Error) -> RetryDecision {
    let msg = error.to_string();
    if msg.contains("429") || msg.contains("rate_limited") {
        if let Some(after) = parse_retry_after(&msg) {
            return RetryDecision::Retry(Duration::from_secs(after));
        }
        return RetryDecision::Retry(Duration::from_secs(60));
    }
    if is_transient_network_error(error) {
        return RetryDecision::Retry(Duration::ZERO);
    }
    RetryDecision::Fail
}

fn parse_retry_after(msg: &str) -> Option<u64> {
    let prefix = "retry_after:";
    let pos = msg.find(prefix)?;
    msg[pos + prefix.len()..]
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
}

fn is_transient_network_error(error: &anyhow::Error) -> bool {
    if let Some(req_err) = error.downcast_ref::<reqwest::Error>() {
        return req_err.is_connect() || req_err.is_timeout() || req_err.is_request();
    }
    let msg = error.to_string().to_lowercase();
    msg.contains("connection") || msg.contains("timeout") || msg.contains("timed out")
        || msg.contains("reset") || msg.contains("eof")
}

async fn with_retry<T, F, Fut>(mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempts >= MAX_RETRIES {
                    return Err(e);
                }
                match retry_decision(&e) {
                    RetryDecision::Fail => return Err(e),
                    RetryDecision::Retry(override_delay) => {
                        let delay = if override_delay.is_zero() {
                            calculate_backoff(attempts)
                        } else {
                            override_delay
                        };
                        tracing::debug!(attempt = attempts, ?delay, "Retrying after error: {e}");
                        tokio::time::sleep(delay).await;
                        attempts += 1;
                    }
                }
            }
        }
    }
}

impl SlackApi {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("slack-zc/0.2")
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            user_cache: Arc::new(RwLock::new(UserCache {
                users: HashMap::new(),
                updated_at: None,
            })),
        }
    }

    async fn get_users_cached(&self, token: &str) -> HashMap<String, User> {
        {
            let cache = self.user_cache.read().await;
            if let Some(updated_at) = cache.updated_at {
                if updated_at.elapsed() < USER_CACHE_TTL {
                    return cache.users.clone();
                }
            }
        }
        let mut cache = self.user_cache.write().await;
        // Double-check after acquiring write lock
        if let Some(updated_at) = cache.updated_at {
            if updated_at.elapsed() < USER_CACHE_TTL {
                return cache.users.clone();
            }
        }
        match self.list_users(token).await {
            Ok(users) => {
                let users_map: HashMap<String, User> =
                    users.into_iter().map(|u| (u.id.clone(), u)).collect();
                cache.users = users_map.clone();
                cache.updated_at = Some(Instant::now());
                users_map
            }
            Err(_) => cache.users.clone(),
        }
    }

    pub async fn test_auth(&self, token: &str) -> Result<(String, String, String)> {
        let response = self
            .client
            .post(format!("{}/auth.test", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let team_id = data
                .get("team_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let team = data
                .get("team")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let user_id = data
                .get("user_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Ok((team_id, team, user_id))
        } else {
            Err(anyhow!(
                "Auth test failed: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn list_channels(&self, token: &str) -> Result<Vec<Channel>> {
        let response = self
            .client
            .get(format!("{}/conversations.list", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("types", "public_channel,private_channel")])
            .query(&[("exclude_archived", "true")])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(anyhow!(
                "Failed to list channels: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ));
        }

        let empty: Vec<serde_json::Value> = Vec::new();
        let channels = data
            .get("channels")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);

        Ok(channels
            .iter()
            .filter_map(|c| {
                Some(Channel {
                    id: c.get("id")?.as_str()?.to_string(),
                    name: c.get("name")?.as_str()?.to_string(),
                    is_dm: false,
                    is_group: c.get("is_group").and_then(|v| v.as_bool()).unwrap_or(false),
                    is_im: false,
                    unread_count: 0,
                    purpose: c
                        .get("purpose")
                        .and_then(|p| p.get("value"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    topic: c
                        .get("topic")
                        .and_then(|t| t.get("value"))
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    user: None,
                })
            })
            .collect())
    }

    pub async fn list_dms(&self, token: &str) -> Result<Vec<Channel>> {
        let response = self
            .client
            .get(format!("{}/conversations.list", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("types", "im")])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(anyhow!(
                "Failed to list DMs: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ));
        }

        let empty: Vec<serde_json::Value> = Vec::new();
        let channels = data
            .get("channels")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);

        Ok(channels
            .iter()
            .filter_map(|c| {
                let name = c.get("user")?.as_str().map(|s| s.to_string())?;
                Some(Channel {
                    id: c.get("id")?.as_str()?.to_string(),
                    name,
                    is_dm: true,
                    is_group: false,
                    is_im: true,
                    unread_count: 0,
                    purpose: None,
                    topic: None,
                    user: c.get("user").and_then(|v| v.as_str()).map(String::from),
                })
            })
            .collect())
    }

    pub async fn get_history(
        &self,
        token: &str,
        channel_id: &str,
        limit: u32,
    ) -> Result<Vec<Message>> {
        let channel_id = channel_id.to_string();
        let token = token.to_string();
        
        with_retry(move || {
            let channel_id = channel_id.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .get(format!("{}/conversations.history", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .query(&[("channel", channel_id.as_str())])
                    .query(&[("limit", limit.to_string())])
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    return Err(anyhow!("Failed to get history: {}", error_msg));
                }

                let empty: Vec<serde_json::Value> = Vec::new();
                let messages = data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty);
                let users_map = self.get_users_cached(&token).await;

                Ok(messages
                    .iter()
                    .filter_map(|m| Message::from_slack_api(m, &users_map))
                    .rev()
                    .collect())
            }
        }).await
    }

    pub async fn send_message(&self, token: &str, channel_id: &str, text: &str) -> Result<String> {
        let channel_id = channel_id.to_string();
        let text = text.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let text = text.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/chat.postMessage", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "text": text,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    data.get("ts")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .ok_or_else(|| anyhow!("No ts in response"))
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to send message: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn send_message_to_thread(
        &self,
        token: &str,
        channel_id: &str,
        text: &str,
        thread_ts: &str,
    ) -> Result<String> {
        let channel_id = channel_id.to_string();
        let text = text.to_string();
        let thread_ts = thread_ts.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let text = text.clone();
            let thread_ts = thread_ts.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/chat.postMessage", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "text": text,
                        "thread_ts": thread_ts,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    data.get("ts")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .ok_or_else(|| anyhow!("No ts in response"))
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to send thread message: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn list_users(&self, token: &str) -> Result<Vec<User>> {
        let token = token.to_string();

        with_retry(move || {
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .get(format!("{}/users.list", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    return Err(anyhow!("Failed to list users: {}", error_msg));
                }

                let empty: Vec<serde_json::Value> = Vec::new();
                let members = data
                    .get("members")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty);

                Ok(members
                    .iter()
                    .filter_map(|u| {
                        let profile = u.get("profile")?;
                        Some(User {
                            id: u.get("id")?.as_str()?.to_string(),
                            name: u.get("name")?.as_str()?.to_string(),
                            display_name: profile
                                .get("display_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            real_name: profile
                                .get("real_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            email: profile
                                .get("email")
                                .and_then(|v| v.as_str())
                                .map(String::from),
                        })
                    })
                    .collect())
            }
        }).await
    }

    pub async fn get_user(&self, token: &str, user_id: &str) -> Result<User> {
        let response = self
            .client
            .get(format!("{}/users.info", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("user", user_id)])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let user = data
                .get("user")
                .ok_or_else(|| anyhow!("No user in response"))?;
            let profile = user
                .get("profile")
                .ok_or_else(|| anyhow!("No profile in response"))?;

            Ok(User {
                id: user
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: user
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                display_name: profile
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                real_name: profile
                    .get("real_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                email: profile
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
        } else {
            Err(anyhow!(
                "Failed to get user: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn get_socket_mode_url(&self, xapp_token: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/apps.connections.open", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", xapp_token))
            .send()
            .await?;

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            data.get("url")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| anyhow!("No URL in response"))
        } else {
            Err(anyhow!(
                "Failed to get socket mode URL: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn update_message(
        &self,
        token: &str,
        channel_id: &str,
        ts: &str,
        text: &str,
    ) -> Result<()> {
        let channel_id = channel_id.to_string();
        let ts = ts.to_string();
        let text = text.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let ts = ts.clone();
            let text = text.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/chat.update", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "ts": ts,
                        "text": text,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Ok(())
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to update message: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn delete_message(&self, token: &str, channel_id: &str, ts: &str) -> Result<()> {
        let channel_id = channel_id.to_string();
        let ts = ts.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let ts = ts.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/chat.delete", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "ts": ts,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Ok(())
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to delete message: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn add_reaction(
        &self,
        token: &str,
        channel_id: &str,
        ts: &str,
        reaction: &str,
    ) -> Result<()> {
        let channel_id = channel_id.to_string();
        let ts = ts.to_string();
        let reaction = reaction.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let ts = ts.clone();
            let reaction = reaction.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/reactions.add", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "timestamp": ts,
                        "name": reaction,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Ok(())
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to add reaction: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn remove_reaction(
        &self,
        token: &str,
        channel_id: &str,
        ts: &str,
        reaction: &str,
    ) -> Result<()> {
        let channel_id = channel_id.to_string();
        let ts = ts.to_string();
        let reaction = reaction.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let ts = ts.clone();
            let reaction = reaction.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .post(format!("{}/reactions.remove", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .json(&serde_json::json!({
                        "channel": channel_id,
                        "timestamp": ts,
                        "name": reaction,
                    }))
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    Ok(())
                } else {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    Err(anyhow!("Failed to remove reaction: {}", error_msg))
                }
            }
        }).await
    }

    pub async fn get_thread_replies(
        &self,
        token: &str,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<Message>> {
        let channel_id = channel_id.to_string();
        let thread_ts = thread_ts.to_string();
        let token = token.to_string();

        with_retry(move || {
            let channel_id = channel_id.clone();
            let thread_ts = thread_ts.clone();
            let token = token.clone();
            async move {
                let response = self
                    .client
                    .get(format!("{}/conversations.replies", SLACK_API_BASE))
                    .header("Authorization", format!("Bearer {}", token))
                    .query(&[("channel", channel_id.as_str())])
                    .query(&[("ts", thread_ts.as_str())])
                    .send()
                    .await?;

                let status = response.status();
                let data: Value = response.json().await?;

                if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let error_msg = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if error_msg == "rate_limited" || status.as_u16() == 429 {
                        return Err(anyhow!("429"));
                    }
                    return Err(anyhow!("Failed to get thread replies: {}", error_msg));
                }

                let empty: Vec<serde_json::Value> = Vec::new();
                let messages = data
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&empty);
                let users_map = self.get_users_cached(&token).await;

                Ok(messages
                    .iter()
                    .filter_map(|m| Message::from_slack_api(m, &users_map))
                    .collect())
            }
        }).await
    }

    pub async fn upload_file(
        &self,
        token: &str,
        channel_id: &str,
        file_path: &str,
        title: Option<&str>,
        comment: Option<&str>,
    ) -> Result<String> {
        let file_content = tokio::fs::read(file_path).await?;
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        let channel_id_owned = channel_id.to_string();
        let title_owned = title
            .map(|t| t.to_string())
            .unwrap_or_else(|| file_name.clone());

        let mut form = reqwest::multipart::Form::new()
            .text("channels", channel_id_owned)
            .text("title", title_owned)
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_content).file_name(file_name),
            );

        if let Some(c) = comment {
            form = form.text("initial_comment", c.to_string());

            let response = self
                .client
                .post(format!("{}/files.upload", SLACK_API_BASE))
                .header("Authorization", format!("Bearer {}", token))
                .multipart(form)
                .send()
                .await?;

            let data: Value = response.json().await?;

            if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                return data
                    .get("file")
                    .and_then(|f| f.get("id"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .ok_or_else(|| anyhow!("No file id in response"));
            } else {
                return Err(anyhow!(
                    "Failed to upload file: {:?}",
                    data.get("error").and_then(|v| v.as_str())
                ));
            }
        }

        let response = self
            .client
            .post(format!("{}/files.upload", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .multipart(form)
            .send()
            .await?;

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            data.get("file")
                .and_then(|f| f.get("id"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| anyhow!("No file id in response"))
        } else {
            Err(anyhow!(
                "Failed to upload file: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn get_file_info(&self, token: &str, file_id: &str) -> Result<FileInfo> {
        let response = self
            .client
            .get(format!("{}/files.info", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("file", file_id)])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            let file = data
                .get("file")
                .ok_or_else(|| anyhow!("No file in response"))?;

            Ok(FileInfo {
                id: file
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: file
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                mimetype: file
                    .get("mimetype")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                url_private: file
                    .get("url_private")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                url_private_download: file
                    .get("url_private_download")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                size: file.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                title: file.get("title").and_then(|v| v.as_str()).map(String::from),
                filetype: file
                    .get("filetype")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
        } else {
            Err(anyhow!(
                "Failed to get file info: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn download_file(&self, url: &str, token: &str, dest_path: &str) -> Result<()> {
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let bytes = response.bytes().await?;
        tokio::fs::write(dest_path, bytes).await?;

        Ok(())
    }
}
