use super::*;

impl App {
    pub(super) fn switch_workspace(&mut self, idx: usize) {
        if idx < self.workspaces.len() {
            self.active_workspace = idx;
            self.channels = self.workspaces[idx].channels.clone();
            self.selected_channel = None;
            self.scroll_offset = 0;

            if let Some(ref mut session) = self.session {
                if let Some(ws) = self.workspaces.get(idx) {
                    session.set_active_workspace(&ws.workspace.team_id);
                    if let Err(e) = session.save() {
                        self.report_error("Failed to save workspace selection", e);
                    } else {
                        self.clear_error();
                    }
                }
            }
        }
    }

    pub(super) fn select_channel(&mut self, idx: usize) {
        self.selected_channel = Some(idx);
        self.scroll_offset = 0;

        if let Some(channel) = self.channels.get(idx) {
            let ws = self.workspaces.get(self.active_workspace);
            if let Some(ws) = ws {
                let channel_id = channel.id.clone();
                let token = ws.workspace.xoxp_token.clone();
                let api = self.slack_api.clone();
                self.spawn_app_task(async move {
                    match api.get_history(&token, &channel_id, 50).await {
                        Ok(messages) => AppAsyncEvent::ChannelHistoryLoaded {
                            channel_id,
                            messages,
                            error: None,
                        },
                        Err(e) => AppAsyncEvent::ChannelHistoryLoaded {
                            channel_id,
                            messages: Vec::new(),
                            error: Some(e.to_string()),
                        },
                    }
                });
            }
        }
    }

    pub(super) fn handle_input_submit(&mut self) -> Result<()> {
        let text = self.input.buffer.clone();
        if text.is_empty() {
            return Ok(());
        }

        match self.input.mode {
            InputMode::Normal => {
                if let Some(channel) = self.get_active_channel_id() {
                    if let Some(ws) = self.workspaces.get(self.active_workspace) {
                        let token = ws.workspace.xoxp_token.clone();
                        let context = "Failed to send message".to_string();
                        let api = self.slack_api.clone();
                        self.spawn_app_task(async move {
                            let error = api
                                .send_message(&token, &channel, &text)
                                .await
                                .err()
                                .map(|e| e.to_string());
                            AppAsyncEvent::SlackSendResult { context, error }
                        });
                    }
                }
            }
            InputMode::AgentCommand => {
                self.handle_agent_command(&text)?;
            }
            InputMode::AgentMention => {
                if let Some(channel) = self.get_active_channel_id() {
                    if let Some(ws) = self.workspaces.get(self.active_workspace) {
                        let token = ws.workspace.xoxp_token.clone();
                        let context = "Failed to send mention".to_string();
                        let api = self.slack_api.clone();
                        self.spawn_app_task(async move {
                            let error = api
                                .send_message(&token, &channel, &text)
                                .await
                                .err()
                                .map(|e| e.to_string());
                            AppAsyncEvent::SlackSendResult { context, error }
                        });
                    }
                }
            }
        }

        self.input.clear();
        Ok(())
    }

    pub(super) fn handle_agent_command(&mut self, text: &str) -> Result<()> {
        use slack_zc_agent::commands::{process_command, CommandType};

        let (cmd_name, args) = match process_command(text) {
            Some((cmd, args)) => (cmd, args),
            None => {
                return Ok(());
            }
        };

        let command = CommandType::from_command(&cmd_name, &args);

        let channel_id = self.get_active_channel_id().unwrap_or_default();
        let user_id = self
            .workspaces
            .get(self.active_workspace)
            .and_then(|ws| ws.workspace.user_id.clone())
            .unwrap_or_else(|| "UNKNOWN_USER".to_string());

        let payload = command.to_webhook_payload(&channel_id, &user_id);

        if let Some(ref mut runner) = self.agent_runner {
            if let Some(gateway) = runner.get_gateway().cloned() {
                self.agent_processing = true;
                let command_text = text.to_string();
                let channel = self.get_active_channel_id();
                let token = self
                    .workspaces
                    .get(self.active_workspace)
                    .map(|ws| ws.workspace.xoxp_token.clone());
                let thread_ts = channel
                    .as_ref()
                    .and_then(|ch| self.active_threads.get(ch).cloned());
                let api = self.slack_api.clone();
                self.spawn_app_task(async move {
                    let response =
                        match timeout(Duration::from_secs(15), gateway.send_to_agent(&payload))
                            .await
                        {
                            Ok(Ok(text)) => text,
                            Ok(Err(e)) => {
                                return AppAsyncEvent::AgentCommandFinished {
                                    command: command_text,
                                    response: None,
                                    error: Some(format!("Agent command failed: {}", e)),
                                }
                            }
                            Err(_) => {
                                return AppAsyncEvent::AgentCommandFinished {
                                    command: command_text,
                                    response: None,
                                    error: Some(
                                        "Agent command failed: timed out after 15s".to_string(),
                                    ),
                                }
                            }
                        };

                    if let (Some(channel_id), Some(xoxp_token)) = (channel, token) {
                        let post_result = if let Some(ts) = thread_ts {
                            api.send_message_to_thread(&xoxp_token, &channel_id, &response, &ts)
                                .await
                        } else {
                            api.send_message(&xoxp_token, &channel_id, &response).await
                        };
                        if let Err(e) = post_result {
                            return AppAsyncEvent::AgentCommandFinished {
                                command: command_text,
                                response: None,
                                error: Some(format!("Failed to post agent response: {}", e)),
                            };
                        }
                    }

                    AppAsyncEvent::AgentCommandFinished {
                        command: command_text,
                        response: Some(response),
                        error: None,
                    }
                });
            }
        } else {
            self.report_error("Agent command failed", "agent not connected");
        }

        Ok(())
    }
    pub(super) fn get_active_channel_id(&self) -> Option<String> {
        self.selected_channel
            .and_then(|idx| self.channels.get(idx).map(|ch| ch.id.clone()))
    }
    pub(super) fn toggle_thread_collapse(&mut self, channel_id: &str) {
        if let Some(threads) = self.threads.get_mut(channel_id) {
            for thread in threads.iter_mut() {
                thread.toggle_collapse();
            }
        }
    }

    pub(super) fn start_edit_message(&mut self) -> Result<()> {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    if let Some(msg) = messages.back() {
                        let current_user = self
                            .workspaces
                            .get(self.active_workspace)
                            .and_then(|ws| ws.workspace.user_id.clone());

                        if current_user.as_ref() == Some(&msg.user_id) {
                            self.edit_message = Some(EditState {
                                channel_id: ch.id.clone(),
                                ts: msg.ts.clone(),
                                original_text: msg.text.clone(),
                            });
                            self.input.buffer = msg.text.clone();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn delete_selected_message(&mut self) -> Result<()> {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    if let Some(msg) = messages.back() {
                        let current_user = self
                            .workspaces
                            .get(self.active_workspace)
                            .and_then(|ws| ws.workspace.user_id.clone());

                        if current_user.as_ref() == Some(&msg.user_id) {
                            if let Some(ws) = self.workspaces.get(self.active_workspace) {
                                let channel_id = ch.id.clone();
                                let ts = msg.ts.clone();
                                let token = ws.workspace.xoxp_token.clone();
                                let api = self.slack_api.clone();
                                self.spawn_app_task(async move {
                                    let error = api
                                        .delete_message(&token, &channel_id, &ts)
                                        .await
                                        .err()
                                        .map(|e| e.to_string());
                                    AppAsyncEvent::SlackSendResult {
                                        context: "Failed to delete message".to_string(),
                                        error,
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn show_reaction_picker(&mut self) -> Result<()> {
        self.context_menu = Some(ContextMenu {
            x: 10,
            y: 10,
            items: vec![
                ContextMenuItem {
                    label: "👍 +1".to_string(),
                    action: ContextMenuAction::React,
                },
                ContextMenuItem {
                    label: "❤️ heart".to_string(),
                    action: ContextMenuAction::React,
                },
                ContextMenuItem {
                    label: "😄 laugh".to_string(),
                    action: ContextMenuAction::React,
                },
                ContextMenuItem {
                    label: "😮 wow".to_string(),
                    action: ContextMenuAction::React,
                },
                ContextMenuItem {
                    label: "😢 sad".to_string(),
                    action: ContextMenuAction::React,
                },
                ContextMenuItem {
                    label: "😡 angry".to_string(),
                    action: ContextMenuAction::React,
                },
            ],
            selected: 0,
        });
        Ok(())
    }

    pub(super) fn copy_selected_message(&mut self) -> Result<()> {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    if let Some(msg) = messages.back() {
                        let clipped = if msg.text.chars().count() > 16_384 {
                            msg.text.chars().take(16_384).collect::<String>()
                        } else {
                            msg.text.clone()
                        };
                        #[cfg(target_os = "linux")]
                        {
                            let result = std::process::Command::new("xclip")
                                .arg("-selection")
                                .arg("clipboard")
                                .arg("-i")
                                .arg(&clipped)
                                .output();
                            match result {
                                Ok(output) if output.status.success() => self.clear_error(),
                                Ok(output) => self.report_error(
                                    "Failed to copy message to clipboard",
                                    format!("xclip exited with {}", output.status),
                                ),
                                Err(e) => {
                                    self.report_error("Failed to copy message to clipboard", e)
                                }
                            }
                        }
                        #[cfg(target_os = "macos")]
                        {
                            let result =
                                std::process::Command::new("pbcopy").arg(&clipped).output();
                            match result {
                                Ok(output) if output.status.success() => self.clear_error(),
                                Ok(output) => self.report_error(
                                    "Failed to copy message to clipboard",
                                    format!("pbcopy exited with {}", output.status),
                                ),
                                Err(e) => {
                                    self.report_error("Failed to copy message to clipboard", e)
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn handle_context_menu_action(&mut self) {
        if let Some(ref menu) = self.context_menu {
            let action = menu.items[menu.selected].action.clone();
            self.context_menu = None;

            match action {
                ContextMenuAction::Reply => {
                    if let Some(ref channel) = self.selected_channel {
                        if let Some(ch) = self.channels.get(*channel) {
                            if let Some(messages) = self.messages.get(&ch.id) {
                                if let Some(msg) = messages.back() {
                                    self.active_threads.insert(ch.id.clone(), msg.ts.clone());
                                }
                            }
                        }
                    }
                }
                ContextMenuAction::Edit => {
                    if let Err(e) = self.start_edit_message() {
                        self.report_error("Failed to start editing message", e);
                    }
                }
                ContextMenuAction::Delete => {
                    if let Err(e) = self.delete_selected_message() {
                        self.report_error("Failed to delete message", e);
                    }
                }
                ContextMenuAction::Copy => {
                    if let Err(e) = self.copy_selected_message() {
                        self.report_error("Failed to copy message", e);
                    }
                }
                ContextMenuAction::ViewThread => {
                    if let Some(ref channel) = self.selected_channel {
                        if let Some(ch) = self.channels.get(*channel) {
                            let channel_id = ch.id.clone();
                            self.load_thread(&channel_id);
                        }
                    }
                }
                ContextMenuAction::React => {
                    self.add_reaction_to_message("+1");
                }
            }
        }
    }

    pub(super) fn save_edited_message(&mut self) -> Result<()> {
        if let Some(ref edit_state) = self.edit_message {
            if let Some(ws) = self.workspaces.get(self.active_workspace) {
                let text = self.input.buffer.clone();
                let channel_id = edit_state.channel_id.clone();
                let ts = edit_state.ts.clone();
                let token = ws.workspace.xoxp_token.clone();
                let api = self.slack_api.clone();
                self.spawn_app_task(async move {
                    let error = api
                        .update_message(&token, &channel_id, &ts, &text)
                        .await
                        .err()
                        .map(|e| e.to_string());
                    AppAsyncEvent::SlackSendResult {
                        context: "Failed to update message".to_string(),
                        error,
                    }
                });
            }
            self.edit_message = None;
            self.input.clear();
        }
        Ok(())
    }

    pub(super) fn add_reaction_to_message(&mut self, reaction: &str) {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    if let Some(msg) = messages.back() {
                        if let Some(ws) = self.workspaces.get(self.active_workspace) {
                            let channel_id = ch.id.clone();
                            let ts = msg.ts.clone();
                            let token = ws.workspace.xoxp_token.clone();
                            let reaction = reaction.to_string();
                            let api = self.slack_api.clone();
                            self.spawn_app_task(async move {
                                let error = api
                                    .add_reaction(&token, &channel_id, &ts, &reaction)
                                    .await
                                    .err()
                                    .map(|e| e.to_string());
                                AppAsyncEvent::SlackSendResult {
                                    context: "Failed to add reaction".to_string(),
                                    error,
                                }
                            });
                        }
                    }
                }
            }
        }
    }

    pub(super) fn load_thread(&mut self, channel_id: &str) {
        let token = match self.workspaces.get(self.active_workspace) {
            Some(ws) => ws.workspace.xoxp_token.clone(),
            None => return,
        };

        let shared_api = self.slack_api.clone();
        if let Some(messages) = self.messages.get(channel_id).cloned() {
            for msg in messages.iter() {
                if msg.reply_count.is_some_and(|c| c > 0) {
                    let channel_id = channel_id.to_string();
                    let thread_ts = msg.ts.clone();
                    let token = token.clone();
                    let api = shared_api.clone();
                    self.spawn_app_task(async move {
                        match api
                            .get_thread_replies(&token, &channel_id, &thread_ts)
                            .await
                        {
                            Ok(replies) => AppAsyncEvent::ThreadRepliesLoaded {
                                channel_id,
                                parent_ts: thread_ts,
                                replies,
                                error: None,
                            },
                            Err(e) => AppAsyncEvent::ThreadRepliesLoaded {
                                channel_id,
                                parent_ts: thread_ts,
                                replies: Vec::new(),
                                error: Some(e.to_string()),
                            },
                        }
                    });
                }
            }
        }
    }

    pub(super) fn hit_test_message(&self, col: u16, row: u16) -> Option<(String, String)> {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    let layout = self.layout.get_panels();
                    for panel in layout {
                        if panel.panel_type == PanelType::Messages {
                            if col < panel.rect.x || col >= panel.rect.x + panel.rect.width {
                                continue;
                            }
                            let relative_row = row.saturating_sub(panel.rect.y);
                            let msg_index = (relative_row as usize + self.scroll_offset)
                                .saturating_sub(panel.rect.y as usize);

                            if let Some(msg) = messages.get(msg_index) {
                                return Some((ch.id.clone(), msg.ts.clone()));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub(super) fn jump_to_timestamp(&mut self) -> Result<()> {
        let target_ts = &self.jump_to_time_buffer;

        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(messages) = self.messages.get(&ch.id) {
                    for (idx, msg) in messages.iter().enumerate() {
                        let msg_time = msg.timestamp.format("%H:%M").to_string();
                        let msg_date = msg.timestamp.format("%Y-%m-%d").to_string();

                        if msg_time == *target_ts
                            || msg_date == *target_ts
                            || msg.ts.starts_with(target_ts)
                        {
                            self.scroll_offset = idx.saturating_sub(5);
                            return Ok(());
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub(super) fn load_history_for_date(&mut self) -> Result<()> {
        if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(ws) = self.workspaces.get(self.active_workspace) {
                    let channel_id = ch.id.clone();
                    let token = ws.workspace.xoxp_token.clone();

                    let api = self.slack_api.clone();
                    self.spawn_app_task(async move {
                        match api.get_history(&token, &channel_id, 100).await {
                            Ok(messages) => AppAsyncEvent::ChannelHistoryLoaded {
                                channel_id,
                                messages,
                                error: None,
                            },
                            Err(e) => AppAsyncEvent::ChannelHistoryLoaded {
                                channel_id,
                                messages: Vec::new(),
                                error: Some(e.to_string()),
                            },
                        }
                    });
                }
            }
        }
        Ok(())
    }
}
