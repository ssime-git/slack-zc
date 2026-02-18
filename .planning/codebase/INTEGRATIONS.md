# External Integrations

**Analysis Date:** 2026-02-18

## APIs & External Services

**Slack API:**
- REST API endpoints for chat, user info, channel operations
  - SDK/Client: Custom client built with reqwest (see `crates/slack/src/api.rs`)
  - Auth: Bearer token authentication with `xoxp_token` (user token) and `xapp_token` (app token)
  - Base URL: `https://slack.com/api`
  - Key endpoints:
    - `auth.test` - Token validation
    - `chat.postMessage` - Send messages
    - `chat.update` - Update messages
    - `chat.delete` - Delete messages
    - `reactions.add` - Add emoji reactions
    - `conversations.list` - List channels/DMs
    - `conversations.history` - Fetch message history
    - `users.info` - Get user details
    - `users.list` - List all workspace users
    - `apps.connections.open` - Initialize Socket Mode connection

**Slack Socket Mode:**
- Real-time message streaming via WebSocket
  - Technology: WebSocket via `tokio-tungstenite` with rustls-tls-webpki-roots
  - Connection: Obtained via `apps.connections.open` API endpoint
  - Events handled:
    - Message delivery and updates
    - User typing indicators
    - Channel join/leave events
    - Connection lifecycle management
  - Acknowledgment: Server-side message acknowledgment via envelope_id
  - Implementation: `crates/slack/src/socket.rs`

**ZeroClaw Agent Gateway:**
- Internal local HTTP gateway for AI agent integration
  - SDK/Client: Custom HTTP client built with reqwest (see `crates/agent/src/gateway.rs`)
  - Base URL: `http://localhost:{gateway_port}` (default: 8080)
  - Connection: Local TCP connection only, no TLS
  - Auth: Bearer token-based authentication
  - Endpoints:
    - `POST /pair` - Pair with agent gateway using code, returns bearer token
    - `GET /health` - Health check endpoint
    - `POST /webhook` - Send message payload to agent for processing
  - Configuration: `gateway_port` set in config, default 8080
  - Implementation: `crates/agent/src/gateway.rs`

## Authentication & Identity

**Slack OAuth 2.0:**
- Authorization flow: Custom OAuth implementation
  - Endpoint: `https://slack.com/api/oauth.v2.access`
  - Grant type: Authorization code flow
  - Client credentials: Stored in config as `slack.client_id` and `slack.client_secret`
  - Callback: Local redirect HTTP server on `redirect_port` (default: 3000)
  - Response types: Access tokens (xoxp_token, xapp_token) and team/user info
  - Implementation: `crates/slack/src/auth.rs` function `exchange_oauth_code()`

**ZeroClaw Pairing:**
- Code-based pairing mechanism
  - Initial pairing: HTTP request with `X-Pairing-Code` header to `/pair` endpoint
  - Token exchange: Receives bearer token for subsequent authenticated requests
  - State: Bearer token stored in session file (encrypted)

## Session Management

**Storage:**
- Format: JSON file encrypted with AES-256-GCM
- Location: Platform data directory (`session.json`)
- Key management: 32-byte secret key stored locally at `.secret_key` with 0o600 permissions
- Contents:
  - Workspaces array (multiple Slack workspaces supported)
  - Each workspace stores: team_id, team_name, xoxp_token, xapp_token, user_id, active flag
  - ZeroClaw bearer token: `zeroclaw_bearer` field
- Encryption library: `aes_gcm` v0.10 (AES-256-GCM authenticated encryption)

## Data Storage

**Persistence:**
- No database or external storage backend
- In-memory state only, no persistent caching

**Local Files:**
- Session file: Encrypted JSON with Slack tokens
- Secret key: Raw bytes for AES decryption
- Config file: TOML format
- Directory: Uses `directories` crate for platform-specific paths

## Caching

**User Cache:**
- In-memory HashMap cache with TTL
- TTL: 600 seconds (10 minutes)
- Scope: Per-API-client instance
- Implementation: `UserCache` struct in `crates/slack/src/api.rs`
- Use case: User ID to User object mapping for message enrichment

**Socket Mode User Display Names:**
- In-memory HashMap cache
- No explicit TTL (cache for connection lifetime)
- Scope: Per-socket connection
- Implementation: `user_display_names` field in `SocketModeClient`

## Monitoring & Observability

**Logging:**
- Framework: Tracing 0.1 with tracing-subscriber 0.3
- Levels: debug, info, warn, error
- Implementations in:
  - `crates/slack/src/api.rs` - Retry attempts and backoff
  - `crates/slack/src/socket.rs` - Connection lifecycle, frame sizes, errors
  - `crates/agent/src/gateway.rs` - Pairing success, health check failures
- Format: Structured logging with contextual information

**Error Handling:**
- Custom error types in `crates/slack/src/error.rs`
- API errors include: Auth failures, rate limiting, network errors, validation, timeouts
- Retry strategy: Exponential backoff with jitter for 429 (rate limit) responses
- Max retries: 3 attempts with base delay of 1 second and exponential multiplication

## Rate Limiting

**Slack API:**
- Automatic retry mechanism with exponential backoff
- Detection: Error message contains "429" or "rate_limit"
- Backoff calculation: `BASE_DELAY_MS * 2^attempt + jitter(0-500ms)`
- Max retries: 3 attempts
- Timeout: 15 seconds for agent gateway, 20 seconds for OAuth, 60 seconds for WebSocket reads
- Implementation: `with_retry()` function in `crates/slack/src/api.rs`

## Configuration

**Required env vars or config:**
- `slack.client_id` - OAuth app client ID
- `slack.client_secret` - OAuth app client secret (sensitive)
- `zeroclaw.gateway_port` - Port for local agent gateway
- `zeroclaw.binary_path` - Path to zeroclaw binary
- `llm.provider` - LLM provider name (default: "openrouter")
- `llm.api_key` - LLM provider API key (sensitive)

**Secrets location:**
- Config file: `config/default.toml` or platform config directory
- Note: Config file is NOT encrypted, sensitive values should be set at runtime or via environment
- Session data: Encrypted with AES-256-GCM to `session.json`
- Secret key: Stored in `.secret_key` with restricted permissions

## Webhooks & Callbacks

**Incoming - Slack Socket Mode:**
- WebSocket-based event streaming
- Events: message, typing, channel events
- No HTTP webhook endpoints exposed by slack-zc
- Connection managed by `SocketModeClient` in `crates/slack/src/socket.rs`

**Outgoing - To ZeroClaw Agent:**
- HTTP POST to local gateway `/webhook` endpoint
- Payload: JSON message objects for AI processing
- Bearer token authentication via Authorization header
- Implementation: `send_to_agent()` method in `crates/agent/src/gateway.rs`
- Response: Text response bounded to 20,000 characters

## Version Information

**Slack Workspace Support:**
- Multiple simultaneous Slack workspace connections
- Workspace switching via team_id selection
- Session maintains active workspace state

**OAuth Flow Implementation:**
- Custom OAuth2 client (not using oauth2 crate) for Slack
- Slack API version: v2 endpoint (`oauth.v2.access`)
- User agent: "slack-zc/0.2" for all HTTP requests

---

*Integration audit: 2026-02-18*
