use crate::types::{Channel, FileInfo, Message, User};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const SLACK_API_BASE: &str = "https://slack.com/api";
const USER_CACHE_TTL: Duration = Duration::from_secs(600);

struct UserCache {
    users: HashMap<String, User>,
    updated_at: Option<Instant>,
}

#[derive(Clone)]
pub struct SlackApi {
    client: Client,
    user_cache: Arc<RwLock<UserCache>>,
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

    pub async fn test_auth(&self, token: &str) -> Result<(String, String)> {
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
            Ok((team_id, team))
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
        let response = self
            .client
            .get(format!("{}/conversations.history", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("channel", channel_id)])
            .query(&[("limit", limit.to_string())])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(anyhow!(
                "Failed to get history: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ));
        }

        let empty: Vec<serde_json::Value> = Vec::new();
        let messages = data
            .get("messages")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);
        let users_map = self.get_users_cached(token).await;

        Ok(messages
            .iter()
            .filter_map(|m| Message::from_slack_api(m, &users_map))
            .rev()
            .collect())
    }

    pub async fn send_message(&self, token: &str, channel_id: &str, text: &str) -> Result<String> {
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            data.get("ts")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| anyhow!("No ts in response"))
        } else {
            Err(anyhow!(
                "Failed to send message: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn send_message_to_thread(
        &self,
        token: &str,
        channel_id: &str,
        text: &str,
        thread_ts: &str,
    ) -> Result<String> {
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            data.get("ts")
                .and_then(|v| v.as_str())
                .map(String::from)
                .ok_or_else(|| anyhow!("No ts in response"))
        } else {
            Err(anyhow!(
                "Failed to send thread message: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn list_users(&self, token: &str) -> Result<Vec<User>> {
        let response = self
            .client
            .get(format!("{}/users.list", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let data: Value = response.json().await?;

        if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(anyhow!(
                "Failed to list users: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ));
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to update message: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn delete_message(&self, token: &str, channel_id: &str, ts: &str) -> Result<()> {
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to delete message: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn add_reaction(
        &self,
        token: &str,
        channel_id: &str,
        ts: &str,
        reaction: &str,
    ) -> Result<()> {
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to add reaction: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn remove_reaction(
        &self,
        token: &str,
        channel_id: &str,
        ts: &str,
        reaction: &str,
    ) -> Result<()> {
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

        let data: Value = response.json().await?;

        if data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to remove reaction: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ))
        }
    }

    pub async fn get_thread_replies(
        &self,
        token: &str,
        channel_id: &str,
        thread_ts: &str,
    ) -> Result<Vec<Message>> {
        let response = self
            .client
            .get(format!("{}/conversations.replies", SLACK_API_BASE))
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("channel", channel_id)])
            .query(&[("ts", thread_ts)])
            .send()
            .await?;

        let data: Value = response.json().await?;

        if !data.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
            return Err(anyhow!(
                "Failed to get thread replies: {:?}",
                data.get("error").and_then(|v| v.as_str())
            ));
        }

        let empty: Vec<serde_json::Value> = Vec::new();
        let messages = data
            .get("messages")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);
        let users_map = self.get_users_cached(token).await;

        Ok(messages
            .iter()
            .filter_map(|m| Message::from_slack_api(m, &users_map))
            .collect())
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

impl Default for SlackApi {
    fn default() -> Self {
        Self::new()
    }
}
