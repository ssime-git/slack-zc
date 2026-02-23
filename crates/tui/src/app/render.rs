use super::*;

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        if self.is_loading {
            self.render_loading(frame, area);
            return;
        }

        if let Some(ref onboarding) = self.onboarding {
            self.render_onboarding(frame, area, onboarding);
            return;
        }

        if self.show_help {
            self.render_help(frame, area);
            return;
        }

        if self.show_workspace_picker {
            self.render_workspace_picker(frame, area);
            return;
        }

        if self.show_channel_search {
            self.render_channel_search(frame, area);
            return;
        }

        self.layout.calculate_layout(area);

        let panels = self.layout.get_panels().to_vec();

        for panel in panels {
            match panel.panel_type {
                PanelType::Topbar => self.render_topbar(frame, panel.rect),
                PanelType::Sidebar => self.render_sidebar(frame, panel.rect),
                PanelType::Messages => self.render_messages(frame, panel.rect),
                PanelType::AgentPanel => self.render_agent_panel(frame, panel.rect),
                PanelType::InputBar => self.render_input_bar(frame, panel.rect),
            }
        }

        if let Some(ref context_menu) = self.context_menu {
            self.render_context_menu(frame, area, context_menu);
        }

        if let Some(ref edit_state) = self.edit_message {
            self.render_edit_message(frame, area, edit_state);
        }

        if self.show_jump_to_time {
            self.render_jump_to_time(frame, area);
        }

        if self.show_error_details {
            self.render_error_details(frame, area);
        }
    }

    fn render_loading(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};
        let text = format!("\n\n  {}  \n\n", self.loading_message);
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title(" slack-zc "))
            .centered();
        frame.render_widget(paragraph, area);
    }

    fn render_onboarding(&self, frame: &mut Frame, area: Rect, state: &OnboardingState) {
        use ratatui::widgets::{Block, Borders, Paragraph};

        let content = match state.current_screen {
            OnboardingScreen::Welcome => {
                "\n\n  Welcome to slack-zc!\n\n  A terminal Slack client with ZeroClaw AI integration.\n\n  This wizard will help you set up:\n    1. Slack workspace connection\n    2. ZeroClaw agent pairing\n\n  Press [Enter] to continue, [Esc] to quit\n".to_owned()
            }
            OnboardingScreen::SlackCredentials => {
                let client_id_display = if state.selected_field == 0 {
                    format!("{} [editing]", if state.client_id.is_empty() { "[not set]" } else { &state.client_id })
                } else {
                    if state.client_id.is_empty() { "[not set]" } else { &state.client_id }.to_string()
                };
                let client_secret_display = if state.selected_field == 1 {
                    format!("{} [editing]", if state.client_secret.is_empty() { "[not set]" } else { "********" })
                } else {
                    if state.client_secret.is_empty() { "[not set]" } else { "********" }.to_string()
                };
                format!(
                    "\n\n  Enter your Slack App credentials:\n\n  Client ID:    {}\n  Client Secret: {}\n\n  Press [Tab] to switch fields,\n  type to enter values,\n  [Enter] to continue, [Esc] to go back\n",
                    client_id_display,
                    client_secret_display
                )
            }
            OnboardingScreen::OAuthFlow => {
                if state.oauth_code.is_empty() {
                    if let Some(ref url) = state.oauth_url {
                        format!(
                            "\n\n  OAuth authentication:\n\n  1. Visit: {}\n\n  2. Authorize the app\n\n  3. Copy the code from URL and enter below:\n\n  Code: [enter code here]\n\n  Press [Enter] to exchange code for tokens,\n  [c] to copy URL to clipboard,\n  [Esc] to go back\n",
                            url
                        )
                    } else {
                        "\n\n  OAuth authentication:\n\n  Press [Enter] to generate OAuth URL,\n  or [Esc] to go back\n".to_owned()
                    }
                } else if let Some(ref url) = state.oauth_url {
                    format!(
                        "\n\n  OAuth authentication:\n\n  1. Visit: {}\n\n  2. Authorize the app\n\n  3. Your code: {}\n\n  Press [Enter] to exchange code for tokens,\n  [c] to copy URL,\n  [Esc] to go back\n",
                        url,
                        state.oauth_code
                    )
                } else {
                    "\n\n  OAuth authentication:\n\n  Press [Enter] to generate OAuth URL,\n  or [Esc] to go back\n".to_owned()
                }
            }
            OnboardingScreen::ZeroClawCheck => {
                "\n\n  ZeroClaw Agent Setup:\n\n  Checking for ZeroClaw binary...\n\n  If not found, install with:\n    curl -LsSf ... | sh\n\n  Press [Enter] to continue, [Esc] to go back\n".to_owned()
            }
            OnboardingScreen::ZeroClawPairing => {
                format!(
                    "\n\n  Pairing with ZeroClaw gateway:\n\n  Code: {}\n\n  Check the terminal where zeroclaw is running\n  for the 6-digit pairing code.\n\n  Press [Enter] to continue, [Esc] to go back\n",
                    state.pairing_code.as_deref().unwrap_or("waiting...")
                )
            }
            OnboardingScreen::Complete => {
                "\n\n  Setup Complete!\n\n  You are now ready to use slack-zc.\n\n  Press [Enter] to launch the main interface.\n\n".to_owned()
            }
        };

        let title = match state.current_screen {
            OnboardingScreen::Welcome => "Welcome",
            OnboardingScreen::SlackCredentials => "Slack Credentials",
            OnboardingScreen::OAuthFlow => "OAuth Flow",
            OnboardingScreen::ZeroClawCheck => "ZeroClaw Check",
            OnboardingScreen::ZeroClawPairing => "ZeroClaw Pairing",
            OnboardingScreen::Complete => "Complete!",
        };

        let paragraph = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Onboarding - {} ", title)),
            )
            .centered();
        frame.render_widget(paragraph, area);
    }

    fn render_help(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};
        let help_text = self.keybinds.help_text();
        let popup_area = self.centered_rect(60, 70, area);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(help_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Help - Press ? to close "),
            ),
            popup_area,
        );
    }

    fn render_workspace_picker(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
        let popup_area = self.centered_rect(50, 50, area);

        let items: Vec<ListItem> = self
            .workspaces
            .iter()
            .enumerate()
            .map(|(i, ws)| {
                let prefix = if i == self.active_workspace {
                    "> "
                } else {
                    "  "
                };
                ListItem::new(format!("{}{}", prefix, ws.workspace.team_name))
            })
            .collect();

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title(" Workspaces ")),
            popup_area,
        );
    }

    fn render_channel_search(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};
        let popup_area = self.centered_rect(50, 10, area);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(format!("Search: {}", self.search_query)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Channel Search "),
            ),
            popup_area,
        );
    }

    fn render_topbar(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Paragraph};

        let workspace_tabs: Vec<String> = self
            .workspaces
            .iter()
            .enumerate()
            .map(|(i, ws)| {
                if i == self.active_workspace {
                    format!("[{}]", ws.workspace.team_name)
                } else {
                    format!(" {} ", ws.workspace.team_name)
                }
            })
            .collect();

        let agent_indicator = match self.agent_status {
            AgentStatus::Active => "zeroclaw: ● active",
            _ => "zeroclaw: ○ inactive",
        };

        let typing_indicator = if let Some(ref channel) = self.selected_channel {
            if let Some(ch) = self.channels.get(*channel) {
                if let Some(users) = self.typing_users.get(&ch.id) {
                    if !users.is_empty() {
                        let typing_names: Vec<String> = users.iter().take(3).cloned().collect();
                        let typing_str = typing_names.join(", ");
                        if users.len() > 3 {
                            format!(" typing: {}...", typing_str)
                        } else {
                            format!(" typing: {}", typing_str)
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let focus_indicator = match self.focus {
            Focus::Sidebar => "[sidebar]",
            Focus::Messages => "[messages]",
            Focus::Input => "[input]",
        };

        let text = format!(
            " ● {}{}   {}{}   {}   [Tab] focus   [?] help",
            workspace_tabs.join(" "),
            typing_indicator,
            agent_indicator,
            if self.last_error.is_some() {
                "   ⚠ error"
            } else {
                ""
            },
            focus_indicator,
        );

        frame.render_widget(Paragraph::new(text).block(Block::default()), area);
    }

    fn render_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::widgets::{Block, Borders, List, ListItem};

        let is_focused = self.focus == Focus::Sidebar;

        // Visible rows inside the border (height minus 2 for borders, minus 1 for header).
        let visible_rows = area.height.saturating_sub(3) as usize;

        // Keep the cursor visible by adjusting sidebar_scroll.
        if visible_rows > 0 {
            if self.sidebar_cursor < self.sidebar_scroll {
                self.sidebar_scroll = self.sidebar_cursor;
            } else if self.sidebar_cursor >= self.sidebar_scroll + visible_rows {
                self.sidebar_scroll = self.sidebar_cursor + 1 - visible_rows;
            }
        }

        let mut items: Vec<ListItem> = vec![];

        let channels_title = if self.search_query.is_empty() {
            " CHANNELS ".to_string()
        } else {
            format!(" CHANNELS [{}] ", self.search_query)
        };
        items.push(
            ListItem::new(channels_title).style(
                Style::default().add_modifier(Modifier::BOLD),
            ),
        );

        // Filter channels by search query
        let filtered_channels: Vec<_> = if self.search_query.is_empty() {
            self.channels.clone()
        } else {
            let query = self.search_query.to_lowercase();
            self.channels.iter()
                .filter(|ch| ch.name.to_lowercase().contains(&query) || (ch.user.as_ref().map_or(false, |u| u.to_lowercase().contains(&query))))
                .cloned()
                .collect()
        };

        // Adjust sidebar_cursor if out of bounds
        if self.sidebar_cursor >= filtered_channels.len() && !filtered_channels.is_empty() {
            self.sidebar_cursor = filtered_channels.len() - 1;
        } else if filtered_channels.is_empty() {
            self.sidebar_cursor = 0;
        }

        let end = (self.sidebar_scroll + visible_rows).min(filtered_channels.len());
        for i in self.sidebar_scroll..end {
            let channel = &filtered_channels[i];
            let is_selected = Some(i) == self.selected_channel;
            let is_cursor = i == self.sidebar_cursor && is_focused;

            let prefix = if is_cursor && is_selected {
                ">> "
            } else if is_cursor {
                " > "
            } else if is_selected {
                " # "
            } else {
                "   "
            };

            let name = channel.display_name();
            let unread = if channel.unread_count > 0 {
                format!(" {}", channel.unread_count)
            } else {
                String::new()
            };

            let style = if is_cursor {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };

            items.push(ListItem::new(format!("{}{}{}", prefix, name, unread)).style(style));
        }

        let border_style = if is_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        frame.render_widget(
            List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Channels ")
                    .border_style(border_style),
            ),
            area,
        );
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let content = if let Some(ref channel) = self.selected_channel {
            self.channels.get(*channel).and_then(|ch| {
                self.messages.get(&ch.id).map(|msgs| {
                    let mut lines: Vec<String> = Vec::new();

                    for m in msgs.iter() {
                        if let Some(ref user_id) = self.message_filter.user_id {
                            if &m.user_id != user_id {
                                continue;
                            }
                        }

                        if m.is_deleted {
                            lines
                                .push(format!("{} [message deleted]", m.timestamp.format("%H:%M")));
                            continue;
                        }

                        let thread_indicator = if m.thread_ts.is_some() {
                            "  ↳ "
                        } else if m.reply_count.is_some_and(|c| c > 0) {
                            "  ⇩ "
                        } else {
                            ""
                        };

                        let edited_indicator = if m.is_edited { " (edited)" } else { "" };

                        let mut line = format!(
                            "{}{} {}{}: {}",
                            thread_indicator,
                            m.timestamp.format("%H:%M"),
                            m.username,
                            edited_indicator,
                            m.text
                        );

                        if !m.reactions.is_empty() {
                            let reactions_str: Vec<String> = m
                                .reactions
                                .iter()
                                .map(|r| format!("{}:{}", r.name, r.count))
                                .collect();
                            line.push_str(&format!(" [{}]", reactions_str.join(" ")));
                        }

                        if let Some(reply_count) = m.reply_count {
                            if reply_count > 0 {
                                line.push_str(&format!(" (+{} replies)", reply_count));
                            }
                        }

                        lines.push(line);

                        if self.message_filter.show_threads {
                            if let Some(thread_key) = m.thread_ts.clone().or(Some(m.ts.clone())) {
                                if let Some(threads) = self.threads.get(&ch.id) {
                                    if let Some(thread) =
                                        threads.iter().find(|t| t.parent_ts == thread_key)
                                    {
                                        if !thread.is_collapsed {
                                            for reply in &thread.replies {
                                                let reply_line = format!(
                                                    "    ↳ {} {}: {}",
                                                    reply.timestamp.format("%H:%M"),
                                                    reply.username,
                                                    reply.text
                                                );
                                                lines.push(reply_line);
                                            }
                                        } else {
                                            lines.push(format!(
                                                "    [{} replies - press t to expand]",
                                                thread.replies.len()
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    lines.join("\n")
                })
            })
        } else {
            None
        };

        let text = content.unwrap_or_else(|| "Select a channel to view messages".to_string());

        let border_style = if self.focus == Focus::Messages {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        frame.render_widget(
            Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(border_style),
                )
                .scroll((self.scroll_offset as u16, 0)),
            area,
        );
    }

    fn render_agent_panel(&self, frame: &mut Frame, area: Rect) {
        use ratatui::layout::Alignment;
        use ratatui::widgets::{Block, Borders, Paragraph};

        if let Some(ref dialog) = self.confirmation_dialog {
            self.render_confirmation_dialog(frame, area, dialog);
            return;
        }

        if let (Some(start_time), Some(cmd)) = (self.loading_start_time, &self.loading_command) {
            let elapsed = start_time.elapsed().as_secs();
            let loading_text = format!("Processing {}... ({}s)", cmd, elapsed);

            frame.render_widget(
                Paragraph::new(loading_text)
                    .block(Block::default().borders(Borders::ALL).title(" Agent "))
                    .alignment(Alignment::Center),
                area,
            );
            return;
        }

        let status = match self.agent_status {
            AgentStatus::Unavailable => "⚠ unavailable",
            AgentStatus::Starting => "▶ starting...",
            AgentStatus::Pairing => "⚙ pairing...",
            AgentStatus::Active => "● active",
            AgentStatus::Error(ref e) => &format!("✗ {}", e),
        };

        let mut text = format!("⚡ ZEROCLAW\n\nStatus: {}\n\n", status);

        text.push_str("Commands:\n");
        text.push_str("  /résume [#channel]\n");
        text.push_str("  /draft [intent]\n");
        text.push_str("  /cherche [text]\n\n");

        if !self.agent_responses.is_empty() {
            text.push_str("── Recent ──\n");
            for resp in self.agent_responses.iter().take(5) {
                let time = resp.timestamp.format("%H:%M").to_string();
                text.push_str(&format!(
                    "{} {}: {}\n",
                    time,
                    resp.command,
                    if resp.response.len() > 30 {
                        &resp.response[..30]
                    } else {
                        &resp.response
                    }
                ));
            }
        }

        frame.render_widget(
            Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Agent ")),
            area,
        );
    }

    fn render_confirmation_dialog(&self, frame: &mut Frame, area: Rect, dialog: &ConfirmationDialog) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        frame.render_widget(Clear, area);

        let context = dialog.context_channel.as_deref().unwrap_or("none");
        let content = format!(
            "Command: {}\n\nPrompt (editable): {}\n\nContext: {}\n\n[Enter] Confirm  [Esc] Cancel",
            dialog.command, dialog.prompt, context
        );

        frame.render_widget(
            Paragraph::new(content).block(Block::default().borders(Borders::ALL).title(" Confirm Command ")),
            area,
        );
    }

    fn render_channel_picker(&self, frame: &mut Frame, input_area: Rect, picker: &ChannelPicker) {
        use ratatui::style::{Modifier, Style};
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState};

        let max_visible = 8u16;
        let picker_height = (picker.filtered_channels.len() as u16 + 2).min(max_visible);
        if picker_height < 2 {
            return;
        }

        let base_y = input_area.y.saturating_add(input_area.height);
        let picker_area = Rect::new(
            input_area.x,
            base_y.min(frame.area().height.saturating_sub(picker_height)),
            input_area.width,
            picker_height,
        );

        frame.render_widget(Clear, picker_area);

        let items: Vec<ListItem> = picker
            .filtered_channels
            .iter()
            .map(|ch| ListItem::new(format!("#{}", ch.name)))
            .collect();

        let mut list_state = ListState::default();
        if !items.is_empty() {
            list_state.select(Some(picker.selected_index.min(items.len().saturating_sub(1))));
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Channel Picker: {} ", picker.query)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        frame.render_stateful_widget(list, picker_area, &mut list_state);
    }

    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Paragraph};

        let mode_indicator = match self.input.mode {
            InputMode::Normal => "[💬]",
            InputMode::AgentCommand => "[⚡]",
            InputMode::AgentMention => "[🤖]",
        };

        let text = format!("{} > {}", mode_indicator, self.input.buffer);
        let text = if self.agent_processing {
            format!("{}   [agent processing]", text)
        } else if self.focus == Focus::Input {
            format!("{}█", text)
        } else {
            text
        };

        let border_style = if self.focus == Focus::Input {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style),
            ),
            area,
        );

        if let Some(ref picker) = self.channel_picker {
            self.render_channel_picker(frame, area, picker);
        }
    }

    fn render_context_menu(&self, frame: &mut Frame, area: Rect, menu: &ContextMenu) {
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem};

        let menu_width = menu.items.iter().map(|i| i.label.len()).max().unwrap_or(10) as u16 + 4;
        let menu_height = menu.items.len() as u16 + 2;

        let menu_area = Rect::new(
            menu.x,
            menu.y,
            menu_width.min(area.width.saturating_sub(menu.x)),
            menu_height.min(area.height.saturating_sub(menu.y)),
        );

        frame.render_widget(Clear, menu_area);

        let items: Vec<ListItem> = menu
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                if i == menu.selected {
                    ListItem::new(format!(" > {} ", item.label))
                } else {
                    ListItem::new(format!("   {} ", item.label))
                }
            })
            .collect();

        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title(" Actions ")),
            menu_area,
        );
    }

    fn render_edit_message(&self, frame: &mut Frame, area: Rect, edit_state: &EditState) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let popup_area = self.centered_rect(60, 20, area);

        frame.render_widget(Clear, popup_area);

        let text = format!(
            "Editing message:\n\n{}\n\n[Enter] to save, [Esc] to cancel",
            edit_state.original_text
        );

        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Edit Message "),
            ),
            popup_area,
        );
    }

    fn render_error_details(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let popup_area = self.centered_rect(60, 20, area);
        let details = self
            .last_error
            .as_deref()
            .unwrap_or("No error details available.");
        let content_width = popup_area.width.saturating_sub(2) as usize;
        let content_lines = popup_area.height.saturating_sub(4) as usize;
        let wrapped_details = Self::wrap_and_truncate_text(details, content_width, content_lines);
        let text = format!("{}\n\n[Esc] or [Enter] to close", wrapped_details);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Error Details "),
            ),
            popup_area,
        );
    }

    fn wrap_and_truncate_text(input: &str, width: usize, max_lines: usize) -> String {
        if width == 0 || max_lines == 0 {
            return "... (truncated)".to_string();
        }

        let mut out = Vec::new();
        let mut truncated = false;

        for raw_line in input.lines() {
            if raw_line.is_empty() {
                if out.len() >= max_lines {
                    truncated = true;
                    break;
                }
                out.push(String::new());
                continue;
            }

            let mut current = String::new();
            for word in raw_line.split_whitespace() {
                if word.chars().count() > width {
                    if !current.is_empty() {
                        if out.len() >= max_lines {
                            truncated = true;
                            break;
                        }
                        out.push(std::mem::take(&mut current));
                    }

                    let mut chunk = String::new();
                    for ch in word.chars() {
                        chunk.push(ch);
                        if chunk.chars().count() == width {
                            if out.len() >= max_lines {
                                truncated = true;
                                break;
                            }
                            out.push(std::mem::take(&mut chunk));
                        }
                    }
                    if truncated {
                        break;
                    }
                    if !chunk.is_empty() {
                        current = chunk;
                    }
                    continue;
                }

                let candidate = if current.is_empty() {
                    word.to_string()
                } else {
                    format!("{current} {word}")
                };

                if candidate.chars().count() <= width {
                    current = candidate;
                } else {
                    if out.len() >= max_lines {
                        truncated = true;
                        break;
                    }
                    out.push(std::mem::take(&mut current));
                    current = word.to_string();
                }
            }

            if truncated {
                break;
            }

            if !current.is_empty() {
                if out.len() >= max_lines {
                    truncated = true;
                    break;
                }
                out.push(current);
            }
        }

        if truncated || out.len() > max_lines {
            out.truncate(max_lines.saturating_sub(1));
            out.push("... (truncated)".to_string());
        }

        out.join("\n")
    }

    fn render_jump_to_time(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let popup_area = self.centered_rect(40, 15, area);

        frame.render_widget(Clear, popup_area);

        let text = format!(
            "Jump to time (HH:MM or YYYY-MM-DD):\n\n{}\n\n[Enter] to jump, [Esc] to cancel",
            self.jump_to_time_buffer
        );

        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Jump to Message "),
            ),
            popup_area,
        );
    }

    fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
                ratatui::layout::Constraint::Percentage(percent_y),
                ratatui::layout::Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
                ratatui::layout::Constraint::Percentage(percent_x),
                ratatui::layout::Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
