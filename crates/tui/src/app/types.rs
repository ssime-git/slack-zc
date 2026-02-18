#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub command: String,
    pub response: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub enum AppAsyncEvent {
    SlackSendResult {
        context: String,
        error: Option<String>,
    },
    ChannelHistoryLoaded {
        channel_id: String,
        messages: Vec<slack_zc_slack::types::Message>,
        error: Option<String>,
    },
    ThreadRepliesLoaded {
        channel_id: String,
        parent_ts: String,
        replies: Vec<slack_zc_slack::types::Message>,
        error: Option<String>,
    },
    AgentCommandFinished {
        command: String,
        response: Option<String>,
        error: Option<String>,
    },
    OAuthCompleted {
        workspace: Option<slack_zc_slack::types::Workspace>,
        error: Option<String>,
    },
    ZeroClawPairingFinished {
        runner: Option<slack_zc_agent::AgentRunner>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub x: u16,
    pub y: u16,
    pub items: Vec<ContextMenuItem>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct ContextMenuItem {
    pub label: String,
    pub action: ContextMenuAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    Reply,
    React,
    Edit,
    Delete,
    Copy,
    ViewThread,
}

#[derive(Debug, Clone)]
pub struct EditState {
    pub channel_id: String,
    pub ts: String,
    pub original_text: String,
}

#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub user_id: Option<String>,
    pub show_threads: bool,
}

impl Default for MessageFilter {
    fn default() -> Self {
        Self {
            user_id: None,
            show_threads: true,
        }
    }
}
