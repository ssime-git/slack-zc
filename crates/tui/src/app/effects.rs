use super::*;
use std::time::Duration;
use tokio::time::sleep;

async fn with_init_retry<T, F, Fut>(operation: F, context: &str) -> Result<T, anyhow::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
    let mut attempts = 0;
    let max_attempts = 3;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts >= max_attempts {
                    return Err(anyhow::anyhow!(
                        "{} failed after {} attempts: {}",
                        context,
                        max_attempts,
                        e
                    ));
                }
                let delay = Duration::from_millis(500 * 2u64.pow(attempts - 1));
                tracing::warn!(
                    "{} attempt {}/{} failed: {}. Retrying in {:?}...",
                    context,
                    attempts,
                    max_attempts,
                    e,
                    delay
                );
                sleep(delay).await;
            }
        }
    }
}

impl App {
    pub async fn init(&mut self, _config: &Config) -> Result<()> {
        tracing::info!("Starting app initialization...");
        let mut session_opt = Session::load()?;

        if session_opt.is_some() {
            tracing::info!("Session loaded successfully");
        } else {
            tracing::warn!("No session found, checking environment variables...");
        }

        if session_opt.is_none() {
            if let (Ok(app_token), Ok(user_token)) = (
                std::env::var("SLACK_APP_TOKEN"),
                std::env::var("SLACK_USER_TOKENS"),
            ) {
                match self.slack_api.test_auth(&user_token).await {
                    Ok((team_id, team_name, user_id)) => {
                        let mut session = Session {
                            workspaces: Vec::new(),
                            zeroclaw_bearer: None,
                        };
                        let workspace = Workspace {
                            team_id,
                            team_name,
                            xoxp_token: user_token,
                            xapp_token: app_token,
                            user_id: Some(user_id),
                            active: true,
                            last_channel_id: None,
                        };
                        session.add_workspace(workspace);
                        if let Err(e) = session.save() {
                            tracing::error!("Failed to save session from env: {}", e);
                        } else {
                            session_opt = Some(session);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Env token auth test failed: {}", e);
                    }
                }
            }
        }

        if let Some(session) = session_opt {
            tracing::info!(
                "Initializing with {} workspace(s)",
                session.workspaces.len()
            );
            self.session = Some(session.clone());

            for (ws_idx, workspace) in session.workspaces.iter().enumerate() {
                tracing::info!(
                    "Workspace {}: {} (team_id: {})",
                    ws_idx,
                    workspace.team_name,
                    workspace.team_id
                );
                let mut ws_state = WorkspaceState::new(workspace.clone());
                let token = workspace.xoxp_token.clone();
                let api = self.slack_api.clone();

                // Test auth first
                match api.test_auth(&token).await {
                    Ok((_, team_name, _)) => {
                        tracing::info!("Auth test passed for {}", team_name);
                    }
                    Err(e) => {
                        tracing::error!("Auth test failed: {}", e);
                        self.report_error("Slack authentication failed", e);
                        continue;
                    }
                }

                if let Some(ref event_tx) = self.event_tx {
                    let socket_client = slack_zc_slack::socket::SocketModeClient::new(
                        workspace.xapp_token.clone(),
                        workspace.xoxp_token.clone(),
                        event_tx.clone(),
                    );
                    ws_state.socket_task = Some(tokio::spawn(async move {
                        socket_client.run().await;
                    }));
                }

                match crate::cache::load_workspace_channels(&workspace.team_id) {
                    Ok(Some(cached_channels)) => {
                        tracing::info!(
                            "Loaded {} cached channels for workspace {}",
                            cached_channels.len(),
                            workspace.team_name
                        );
                        ws_state.channels = cached_channels;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load cached channels for workspace {}: {}",
                            workspace.team_name,
                            e
                        );
                    }
                }

                self.workspaces.push(ws_state);

                let team_id = workspace.team_id.clone();
                let team_name = workspace.team_name.clone();
                let token = workspace.xoxp_token.clone();
                let api = self.slack_api.clone();
                let app_async_tx = self.app_async_tx.clone();
                self.spawn_app_task(async move {
                    tracing::info!("Loading channels for {} in background...", team_name);
                    let mut channel_cursor: Option<String> = None;
                    let mut dm_cursor: Option<String> = None;
                    let mut loaded_total = 0usize;
                    let Some(app_async_tx) = app_async_tx else {
                        return AppAsyncEvent::WorkspaceChannelsLoaded {
                            team_id,
                            channels: Vec::new(),
                            append: true,
                            done: true,
                            error: Some("Internal app event channel unavailable".to_string()),
                        };
                    };

                    loop {
                        match with_init_retry(
                            || async {
                                api.list_channels_page(&token, channel_cursor.as_deref())
                                    .await
                            },
                            "Channel loading",
                        )
                        .await
                        {
                            Ok((channels, next_cursor)) => {
                                loaded_total += channels.len();
                                tracing::info!(
                                    "Loaded {} regular channels for workspace {} (total: {})",
                                    channels.len(),
                                    team_name,
                                    loaded_total
                                );
                                let append = channel_cursor.is_some();
                                let done = false;
                                if !channels.is_empty() || !append || done {
                                    let _ = App::send_app_event(
                                        &app_async_tx,
                                        AppAsyncEvent::WorkspaceChannelsLoaded {
                                            team_id: team_id.clone(),
                                            channels,
                                            append,
                                            done,
                                            error: None,
                                        },
                                    );
                                }
                                if next_cursor.is_none() {
                                    break;
                                }
                                channel_cursor = next_cursor;
                            }
                            Err(e) => {
                                tracing::error!("Failed to load channels for {}: {}", team_name, e);
                                return AppAsyncEvent::WorkspaceChannelsLoaded {
                                    team_id,
                                    channels: Vec::new(),
                                    append: true,
                                    done: true,
                                    error: Some(format!("Failed to load channels: {}", e)),
                                };
                            }
                        }
                    }

                    loop {
                        match with_init_retry(
                            || async { api.list_dms_page(&token, dm_cursor.as_deref()).await },
                            "DM loading",
                        )
                        .await
                        {
                            Ok((dms, next_cursor)) => {
                                loaded_total += dms.len();
                                tracing::info!(
                                    "Loaded {} DMs for workspace {} (total: {})",
                                    dms.len(),
                                    team_name,
                                    loaded_total
                                );
                                let done = next_cursor.is_none();
                                if !dms.is_empty() || done {
                                    let _ = App::send_app_event(
                                        &app_async_tx,
                                        AppAsyncEvent::WorkspaceChannelsLoaded {
                                            team_id: team_id.clone(),
                                            channels: dms,
                                            append: true,
                                            done,
                                            error: None,
                                        },
                                    );
                                }
                                if next_cursor.is_none() {
                                    break;
                                }
                                dm_cursor = next_cursor;
                            }
                            Err(e) => {
                                tracing::error!("Failed to load DMs for {}: {}", team_name, e);
                                return AppAsyncEvent::WorkspaceChannelsLoaded {
                                    team_id,
                                    channels: Vec::new(),
                                    append: true,
                                    done: true,
                                    error: Some(format!("Failed to load DMs: {}", e)),
                                };
                            }
                        }
                    }

                    tracing::info!("Finished background channel loading for {}", team_name);
                    AppAsyncEvent::WorkspaceChannelsLoaded {
                        team_id,
                        channels: Vec::new(),
                        append: true,
                        done: true,
                        error: None,
                    }
                });
            }

            let active_team_id = session
                .workspaces
                .iter()
                .find(|w| w.active)
                .map(|w| w.team_id.clone());

            let resolved_active_idx = active_team_id
                .as_ref()
                .and_then(|team_id| {
                    self.workspaces
                        .iter()
                        .position(|ws| ws.workspace.team_id == *team_id)
                })
                .or_else(|| (!self.workspaces.is_empty()).then_some(0));

            if let Some(active_idx) = resolved_active_idx {
                self.active_workspace = active_idx;
                self.channels = self.workspaces[active_idx].channels.clone();
            } else {
                tracing::warn!("No workspace could be initialized successfully");
                self.channels.clear();
                self.selected_channel = None;
            }

            self.is_loading = false;
            self.clear_error();

            // Auto-start zeroclaw agent
            self.start_zeroclaw_auto();
        } else {
            self.onboarding = Some(OnboardingState::new());
            self.is_loading = false;
        }

        Ok(())
    }
    pub(super) fn start_zeroclaw_auto(&mut self) {
        if !self.config.zeroclaw.auto_start {
            return;
        }

        let binary_path = self.config.zeroclaw.binary_path.clone();
        let gateway_port = slack_zc_slack::auth::load_zeroclaw_gateway_port()
            .unwrap_or(self.config.zeroclaw.gateway_port);

        // Try to get bearer from session first, then from OpenClaw config
        let bearer = self
            .session
            .as_ref()
            .and_then(|s| s.zeroclaw_bearer.clone())
            .or_else(|| {
                tracing::info!("No bearer in session, trying to load from OpenClaw config");
                slack_zc_slack::auth::load_openclaw_bearer()
            });

        if bearer.is_some() {
            tracing::info!(
                "Found ZeroClaw bearer token (source: {})",
                if self
                    .session
                    .as_ref()
                    .and_then(|s| s.zeroclaw_bearer.clone())
                    .is_some()
                {
                    "session"
                } else {
                    "openclaw config"
                }
            );
        }

        self.agent_status = AgentStatus::Starting;
        self.spawn_app_task(async move {
            let mut runner = AgentRunner::new(binary_path, gateway_port);
            if let Err(e) = runner.check_binary().await {
                return AppAsyncEvent::ZeroClawConnectionFinished {
                    runner: None,
                    error: Some(format!("ZeroClaw binary not found: {}", e)),
                };
            }

            if let Some(bearer) = bearer {
                match runner.connect_with_bearer(&bearer).await {
                    Ok(_) => AppAsyncEvent::ZeroClawConnectionFinished {
                        runner: Some(runner),
                        error: None,
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Existing gateway connection with bearer failed ({}), attempting to start a new gateway",
                            e
                        );
                        match runner.start_with_bearer(&bearer).await {
                            Ok(_) => AppAsyncEvent::ZeroClawConnectionFinished {
                                runner: Some(runner),
                                error: None,
                            },
                            Err(start_err) => match runner.start_embedded_gateway().await {
                                Ok(_) => AppAsyncEvent::ZeroClawConnectionFinished {
                                    runner: Some(runner),
                                    error: None,
                                },
                                Err(embedded_err) => AppAsyncEvent::ZeroClawConnectionFinished {
                                    runner: None,
                                    error: Some(format!(
                                        "ZeroClaw bearer auth failed: {}; embedded gateway start failed: {}. Run `zeroclaw gateway --port {}` or refresh your ZeroClaw login with `zeroclaw onboard`.",
                                        start_err, embedded_err, gateway_port
                                    )),
                                },
                            },
                        }
                    }
                }
            } else {
                match runner.connect_to_running_gateway().await {
                    Ok(_) => AppAsyncEvent::ZeroClawConnectionFinished {
                        runner: Some(runner),
                        error: None,
                    },
                    Err(connect_err) => match runner.start_embedded_gateway().await {
                        Ok(_) => AppAsyncEvent::ZeroClawConnectionFinished {
                            runner: Some(runner),
                            error: None,
                        },
                        Err(embedded_err) => AppAsyncEvent::ZeroClawConnectionFinished {
                            runner: None,
                            error: Some(format!(
                                "ZeroClaw auto-connect failed: {}; embedded gateway start failed: {}. Run `zeroclaw onboard` to create local credentials, then start `zeroclaw gateway --port {}`.",
                                connect_err, embedded_err, gateway_port
                            )),
                        },
                    },
                }
            }
        });
    }

    pub(super) fn complete_oauth(&mut self, code: &str) -> Result<()> {
        if let Some(ref onboarding) = self.onboarding {
            let client_id = onboarding.client_id.clone();
            let client_secret = onboarding.client_secret.clone();
            let code = code.to_string();
            let redirect_port = self.config.slack.redirect_port;
            self.spawn_app_task(async move {
                let result = slack_zc_slack::auth::exchange_oauth_code(
                    &client_id,
                    &client_secret,
                    &code,
                    &format!("http://localhost:{}", redirect_port),
                )
                .await;

                match result {
                    Ok(response) => AppAsyncEvent::OAuthCompleted {
                        workspace: Some(Workspace {
                            team_id: response.team.id,
                            team_name: response.team.name,
                            xoxp_token: response.authed_user.access_token,
                            xapp_token: response.access_token,
                            user_id: Some(response.authed_user.id),
                            active: true,
                            last_channel_id: None,
                        }),
                        error: None,
                    },
                    Err(e) => AppAsyncEvent::OAuthCompleted {
                        workspace: None,
                        error: Some(App::actionable_error(&e)),
                    },
                }
            });
        }
        Ok(())
    }
    pub fn process_slack_events(&mut self) {
        if let Some(ref mut rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    SlackEvent::Message { channel, message } => {
                        if let Some(ref thread_ts) = message.thread_ts {
                            self.active_threads
                                .insert(channel.clone(), thread_ts.clone());
                            self.threads.entry(channel.clone()).or_default();
                        }
                        self.messages.entry(channel).or_default().push_back(message);
                    }
                    SlackEvent::UserTyping { channel, user } => {
                        tracing::debug!("User {} typing in {}", user, channel);
                        let channel_key = channel.clone();
                        let user_value = user.clone();
                        self.typing_users.entry(channel_key.clone()).or_default();
                        if let Some(users) = self.typing_users.get_mut(&channel_key) {
                            if !users.contains(&user_value) {
                                users.push(user_value);
                            }
                        }
                    }
                    SlackEvent::Connected => {
                        tracing::info!("Socket Mode connected");
                    }
                    SlackEvent::Disconnected => {
                        tracing::info!("Socket Mode disconnected");
                    }
                    _ => {}
                }
            }
        }

        let mut async_events = Vec::new();
        if let Some(ref mut rx) = self.app_async_rx {
            while let Ok(event) = rx.try_recv() {
                async_events.push(event);
            }
        }

        for event in async_events {
            match event {
                AppAsyncEvent::SlackSendResult {
                    context,
                    channel_id,
                    error,
                } => {
                    if let Some(err) = error {
                        self.report_error(&context, err);
                    } else {
                        self.clear_error();
                        if let Some(ch_id) = channel_id {
                            if let Some(ws) = self.workspaces.get(self.active_workspace) {
                                let token = ws.workspace.xoxp_token.clone();
                                let api = self.slack_api.clone();
                                self.spawn_app_task(async move {
                                    match api.get_history(&token, &ch_id, 50).await {
                                        Ok(messages) => AppAsyncEvent::ChannelHistoryLoaded {
                                            channel_id: ch_id,
                                            messages,
                                            error: None,
                                        },
                                        Err(e) => AppAsyncEvent::ChannelHistoryLoaded {
                                            channel_id: ch_id,
                                            messages: Vec::new(),
                                            error: Some(App::actionable_error(&e)),
                                        },
                                    }
                                });
                            }
                        }
                    }
                }
                AppAsyncEvent::ChannelHistoryLoaded {
                    channel_id,
                    messages,
                    error,
                } => {
                    if let Some(err) = error {
                        self.report_error("Failed to load channel history", err);
                    } else {
                        self.messages.insert(channel_id, messages.into());
                        self.clear_error();
                    }
                }
                AppAsyncEvent::ThreadRepliesLoaded {
                    channel_id,
                    parent_ts,
                    replies,
                    error,
                } => {
                    if let Some(err) = error {
                        self.report_error("Failed to load thread replies", err);
                    } else if !replies.is_empty() {
                        let threads = self.threads.entry(channel_id.clone()).or_default();
                        if let Some(existing) =
                            threads.iter_mut().find(|t| t.parent_ts == parent_ts)
                        {
                            existing.replies = replies;
                        } else {
                            let mut thread = Thread::new(&parent_ts, &channel_id);
                            thread.replies = replies;
                            threads.push(thread);
                        }
                        self.clear_error();
                    }
                }
                AppAsyncEvent::AgentCommandFinished {
                    command,
                    response,
                    error,
                } => {
                    self.agent_processing = false;
                    self.loading_start_time = None;
                    self.loading_command = None;
                    if let Some(err) = error {
                        self.report_error("Agent command failed", err);
                    } else if let Some(resp) = response {
                        self.agent_responses.push_front(AgentResponse {
                            command,
                            response: resp,
                            timestamp: Utc::now(),
                        });
                        if self.agent_responses.len() > 50 {
                            self.agent_responses.pop_back();
                        }
                        self.clear_error();
                    } else {
                        self.clear_error();
                    }
                }
                AppAsyncEvent::OAuthCompleted { workspace, error } => {
                    if let Some(err) = error {
                        self.report_error("OAuth completion failed", err.clone());
                        if let Some(ref mut onboarding) = self.onboarding {
                            onboarding.error_message = Some(err);
                        }
                    } else if let Some(workspace) = workspace {
                        let mut session = self.session.take().unwrap_or(Session {
                            workspaces: Vec::new(),
                            zeroclaw_bearer: None,
                        });
                        for w in &mut session.workspaces {
                            w.active = false;
                        }
                        session.add_workspace(workspace);
                        if let Err(e) = session.save() {
                            self.report_error("Failed to persist OAuth session", e);
                        } else {
                            self.session = Some(session);
                            if let Some(ref mut onboarding) = self.onboarding {
                                onboarding.error_message = None;
                                onboarding.next_screen();
                            }
                            self.clear_error();
                        }
                    }
                }
                AppAsyncEvent::WorkspaceChannelsLoaded {
                    team_id,
                    channels,
                    append,
                    done,
                    error,
                } => {
                    if let Some(ws_idx) = self
                        .workspaces
                        .iter()
                        .position(|ws| ws.workspace.team_id == team_id)
                    {
                        if append {
                            self.workspaces[ws_idx].channels.extend(channels.clone());
                        } else {
                            self.workspaces[ws_idx].channels = channels.clone();
                        }
                        tracing::info!(
                            "Workspace {} channels updated: {} entries (done: {})",
                            self.workspaces[ws_idx].workspace.team_name,
                            self.workspaces[ws_idx].channels.len(),
                            done
                        );

                        if ws_idx == self.active_workspace {
                            if append {
                                self.channels.extend(channels);
                            } else {
                                self.channels = channels;
                            }
                            if self.sidebar_cursor >= self.channels.len()
                                && !self.channels.is_empty()
                            {
                                self.sidebar_cursor = self.channels.len() - 1;
                            }

                            if self.selected_channel.is_none() {
                                if let Some(last_channel_id) =
                                    self.workspaces[ws_idx].workspace.last_channel_id.clone()
                                {
                                    if let Some(channel_idx) =
                                        self.channels.iter().position(|c| c.id == last_channel_id)
                                    {
                                        self.sidebar_cursor = channel_idx;
                                        self.selected_channel = Some(channel_idx);
                                        let channel_id = last_channel_id;
                                        let token =
                                            self.workspaces[ws_idx].workspace.xoxp_token.clone();
                                        let api = self.slack_api.clone();
                                        self.spawn_app_task(async move {
                                            match api.get_history(&token, &channel_id, 50).await {
                                                Ok(messages) => {
                                                    AppAsyncEvent::ChannelHistoryLoaded {
                                                        channel_id,
                                                        messages,
                                                        error: None,
                                                    }
                                                }
                                                Err(e) => AppAsyncEvent::ChannelHistoryLoaded {
                                                    channel_id,
                                                    messages: Vec::new(),
                                                    error: Some(App::actionable_error(&e)),
                                                },
                                            }
                                        });
                                    } else if !self.channels.is_empty() && done {
                                        self.sidebar_cursor =
                                            self.sidebar_cursor.min(self.channels.len() - 1);
                                        self.selected_channel = Some(self.sidebar_cursor);
                                        let channel_id =
                                            self.channels[self.sidebar_cursor].id.clone();
                                        let token =
                                            self.workspaces[ws_idx].workspace.xoxp_token.clone();
                                        let api = self.slack_api.clone();
                                        self.spawn_app_task(async move {
                                            match api.get_history(&token, &channel_id, 50).await {
                                                Ok(messages) => {
                                                    AppAsyncEvent::ChannelHistoryLoaded {
                                                        channel_id,
                                                        messages,
                                                        error: None,
                                                    }
                                                }
                                                Err(e) => AppAsyncEvent::ChannelHistoryLoaded {
                                                    channel_id,
                                                    messages: Vec::new(),
                                                    error: Some(App::actionable_error(&e)),
                                                },
                                            }
                                        });
                                    }
                                } else if !self.channels.is_empty() && done {
                                    self.sidebar_cursor =
                                        self.sidebar_cursor.min(self.channels.len() - 1);
                                }
                            }
                        }
                    }

                    if let Some(err) = error {
                        self.report_error("Workspace channel refresh failed", err);
                    } else {
                        if let Some(ws_idx) = self
                            .workspaces
                            .iter()
                            .position(|ws| ws.workspace.team_id == team_id)
                        {
                            if let Err(e) = crate::cache::save_workspace_channels(
                                &team_id,
                                &self.workspaces[ws_idx].channels,
                            ) {
                                tracing::warn!(
                                    "Failed to save cached channels for workspace {}: {}",
                                    self.workspaces[ws_idx].workspace.team_name,
                                    e
                                );
                            } else if done {
                                tracing::info!(
                                    "Saved {} cached channels for workspace {}",
                                    self.workspaces[ws_idx].channels.len(),
                                    self.workspaces[ws_idx].workspace.team_name
                                );
                            } else {
                                tracing::debug!(
                                    "Updated cached channels for workspace {}: {} entries",
                                    self.workspaces[ws_idx].workspace.team_name,
                                    self.workspaces[ws_idx].channels.len()
                                );
                            }
                        }
                        self.clear_error();
                    }
                }
                AppAsyncEvent::ZeroClawConnectionFinished { runner, error } => {
                    if let Some(err) = error {
                        self.agent_status = AgentStatus::Error(err.clone());
                        self.report_error("ZeroClaw connection failed", err);
                    } else if let Some(runner) = runner {
                        self.agent_status = AgentStatus::Active;

                        if let Some(gateway) = runner.get_gateway() {
                            if let Some(bearer) = gateway.get_bearer() {
                                if let Some(ref mut session) = self.session {
                                    session.zeroclaw_bearer = Some(bearer.clone());
                                    if let Err(e) = session.save() {
                                        tracing::error!("Failed to save zeroclaw bearer: {}", e);
                                    }
                                }
                            }
                        }

                        self.agent_runner = Some(runner);
                        self.clear_error();
                    }
                }
            }
        }
    }
}
