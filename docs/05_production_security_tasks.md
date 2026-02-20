# Production & Security Hardening Tasks (MVP)

## Goal
Stabilize runtime behavior and reduce security exposure for MVP release quality.

## Status
In progress

## Criteria Re-evaluation (Current)
- Architecture: 8/10
- Modularity: 7.5/10
- Maintainability: 7/10
- Professional practices: 7/10
- Production readiness: 7/10
- Security best practices: 7/10

## Notes on scoring
- Improved significantly with async task pipeline and reduced UI blocking.
- Remaining gaps are mostly operational hardening: retries/rate limits, diagnostics, and broader test coverage.

## Completed in this pass
- [x] Add default HTTP hardening for Slack REST client (`user-agent`, connect timeout, request timeout).
- [x] Add timeout and user-agent for OAuth token exchange client.
- [x] Stop logging token-bearing Socket Mode URLs in plaintext (log redacted URL only).
- [x] Stop logging raw websocket payload bodies; log frame size instead.
- [x] Add Socket Mode user cache with TTL to avoid `users.list` on every message.
- [x] Enforce secure file mode on encrypted session file writes (`0600` on Unix).
- [x] Remove UI-thread `block_on` from interactive app flows and move network calls to async workers.

## Remaining high-priority tasks
- [ ] Add retry/backoff policy for Slack REST endpoints with explicit handling for 429 rate limits.
- [ ] Add structured error categories (auth/network/rate-limit/validation) and user-facing remediation hints.
- [ ] Add secure token lifecycle controls (rotation path and logout/purge flow).
- [ ] Add integration tests for Slack API error scenarios and Socket Mode reconnect behavior.

## Next Priorities (Ranked)
1. Implement Slack API retry + 429-aware backoff with jitter.
2. Add typed app error model and map errors to actionable user messages.
3. Add integration tests for reconnect, retry, and degraded network scenarios.
4. Implement token purge/logout and safer token rotation path.
5. Add health diagnostics view/command for support and production triage.

## Security checklist
- [x] Session data encrypted at rest (AES-GCM).
- [x] Secret/session files restricted to owner permissions on Unix.
- [x] Sensitive tokens redacted from error messages and logs.
- [x] Outbound network requests bounded by timeout.
- [ ] Comprehensive secret redaction coverage in all logs.
- [ ] Dependency vulnerability scan in CI.

## Production readiness checklist
- [x] Build/test baseline passing locally.
- [x] Socket reconnect backoff exists.
- [x] API request timeout defaults exist.
- [x] Async/non-blocking UI I/O flow.
- [ ] Rate-limit aware retry behavior.
- [ ] Health diagnostics command for supportability.
