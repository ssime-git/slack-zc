# Coding Conventions

**Analysis Date:** 2026-02-18

## Naming Patterns

**Files:**
- Lowercase with underscores: `socket.rs`, `gateway.rs`, `actions.rs`, `input.rs`, `state.rs`
- Directory names also lowercase: `crates/slack`, `crates/tui`, `crates/agent`
- Modules organized by functionality, not layers

**Functions:**
- snake_case for all function names: `test_auth()`, `list_channels()`, `get_history()`, `handle_input_submit()`
- Async functions use `async fn` keyword
- Helper/private functions use `fn` prefix (not `_`)
- Method names on impl blocks follow snake_case: `get_users_cached()`, `display_name()`, `toggle_collapse()`

**Variables:**
- snake_case for all variables and parameters: `user_id`, `channel_id`, `retry_after`, `attempt_count`
- Abbreviations are expanded (not shortened): `channel_id` not `ch_id`
- Prefix private fields with no underscore convention: `cursor_position`, `user_display_names`, `user_cache_updated_at`

**Types & Structs:**
- PascalCase for struct names: `Channel`, `Message`, `User`, `SlackApi`, `InputState`, `WorkspaceState`, `AgentRunner`
- PascalCase for enum names: `InputMode`, `SlackEvent`, `AppAsyncEvent`, `ContextMenuAction`, `AgentStatus`
- PascalCase for type aliases: `ApiResult<T>`

**Constants:**
- SCREAMING_SNAKE_CASE: `SLACK_API_BASE`, `USER_CACHE_TTL`, `MAX_RETRIES`, `BASE_DELAY_MS`
- Top-level module constants placed before main implementations

## Code Style

**Formatting:**
- Rust edition 2021 (all crates use `edition = "2021"`)
- Standard Rust formatting conventions (standard 4-space indentation)
- No visible formatter config files - uses rustfmt defaults

**Linting:**
- No explicit .clippy or eslint config present
- Code compiles without clippy warnings (see git log: "Fix clippy warnings")
- Uses standard Rust idioms and patterns

## Import Organization

**Order:**
1. External crate imports (workspace dependencies, third-party crates)
2. Internal module imports with `use`
3. Relative imports from parent/sibling modules with `use super::*` or specific imports

**Examples from codebase:**

In `crates/slack/src/api.rs`:
```rust
use crate::types::{Channel, FileInfo, Message, User};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use rand::Rng;
```

In `crates/tui/src/app/mod.rs`:
```rust
use crate::input::{InputMode, InputState};
use crate::keybinds::Keybinds;
use crate::onboarding::{OnboardingScreen, OnboardingState};
use crate::ui::layout::{DragTarget, LayoutState};
use crate::Config;
use anyhow::Result;
use chrono::Utc;
use ratatui::crossterm::event::{...};
```

In `crates/tui/src/app/actions.rs`:
```rust
use super::*;
```

**Path Aliases:**
- Not used in this codebase
- Imports use explicit paths

## Error Handling

**Strategy:** Uses `anyhow::Result<T>` for all fallible operations

**Patterns:**

1. **Custom error types with thiserror** (`crates/slack/src/error.rs`):
```rust
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limited. Retry after {retry_after}s")]
    RateLimited { retry_after: u64 },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type ApiResult<T> = Result<T, ApiError>;
```

2. **Error conversion utilities:**
```rust
pub fn map_anyhow_error(e: anyhow::Error) -> ApiError {
    let msg = e.to_string();
    if msg.contains("429") || msg.contains("rate_limit") {
        ApiError::RateLimited { retry_after: 60 }
    } else if msg.contains("not_authed") || msg.contains("invalid_auth") {
        ApiError::Auth(msg)
    } else {
        ApiError::Api(msg)
    }
}
```

3. **Error methods on enums:**
```rust
impl ApiError {
    pub fn user_message(&self) -> &'static str {
        match self { ... }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self, ApiError::RateLimited { .. } | ApiError::Network(_))
    }
}
```

4. **Question mark operator for propagation:**
```rust
let team_id = data
    .get("team_id")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
```

5. **Match-based error handling in effects/handlers:**
```rust
Err(e) => {
    let is_rate_limited = is_429_error(&e);
    if !is_rate_limited || attempts >= MAX_RETRIES {
        return Err(e);
    }
    let delay = calculate_backoff(attempts);
    tracing::debug!("Rate limited, retrying in {:?}", delay);
    tokio::time::sleep(delay).await;
}
```

## Logging

**Framework:** `tracing` crate (v0.1)

**Patterns:**
- Used selectively for debugging, connection events, and error conditions
- Levels employed: `debug`, `info`, `warn`
- Log before significant operations or state changes

**Usage examples from codebase:**

```rust
// In socket.rs (connection lifecycle):
tracing::info!("Connecting to Socket Mode at {}", Self::redact_socket_url(&url));
tracing::info!("WebSocket connected");
tracing::error!("Socket mode error: {}. Reconnecting in {:?}", e, backoff);
tracing::debug!("ZeroClaw stdout: {}", line);

// In api.rs (retry logic):
tracing::debug!("Rate limited, retrying in {:?}", delay);

// In app/mod.rs (error reporting):
tracing::warn!("{message}");

// In app/input.rs (setup operations):
tracing::info!("OAuth URL generated (redacted)");
```

**Sensitive data redaction:**
```rust
fn redact_sensitive(input: &str) -> String {
    input
        .replace("xoxp-", "xoxp-[REDACTED]-")
        .replace("xapp-", "xapp-[REDACTED]-")
        .replace("xoxb-", "xoxb-[REDACTED]-")
        .replace("Bearer ", "Bearer [REDACTED]")
}
```

## Comments

**When to Comment:**
- Comments are minimal - code is preferred to be self-documenting
- Brief inline comments only for non-obvious logic
- No block-level documentation comments visible

**Example - rare usage in codebase:**
```rust
// Double-check after acquiring write lock
if let Some(updated_at) = cache.updated_at {
    if updated_at.elapsed() < USER_CACHE_TTL {
        return cache.users.clone();
    }
}
```

## Function Design

**Size:**
- Functions range from 5-20 lines (most helper methods)
- Longer functions (50-100 lines) decompose complex state management (e.g., `handle_input_submit()`, `connect_and_listen()`)
- No functions exceed 200 lines

**Parameters:**
- Use explicit types, not tuple/generic conversions
- Common pattern: `&self`, `&str` for tokens/IDs, `usize` for indices
- Methods take owned `String` when storing, borrowed `&str` when querying

**Return Values:**
```rust
// Standard async API return:
pub async fn test_auth(&self, token: &str) -> Result<(String, String)>

// Collections:
pub async fn list_channels(&self, token: &str) -> Result<Vec<Channel>>

// Optional values:
pub fn get_active_channel_id(&self) -> Option<String>

// Boolean predicates:
pub fn is_retryable(&self) -> bool
```

## Module Design

**Exports:**
- Use `pub use` to re-export key types from modules:
  ```rust
  // crates/slack/src/lib.rs
  pub mod api;
  pub mod auth;
  pub mod error;
  pub mod socket;
  pub mod types;

  pub use error::{ApiError, ApiResult};
  pub use types::*;
  ```

- Explicit re-exports in `mod.rs` files:
  ```rust
  // crates/tui/src/app/mod.rs
  pub use state::App;
  pub use types::{
      AgentResponse, AppAsyncEvent, ContextMenu, ContextMenuAction,
      ContextMenuItem, EditState, MessageFilter,
  };
  ```

**Barrel Files:**
- Used in app modules (`crates/tui/src/app/mod.rs`) to organize submodules
- Declares submodules and selectively re-exports important types
- Reduces imports from other modules (can use `use crate::app::App` instead of full path)

**Internal module organization:**
```rust
// In module declaration files (mod.rs):
mod actions;
mod effects;
mod input;
mod render;
mod state;
mod types;

pub use state::App;
pub use types::{...};

impl App {
    // Shared methods
}

#[cfg(test)]
mod tests {
    // Tests colocated with module
}
```

## Workspace Organization

**Crate structure:**
- `crates/slack` - Slack API client, WebSocket handling, types
- `crates/tui` - Terminal UI application
- `crates/agent` - ZeroClaw agent integration (runner, gateway client)
- Workspace-level `Cargo.toml` defines shared dependencies

**Module conventions within crates:**
- Each crate follows the same structure: `src/lib.rs` declares modules, submodules in dedicated files/folders
- Impl blocks are split by concern: one per file in some cases (`socket.rs`, `auth.rs`, `api.rs` for `slack` crate)
- State management separated from rendering (`app/state.rs` vs `app/render.rs`)

---

*Convention analysis: 2026-02-18*
