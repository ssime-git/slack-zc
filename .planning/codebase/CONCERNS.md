# Codebase Concerns

**Analysis Date:** 2026-02-18

## Tech Debt

**Excessive Cloning in Message Processing:**
- Issue: High-frequency message clone operations in state management and event processing
- Files: `crates/tui/src/app/effects.rs` (lines 6-32), `crates/tui/src/app/state.rs` (line 14), `crates/slack/src/api.rs` (lines 171, 179, 186)
- Impact: Performance degradation with high message volume; messages cloned for HashMap operations, async task spawning, and cache management
- Fix approach: Implement Arc<Message> or Rc<Message> for shared ownership; use move semantics and references where possible; refactor cache strategy to avoid repeated cloning of user maps

**Uncontrolled Message Buffer Growth:**
- Issue: `messages` HashMap with unbounded VecDeque accumulation per channel
- Files: `crates/tui/src/app/state.rs` (line 14), `crates/tui/src/app/effects.rs` (lines 112-115)
- Impact: Memory leak potential; long-running sessions will consume unbounded memory as messages accumulate without any trimming or retention policy
- Fix approach: Implement message limit per channel (configurable window like "keep last 500 messages"); add periodic cleanup of old messages; implement circular buffer with fixed capacity

**Double-Check Pattern Race Condition:**
- Issue: User cache double-check pattern in SlackApi may suffer from TOCTOU (Time-of-check-Time-of-use) between checks
- Files: `crates/slack/src/api.rs` (lines 166-192)
- Impact: Cache coherence issues under concurrent access; multiple threads might attempt simultaneous refreshes
- Fix approach: Use RwLock::try_write() with immediate retry; implement generation numbers or version tags for cache invalidation

**Unbounded Retry Backoff Without Upper Cap:**
- Issue: Exponential backoff in retry logic can grow exponentially indefinitely
- Files: `crates/slack/src/api.rs` (lines 113-117) - `2^attempt` grows exponentially
- Impact: Could cause excessive delays; BASE_DELAY_MS (1000ms) * 2^2 = 4s, 2^3 = 8s, 2^4 = 16s - problematic for interactive app
- Fix approach: Add configurable maximum delay cap (e.g., 30 seconds); consider jitter distribution to prevent thundering herd

## Known Bugs

**File Upload Form Logic Error:**
- Symptoms: File upload succeeds or fails inconsistently; response may be undefined
- Files: `crates/slack/src/api.rs` (lines 744-822)
- Trigger: When comment is provided, form is sent twice - once inside the `if` block and again after
- Workaround: None; requires code fix
- Fix: Remove the duplicate upload request; consolidate comment handling into single path

**OAuth Code Exchange Never Validates Bearer Token:**
- Symptoms: Session saved with potentially invalid tokens
- Files: `crates/slack/src/auth.rs` (lines 215-247)
- Trigger: OAuth response parsed without validation of token format or length
- Workaround: Manual token verification after pairing
- Fix: Validate token structure (should start with 'xoxp-' or 'xoxb-') before storing; add length checks

**ZeroClaw Process Leak on Error:**
- Symptoms: ZeroClaw gateway process left running after pairing failure
- Files: `crates/agent/src/runner.rs` (lines 53-95) - child process not killed on error path
- Trigger: If `gateway.pair()` fails (line 89), child process continues running in background
- Workaround: Manual `kill -9` on zeroclaw process
- Fix: Wrap child process in managed structure that kills on drop; add explicit cleanup before returning error

**Silent Slack API Error Swallowing:**
- Symptoms: Failed API calls don't propagate full error context
- Files: `crates/slack/src/api.rs` (lines 145-147) - error detection uses string matching on error messages
- Trigger: Slack API returns error field that's not "429" and not "rate_limited"
- Workaround: None
- Fix: Parse error response into typed Error enum; check multiple error signal fields (error, response_metadata.messages, ok=false)

**Hardcoded OAuth Redirect URI:**
- Symptoms: Cannot use different ports or domains for OAuth flow
- Files: `crates/tui/src/app/effects.rs` (line 77) - hardcoded "http://localhost:3000"
- Trigger: User attempts to run on non-standard port
- Workaround: Modify source code
- Fix: Make redirect URI configurable via config file or environment variable

## Security Considerations

**Token Storage Without Encryption Verification:**
- Risk: Session encryption relies on machine-local secret key stored on disk
- Files: `crates/slack/src/auth.rs` (lines 46-79) - secret key stored unencrypted at `~/.local/share/slack-zc/slack-zc/.secret_key`
- Current mitigation: File permissions set to 0o600 on Unix; Windows has no equivalent protection
- Recommendations:
  - Add OS keychain integration (keyring crate) for credential storage
  - Hash the secret key with machine identifier to prevent key theft attacks
  - Document security model clearly (file system depends on OS security)

**Bearer Token in Error Messages:**
- Risk: Gateway bearer token visible in error logs and user-facing error messages
- Files: `crates/agent/src/gateway.rs` (lines 48, 88) - bearer token in error messages not redacted
- Current mitigation: Partial redaction in `crates/tui/src/app/mod.rs` (lines 50-56) only masks in UI, not logs
- Recommendations:
  - Implement error type that redacts sensitive fields
  - Remove bearer token from Err() results in gateway
  - Ensure all error paths use redaction wrapper

**WebSocket Token Visible in Logs:**
- Risk: Socket mode URL contains query parameters with tokens
- Files: `crates/slack/src/socket.rs` (lines 219-224) - redaction only happens in one log statement
- Current mitigation: Manual redaction at log point
- Recommendations: Implement automatic URI sanitization that redacts all token-like parameters; apply at WebSocket connect point

**Slack Token Logged in Test Code:**
- Risk: Test fixtures may contain real token patterns
- Files: `crates/slack/src/api.rs` (lines 88-93) - tests with "fake_token"
- Current mitigation: Tests use fake tokens
- Recommendations: Ensure no real tokens are ever committed; add git pre-commit hook to detect token patterns

## Performance Bottlenecks

**User Cache Full Reload on Every Refresh:**
- Problem: User cache forces complete reload of all workspace users for display name resolution
- Files: `crates/slack/src/socket.rs` (lines 255-273) - loads all users to build HashMap
- Cause: No incremental update; every message received may trigger full user list fetch
- Improvement path: Cache individual users; implement delta updates; use user_id as display name fallback

**Message Timestamp Parsing Repeated Per Message:**
- Problem: Every message parses timestamp with split() and parse<i64>() chains
- Files: `crates/slack/src/types.rs` (lines 58), `crates/slack/src/socket.rs` (lines 197-206)
- Cause: No caching of parsed timestamps; inefficient string parsing in hot path
- Improvement path: Store parsed timestamp in Message struct; cache timestamp format parsing

**HashMap Rebuilding on Every Channel History Load:**
- Problem: User map rebuilt for every history fetch, cloning all users
- Files: `crates/slack/src/api.rs` (lines 356, 736)
- Cause: `get_users_cached()` returns HashMap clone; users matched in filter_map on every message
- Improvement path: Cache user display names separately; pass reference to cached HashMap, not clone

**Unbounded Agent Response Queue:**
- Problem: agent_responses VecDeque grows without limit
- Files: `crates/tui/src/app/state.rs` (line 13)
- Cause: No removal policy for old responses
- Improvement path: Limit to last N responses (e.g., 100); implement time-based expiration

## Fragile Areas

**Socket Mode Reconnection Logic:**
- Files: `crates/slack/src/socket.rs` (lines 50-67)
- Why fragile: Exponential backoff can delay reconnection indefinitely; no maximum backoff cap enforced; backoff state not exposed to caller for health monitoring
- Safe modification: Add explicit max_backoff constant (currently implicit); expose backoff state to App for UI feedback; add heartbeat mechanism
- Test coverage: Only basic event enum tests exist; no connection failure/recovery tests

**Onboarding State Machine:**
- Files: `crates/tui/src/app/input.rs` (lines 42-94), `crates/tui/src/onboarding/mod.rs`
- Why fragile: Complex multi-screen flow with deeply nested conditionals; no state invariant enforcement; easy to transition to invalid states
- Safe modification: Use typed state enum instead of OnboardingScreen + field flags; add transition validation methods; add state transition tests
- Test coverage: No onboarding flow tests; modal UI state tests missing

**File Upload Duplicate Logic:**
- Files: `crates/slack/src/api.rs` (lines 744-822)
- Why fragile: Form initialized before loop; sent twice when comment present; conditional return creates dead code path
- Safe modification: Refactor to build form once; separate comment handling into preparatory step before send; add unit tests for upload paths
- Test coverage: No file upload tests

**Task Spawning Error Suppression:**
- Files: `crates/tui/src/app/mod.rs` (lines 58-68) - `let _ = tx.send(event)` silently drops send errors
- Why fragile: Receiver might be dropped; error silently lost; task completes without feedback
- Safe modification: Log dropped errors; add monitoring for task completion; consider using channel with bounded queue and backpressure
- Test coverage: No async task completion tests

## Scaling Limits

**Single-Threaded Workspace Processing:**
- Current capacity: Single workspace at a time; 50 messages per channel history fetch
- Limit: Adding workspaces loads all sequentially; message history limited by API window
- Scaling path:
  - Batch workspace initialization with tokio::spawn
  - Implement pagination for historical message loading (cursor-based)
  - Add background sync for channel message deltas

**Memory Unbounded by Message Count:**
- Current capacity: No limit on messages stored; grows linearly with activity
- Limit: Long-running sessions (1000s of messages) consume significant RAM
- Scaling path: Implement message window (keep last N per channel); implement file-based message history

**Slack API Rate Limiting:**
- Current capacity: Fixed 3 retries with backoff
- Limit: High-frequency operations (typing indicators, thread loads) can hit rate limits
- Scaling path: Implement token bucket algorithm; track rate limit headers from Slack responses; implement priority queue for API calls

## Dependencies at Risk

**Tokio with "full" Features:**
- Risk: Brings in all tokio features including rarely-used ones; increases attack surface and compile time
- Files: `Cargo.toml` (line 6)
- Impact: Larger binary; potential security issues in unused features
- Migration plan: Audit which tokio features actually used; replace "full" with minimal set (rt, sync, time, macros)

**Old TUI Libraries:**
- Risk: Ratatui 0.29 and Crossterm 0.28 are aging; check for CVEs
- Files: `crates/tui/Cargo.toml` (lines 14-15)
- Impact: May have known vulnerabilities
- Migration plan: Update to latest versions; test UI rendering compatibility

**No Explicit TLS Validation:**
- Risk: reqwest configured with rustls-tls but no explicit certificate pinning
- Files: `Cargo.toml` (line 9)
- Impact: Vulnerable to MITM if system CA compromised
- Migration plan: Consider certificate pinning for slack.com endpoints; add TLS verification tests

## Missing Critical Features

**No Graceful Shutdown Sequence:**
- Problem: App termination doesn't wait for pending tasks
- Blocks: Long-running operations may be cut off; unsaved state lost
- Fix approach: Implement shutdown channel broadcast; wait for all spawned tasks with timeout; persist pending operations

**No Message History Persistence:**
- Problem: Messages lost on restart
- Blocks: Cannot browse history after quit
- Fix approach: Implement SQLite or local JSON message store; add message sync on startup

**No Rate Limit Awareness:**
- Problem: App doesn't adapt to Slack rate limits
- Blocks: Heavy usage causes exponential backoff delays without user feedback
- Fix approach: Parse rate-limit headers from responses; implement adaptive queue; show rate limit status in UI

**No Offline Mode:**
- Problem: App requires active Slack connection
- Blocks: Cannot read cached messages when Slack is unreachable
- Fix approach: Implement message cache layer; queue outgoing messages; sync when reconnected

## Test Coverage Gaps

**No Socket Mode Integration Tests:**
- What's not tested: WebSocket connection, message parsing, event dispatch
- Files: `crates/slack/src/socket.rs` (lines 276-330 - only basic unit tests)
- Risk: Reconnection logic, error handling, and event streaming untested
- Priority: High - core real-time functionality

**No File Upload Tests:**
- What's not tested: Multi-part form handling, error cases, response parsing
- Files: `crates/slack/src/api.rs` (lines 744-822 - no tests)
- Risk: Upload could silently fail due to form building bug
- Priority: High - data loss potential

**No Onboarding Flow Tests:**
- What's not tested: OAuth exchange, workspace save, state transitions
- Files: `crates/tui/src/onboarding/` (full module untested)
- Risk: OAuth flow breaks silently; tokens saved incorrectly
- Priority: High - blocks new users

**No Concurrent Message Handling Tests:**
- What's not tested: Race conditions in message dispatch, thread safety
- Files: `crates/tui/src/app/effects.rs` - concurrent channel/workspace operations
- Risk: Data corruption from concurrent HashMap access
- Priority: Medium - subtle race conditions

**No Agent Runner Lifecycle Tests:**
- What's not tested: Process startup, pairing, error recovery, cleanup
- Files: `crates/agent/src/runner.rs` (lines 27-152 - no tests)
- Risk: ZeroClaw process leaks; pairing failures not handled
- Priority: Medium - process resource leak

---

*Concerns audit: 2026-02-18*
