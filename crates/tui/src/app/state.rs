use super::*;
use std::time::Instant;

pub struct App {
    pub should_quit: bool,
    pub session: Option<Session>,
    pub config: Config,
    pub workspaces: Vec<WorkspaceState>,
    pub active_workspace: usize,
    pub layout: LayoutState,
    pub input: InputState,
    pub keybinds: Keybinds,
    pub agent_runner: Option<AgentRunner>,
    pub agent_status: AgentStatus,
    pub agent_responses: VecDeque<AgentResponse>,
    pub messages: HashMap<String, VecDeque<Message>>,
    pub threads: HashMap<String, Vec<Thread>>,
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
    pub app_async_tx: Option<mpsc::UnboundedSender<AppAsyncEvent>>,
    pub app_async_rx: Option<mpsc::UnboundedReceiver<AppAsyncEvent>>,
    pub channels: Vec<Channel>,
    pub selected_channel: Option<usize>,
    pub active_threads: HashMap<String, String>,
    pub agent_processing: bool,
    pub loading_start_time: Option<Instant>,
    pub loading_command: Option<String>,
    pub is_loading: bool,
    pub loading_message: String,
    pub typing_users: HashMap<String, Vec<String>>,
    pub context_menu: Option<ContextMenu>,
    pub selected_message: Option<(String, String)>,
    pub edit_message: Option<EditState>,
    pub message_filter: MessageFilter,
    pub show_jump_to_time: bool,
    pub jump_to_time_buffer: String,
    pub show_user_filter: bool,
    pub last_error: Option<String>,
    pub show_error_details: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl App {
    pub fn new(config: Config) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (app_async_tx, app_async_rx) = mpsc::unbounded_channel();

        Self {
            should_quit: false,
            session: None,
            config,
            workspaces: Vec::new(),
            active_workspace: 0,
            layout: LayoutState::default(),
            input: InputState::new(),
            keybinds: Keybinds,
            agent_runner: None,
            agent_status: AgentStatus::Unavailable,
            agent_responses: VecDeque::new(),
            messages: HashMap::new(),
            threads: HashMap::new(),
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
            app_async_tx: Some(app_async_tx),
            app_async_rx: Some(app_async_rx),
            channels: Vec::new(),
            selected_channel: None,
            active_threads: HashMap::new(),
            agent_processing: false,
            loading_start_time: None,
            loading_command: None,
            is_loading: true,
            loading_message: "Loading...".to_string(),
            typing_users: HashMap::new(),
            context_menu: None,
            selected_message: None,
            edit_message: None,
            message_filter: MessageFilter::default(),
            show_jump_to_time: false,
            jump_to_time_buffer: String::new(),
            show_user_filter: false,
            last_error: None,
            show_error_details: false,
        }
    }
}
