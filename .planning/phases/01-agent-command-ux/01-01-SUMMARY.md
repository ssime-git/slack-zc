---
phase: 01-agent-command-ux
plan: 01
subsystem: ui
tags: [zeroclaw, timeout, config, error-handling]
requires:
  - phase: none
    provides: baseline agent command flow
provides:
  - Configurable gateway webhook timeout via `zeroclaw.timeout_seconds`
  - Default timeout value in shipped config template
  - Detailed timeout/failure messages with retry guidance in agent panel
affects: [agent-command-ux, reliability]
tech-stack:
  added: []
  patterns: [config-driven timeout control, actionable user-facing error messaging]
key-files:
  created: [crates/tui/src/config.rs, config/default.toml]
  modified: [crates/tui/src/app/actions.rs]
key-decisions:
  - "Use `zeroclaw.timeout_seconds` as the single source for webhook timeout duration."
  - "Include `Press R to retry` in timeout and gateway failure messages for immediate recovery guidance."
patterns-established:
  - "Agent webhook timeouts are configurable in config rather than hardcoded in async callsites."
  - "Agent command failures show duration and remediation hints in the agent panel."
duration: 1 min
completed: 2026-02-18
---

# Phase 1 Plan 1: Gateway Timeout Configurability Summary

**Config-driven 30s default gateway timeout with actionable agent-panel errors and built-in retry hinting.**

## Performance

- **Duration:** 1 min
- **Started:** 2026-02-18T23:01:41Z
- **Completed:** 2026-02-18T23:02:51Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Added `timeout_seconds` to `ZeroClawConfig` with a default value of `30`.
- Added `timeout_seconds = 30` under `[zeroclaw]` in `config/default.toml`.
- Updated agent webhook timeout handling to use config and return detailed timeout/failure messages with `Press R to retry`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add timeout_seconds to ZeroClawConfig** - `088a7dd` (feat)
2. **Task 2: Add timeout_seconds to default.toml** - `0ed1643` (feat)
3. **Task 3: Update actions.rs timeout and errors** - `3ef38e6` (feat)

## Files Created/Modified
- `crates/tui/src/config.rs` - Adds `zeroclaw.timeout_seconds` to runtime config model and default values.
- `config/default.toml` - Adds default timeout setting in distributed config template.
- `crates/tui/src/app/actions.rs` - Uses configurable timeout and returns richer user-facing error messages with retry hint.

## Decisions Made
- Routed webhook timeout duration through `self.config.zeroclaw.timeout_seconds` to remove hardcoded timeout behavior.
- Standardized agent command error messaging to always include retry guidance (`Press R to retry`) for both timeout and transport failures.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Corrected verification command package name**
- **Found during:** Task 3 (Update actions.rs to use configurable timeout with detailed error)
- **Issue:** Plan verification command used `cargo check --package tui`, but workspace package name is `slack-zc`.
- **Fix:** Ran `cargo check -p slack-zc` for equivalent crate-level verification.
- **Files modified:** None
- **Verification:** `cargo check -p slack-zc` completed successfully.
- **Committed in:** 3ef38e6 (task commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Verification path corrected without scope changes; planned behavior delivered fully.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Timeout behavior is now configurable and non-blocking for UX.
- Ready for `01-02-PLAN.md` in Phase 1.

---
*Phase: 01-agent-command-ux*
*Completed: 2026-02-18*

## Self-Check: PASSED
