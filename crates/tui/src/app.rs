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
use slack_zc_slack::types::{Channel, Message, Workspace, WorkspaceState};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::warn;

pub struct App {
    pub should_quit: bool,
    pub session: Option<Session>,
    pub workspaces: Vec<WorkspaceState>,
    pub active_workspace: usize,
    pub layout: LayoutState,
    pub input: InputState,
    pub keybinds: Keybinds,
    pub agent_runner: Option<AgentRunner>,
    pub agent_status: AgentStatus,
    pub agent_responses: VecDeque<AgentResponse>,
    pub messages: HashMap<String, VecDeque<Message>>,
    pub scroll_offset: usize,
    pub show_help: bool,
    pub onboarding: Option<OnboardingState>,
    pub show_workspace_picker: bool,
    pub show_channel_search: bool,
    pub search_query: String,
    pub drag_target: Option<DragTarget>,
    pub last_mouse_pos: (u16, u16),
    pub slack_api: SlackApi,
    pub event_tx: Option<mpsc::UnboundedSender<SlackEvent>>,
    pub event_rx: Option<mpsc::UnboundedReceiver<SlackEvent>>,
    pub channels: Vec<Channel>,
    pub selected_channel: Option<usize>,
    pub active_threads: HashMap<String, String>,
    pub agent_processing: bool,
    pub is_loading: bool,
    pub loading_message: String,
    pub oauth_pending: bool,
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub command: String,
    pub response: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl App {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            should_quit: false,
            session: None,
            workspaces: Vec::new(),
            active_workspace: 0,
            layout: LayoutState::default(),
            input: InputState::new(),
            keybinds: Keybinds::default(),
            agent_runner: None,
            agent_status: AgentStatus::Unavailable,
            agent_responses: VecDeque::new(),
            messages: HashMap::new(),
            scroll_offset: 0,
            show_help: false,
            onboarding: None,
            show_workspace_picker: false,
            show_channel_search: false,
            search_query: String::new(),
            drag_target: None,
            last_mouse_pos: (0, 0),
            slack_api: SlackApi::new(),
            event_tx: Some(event_tx),
            event_rx: Some(event_rx),
            channels: Vec::new(),
            selected_channel: None,
            active_threads: HashMap::new(),
            agent_processing: false,
            is_loading: true,
            loading_message: "Loading...".to_string(),
            oauth_pending: false,
        }
    }

    pub async fn init(&mut self, config: &Config) -> Result<()> {
        if let Some(session) = Session::load()? {
            self.session = Some(session.clone());

            for workspace in &session.workspaces {
                let mut ws_state = WorkspaceState::new(workspace.clone());

                match self.slack_api.list_channels(&workspace.xoxp_token).await {
                    Ok(channels) => ws_state.channels = channels,
                    Err(e) => warn!("Failed to load channels: {}", e),
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
        } else {
            self.onboarding = Some(OnboardingState::new());
            self.is_loading = false;
        }

        Ok(())
    }

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

        let panels = self.layout.get_panels();

        for panel in panels {
            match panel.panel_type {
                PanelType::Topbar => self.render_topbar(frame, panel.rect),
                PanelType::Sidebar => self.render_sidebar(frame, panel.rect),
                PanelType::Messages => self.render_messages(frame, panel.rect),
                PanelType::AgentPanel => self.render_agent_panel(frame, panel.rect),
                PanelType::InputBar => self.render_input_bar(frame, panel.rect),
            }
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
                } else {
                    if let Some(ref url) = state.oauth_url {
                        format!(
                            "\n\n  OAuth authentication:\n\n  1. Visit: {}\n\n  2. Authorize the app\n\n  3. Your code: {}\n\n  Press [Enter] to exchange code for tokens,\n  [c] to copy URL,\n  [Esc] to go back\n",
                            url,
                            state.oauth_code
                        )
                    } else {
                        "\n\n  OAuth authentication:\n\n  Press [Enter] to generate OAuth URL,\n  or [Esc] to go back\n".to_owned()
                    }
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

        let text = format!(
            " ● {}   {}   [?] help",
            workspace_tabs.join(" "),
            agent_indicator
        );

        frame.render_widget(Paragraph::new(text).block(Block::default()), area);
    }

    fn render_sidebar(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, List, ListItem};

        let mut items: Vec<ListItem> = vec![];

        items.push(
            ListItem::new(" CHANNELS ").style(
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::BOLD),
            ),
        );

        for (i, channel) in self.channels.iter().enumerate() {
            let prefix = if Some(i) == self.selected_channel {
                "> "
            } else {
                "  "
            };
            let name = channel.display_name();
            let unread = if channel.unread_count > 0 {
                format!(" {}", channel.unread_count)
            } else {
                String::new()
            };
            items.push(ListItem::new(format!("{}{}{}", prefix, name, unread)));
        }

        frame.render_widget(
            List::new(items).block(Block::default().borders(Borders::ALL).title(" Channels ")),
            area,
        );
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};

        let messages = if let Some(ref channel) = self.selected_channel {
            self.channels.get(*channel).and_then(|ch| {
                self.messages.get(&ch.id).map(|msgs| {
                    msgs.iter()
                        .map(|m| {
                            format!("{} {}: {}", m.timestamp.format("%H:%M"), m.username, m.text)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            })
        } else {
            None
        };

        let text = messages.unwrap_or_else(|| "Select a channel to view messages".to_string());

        frame.render_widget(
            Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL))
                .scroll((self.scroll_offset as u16, 0)),
            area,
        );
    }

    fn render_agent_panel(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};

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

    fn render_input_bar(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};

        let mode_indicator = match self.input.mode {
            InputMode::Normal => "[💬]",
            InputMode::AgentCommand => "[⚡]",
            InputMode::AgentMention => "[🤖]",
        };

        let text = format!("{} > {}", mode_indicator, self.input.buffer);
        let text = if self.agent_processing {
            format!("{}   [agent processing]", text)
        } else {
            text
        };

        frame.render_widget(
            Paragraph::new(text).block(Block::default().borders(Borders::ALL)),
            area,
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

        if let Some(ref mut onboarding) = self.onboarding {
            match key.code {
                KeyCode::Enter => {
                    if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        if onboarding.oauth_url.is_none()
                            && !onboarding.client_id.is_empty()
                            && !onboarding.client_secret.is_empty()
                        {
                            let _ = onboarding.generate_oauth_url(3000);
                        } else if !onboarding.oauth_code.is_empty() {
                            let code = onboarding.oauth_code.clone();
                            drop(onboarding);
                            if let Err(e) = self.complete_oauth(&code) {
                                if let Some(ref mut o) = self.onboarding {
                                    o.error_message = Some(e.to_string());
                                }
                            } else {
                                if let Some(ref mut o) = self.onboarding {
                                    o.next_screen();
                                }
                            }
                        }
                    } else if matches!(onboarding.current_screen, OnboardingScreen::ZeroClawPairing)
                    {
                        drop(onboarding);
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
                    if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        if let Some(ref url) = onboarding.oauth_url {
                            tracing::info!("OAuth URL: {}", url);
                        }
                    }
                }
                KeyCode::Char(c) => {
                    if matches!(
                        onboarding.current_screen,
                        OnboardingScreen::SlackCredentials
                    ) {
                        onboarding.current_field_value().push(c);
                    } else if matches!(onboarding.current_screen, OnboardingScreen::OAuthFlow) {
                        if onboarding.oauth_url.is_some() {
                            onboarding.oauth_code.push(c);
                        }
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

        match key.code {
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_workspace_picker = true;
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_channel_search = true;
            }
            KeyCode::Up => {
                if self.scroll_offset > 0 {
                    self.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                self.scroll_offset += 1;
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::ALT) => {
                if let Some(selected) = self.selected_channel {
                    if selected < self.channels.len().saturating_sub(1) {
                        self.select_channel(selected + 1);
                    }
                } else if !self.channels.is_empty() {
                    self.select_channel(0);
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::ALT) => {
                if let Some(selected) = self.selected_channel {
                    if selected > 0 {
                        self.select_channel(selected - 1);
                    }
                }
            }
            KeyCode::Char(c) => {
                self.input.handle_char(c);
            }
            KeyCode::Backspace => {
                self.input.handle_backspace();
            }
            KeyCode::Enter => {
                self.handle_input_submit()?;
            }
            KeyCode::Esc => {
                self.input.clear();
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

    fn switch_workspace(&mut self, idx: usize) {
        if idx < self.workspaces.len() {
            self.active_workspace = idx;
            self.channels = self.workspaces[idx].channels.clone();
            self.selected_channel = None;
            self.scroll_offset = 0;

            if let Some(ref mut session) = self.session {
                if let Some(ref ws) = self.workspaces.get(idx) {
                    session.set_active_workspace(&ws.workspace.team_id);
                    let _ = session.save();
                }
            }
        }
    }

    fn select_channel(&mut self, idx: usize) {
        self.selected_channel = Some(idx);
        self.scroll_offset = 0;

        if let Some(channel) = self.channels.get(idx) {
            let ws = self.workspaces.get(self.active_workspace);
            if let Some(ws) = ws {
                let rt = tokio::runtime::Handle::current();
                let channel_id = channel.id.clone();
                let messages: Vec<Message> = rt.block_on(async {
                    let api = SlackApi::new();
                    api.get_history(&ws.workspace.xoxp_token, &channel_id, 50)
                        .await
                        .unwrap_or_default()
                });

                let msg_deque: VecDeque<Message> = messages.into();
                self.messages.insert(channel_id, msg_deque);
            }
        }
    }

    fn handle_input_submit(&mut self) -> Result<()> {
        let text = self.input.buffer.clone();
        if text.is_empty() {
            return Ok(());
        }

        match self.input.mode {
            InputMode::Normal => {
                if let Some(channel) = self.get_active_channel_id() {
                    if let Some(ref ws) = self.workspaces.get(self.active_workspace) {
                        let rt = tokio::runtime::Handle::current();
                        let _ = rt.block_on(async {
                            self.slack_api
                                .send_message(&ws.workspace.xoxp_token, &channel, &text)
                                .await
                        });
                    }
                }
            }
            InputMode::AgentCommand => {
                self.handle_agent_command(&text)?;
            }
            InputMode::AgentMention => {
                if let Some(channel) = self.get_active_channel_id() {
                    if let Some(ref ws) = self.workspaces.get(self.active_workspace) {
                        let rt = tokio::runtime::Handle::current();
                        let _ = rt.block_on(async {
                            self.slack_api
                                .send_message(&ws.workspace.xoxp_token, &channel, &text)
                                .await
                        });
                    }
                }
            }
        }

        self.input.clear();
        Ok(())
    }

    fn handle_agent_command(&mut self, text: &str) -> Result<()> {
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
            if let Some(gateway) = runner.get_gateway() {
                self.agent_processing = true;
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(async {
                    timeout(Duration::from_secs(15), gateway.send_to_agent(&payload)).await
                });

                match result {
                    Ok(Ok(response)) => {
                        self.agent_responses.push_front(AgentResponse {
                            command: text.to_string(),
                            response: response.clone(),
                            timestamp: Utc::now(),
                        });

                        if self.agent_responses.len() > 50 {
                            self.agent_responses.pop_back();
                        }

                        let channel = self.get_active_channel_id();
                        if let Some(ch) = channel {
                            if let Some(ref ws) = self.workspaces.get(self.active_workspace) {
                                let rt = tokio::runtime::Handle::current();
                                let thread_ts = self.active_threads.get(&ch).cloned();
                                if let Some(ts) = thread_ts {
                                    let _ = rt.block_on(async {
                                        self.slack_api
                                            .send_message_to_thread(
                                                &ws.workspace.xoxp_token,
                                                &ch,
                                                &response,
                                                &ts,
                                            )
                                            .await
                                    });
                                } else {
                                    let _ = rt.block_on(async {
                                        self.slack_api
                                            .send_message(&ws.workspace.xoxp_token, &ch, &response)
                                            .await
                                    });
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Agent command failed: {}", e);
                    }
                    Err(_) => {
                        tracing::error!("Agent command timed out after 15s");
                    }
                }
                self.agent_processing = false;
            }
        } else {
            tracing::warn!("Agent not connected, cannot process command");
        }

        Ok(())
    }

    fn start_zeroclaw_pairing(&mut self) {
        let rt = tokio::runtime::Handle::current();

        let mut runner = AgentRunner::new("zeroclaw".to_string(), 8080);

        let result = rt.block_on(async { runner.check_binary().await });

        match result {
            Ok(()) => {
                self.agent_status = AgentStatus::Pairing;
                let result = rt.block_on(async { runner.start_and_pair().await });
                match result {
                    Ok(_) => {
                        self.agent_status = AgentStatus::Active;
                        self.agent_runner = Some(runner);
                    }
                    Err(e) => {
                        self.agent_status = AgentStatus::Error(e.to_string());
                    }
                }
            }
            Err(e) => {
                self.agent_status = AgentStatus::Error(e.to_string());
            }
        }
    }

    fn complete_oauth(&mut self, code: &str) -> Result<()> {
        if let Some(ref onboarding) = self.onboarding {
            let rt = tokio::runtime::Handle::current();

            let result = rt.block_on(async {
                slack_zc_slack::auth::exchange_oauth_code(
                    &onboarding.client_id,
                    &onboarding.client_secret,
                    code,
                    "http://localhost:3000",
                )
                .await
            });

            match result {
                Ok(response) => {
                    let workspace = Workspace {
                        team_id: response.team.id,
                        team_name: response.team.name,
                        xoxp_token: response.authed_user.access_token,
                        xapp_token: response.access_token,
                        user_id: Some(response.authed_user.id),
                        active: true,
                    };

                    let mut session = self.session.take().unwrap_or(Session {
                        workspaces: Vec::new(),
                        zeroclaw_bearer: None,
                    });

                    for w in &mut session.workspaces {
                        w.active = false;
                    }
                    session.add_workspace(workspace);
                    session.save()?;
                    self.session = Some(session);

                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    fn get_active_channel_id(&self) -> Option<String> {
        self.selected_channel
            .and_then(|idx| self.channels.get(idx).map(|ch| ch.id.clone()))
    }

    pub fn process_slack_events(&mut self) {
        if let Some(ref mut rx) = self.event_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    SlackEvent::Message { channel, message } => {
                        if let Some(ref thread_ts) = message.thread_ts {
                            self.active_threads
                                .insert(channel.clone(), thread_ts.clone());
                        }
                        self.messages
                            .entry(channel)
                            .or_insert_with(VecDeque::new)
                            .push_back(message);
                    }
                    SlackEvent::UserTyping { channel, user } => {
                        tracing::debug!("User {} typing in {}", user, channel);
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
    }
}

#[derive(Debug, Clone, Copy)]
enum HitTarget {
    Channel(usize),
    WorkspaceTab(usize),
    SidebarDivider,
    AgentDivider,
}

#[cfg(test)]
mod tests {
    use super::App;
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
        }
    }

    #[test]
    fn routes_messages_to_their_source_channel() {
        let mut app = App::new();
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
        let mut app = App::new();
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
