# AGENT.md - Code Best Practices for slack-zc

## Project Overview

**slack-zc** is a terminal Slack client with ZeroClaw AI agent integration, built with Rust/Ratatui.

## Architecture Principles

### 1. Workspace Structure
- **3-crate workspace**: `tui`, `slack`, `agent`
- **TUI crate**: User interface, event handling, rendering
- **Slack crate**: API client, Socket Mode, OAuth, types
- **Agent crate**: ZeroClaw gateway, command parsing

### 2. Module Boundaries
```
slack-zc/
├── crates/
│   ├── tui/      # UI only - no direct API calls
│   ├── slack/    # All Slack API interaction
│   └── agent/    # ZeroClaw agent communication
```

## Coding Standards

### Error Handling
- Use `anyhow::Result` for propagation-friendly errors
- Create typed error enums for distinct failure modes
- Include user-facing remediation hints in errors
- Never expose raw tokens in error messages

```rust
// Good: typed error with context
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Rate limited. Retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

// Bad: generic error loses context
pub fn foo() -> Result<()> { Err(anyhow!("failed")) }
```

### Async Patterns
- **Never block the UI thread** with `block_on` in interactive flows
- Use tokio spawn for fire-and-forget operations
- All network calls must be async

```rust
// Good: async in App context
rt.block_on(async {
    api.get_history(&token, &channel_id, 50).await
});

// Bad: blocks UI thread
fn on_key_press() {
    let rt = tokio::runtime::Handle::current();
    rt.block_on(api.call()); // Blocks event loop
}
```

### Security Practices
- **Never log tokens or secrets** - always redact
- Use `0600` file permissions for session data
- Encrypt sensitive data at rest (AES-GCM)
- Set timeouts on all HTTP clients (connect + request)

```rust
// Good: redact sensitive data in logs
tracing::info!("API call to {} completed", redact_url(url));

// Good: secure file permissions
std::fs::set_permissions(path, Permissions::from_mode(0o600))?;

// Good: timeouts on HTTP client
reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .connect_timeout(Duration::from_secs(10))
    .build()?;
```

### Rate Limiting
- Implement retry with exponential backoff + jitter
- Handle 429 responses explicitly
- Track rate limits per workspace

```rust
async fn with_retry<F, T>(mut f: F) -> Result<T>
where
    F: FnMut() -> futures::future::BoxFuture<Result<T>>,
{
    let mut attempts = 0;
    let max_attempts = 3;
    
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if is_rate_limited(&e) && attempts < max_attempts => {
                let delay = calculate_backoff(attempts);
                tokio::time::sleep(delay).await;
                attempts += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### Type Safety
- Use newtypes for domain concepts
- Avoid `String` when `&str` suffices
- Prefer enums over boolean flags

```rust
// Good: typed channel ID
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChannelId(String);

// Good: enum for input modes
enum InputMode {
    Normal,
    AgentCommand,
    AgentMention,
}

// Bad: boolean explosion
struct Config {
    is_agent_mode: bool,
    is_editing: bool,
    is_searching: bool,
}
```

### Testing
- Unit tests for pure functions
- Integration tests for API error scenarios
- Test Socket Mode reconnect behavior

## MVP Quality Checklist

### Before Code Review
- [ ] No `unwrap()` in production code
- [ ] No `expect()` without context
- [ ] All errors have user-friendly messages
- [ ] No sensitive data in logs
- [ ] Timeouts on all network calls
- [ ] File permissions set correctly (0600)

### Before Release
- [ ] Retry + backoff for API calls
- [ ] Proper error categories (auth/network/rate-limit)
- [ ] Rate limit handling (429 responses)
- [ ] Token rotation path exists
- [ ] Health diagnostics available
- [ ] Integration tests pass

## Key Dependencies

```toml
# Required for this project
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls", "multipart"] }
ratatui = "0.29"
anyhow = "1"
thiserror = "1"
tracing = "0.1"
aes-gcm = "0.10"
```

## Anti-Patterns to Avoid

1. **Blocking the event loop**: Never use `block_on` in key handlers
2. **Silent failures**: Always log or propagate errors
3. **Leaking tokens**: Redact all sensitive data from logs
4. **No timeouts**: Every network call needs bounds
5. **Global state**: Prefer dependency injection
6. **Magic strings**: Use constants or enums

## File Organization

```
crates/slack/src/
├── api.rs          # Slack REST client
├── auth.rs         # OAuth + session management  
├── socket.rs       # Socket Mode client
├── types.rs        # Domain types (Channel, Message, User)
└── lib.rs          # Public API exports

crates/tui/src/
├── app.rs          # Main application state + handlers
├── main.rs         # Entry point
├── config.rs       # Configuration loading
├── input.rs        # Input mode handling
├── keybinds.rs    # Keyboard shortcuts
├── ui/             # Rendering components
│   ├── layout.rs
│   └── panel.rs
└── onboarding/     # Setup wizard
```

## Performance Considerations

- Cache user info with TTL (avoid `users.list` on every message)
- Lazy-load message history
- Use `&str` for string slices where possible
- Batch API calls when possible

## Documentation

- Document public APIs with doc comments
- Include error cases in function docs
- Keep README updated with current status

---

# Critical Fixes & Code Review Items

This section documents specific bugs and issues identified during code review that **must be fixed**.

## 1. Dead Code & Unsafe Patterns

### OAuth Server (DELETE)
- **File**: `crates/slack/src/oauth_server.rs`
- **Issue**: `start_oauth_server` calls `std::process::exit(0)` on OAuth redirect, killing the TUI process
- **Fix**: Delete `oauth_server.rs` entirely. Remove `pub mod oauth_server` from `lib.rs`.

### AgentRunner Drop (FIX)
- **File**: `crates/agent/src/runner.rs`
- **Issue**: Drop impl calls `tokio::runtime::Handle::current().block_on(...)` which panics if runtime is shut down
- **Fix**:
```rust
impl Drop for AgentRunner {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        self.gateway = None;
    }
}
```

## 2. Security Issues

### Credential Logging (FIX)
- **File**: `crates/agent/src/runner.rs`
- **Issue**: Line 84 logs pairing code in plaintext: `info!("Found pairing code: {}", code)`
- **Fix**: `info!("ZeroClaw pairing code obtained (redacted)")`

### Token Redaction (ALWAYS)
- Never log URLs containing tokens
- Use redacted format: `info!("API call to {} completed", redact_url(url))`

## 3. Performance Issues

### User Cache (REQUIRED)
- **File**: `crates/slack/src/api.rs`
- **Issue**: `get_history` and `get_thread_replies` call `list_users` on every invocation, burning rate limits
- **Fix**: Add TTL-based user cache to `SlackApi`:

```rust
const USER_CACHE_TTL: Duration = Duration::from_secs(600);

#[derive(Clone)]
pub struct SlackApi {
    client: Client,
    user_cache: Arc<tokio::sync::RwLock<UserCache>>,
}

struct UserCache {
    users: HashMap<String, User>,
    updated_at: Option<Instant>,
}

impl SlackApi {
    pub async fn get_users_cached(&self, token: &str) -> HashMap<String, User> {
        let cache = self.user_cache.read().await;
        if let Some(ref updated_at) = cache.updated_at {
            if updated_at.elapsed() < USER_CACHE_TTL && !cache.users.is_empty() {
                return cache.users.clone();
            }
        }
        drop(cache);

        let users = self.list_users(token).await.unwrap_or_default();
        let mut cache = self.user_cache.write().await;
        cache.users = users.clone();
        cache.updated_at = Some(Instant::now());
        users.into_iter().map(|u| (u.id.clone(), u)).collect()
    }
}
```

## 4. Clippy Warnings (MUST FIX)

### needless_borrow
```rust
// Bad
if let Some(ref ws) = self.workspaces.get(idx) { ... }

// Good  
if let Some(ws) = self.workspaces.get(idx) { ... }
```

### unnecessary_map_or
```rust
// Bad
.map_or(false, |c| c > 0)

// Good
.is_some_and(|c| c > 0)
```

### iter_nth on VecDeque
```rust
// Bad
.iter().nth(n)

// Good
.get(n)
```

### collapsible_if
```rust
// Bad
if condition {
    if other { ... }
}

// Good
if condition && other { ... }
```

### manual_strip
```rust
// Bad
if s.starts_with('#') {
    s[1..].to_string()
}

// Good
if let Some(stripped) = s.strip_prefix('#') {
    stripped.to_string()
}
```

### unwrap_or_default
```rust
// Bad
.or_insert_with(Vec::new)

// Good
.or_default()
```

### new_without_default
- Add `impl Default` for `SlackApi`, `InputState`, `OnboardingState`, `App`

## 5. App Task Cloning

- **File**: `crates/tui/src/app.rs`
- **Issue**: Task closures create `SlackApi::new()` fresh each time, losing cache
- **Fix**: Clone api before spawning:
```rust
let api = self.slack_api.clone();
self.spawn_app_task(async move {
    match api.get_history(&token, &channel_id, 50).await { ... }
});
```

## Verification Commands

After any changes, run:
```bash
cargo build           # 0 errors, 0 warnings
cargo clippy -- -W clippy::all  # 0 warnings
cargo test            # all tests pass
```
