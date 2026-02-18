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

        Some(Self {
            ts,
            user_id,
            username,
            text,
            thread_ts,
            timestamp,
            is_agent: false,
            reactions: Vec::new(),
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
