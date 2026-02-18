# slack-zc

## What This Is

slack-zc is a terminal Slack client (TUI) built in Rust that integrates ZeroClaw AI directly into the Slack experience. Developers can read channels, send messages, and invoke AI commands (`/résume`, `/draft`, `/cherche`) without leaving the terminal. The ZeroClaw agent runs as a local subprocess paired via a gateway webhook.

## Core Value

AI-assisted Slack without a browser — slash commands that trigger ZeroClaw responses posted back to Slack threads, all from a keyboard-driven TUI.

## Requirements

### Validated

- ✓ 3-column TUI layout (sidebar, messages panel, agent panel) — Phase 1
- ✓ Slack Socket Mode connection with reconnect backoff — Phase 1
- ✓ OAuth flow with AES-GCM encrypted session storage (0600 permissions) — Phase 1
- ✓ Channel list and message history loading — Phase 1
- ✓ Send messages to channels — Phase 1
- ✓ ZeroClaw AgentRunner — spawn subprocess, pairing flow, shutdown — Phase 1
- ✓ GatewayClient — pair, health_check, send_to_agent via webhook — Phase 1
- ✓ Onboarding screens (welcome → credentials → OAuth → pairing) — Phase 1
- ✓ `/résume`, `/draft`, `/cherche` command parsing and webhook dispatch — Phase 2
- ✓ Thread-aware agent replies (tracks thread_ts, posts to thread) — Phase 2
- ✓ Agent response display in agent panel — Phase 2
- ✓ UI polish: thread view, emoji reactions, message edit/delete — Phase 3
- ✓ Typing indicators, unread badges — Phase 3
- ✓ File upload/download — Phase 3
- ✓ Context menus, navigation, date picker — Phase 3
- ✓ HTTP hardening: timeouts, user-agent, redacted logs — Production hardening pass

### Active

- [ ] Slack API retry + 429-aware backoff with jitter
- [ ] Typed error model with user-facing remediation hints (auth/network/rate-limit/validation)
- [ ] Secure token lifecycle: logout flow and token purge
- [ ] Integration tests for reconnect, retry, and degraded network scenarios
- [ ] Comprehensive secret redaction in all log paths
- [ ] Dependency vulnerability scan in CI (GitHub Actions)
- [ ] Gateway timeout handling for agent webhook calls
- [ ] Loading indicator during agent command processing
- [ ] Docker distribution: multi-stage Dockerfile, docker-compose, GHCR publishing
- [ ] GitHub Actions CI/CD: build + push on tag, release workflow

### Out of Scope

- Native mobile or desktop app — terminal-only by design
- Multi-workspace support — single workspace per session for v1
- Slack web API fallback (non-Socket Mode) — Socket Mode is the architecture
- Replacing Slack's full feature set — focused on core messaging + AI commands

## Context

Three-crate Rust workspace: `crates/slack/` (Slack API + WebSocket), `crates/agent/` (ZeroClaw gateway), `crates/tui/` (Ratatui app, main binary). Stack: Rust 2021, Tokio 1, Ratatui 0.29, Crossterm 0.28, Reqwest 0.12, tokio-tungstenite 0.26, rustls (no OpenSSL). Session encrypted AES-256-GCM. Config at `~/.config/slack-zc/config.toml`.

Phase 1 and Phase 3 are complete. Phase 2 is mostly done (timeout handling and loading indicator remain). Production hardening pass ran (security fixes committed as e209e9e). Phase 4 (Docker) not started.

## Constraints

- **Tech stack**: Rust only — no Node, Python, or shell scripts for core logic
- **TLS**: rustls exclusively — no OpenSSL dependency for portability
- **Distribution**: statically linked binary + Docker image; no package manager dependencies at runtime
- **Security**: Tokens must never appear in logs; session files must be 0600 on Unix

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust + Ratatui for TUI | Performance + single binary distribution | ✓ Good |
| Socket Mode (not RTM) | Modern Slack API, no deprecated RTM | ✓ Good |
| rustls over OpenSSL | Cross-platform, no C deps | ✓ Good |
| AES-GCM session encryption | Credentials at rest secured | ✓ Good |
| ZeroClaw via subprocess + webhook | Decoupled agent lifecycle | — Pending |
| Multi-stage Docker build | Minimal image size, alpine runtime | — Pending |

---
*Last updated: 2026-02-18 after initialization from docs/ phase task files*
