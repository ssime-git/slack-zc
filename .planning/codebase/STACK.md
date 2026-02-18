# Technology Stack

**Analysis Date:** 2026-02-18

## Languages

**Primary:**
- Rust 2021 edition - All core application logic and CLI

## Runtime

**Environment:**
- Tokio 1.x - Async runtime for concurrent operations

**Package Manager:**
- Cargo - Rust package manager
- Lockfile: Present (`Cargo.lock`)

## Frameworks

**Core:**
- Ratatui 0.29 - TUI rendering and terminal UI components
- Crossterm 0.28 - Terminal event handling and I/O abstraction

**Async/Networking:**
- Tokio 1 (workspace feature: "full") - Async runtime with all features
- Tokio-tungstenite 0.26 - WebSocket client with TLS support
- Reqwest 0.12 - HTTP client library
- Futures 0.3 - Future utilities and stream operations

**Serialization:**
- Serde 1.x - Serialization/deserialization framework with derive support
- Serde_json 1.x - JSON parsing and generation
- Toml 0.8 - TOML configuration file parsing

**Testing:**
- Tokio test framework - Built-in async testing via `#[tokio::test]`

**Build/Dev:**
- Cargo workspace - Multi-crate project structure

## Key Dependencies

**Critical:**
- Tokio 1 (features: "full") - Core async runtime and utilities
- Tokio-tungstenite 0.26 (features: "rustls-tls-webpki-roots") - WebSocket connections to Slack with TLS
- Reqwest 0.12 (features: "json", "rustls-tls", "multipart") - HTTP client for Slack REST API
- Aes-gcm 0.10 - AES-256-GCM encryption for session token storage

**Utilities:**
- Anyhow 1 - Error handling and context
- Thiserror 1 - Structured error types with derive macros
- Regex 1 - Pattern matching for message parsing
- Chrono 0.4 (features: "serde") - Date/time handling with serialization
- Rand 0.8 - Random number generation for crypto and backoff
- Directories 5 - Cross-platform user/config directory resolution
- URL 2 - URL parsing and manipulation
- Tokio-util 0.7 - Additional Tokio utilities
- Tracing 0.1 - Structured logging framework
- Tracing-subscriber 0.3 - Log output formatting and filtering
- Color-eyre 0.6 - Pretty error reporting with colors
- OAuth2 4 - OAuth 2.0 client implementation

## Architecture

**Workspace Structure:**
The project is organized as a Rust workspace with three independent crates:

```
crates/
├── slack/       # Slack API client and WebSocket integration
├── agent/       # Agent gateway client for ZeroClaw integration
└── tui/         # Terminal UI application (main binary)
```

**Crate Dependencies:**
- `tui` depends on both `slack` and `agent` crates
- `slack` and `agent` are independently composable

## Configuration

**Environment:**
- TOML-based configuration at platform-specific config directory
- Location: Determined by `ProjectDirs::from("com", "slack-zc", "slack-zc")`
- Fallback: `config/default.toml` in project root

**Configuration File Location:**
- Unix/Linux: `~/.config/slack-zc/config.toml`
- macOS: `~/Library/Application Support/slack-zc/config.toml`
- Windows: `%APPDATA%\slack-zc\slack-zc\config\config.toml`

**Session Persistence:**
- Location: Platform data directory `session.json`
- Encryption: AES-256-GCM with stored 32-byte key at `.secret_key`
- Permissions: Unix file mode 0o600 (read/write owner only)

## Platform Requirements

**Development:**
- Rust toolchain with cargo
- OpenSSL/TLS development libraries (for rustls-tls)

**Production:**
- Binary targets: Linux, macOS, Windows
- Runtime dependencies: None (fully statically linked with rustls)

## SSL/TLS

**Implementation:**
- Rustls 0.x - Pure Rust TLS implementation (via tokio-tungstenite and reqwest features)
- No OpenSSL dependency - Uses `rustls-tls-webpki-roots` for certificate validation
- WebPKI root certificates bundled at compile time

---

*Stack analysis: 2026-02-18*
