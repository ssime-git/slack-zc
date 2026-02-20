use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub is_dm: bool,
    pub is_group: bool,
    pub is_im: bool,
    pub unread_count: u32,
    pub purpose: Option<String>,
    pub topic: Option<String>,
    pub user: Option<String>,
}

impl Channel {
    pub fn display_name(&self) -> String {
        if self.is_dm {
            format!("@ {}", self.name)
        } else {
            format!("# {}", self.name)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub ts: String,
    pub user_id: String,
    pub username: String,
    pub text: String,
    pub thread_ts: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub is_agent: bool,
    pub reactions: Vec<Reaction>,
    pub is_edited: bool,
    pub is_deleted: bool,
    pub files: Vec<File>,
    pub reply_count: Option<u32>,
    pub last_read: Option<String>,
}

impl Message {
    pub fn from_slack_api(msg: &serde_json::Value, users: &HashMap<String, User>) -> Option<Self> {
        let ts = msg.get("ts")?.as_str()?.to_string();
        let user_id = msg.get("user")?.as_str()?.to_string();
        let username = users
            .get(&user_id)
            .map(|u| u.display_name())
            .unwrap_or_else(|| user_id.clone());
        let text = msg.get("text")?.as_str()?.to_string();
        let thread_ts = msg
            .get("thread_ts")
            .and_then(|t| t.as_str())
            .map(String::from);
        let timestamp = DateTime::from_timestamp(ts.split('.').next()?.parse::<i64>().ok()?, 0)?;

        let reactions: Vec<Reaction> = msg
            .get("reactions")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(Reaction {
                            name: r.get("name")?.as_str()?.to_string(),
                            count: r.get("count")?.as_u64()? as u32,
                            users: r
                                .get("users")
                                .and_then(|u| u.as_array())
                                .map(|users| {
                                    users
                                        .iter()
                                        .filter_map(|u| u.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let is_edited = msg.get("edited").is_some();
        let is_deleted = msg.get("deleted_at").is_some()
            || msg
                .get("is_deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        let files: Vec<File> = msg
            .get("files")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        Some(File {
                            id: f.get("id")?.as_str()?.to_string(),
                            name: f.get("name")?.as_str()?.to_string(),
                            mimetype: f.get("mimetype").and_then(|m| m.as_str()).map(String::from),
                            url_private: f
                                .get("url_private")
                                .and_then(|u| u.as_str())
                                .map(String::from),
                            url_private_download: f
                                .get("url_private_download")
                                .and_then(|u| u.as_str())
                                .map(String::from),
                            size: f.get("size")?.as_u64()? as u32,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let reply_count = msg
            .get("reply_count")
            .and_then(|r| r.as_u64())
            .map(|v| v as u32);

        let last_read = msg
            .get("last_read")
            .and_then(|r| r.as_str())
            .map(String::from);

        Some(Self {
            ts,
            user_id,
            username,
            text,
            thread_ts,
            timestamp,
            is_agent: false,
            reactions,
            is_edited,
            is_deleted,
            files,
            reply_count,
            last_read,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub name: String,
    pub count: u32,
    pub users: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub id: String,
    pub name: String,
    pub mimetype: Option<String>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub id: String,
    pub name: String,
    pub mimetype: Option<String>,
    pub url_private: Option<String>,
    pub url_private_download: Option<String>,
    pub size: u32,
    pub title: Option<String>,
    pub filetype: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Thread {
    pub parent_ts: String,
    pub channel_id: String,
    pub replies: Vec<Message>,
    pub is_collapsed: bool,
}

impl Thread {
    pub fn new(parent_ts: &str, channel_id: &str) -> Self {
        Self {
            parent_ts: parent_ts.to_string(),
            channel_id: channel_id.to_string(),
            replies: Vec::new(),
            is_collapsed: false,
        }
    }

    pub fn toggle_collapse(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub real_name: String,
    pub email: Option<String>,
}

impl User {
    pub fn display_name(&self) -> String {
        if !self.display_name.is_empty() {
            self.display_name.clone()
        } else if !self.real_name.is_empty() {
            self.real_name.clone()
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub team_id: String,
    pub team_name: String,
    pub xoxp_token: String,
    pub xapp_token: String,
    #[serde(default)]
    pub user_id: Option<String>,
    pub active: bool,
}

#[derive(Debug)]
pub struct WorkspaceState {
    pub workspace: Workspace,
    pub channels: Vec<Channel>,
    pub active_channel: Option<String>,
    pub users: HashMap<String, User>,
    pub socket_task: Option<tokio::task::JoinHandle<()>>,
}

impl WorkspaceState {
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            channels: Vec::new(),
            active_channel: None,
            users: HashMap::new(),
            socket_task: None,
        }
    }
}
