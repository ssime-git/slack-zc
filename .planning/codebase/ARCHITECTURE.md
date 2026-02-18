# Architecture

**Analysis Date:** 2026-02-18

## Pattern Overview

**Overall:** Event-driven multi-crate architecture with async task-based state management and three functional layers (TUI presentation, Slack API integration, AI agent orchestration).

**Key Characteristics:**
- Workspace-centric design: Multi-workspace support with workspace switching
- Event-driven message flow: Socket Mode listener → channel dispatcher → message store
- Async spawn pattern: Long-running operations dispatched as tokio tasks, results collected via mpsc channels
- Modular separation: Three independent crates (slack, tui, agent) with clear ownership
- Session encryption: AES-GCM encrypted token storage with per-machine secret keys

## Layers

**Presentation Layer (TUI):**
- Purpose: Terminal-based user interface with mouse/keyboard input handling
- Location: `crates/tui/src/`
- Contains: Event handlers, UI rendering, layout management, onboarding flows, app state
- Depends on: `slack-zc-slack` (types, API, events), `slack-zc-agent` (runner control)
- Used by: Main binary entrypoint at `crates/tui/src/main.rs`

**Slack Integration Layer:**
- Purpose: Slack API communication, WebSocket management, authentication, and data transformation
- Location: `crates/slack/src/`
- Contains: HTTP API client with retry logic, Socket Mode WebSocket listener, token exchange, types
- Depends on: tokio, tokio-tungstenite, reqwest, serde_json, chrono
- Used by: TUI app for channel history, message sending, user lookup

**Agent Orchestration Layer:**
- Purpose: ZeroClaw binary management, gateway client, command routing
- Location: `crates/agent/src/`
- Contains: Agent runner process control, pairing flow, gateway HTTP client, command parsing
- Depends on: reqwest, tokio, regex
- Used by: TUI app for agent command execution and status tracking

## Data Flow

**Application Initialization:**

1. Main reads config from `config/default.toml` or platform-specific config directory
2. `App::new()` creates mpsc channels for Slack events and async app events
3. `App::init()` loads encrypted session (if exists), populates workspaces and channels
4. For each workspace: spawns `SocketModeClient` task to listen for real-time events
5. Main enters event loop: poll for input → dispatch to handlers → render

**Message Reception (Real-time):**

1. Slack Socket Mode → `SocketModeClient::run()` continuous loop
2. Parse JSON envelope, extract message event
3. Send `SlackEvent::Message` via `event_tx` channel to app
4. `App::process_slack_events()` dequeues from `event_rx`, routes to `messages: HashMap<channel_id, VecDeque<Message>>`
5. Tracks thread context in `active_threads: HashMap<channel_id, thread_ts>`
6. Render cycle picks up new messages and displays

**Message Sending:**

1. User types text and presses Enter (InputMode::Normal)
2. `handle_input_submit()` creates async task that calls `SlackApi::send_message(token, channel, text)`
3. Task awaits HTTP POST to `https://slack.com/api/chat.postMessage`
4. Result dispatched as `AppAsyncEvent::SlackSendResult {context, error}`
5. `process_app_async_events()` handles result, updates UI with success/error

**Agent Command Execution:**

1. User enters `/resume #channel` or mentions `@zeroclaw`
2. `CommandType::from_command()` parses command and arguments
3. `to_webhook_payload()` constructs JSON for agent
4. `GatewayClient::send_to_agent()` POSTs to local gateway (default port 8080)
5. Agent processes webhook, returns response
6. Response added to `agent_responses: VecDeque<AgentResponse>`
7. Agent panel re-renders with response

**State Management:**

**Workspace State:**
- `workspaces: Vec<WorkspaceState>` holds all connected workspaces
- `active_workspace: usize` tracks current selection
- Each workspace owns channel list and socket task

**Message State:**
- `messages: HashMap<String, VecDeque<Message>>` - keyed by channel_id
- `threads: HashMap<String, Vec<Thread>>` - keyed by channel_id, contains threads
- `active_threads: HashMap<String, String>` - tracks selected thread per channel

**Input State:**
- `input: InputState` - current text buffer, cursor position, mode (Normal/AgentCommand/AgentMention)
- `InputMode` enum controls how input is interpreted

**UI State:**
- `layout: LayoutState` - sidebar/agent panel widths, panel calculations
- `selected_channel: Option<usize>` - currently selected channel in sidebar
- `show_help`, `show_workspace_picker`, `show_channel_search` - modal visibility flags
- `context_menu: Option<ContextMenu>` - right-click menu state
- `selected_message: Option<(String, String)>` - channel_id, message_ts tuple

**Agent State:**
- `agent_runner: Option<AgentRunner>` - process handle to zeroclaw binary
- `agent_status: AgentStatus` - enum tracking Unavailable/Starting/Pairing/Active/Error
- `agent_responses: VecDeque<AgentResponse>` - command history with responses

## Key Abstractions

**SlackApi:**
- Purpose: HTTP REST client for Slack API with automatic retry on rate limits
- Files: `crates/slack/src/api.rs`
- Pattern: Shared Arc<Client>, exponential backoff retry (max 3 attempts), caches user display names
- Key methods: `get_history()`, `send_message()`, `list_channels()`, `get_thread_replies()`, `get_socket_mode_url()`

**SocketModeClient:**
- Purpose: WebSocket listener for real-time Slack events with auto-reconnect
- Files: `crates/slack/src/socket.rs`
- Pattern: Spawns background task that maintains WebSocket connection with exponential backoff (1s to 30s)
- Parses event envelopes and sends acknowledgments back to Slack
- Emits `SlackEvent` enum variants via unbounded mpsc channel

**Session:**
- Purpose: Persistent storage of workspace tokens and configuration
- Files: `crates/slack/src/auth.rs`
- Pattern: AES-GCM encryption of JSON, key stored in platform config dir with 0600 permissions
- Serializes `Vec<Workspace>` with tokens + zeroclaw bearer token
- Loads on startup, saved when workspace selection changes

**GatewayClient:**
- Purpose: HTTP client for local zeroclaw gateway with request/response flow
- Files: `crates/agent/src/gateway.rs`
- Pattern: Built on reqwest with 15s request timeout, 5s connect timeout
- Handles pairing flow: extracts pairing code from HTTP response, stores bearer token
- `send_to_agent()` includes response truncation (max 20k chars)

**AgentRunner:**
- Purpose: Process lifecycle management for zeroclaw binary
- Files: `crates/agent/src/runner.rs`
- Pattern: Spawns child process, captures stdout to extract pairing code via regex
- Manages stdout reader in background task to drain output
- Status transitions: Unavailable → Starting → Pairing → Active → Error

**App (State Machine):**
- Purpose: Central coordinator managing all state and event dispatch
- Files: `crates/tui/src/app/state.rs` (fields), split across `mod.rs`, `actions.rs`, `effects.rs`, `input.rs`, `render.rs`
- Pattern: Monolithic struct with method impl blocks for different concerns
- Receives events from TUI input loop and Slack socket
- Dispatches async tasks that send results back via channels for non-blocking updates

## Entry Points

**`crates/tui/src/main.rs`:**
- Location: `crates/tui/src/main.rs`
- Triggers: `cargo run` or `slack-zc` binary
- Responsibilities:
  - Initialize crossterm terminal (raw mode, alternate screen)
  - Load config from disk or use defaults
  - Create App instance with mpsc channels
  - Run event loop: poll input → handle → process Slack events → render

**`App::init()` (Async initialization):**
- Location: `crates/tui/src/app/effects.rs:4`
- Triggers: Called in main's tokio runtime, first async operation
- Responsibilities:
  - Load session or enter onboarding
  - List channels for each workspace
  - Spawn SocketModeClient task per workspace
  - Set is_loading = false to enable main UI

**`App::handle_event()` (Event dispatch):**
- Location: `crates/tui/src/app/input.rs:4`
- Triggers: On every user input (keyboard/mouse)
- Responsibilities:
  - Route KeyEvent/MouseEvent to specific handlers
  - Handle global shortcuts (Ctrl+Q quit, Ctrl+H help, Ctrl+E error)
  - Dispatch to onboarding/normal mode handlers

**`App::process_slack_events()` (Event consumption):**
- Location: `crates/tui/src/app/mod.rs:112`
- Triggers: Every render cycle (50ms poll)
- Responsibilities:
  - Drain `event_rx` channel (non-blocking)
  - Route SlackEvent variants (Message, UserTyping, ChannelJoined, etc.)
  - Store messages, track typing indicators, update channel state

**`App::render()` (UI generation):**
- Location: `crates/tui/src/app/render.rs:4`
- Triggers: Every frame in event loop
- Responsibilities:
  - Check loading/onboarding/help screens → return early if showing
  - Calculate layout via LayoutState
  - Dispatch to panel renderers (topbar, sidebar, messages, agent, input)
  - Render overlays (context menu, edit dialog, jump-to-time, error details)

## Error Handling

**Strategy:** Layered error handling with domain-specific error types, user-friendly messages, and error redaction for logs.

**Patterns:**

- **API Layer** (`crates/slack/src/error.rs`): `ApiError` enum with variants (Auth, RateLimited, Network, Validation, Api, Timeout). Each has `user_message()` and `is_retryable()` methods.

- **Retry Logic** (`crates/slack/src/api.rs`): `with_retry()` generic function implements exponential backoff (base 1000ms + jitter) up to 3 attempts. Detects rate limit (429 status) and non-rate-limit errors.

- **Async Result Propagation** (`crates/tui/src/app/types.rs`): `AppAsyncEvent` enum includes error field: `error: Option<String>`. Tasks that fail still return event with error set.

- **User Display** (`crates/tui/src/app/mod.rs:39`): `report_error(context, error)` redacts sensitive tokens (xoxp-, xoxb-, xapp-, Bearer) before storing in `last_error`. Log and display use redacted string.

- **Modal Error Display** (`crates/tui/src/app/input.rs:32`): When `show_error_details = true`, render overlay showing last_error. Dismissable with Esc/Enter/E.

## Cross-Cutting Concerns

**Logging:**
- Framework: `tracing` crate with `tracing-subscriber`
- Configured in TUI app (if tracing initialized at startup)
- Usage: Info level for startup/pairing, debug for WebSocket frames, warn for recoverable errors, error for fatal issues

**Validation:**
- URL validation via `url::Url::parse()` in OAuth flow
- Message text validation: empty string rejected before send
- Channel ID format validation: regex-based pairing code extraction (6 digits)
- Token validation: presence checked before API calls

**Authentication:**
- OAuth 2.0 flow: User authorizes app, receives code, TUI exchanges code for tokens
- Token types: xoxp (user), xoxb (bot), xapp (app-level), bearer (ZeroClaw)
- Session persistence: Encrypted at rest, decrypted on load
- Scope handling: Implicit (Slack API returns granted scopes in response)

**Async Coordination:**
- mpsc unbounded channels used for all inter-task communication
- No shared mutable state via Arc<Mutex<>> (single-threaded event loop pattern)
- Tasks spawned via `tokio::spawn()` in app context, results collected in main loop
- Timeout on Socket Mode receive (60s) triggers reconnect; timeout on pairing code extraction (5s)

**Message Threading:**
- Thread context tracked per-channel: `active_threads: HashMap<channel_id, thread_ts>`
- Reply loading deferred: `get_thread_replies()` called on-demand when user views thread
- Reply storage: `threads: HashMap<channel_id, Vec<Thread>>` holds full reply history

