use super::*;

impl App {
    pub async fn init(&mut self, _config: &Config) -> Result<()> {
        let mut session_opt = Session::load()?;

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
            self.session = Some(session.clone());

            for workspace in &session.workspaces {
                let mut ws_state = WorkspaceState::new(workspace.clone());

                match self.slack_api.list_channels(&workspace.xoxp_token).await {
                    Ok(channels) => ws_state.channels = channels,
                    Err(e) => self.report_error("Failed to load channels", e),
                }


                // Load DMs in addition to channels
                match self.slack_api.list_dms(&workspace.xoxp_token).await {
                    Ok(dms) => {
                        ws_state.channels.extend(dms);
                    }
                    Err(e) => self.report_error("Failed to load DMs", e),
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

                self.workspaces.push(ws_state);
            }

            if let Some(active_idx) = session.workspaces.iter().position(|w| w.active) {
                self.active_workspace = active_idx;
                self.channels = self.workspaces[active_idx].channels.clone();
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
        let gateway_port = self.config.zeroclaw.gateway_port;
        let bearer = self
            .session
            .as_ref()
            .and_then(|s| s.zeroclaw_bearer.clone());

        self.agent_status = AgentStatus::Starting;
        self.spawn_app_task(async move {
            let mut runner = AgentRunner::new(binary_path, gateway_port);
            if let Err(e) = runner.check_binary().await {
                return AppAsyncEvent::ZeroClawPairingFinished {
                    runner: None,
                    error: Some(format!("ZeroClaw binary not found: {}", e)),
                };
            }

            if let Some(bearer) = bearer {
                match runner.start_with_bearer(&bearer).await {
                    Ok(_) => AppAsyncEvent::ZeroClawPairingFinished {
                        runner: Some(runner),
                        error: None,
                    },
                    Err(e) => {
                        // Bearer expired, fall back to pairing
                        tracing::warn!("Bearer auth failed ({}), attempting fresh pairing", e);
                        match runner.start_and_pair().await {
                            Ok(_) => AppAsyncEvent::ZeroClawPairingFinished {
                                runner: Some(runner),
                                error: None,
                            },
                            Err(e) => AppAsyncEvent::ZeroClawPairingFinished {
                                runner: None,
                                error: Some(format!("ZeroClaw pairing failed: {}", e)),
                            },
                        }
                    }
                }
            } else {
                match runner.start_and_pair().await {
                    Ok(_) => AppAsyncEvent::ZeroClawPairingFinished {
                        runner: Some(runner),
                        error: None,
                    },
                    Err(e) => AppAsyncEvent::ZeroClawPairingFinished {
                        runner: None,
                        error: Some(format!("ZeroClaw pairing failed: {}", e)),
                    },
                }
            }
        });
    }

    pub(super) fn start_zeroclaw_pairing(&mut self) {
        let binary_path = self.config.zeroclaw.binary_path.clone();
        let gateway_port = self.config.zeroclaw.gateway_port;
        self.agent_status = AgentStatus::Pairing;
        self.spawn_app_task(async move {
            let mut runner = AgentRunner::new(binary_path, gateway_port);
            if let Err(e) = runner.check_binary().await {
                return AppAsyncEvent::ZeroClawPairingFinished {
                    runner: None,
                    error: Some(format!("ZeroClaw startup failed: {}", e)),
                };
            }
            if let Err(e) = runner.start_and_pair().await {
                return AppAsyncEvent::ZeroClawPairingFinished {
                    runner: None,
                    error: Some(format!("ZeroClaw pairing failed: {}", e)),
                };
            }
            AppAsyncEvent::ZeroClawPairingFinished {
                runner: Some(runner),
                error: None,
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
                        self.messages
                            .entry(channel)
                            .or_default()
                            .push_back(message);
                    }
                    SlackEvent::UserTyping { channel, user } => {
                        tracing::debug!("User {} typing in {}", user, channel);
                        let channel_key = channel.clone();
                        let user_value = user.clone();
                        self.typing_users
                            .entry(channel_key.clone())
                            .or_default();
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
                AppAsyncEvent::SlackSendResult { context, error } => {
                    if let Some(err) = error {
                        self.report_error(&context, err);
                    } else {
                        self.clear_error();
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
                        let threads = self
                            .threads
                            .entry(channel_id.clone())
                            .or_default();
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
                AppAsyncEvent::ZeroClawPairingFinished { runner, error } => {
                    if let Some(err) = error {
                        self.agent_status = AgentStatus::Error(err.clone());
                        self.report_error("ZeroClaw pairing failed", err);
                    } else if let Some(runner) = runner {
                        self.agent_status = AgentStatus::Active;
                        self.agent_runner = Some(runner);
                        self.clear_error();
                    }
                }
            }
        }
    }
}
