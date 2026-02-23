use crate::input::{InputMode, InputState};
use crate::keybinds::Keybinds;
use crate::onboarding::{OnboardingScreen, OnboardingState};
use crate::ui::layout::{DragTarget, LayoutState};
use crate::ui::panel::PanelType;
use crate::Config;
use anyhow::Result;
use chrono::Utc;
use ratatui::crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use ratatui::Frame;
use slack_zc_agent::{AgentRunner, AgentStatus};
use slack_zc_slack::api::SlackApi;
use slack_zc_slack::auth::Session;
use slack_zc_slack::socket::SlackEvent;
use slack_zc_slack::types::{Channel, Message, Thread, Workspace, WorkspaceState};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

mod actions;
mod effects;
mod input;
mod render;
mod state;
mod types;

pub use state::{App, ChannelPicker, ConfirmationDialog, Focus};
pub use types::{
    AgentResponse, AppAsyncEvent, ContextMenu, ContextMenuAction, ContextMenuItem, EditState,
    MessageFilter,
};

impl App {
    pub(super) fn report_error(&mut self, context: &str, error: impl std::fmt::Display) {
        let message = format!("{context}: {}", Self::redact_sensitive(&error.to_string()));
        self.last_error = Some(message.clone());
        tracing::warn!("{message}");
    }

    pub(super) fn actionable_error(error: &anyhow::Error) -> String {
        slack_zc_slack::error::map_anyhow_error_ref(error).user_message().to_string()
    }

    pub(super) fn clear_error(&mut self) {
        self.last_error = None;
        self.show_error_details = false;
    }

    fn redact_sensitive(input: &str) -> String {
        input
            .replace("xoxp-", "xoxp-[REDACTED]-")
            .replace("xapp-", "xapp-[REDACTED]-")
            .replace("xoxb-", "xoxb-[REDACTED]-")
            .replace("Bearer ", "Bearer [REDACTED]")
    }

    pub(super) fn spawn_app_task<F>(&self, future: F)
    where
        F: Future<Output = AppAsyncEvent> + Send + 'static,
    {
        if let Some(tx) = self.app_async_tx.clone() {
            tokio::spawn(async move {
                let event = future.await;
                let _ = tx.send(event);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::Config;
    use chrono::Utc;
    use slack_zc_slack::socket::SlackEvent;
    use slack_zc_slack::types::Message;

    fn sample_message(thread_ts: Option<&str>) -> Message {
        Message {
            ts: "1730000000.100000".to_string(),
            user_id: "U123".to_string(),
            username: "tester".to_string(),
            text: "hello".to_string(),
            thread_ts: thread_ts.map(str::to_string),
            timestamp: Utc::now(),
            is_agent: false,
            reactions: Vec::new(),
            is_edited: false,
            is_deleted: false,
            files: Vec::new(),
            reply_count: None,
            last_read: None,
        }
    }

    #[test]
    fn routes_messages_to_their_source_channel() {
        let mut app = App::new(Config::default());
        let tx = app.event_tx.as_ref().expect("event tx").clone();

        tx.send(SlackEvent::Message {
            channel: "C_ONE".to_string(),
            message: sample_message(None),
        })
        .expect("send first event");
        tx.send(SlackEvent::Message {
            channel: "C_TWO".to_string(),
            message: sample_message(Some("1730000000.100000")),
        })
        .expect("send second event");

        app.process_slack_events();

        assert_eq!(app.messages.get("C_ONE").map(|m| m.len()), Some(1));
        assert_eq!(app.messages.get("C_TWO").map(|m| m.len()), Some(1));
    }

    #[test]
    fn tracks_thread_context_per_channel() {
        let mut app = App::new(Config::default());
        let tx = app.event_tx.as_ref().expect("event tx").clone();

        tx.send(SlackEvent::Message {
            channel: "C_ONE".to_string(),
            message: sample_message(Some("1000.1")),
        })
        .expect("send first thread event");
        tx.send(SlackEvent::Message {
            channel: "C_TWO".to_string(),
            message: sample_message(Some("2000.2")),
        })
        .expect("send second thread event");

        app.process_slack_events();

        assert_eq!(
            app.active_threads.get("C_ONE").map(String::as_str),
            Some("1000.1")
        );
        assert_eq!(
            app.active_threads.get("C_TWO").map(String::as_str),
            Some("2000.2")
        );
    }
}
