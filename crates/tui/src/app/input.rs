use super::*;

impl App {
    pub fn handle_event(&mut self, event: Event) -> Result<bool> {
        match event {
            Event::Key(key) => self.handle_key_event(key),
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            Event::Resize(_, _) => Ok(false),
            _ => Ok(false),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<bool> {
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Ok(true);
        }

        if key.code == KeyCode::Char('?')
            || key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.show_help = !self.show_help;
            return Ok(false);
        }

        if self.show_help {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.show_help = false;
            }
            return Ok(false);
        }

        if self.show_error_details {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('E') => {
                    self.show_error_details = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        if let Some(ref mut onboarding) = self.onboarding {
            match key.code {
                KeyCode::Enter => {
                    if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        if onboarding.oauth_url.is_none()
                            && !onboarding.client_id.is_empty()
                            && !onboarding.client_secret.is_empty()
                        {
                            let _ = onboarding.generate_oauth_url(self.config.slack.redirect_port);
                        } else if !onboarding.oauth_code.is_empty() {
                            let code = onboarding.oauth_code.clone();
                            if let Some(ref mut o) = self.onboarding {
                                o.error_message = Some("Completing OAuth...".to_string());
                            }
                            if let Err(e) = self.complete_oauth(&code) {
                                if let Some(ref mut o) = self.onboarding {
                                    o.error_message = Some(e.to_string());
                                }
                            }
                        }
                    } else if matches!(onboarding.current_screen, OnboardingScreen::ZeroClawPairing)
                    {
                        self.start_zeroclaw_pairing();
                    } else if matches!(onboarding.current_screen, OnboardingScreen::Complete) {
                        self.onboarding = None;
                    } else {
                        onboarding.next_screen();
                    }
                }
                KeyCode::Esc => {
                    if matches!(onboarding.current_screen, OnboardingScreen::Welcome) {
                        self.should_quit = true;
                    } else if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        onboarding.oauth_code.clear();
                        onboarding.oauth_url = None;
                    }
                    onboarding.previous_screen();
                }
                KeyCode::Tab => {
                    if matches!(
                        onboarding.current_screen,
                        OnboardingScreen::SlackCredentials
                    ) {
                        onboarding.toggle_field();
                    }
                }
                KeyCode::Char('c') => {
                    if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow)
                        && onboarding.oauth_url.is_some()
                    {
                        tracing::info!("OAuth URL generated (redacted)");
                    }
                }
                KeyCode::Char(c) => {
                    if matches!(
                        onboarding.current_screen,
                        OnboardingScreen::SlackCredentials
                    ) {
                        onboarding.current_field_value().push(c);
                    } else if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow)
                        && onboarding.oauth_url.is_some()
                    {
                        onboarding.oauth_code.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if matches!(
                        onboarding.current_screen,
                        OnboardingScreen::SlackCredentials
                    ) {
                        onboarding.current_field_value().pop();
                    } else if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        onboarding.oauth_code.pop();
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.show_workspace_picker {
            match key.code {
                KeyCode::Esc => self.show_workspace_picker = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.active_workspace > 0 {
                        self.active_workspace -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.active_workspace < self.workspaces.len().saturating_sub(1) {
                        self.active_workspace += 1;
                    }
                }
                KeyCode::Enter => {
                    self.switch_workspace(self.active_workspace);
                    self.show_workspace_picker = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.show_channel_search {
            match key.code {
                KeyCode::Esc => self.show_channel_search = false,
                KeyCode::Char(c) => self.search_query.push(c),
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Enter => {
                    self.show_channel_search = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.show_jump_to_time {
            match key.code {
                KeyCode::Esc => {
                    self.show_jump_to_time = false;
                    self.jump_to_time_buffer.clear();
                }
                KeyCode::Enter => {
                    self.jump_to_timestamp()?;
                    self.show_jump_to_time = false;
                    self.jump_to_time_buffer.clear();
                }
                KeyCode::Backspace => {
                    self.jump_to_time_buffer.pop();
                }
                KeyCode::Char(c) => {
                    self.jump_to_time_buffer.push(c);
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.confirmation_dialog.is_some() {
            match key.code {
                KeyCode::Enter => {
                    if let Some(dialog) = self.confirmation_dialog.take() {
                        self.dispatch_confirmed_command(&dialog)?;
                    }
                }
                KeyCode::Esc => {
                    self.confirmation_dialog = None;
                }
                KeyCode::Char(c) => {
                    if let Some(dialog) = self.confirmation_dialog.as_mut() {
                        if dialog.is_editing {
                            dialog.prompt.push(c);
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some(dialog) = self.confirmation_dialog.as_mut() {
                        if dialog.is_editing {
                            dialog.prompt.pop();
                        }
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.channel_picker.is_some() {
            match key.code {
                KeyCode::Esc => {
                    self.channel_picker = None;
                }
                KeyCode::Up => {
                    if let Some(picker) = self.channel_picker.as_mut() {
                        if picker.selected_index > 0 {
                            picker.selected_index -= 1;
                        }
                    }
                }
                KeyCode::Down => {
                    if let Some(picker) = self.channel_picker.as_mut() {
                        if picker.selected_index < picker.filtered_channels.len().saturating_sub(1) {
                            picker.selected_index += 1;
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(picker) = self.channel_picker.take() {
                        if let Some(ch) = picker.filtered_channels.get(picker.selected_index) {
                            self.insert_channel_reference(&ch.name, picker.trigger_position);
                            self.fetch_channel_history(&ch.id)?;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(picker) = self.channel_picker.as_mut() {
                        picker.query.push(c);
                        let query = picker.query.to_lowercase();
                        picker.filtered_channels = self
                            .channels
                            .iter()
                            .filter(|ch| ch.name.to_lowercase().contains(&query))
                            .cloned()
                            .collect();
                        picker.selected_index = 0;
                    }
                }
                KeyCode::Backspace => {
                    if let Some(picker) = self.channel_picker.as_mut() {
                        picker.query.pop();
                        let query = picker.query.to_lowercase();
                        picker.filtered_channels = self
                            .channels
                            .iter()
                            .filter(|ch| ch.name.to_lowercase().contains(&query))
                            .cloned()
                            .collect();
                        picker.selected_index = 0;
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_workspace_picker = true;
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_channel_search = true;
            }
            KeyCode::Up => {
                if let Some(ref mut menu) = self.context_menu {
                    if menu.selected > 0 {
                        menu.selected -= 1;
                    }
                    return Ok(false);
                }
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                if let Some(ref mut menu) = self.context_menu {
                    if menu.selected < menu.items.len().saturating_sub(1) {
                        menu.selected += 1;
                    }
                    return Ok(false);
                }
                self.scroll_offset += 1;
            }
            KeyCode::Enter => {
                if self.context_menu.is_some() {
                    self.handle_context_menu_action();
                    return Ok(false);
                }
                if self.edit_message.is_some() {
                    self.save_edited_message()?;
                    return Ok(false);
                }
                self.handle_input_submit()?;
            }
            KeyCode::Esc => {
                if self.context_menu.is_some() {
                    self.context_menu = None;
                    return Ok(false);
                }
                if self.edit_message.is_some() {
                    self.edit_message = None;
                    return Ok(false);
                }
                self.input.clear();
            }
            KeyCode::Char('t') => {
                if let Some(ref channel) = self.selected_channel {
                    if let Some(ch) = self.channels.get(*channel) {
                        let channel_id = ch.id.clone();
                        self.toggle_thread_collapse(&channel_id);
                    }
                }
            }
            KeyCode::Char('e') => {
                self.start_edit_message()?;
            }
            KeyCode::Char('d') => {
                self.delete_selected_message()?;
            }
            KeyCode::Char('D') => {
                self.load_history_for_date()?;
            }
            KeyCode::Char('r') => {
                self.show_reaction_picker()?;
            }
            KeyCode::Char('g') => {
                self.show_jump_to_time = true;
                self.jump_to_time_buffer.clear();
            }
            KeyCode::Char('f') => {
                self.show_user_filter = !self.show_user_filter;
                if self.show_user_filter {
                    if let Some(ref channel) = self.selected_channel {
                        if let Some(ch) = self.channels.get(*channel) {
                            if let Some(messages) = self.messages.get(&ch.id) {
                                if let Some(msg) = messages.back() {
                                    self.message_filter.user_id = Some(msg.user_id.clone());
                                }
                            }
                        }
                    }
                } else {
                    self.message_filter.user_id = None;
                }
            }
            KeyCode::Char('E') => {
                if self.last_error.is_some() {
                    self.show_error_details = !self.show_error_details;
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.copy_selected_message()?;
            }
            KeyCode::Char('#') => {
                if self.edit_message.is_none() {
                    let should_trigger = self.input.buffer.is_empty() || self.input.buffer.ends_with(' ');
                    self.input.handle_char('#');
                    if should_trigger {
                        self.channel_picker = Some(ChannelPicker {
                            query: String::new(),
                            filtered_channels: self.channels.clone(),
                            selected_index: 0,
                            trigger_position: self.input.buffer.len().saturating_sub(1),
                        });
                    }
                }
            }
            KeyCode::Char(c) => {
                if self.edit_message.is_none() {
                    self.input.handle_char(c);
                }
            }
            KeyCode::Backspace => {
                if self.edit_message.is_none() {
                    self.input.handle_backspace();
                }
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<bool> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.last_mouse_pos = (mouse.column, mouse.row);

                if let Some(target) = self.hit_test(mouse.column, mouse.row) {
                    match target {
                        HitTarget::Channel(idx) => {
                            self.select_channel(idx);
                        }
                        HitTarget::WorkspaceTab(idx) => {
                            self.switch_workspace(idx);
                        }
                        HitTarget::SidebarDivider => {
                            self.drag_target = Some(DragTarget::Sidebar);
                        }
                        HitTarget::AgentDivider => {
                            self.drag_target = Some(DragTarget::AgentPanel);
                        }
                    }
                }
                self.context_menu = None;
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(target) = self.hit_test_message(mouse.column, mouse.row) {
                    self.selected_message = Some(target);
                    self.context_menu = Some(ContextMenu {
                        x: mouse.column,
                        y: mouse.row,
                        items: vec![
                            ContextMenuItem {
                                label: "Reply".to_string(),
                                action: ContextMenuAction::Reply,
                            },
                            ContextMenuItem {
                                label: "React".to_string(),
                                action: ContextMenuAction::React,
                            },
                            ContextMenuItem {
                                label: "Edit".to_string(),
                                action: ContextMenuAction::Edit,
                            },
                            ContextMenuItem {
                                label: "Delete".to_string(),
                                action: ContextMenuAction::Delete,
                            },
                            ContextMenuItem {
                                label: "Copy".to_string(),
                                action: ContextMenuAction::Copy,
                            },
                            ContextMenuItem {
                                label: "View Thread".to_string(),
                                action: ContextMenuAction::ViewThread,
                            },
                        ],
                        selected: 0,
                    });
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(target) = self.drag_target {
                    let delta = mouse.column as i16 - self.last_mouse_pos.0 as i16;
                    self.layout.handle_drag(target, delta);
                    self.last_mouse_pos = (mouse.column, mouse.row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.drag_target = None;
            }
            MouseEventKind::ScrollDown => {
                self.scroll_offset += 1;
            }
            MouseEventKind::ScrollUp => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn hit_test(&self, col: u16, row: u16) -> Option<HitTarget> {
        if let (Some(sidebar_rect), Some(agent_rect)) =
            (self.layout.get_sidebar_rect(), self.layout.get_agent_rect())
        {
            if row >= sidebar_rect.y && row < sidebar_rect.y + sidebar_rect.height {
                let sidebar_divider = sidebar_rect.x + sidebar_rect.width;
                if col == sidebar_divider {
                    return Some(HitTarget::SidebarDivider);
                }
            }
            if row >= agent_rect.y && row < agent_rect.y + agent_rect.height {
                let agent_divider = agent_rect.x.saturating_sub(1);
                if col == agent_divider {
                    return Some(HitTarget::AgentDivider);
                }
            }
        }

        let panels = self.layout.get_panels();

        for panel in panels {
            if Self::rect_contains(panel.rect, col, row) {
                return match panel.panel_type {
                    PanelType::Sidebar => self.hit_sidebar(panel.rect, col, row),
                    PanelType::Topbar => self.hit_topbar(panel.rect, col, row),
                    _ => None,
                };
            }
        }

        None
    }

    fn hit_sidebar(&self, rect: Rect, _col: u16, row: u16) -> Option<HitTarget> {
        let relative_row = row.saturating_sub(rect.y + 2);
        let channel_start = 1u16;

        if relative_row >= channel_start
            && relative_row < channel_start + self.channels.len() as u16
        {
            let idx = (relative_row - channel_start) as usize;
            return Some(HitTarget::Channel(idx));
        }

        None
    }

    fn hit_topbar(&self, _rect: Rect, col: u16, row: u16) -> Option<HitTarget> {
        if row != 0 {
            return None;
        }

        let mut current_col = 3u16;
        for (idx, ws) in self.workspaces.iter().enumerate() {
            let tab_width = ws.workspace.team_name.len() as u16 + 4;
            if col >= current_col && col < current_col + tab_width {
                return Some(HitTarget::WorkspaceTab(idx));
            }
            current_col += tab_width + 1;
        }

        None
    }

    fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
        col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
    }
}

#[derive(Debug, Clone, Copy)]
enum HitTarget {
    Channel(usize),
    WorkspaceTab(usize),
    SidebarDivider,
    AgentDivider,
}
