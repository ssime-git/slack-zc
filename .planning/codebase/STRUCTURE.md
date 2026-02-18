# Codebase Structure

**Analysis Date:** 2026-02-18

## Directory Layout

```
slack-zc/
├── Cargo.toml              # Workspace manifest (3 member crates)
├── Cargo.lock              # Dependency lock
├── config/
│   └── default.toml        # Default configuration (Slack credentials, ZeroClaw path, LLM provider)
├── crates/
│   ├── slack/              # Slack API integration crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs      # Module exports
│   │       ├── api.rs      # SlackApi client with retry logic
│   │       ├── socket.rs   # SocketModeClient WebSocket listener
│   │       ├── auth.rs     # OAuth exchange and Session encryption
│   │       ├── error.rs    # ApiError enum and mapping
│   │       └── types.rs    # Channel, Message, User, Workspace, Thread types
│   ├── tui/                # Terminal UI application crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs     # Binary entrypoint, terminal setup, event loop
│   │       ├── lib.rs      # Module exports
│   │       ├── config.rs   # Config struct and loading
│   │       ├── keybinds.rs # Keybinding definitions
│   │       ├── input.rs    # InputState struct and keyboard/mouse handling
│   │       ├── app/        # Core application state and logic
│   │       │   ├── mod.rs  # impl blocks: error handling, task spawning
│   │       │   ├── state.rs # App struct fields (40+ fields for all state)
│   │       │   ├── types.rs # AppAsyncEvent, ContextMenu, EditState, MessageFilter enums
│   │       │   ├── actions.rs # impl: switch_workspace, select_channel, handle_input_submit, etc.
│   │       │   ├── effects.rs # impl: init, start_zeroclaw_pairing, complete_oauth, async initialization
│   │       │   ├── input.rs # impl: handle_event (dispatch), handle_key_event, handle_mouse_event
│   │       │   ├── render.rs # impl: render, render_topbar, render_sidebar, render_messages, etc.
│   │       │   └── [test methods in each]
│   │       ├── ui/         # UI layout and panel definitions
│   │       │   ├── mod.rs  # Module exports
│   │       │   ├── layout.rs # LayoutState: sidebar/agent panel widths, panel rectangles
│   │       │   └── panel.rs # PanelType enum (Topbar, Sidebar, Messages, AgentPanel, InputBar)
│   │       └── onboarding/ # OAuth and setup flows
│   │           ├── mod.rs  # OnboardingState, OnboardingScreen enum
│   │           ├── oauth_flow.rs # OAuth URL generation and code entry UI
│   │           ├── slack_setup.rs # Slack credentials input UI
│   │           └── zc_setup.rs # ZeroClaw pairing UI
│   └── agent/              # ZeroClaw agent orchestration crate
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs      # Module exports (GatewayClient, AgentRunner, AgentStatus)
│           ├── gateway.rs  # GatewayClient HTTP client for zeroclaw gateway
│           ├── runner.rs   # AgentRunner: process control, pairing, bearer token management
│           └── commands.rs # CommandType parsing, process_command, is_agent_mention
├── docs/                   # Documentation
│   ├── 00_PRD.md
│   ├── 01_phase1_tasks.md
│   └── [phase docs...]
├── README.md
└── AGENT.md               # Agent documentation
```

## Directory Purposes

**crates/slack/ - Slack Integration:**
- Purpose: All Slack API communication, real-time events, and data types
- Contains: HTTP client, WebSocket listener, OAuth, encryption, types
- Key files: `api.rs` (REST calls), `socket.rs` (real-time), `auth.rs` (tokens + session), `types.rs` (domain objects)

**crates/tui/ - Terminal User Interface:**
- Purpose: Interactive terminal application with message viewing, input, and agent integration
- Contains: Event loop, input handlers, rendering logic, state management, onboarding flows
- Key files: `main.rs` (entrypoint), `app/` (state machine), `ui/` (layout/panels), `onboarding/` (OAuth flow)

**crates/agent/ - Agent Orchestration:**
- Purpose: ZeroClaw binary lifecycle and command routing
- Contains: Process spawning, pairing ceremony, HTTP gateway client, command parsing
- Key files: `runner.rs` (process control), `gateway.rs` (HTTP client), `commands.rs` (command types)

**config/default.toml - Configuration:**
- Purpose: Application defaults (Slack OAuth credentials, ZeroClaw binary path, LLM provider)
- Format: TOML structure with [slack], [zeroclaw], [llm] sections
- Loaded: Via `Config::load_or_default()` in main, merged with platform-specific config dir

**docs/ - Project Documentation:**
- Purpose: Product requirements, phase-based task breakdowns, security checklist
- Contains: PRD, phase 1-4 task lists, production security tasks

## Key File Locations

**Entry Points:**
- `crates/tui/src/main.rs`: Binary entrypoint (terminal init, event loop)
- `crates/tui/src/app/mod.rs`: App methods and error handling
- `crates/slack/src/lib.rs`: Slack crate exports (ApiError, types, SlackApi, etc.)
- `crates/agent/src/lib.rs`: Agent crate exports (GatewayClient, AgentRunner, AgentStatus)

**Core State Management:**
- `crates/tui/src/app/state.rs`: App struct with 40+ fields (session, workspaces, messages, threads, layout, etc.)
- `crates/tui/src/app/types.rs`: AppAsyncEvent, ContextMenu, EditState, MessageFilter enums

**Event Handling:**
- `crates/tui/src/app/input.rs`: handle_event, handle_key_event, handle_mouse_event
- `crates/tui/src/app/actions.rs`: handle_input_submit, switch_workspace, select_channel, context menu actions

**Async Operations:**
- `crates/tui/src/app/effects.rs`: init, complete_oauth, start_zeroclaw_pairing (spawn_app_task calls)
- `crates/slack/src/api.rs`: SlackApi methods (get_history, send_message, list_channels, with_retry)
- `crates/slack/src/socket.rs`: SocketModeClient::run (WebSocket connection + message processing)

**Rendering:**
- `crates/tui/src/app/render.rs`: render (dispatcher), render_topbar, render_sidebar, render_messages, render_agent_panel, render_input_bar
- `crates/tui/src/ui/layout.rs`: LayoutState::calculate_layout (ratatui Constraint/Layout logic)

**Type Definitions:**
- `crates/slack/src/types.rs`: Channel, Message, User, Workspace, Thread, Reaction, File, WorkspaceState
- `crates/slack/src/error.rs`: ApiError enum with user_message() and is_retryable()

**Authentication & Persistence:**
- `crates/slack/src/auth.rs`: Session struct (encrypt/decrypt), OAuth exchange, secret key management
- `crates/tui/src/config.rs`: Config struct, Config::load_or_default()

**Agent Orchestration:**
- `crates/agent/src/runner.rs`: AgentRunner (binary path, check_binary, start_and_pair, start_with_bearer)
- `crates/agent/src/gateway.rs`: GatewayClient (pair, health_check, send_to_agent)
- `crates/agent/src/commands.rs`: CommandType enum (Resume, Draft, Search, Unknown), to_webhook_payload()

**UI Components:**
- `crates/tui/src/ui/layout.rs`: LayoutState (sidebar/agent widths, panel rectangles, handle_drag)
- `crates/tui/src/ui/panel.rs`: PanelType enum, Panel struct

**Onboarding:**
- `crates/tui/src/onboarding/mod.rs`: OnboardingState (current_screen, screen navigation)
- `crates/tui/src/onboarding/oauth_flow.rs`: OAuth URL generation and code input
- `crates/tui/src/onboarding/slack_setup.rs`: Slack credentials (client_id, client_secret) input
- `crates/tui/src/onboarding/zc_setup.rs`: ZeroClaw setup/pairing UI

## Naming Conventions

**Files:**
- Rust module files: snake_case (e.g., `oauth_flow.rs`, `app_async_event.rs`)
- Structure organization: module per concept (api.rs, socket.rs, auth.rs)
- Test functions: co-located in each file with `#[cfg(test)] mod tests`

**Functions:**
- Public methods: snake_case (e.g., `handle_key_event`, `send_message`, `with_retry`)
- Private helper methods: prefixed with underscore or in impl blocks marked `pub(super)`
- Async functions: return `Future`, named descriptively (`run`, `connect_and_listen`, `send_to_agent`)

**Variables:**
- State fields in App struct: descriptive names in camelCase (e.g., `active_workspace`, `selected_channel`, `last_error`)
- Local variables: snake_case with type hints when needed
- Channel variables: suffixed with `_tx` (sender) or `_rx` (receiver)

**Types:**
- Structs: PascalCase (e.g., `SlackApi`, `SocketModeClient`, `AgentRunner`)
- Enums: PascalCase variants (e.g., `SlackEvent`, `InputMode`, `OnboardingScreen`)
- Type aliases: `ApiResult<T>` = `Result<T, ApiError>`

**Constants:**
- SCREAMING_SNAKE_CASE (e.g., `USER_CACHE_TTL`, `MAX_RETRIES`, `TOPBAR_HEIGHT`, `MIN_SIDEBAR_WIDTH`)

## Where to Add New Code

**New Feature (e.g., reaction emoji picker):**
- Primary code: `crates/tui/src/app/actions.rs` (new action handler method)
- Types: Add variant to relevant enum in `crates/tui/src/app/types.rs` (e.g., ContextMenuAction::AddReaction)
- Rendering: Add render method in `crates/tui/src/app/render.rs`
- Tests: Add tests in respective files using existing test patterns

**New Slack API Endpoint:**
- Implementation: Add method to `SlackApi` impl in `crates/slack/src/api.rs`
- Retry logic: Wrap in `with_retry()` call
- Error mapping: Add to `map_anyhow_error()` if new error patterns emerge
- Types: Add type in `crates/slack/src/types.rs` if new data structures needed

**New Agent Command:**
- Command parsing: Add variant to `CommandType` enum in `crates/agent/src/commands.rs`
- Webhook payload: Add arm to `to_webhook_payload()` match expression
- UI integration: Add case to app action handler in `crates/tui/src/app/actions.rs`

**New UI Component/Modal:**
- Layout: Extend `PanelType` enum in `crates/tui/src/ui/panel.rs` (if full panel) OR add overlay flag to App state in `crates/tui/src/app/state.rs`
- Rendering: Add render method in `crates/tui/src/app/render.rs`, called from render dispatcher
- State: Add fields to App struct in `crates/tui/src/app/state.rs` (show_X flag, data for content)
- Input handling: Add key/mouse cases in `crates/tui/src/app/input.rs`

**New Configuration Option:**
- Definition: Add field to relevant struct in `crates/tui/src/config.rs` (SlackConfig, ZeroClawConfig, or LlmConfig)
- TOML parsing: Config::load_or_default() automatically parses via serde
- Usage: Access via `self.config.slack.client_id` pattern in app

**Tests:**
- Unit tests: Co-located in each file with `#[cfg(test)] mod tests { ... }`
- Test pattern: Use `#[tokio::test]` for async, `#[test]` for sync
- Example: `crates/slack/src/api.rs` has tests for retry logic; `crates/tui/src/app/mod.rs` has tests for message routing

## Special Directories

**target/ - Build Output:**
- Purpose: Compiled binaries, dependencies, intermediate artifacts
- Generated: Yes (by cargo build)
- Committed: No (in .gitignore)

**config/ - Configuration Template:**
- Purpose: Default TOML config file
- Generated: No (manually maintained template)
- Committed: Yes (checked into git)

**docs/ - Documentation:**
- Purpose: Product requirements, task tracking, security checklists
- Generated: No (manually written markdown)
- Committed: Yes

**crates/*/src/app/tests - Inline Tests:**
- Purpose: Unit tests for app logic (message routing, thread tracking)
- Generated: No (hand-written)
- Committed: Yes (in test modules within source files)

