# AGENT.md - slack-zc Integration Notes

Terminal Slack client with ZeroClaw AI integration (Rust/Ratatui). Workspace split:

```text
crates/slack/   Slack auth, REST, Socket Mode, session handling
crates/tui/     App state, rendering, input, onboarding, local cache
crates/agent/   ZeroClaw gateway client and runner
```

## What Actually Matters

These are the implementation constraints discovered while making the project work end to end.

### 1. Keep The TUI Responsive

- Never block startup on `conversations.list` for large workspaces.
- Load cached channels first, then refresh Slack in the background.
- Emit incremental app events as pages arrive instead of waiting for one giant result.
- If Slack returns `429`, retry with backoff but keep the UI usable.

Relevant files:

- `crates/tui/src/app/effects.rs`
- `crates/tui/src/app/types.rs`
- `crates/tui/src/cache.rs`
- `crates/slack/src/api.rs`

### 2. Treat ZeroClaw As A Gateway, Not As A CLI UI

- Do not rely on parsing human-oriented terminal output as the main integration path.
- The stable contract is the local ZeroClaw gateway API.
- `slack-zc` should prefer:
  1. connect to an existing gateway
  2. start a gateway with existing bearer/local state
  3. fail with a clear remediation message
- Pairing code parsing is only legacy fallback behavior and should not drive the normal UX.

Relevant files:

- `crates/agent/src/runner.rs`
- `crates/agent/src/gateway.rs`
- `crates/tui/src/app/effects.rs`

### 3. Use The Real Webhook Contract

- The working gateway contract is prompt-oriented.
- `/webhook` accepts a payload like:

```json
{ "message": "..." }
```

- Do not send a fake structured command protocol unless ZeroClaw officially supports it.
- Parse JSON responses defensively and prefer `response`, then `message`, then raw text.

Relevant files:

- `crates/agent/src/commands.rs`
- `crates/agent/src/gateway.rs`
- `crates/tui/src/app/actions.rs`

### 4. ZeroClaw Config Must Be Reused

- Reuse `~/.zeroclaw/config.toml` and related local state when possible.
- If ZeroClaw already has a configured gateway port, prefer it over the local fallback config.
- If embedded startup is needed, copied config may require provider normalization.
- In practice, `openai-codex` may be the valid active profile even when a generic `openai` value exists elsewhere.

Relevant files:

- `crates/slack/src/auth.rs`
- `crates/agent/src/runner.rs`

### 5. `dry-run` Must Be The Safe Default

- Agent commands are risky in a real Slack workspace.
- Default behavior must keep responses local in the TUI.
- Real Slack posting must be explicitly enabled by config.
- The UI must show the current mode clearly.

Relevant files:

- `crates/tui/src/config.rs`
- `crates/tui/src/app/actions.rs`
- `crates/tui/src/app/render.rs`

### 6. Timeouts Need To Be Per Command

- A single global HTTP timeout is too blunt.
- `/résume` and `/cherche` can need longer than short transport checks.
- Use short connect timeouts, but request-level timeouts for agent commands.
- Prefer `127.0.0.1` over `localhost` for the gateway base URL to avoid local resolution weirdness.

Relevant files:

- `crates/agent/src/gateway.rs`
- `crates/tui/src/app/actions.rs`

### 7. Slack Loading Must Be Honest

- Never silently return partial channel lists as success.
- Detect repeated cursors and page ceilings explicitly.
- Surface pagination errors instead of pretending initialization completed.

Relevant files:

- `crates/slack/src/api.rs`

### 8. Workspace State Cannot Assume Successful Slack Init

- The active workspace in session storage may not exist in initialized runtime state.
- Always resolve runtime workspaces by actual loaded `team_id`.
- Startup must degrade to an empty-but-alive UI, not panic.

Relevant files:

- `crates/tui/src/app/effects.rs`

## Tips And Tricks

### Startup Checklist

When debugging startup, verify in this order:

1. Slack auth works
2. cached channels load
3. background paging starts
4. ZeroClaw becomes `active`
5. agent panel shows `dry-run` or `enabled`

If the TUI opens but feels empty, do not assume a crash. Check whether Slack paging is still running.

### Safe Manual Testing

Use this sequence:

1. `cargo test -q`
2. `cargo run`
3. in the TUI, confirm `Post to Slack: dry-run`
4. press `i`
5. test:
   - `/draft répondre poliment que je regarde demain`
   - `/cherche test intégration`
   - `/résume`

Success criteria:

- ZeroClaw status is `active`
- the command appears under `Recent`
- nothing is posted to Slack

### Useful Logs

`crates/tui/src/main.rs` initializes logging to `slack-zc.log`.

Useful patterns:

- `Dispatching agent command`
- `Agent command completed successfully`
- `Dry-run enabled; agent response kept local`
- `Workspace ... channels updated`
- `Retrying after error: 429`

### Common Failure Modes

- `Webhook failed: 401 Unauthorized`
  - bearer is wrong for the actual gateway endpoint
  - or the contract being called is not the one ZeroClaw expects

- `error sending request for url (...)`
  - usually transport/timeout/base URL issue
  - check `127.0.0.1` vs `localhost`

- Slack loads forever
  - likely large workspace + pagination + `429`
  - the fix is incremental loading, not a longer blocking init

- ZeroClaw `active` but commands fail
  - `/health` working is not enough
  - verify the real webhook path and payload contract

## Code Rules

- No blocking UI thread for network work
- No secret leakage in logs
- No `unwrap()` in normal runtime paths
- Prefer typed errors with actionable messages
- Keep Slack, TUI, and ZeroClaw concerns separated
- Keep docs aligned with actual behavior, especially around `dry-run`

## Before Commit

- `cargo test -q`
- sanity-check the TUI in `dry-run`
- ensure docs match the real current behavior
