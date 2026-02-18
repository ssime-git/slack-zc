# Testing Patterns

**Analysis Date:** 2026-02-18

## Test Framework

**Runner:**
- Built-in Rust test runner (no external test framework like criterion or proptest)
- Tests are executed with `cargo test` command
- Config: Uses `#[tokio::test]` macro for async tests (provided by tokio crate)

**Assertion Library:**
- Standard Rust assertions: `assert!()`, `assert_eq!()`, `assert_ne!()`
- No external assertion library present

**Run Commands:**
```bash
cargo test                      # Run all tests
cargo test --lib               # Run library tests only
cargo test --release           # Run with optimizations
```

## Test File Organization

**Location:**
- Co-located with source code using `#[cfg(test)] mod tests` pattern
- Tests placed at the bottom of the same file they test

**Naming:**
- Test files: Not applicable (inline tests used)
- Test functions: `test_*` naming convention
- Test modules: `tests` (within the `#[cfg(test)]` block)

**Structure:**
- Tests are defined in modules at the end of each file
- Each test is a standalone `#[test]` or `#[tokio::test]` function
- No separate test directory structure

## Test Structure

**Suite Organization:**

In `crates/slack/src/api.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_after_rate_limit() {
        let attempt_count = Arc::new(AtomicU32::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result: Result<&str, _> = with_retry(move || {
            let attempt_count = attempt_count_clone.clone();
            async move {
                let count = attempt_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(anyhow!("429"))
                } else {
                    Ok("success")
                }
            }
        }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }
}
```

In `crates/tui/src/app/mod.rs`:
```rust
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
            is_edited: false,
            is_deleted: false,
            files: Vec::new(),
            reply_count: None,
            last_read: None,
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

        app.process_slack_events();

        assert_eq!(app.messages.get("C_ONE").map(|m| m.len()), Some(1));
    }
}
```

**Patterns:**

1. **Async test pattern:**
```rust
#[tokio::test]
async fn test_retry_success_after_rate_limit() {
    // Test code using await
}
```

2. **Setup pattern - Factory functions:**
```rust
fn sample_message(thread_ts: Option<&str>) -> Message {
    Message {
        ts: "1730000000.100000".to_string(),
        user_id: "U123".to_string(),
        // ... field initialization
    }
}

#[test]
fn test_uses_factory() {
    let msg = sample_message(None);
    // assertions
}
```

3. **State initialization pattern:**
```rust
#[test]
fn test_creates_app() {
    let mut app = App::new();
    // Test initialized state
    assert_eq!(app.messages.len(), 0);
}
```

4. **Closure-based behavior testing (for async retries):**
```rust
let result: Result<&str, _> = with_retry(move || {
    let attempt_count = attempt_count_clone.clone();
    async move {
        let count = attempt_count.fetch_add(1, Ordering::SeqCst);
        if count < 2 {
            Err(anyhow!("429"))
        } else {
            Ok("success")
        }
    }
}).await;

assert!(result.is_ok());
```

5. **Channel/event testing pattern:**
```rust
let tx = app.event_tx.as_ref().expect("event tx").clone();

tx.send(SlackEvent::Message {
    channel: "C_ONE".to_string(),
    message: sample_message(None),
})
.expect("send first event");

app.process_slack_events();
assert_eq!(app.messages.get("C_ONE").map(|m| m.len()), Some(1));
```

## Mocking

**Framework:** No external mocking framework
- Uses closures and trait objects for behavior injection
- Atomic types (`AtomicU32`) for tracking call counts in async code

**Patterns:**

In `crates/slack/src/api.rs` - testing retry behavior without actual network calls:
```rust
#[tokio::test]
async fn test_retry_fails_after_max_attempts() {
    let attempt_count = Arc::new(AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result: Result<&str, _> = with_retry(move || {
        let attempt_count = attempt_count_clone.clone();
        async move {
            attempt_count.fetch_add(1, Ordering::SeqCst);
            Err(anyhow!("429"))
        }
    }).await;

    assert!(result.is_err());
}
```

**What to Mock:**
- Network operations (use `anyhow!()` to simulate errors)
- Async operations (wrap in closures/futures)
- Stateful behavior (use `Arc<AtomicX>` for counters)
- Event emission (use channels directly)

**What NOT to Mock:**
- Type construction (use actual constructors with `::new()`)
- Core data structures (use real `Message`, `Channel` types)
- Simple utility methods (test them directly)

## Fixtures and Factories

**Test Data:**

```rust
// Factory function pattern in crates/tui/src/app/mod.rs
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
        is_edited: false,
        is_deleted: false,
        files: Vec::new(),
        reply_count: None,
        last_read: None,
    }
}
```

**Location:**
- Defined as helper functions within the test module itself
- Not extracted to separate fixture files
- Scoped to their test module (`#[cfg(test)] mod tests`)

## Coverage

**Requirements:** Not enforced
- No coverage target configured in `Cargo.toml`
- No coverage reporting tools visible

**View Coverage:**
```bash
# Using tarpaulin (not configured in project):
cargo tarpaulin

# Using llvm-cov (if installed):
cargo llvm-cov
```

## Test Types

**Unit Tests:**
- Scope: Individual functions and their return values
- Approach: Direct function calls with various inputs
- Examples: `test_retry_success_after_rate_limit`, `test_retry_fails_after_max_attempts`
- Files: `crates/slack/src/api.rs` (testing `calculate_backoff()`, `with_retry()`, caching behavior)

**Integration Tests:**
- Scope: Multi-component interactions (message routing, thread tracking, event handling)
- Approach: Create app state, send events, assert state changes
- Examples: `routes_messages_to_their_source_channel()`, `tracks_thread_context_per_channel()`
- Files: `crates/tui/src/app/mod.rs`

**E2E Tests:**
- Not used in codebase
- Full application testing would require running the binary and UI framework

## Common Patterns

**Async Testing:**

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}

// Testing async retries with closures:
#[tokio::test]
async fn test_retry_success_after_rate_limit() {
    let attempt_count = Arc::new(AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result: Result<&str, _> = with_retry(move || {
        let attempt_count = attempt_count_clone.clone();
        async move {
            let count = attempt_count.fetch_add(1, Ordering::SeqCst);
            if count < 2 {
                Err(anyhow!("429"))
            } else {
                Ok("success")
            }
        }
    }).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
}
```

**Error Testing:**

```rust
#[tokio::test]
async fn test_retry_does_not_retry_non_rate_limit_errors() {
    let attempt_count = Arc::new(AtomicU32::new(0));
    let attempt_count_clone = attempt_count.clone();

    let result: Result<&str, _> = with_retry(move || {
        let attempt_count = attempt_count_clone.clone();
        async move {
            attempt_count.fetch_add(1, Ordering::SeqCst);
            Err(anyhow!("some other error"))  // Non-retryable error
        }
    }).await;

    assert!(result.is_err());
    // Could add: assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
}
```

**Event-Driven Testing:**

```rust
#[test]
fn routes_messages_to_their_source_channel() {
    let mut app = App::new();
    let tx = app.event_tx.as_ref().expect("event tx").clone();

    // Send events through channel
    tx.send(SlackEvent::Message {
        channel: "C_ONE".to_string(),
        message: sample_message(None),
    })
    .expect("send first event");

    // Process and assert
    app.process_slack_events();
    assert_eq!(app.messages.get("C_ONE").map(|m| m.len()), Some(1));
}
```

## Testing Gaps

**Not currently tested:**
- UI rendering logic (`crates/tui/src/app/render.rs` - 671 lines, no tests visible)
- Socket mode real-time connections (`crates/slack/src/socket.rs` - 331 lines, no tests visible)
- Agent integration commands (`crates/agent/src/commands.rs` - 123 lines, no tests visible)
- Gateway client HTTP interactions (`crates/agent/src/gateway.rs` - 103 lines, no tests visible)
- Input handling state machine (`crates/tui/src/input.rs` - 452 lines, no tests visible)
- Complex action handlers (`crates/tui/src/app/actions.rs` - 565 lines, partial coverage)

**Total lines without visible tests: ~2,000+ lines (40% of codebase)**

---

*Testing analysis: 2026-02-18*
