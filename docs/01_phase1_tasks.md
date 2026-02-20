# Phase 1: TUI Fonctionnel + Bridge Bootstrap

## Status: COMPLETED ✓

## Tasks

### Core Infrastructure
- [x] Create workspace Cargo.toml structure
- [x] Set up 3 crates: tui, slack, agent
- [x] Wire App::init() in main.rs ✓
- [x] Implement config loading ✓
- [x] Connect Socket Mode event processing ✓

### TUI Components ✓
- [x] Responsive 3-column layout with drag resize
- [x] Sidebar with channels + workspace tabs
- [x] Messages panel with scroll
- [x] Agent panel status display
- [x] Input bar with mode detection (/, @, normal)
- [x] Hit registry for mouse clicks
- [x] Keybindings

### Slack Integration ✓
- [x] SlackApi client (list_channels, get_history, send_message, etc.)
- [x] Socket Mode connection
- [x] OAuth flow structure
- [x] Session encryption (AES-GCM)

### Agent Integration ✓
- [x] GatewayClient (pair, health_check, send_to_agent)
- [x] AgentRunner (spawn, pairing, shutdown)

### Onboarding
- [x] OnboardingState structure
- [x] Implement Welcome screen ✓
- [x] Implement all onboarding screens (basic UI) ✓
- [x] Wire credentials input to OAuth URL generation ✓
- [x] Wire OAuth code entry and token exchange ✓

### Missing Pieces to Make Fully Functional
- [x] Load config.toml on startup ✓
- [x] Call App::init() from main ✓
- [x] Spawn tokio runtime for async operations ✓
- [x] Process SlackEvent channel in event loop ✓
- [x] Load message history when channel selected ✓
- [x] Implement full onboarding flow UI ✓
- [ ] Wire Socket Mode event channels properly (connect events to UI)

## Dependencies Graph

```
config.toml loading
       ↓
App::init() + tokio runtime
       ↓
├── Socket Mode events → process SlackEvent ✓
├── Onboarding flow (if no session) ✓
└── Channel selection → load message history ✓
```

## Recent Changes

### 2024-XX-XX
- Added OAuth code entry screen (manual code from browser)
- Added OAuth token exchange and session saving
- Added oauth_server module with callback support
- Added message history loading when channel selected
- Added SlackEvent processing in main loop
- Wired OAuth credentials input with field selection (Tab)
- Added ZeroClaw pairing flow with AgentRunner
- Added OAuth URL generation

## Notes
- Using rustls instead of openssl for TLS
- Ratatui 0.29 with crossterm 0.28
- AES-GCM for session encryption with per-session nonce
