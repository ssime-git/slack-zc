# slack-zc

A lightweight terminal Slack client written in Rust. Browse channels, send messages, and use AI-powered features through Slack — all without leaving your terminal.

## Quick Look

```
┌─────────────────────────┬──────────────────────┬──────────────┐
│ CHANNELS                │ MESSAGES             │ ZEROCLAW     │
│ #general       [1]      │ 14:23 alice          │ ZEROCLAW     │
│ #random        [0]      │   hello team!        │              │
│ #dev                    │ 14:25 you            │ Status: ✓    │
│ #design                 │   /draft can you...  │ Commands:    │
│ @bob          [2 unread]│ 14:26 claude         │ /résume      │
│ @alice                  │   Here's a summary   │ /draft       │
└─────────────────────────┴──────────────────────┴──────────────┘
[input: type your message here...]
```

## Features

- **Full Slack in Your Terminal** - Real-time messaging, threads, reactions, file uploads
- **Multi-workspace** - Jump between workspaces with `Ctrl+W`
- **AI-Powered Commands** - Use ZeroClaw to summarize, draft, and analyze with `/résume`, `/draft`, `/cherche`
- **Mouse Support** - Click panels to navigate, drag dividers to resize
- **Search** - `Ctrl+K` to instantly find channels and DMs
- **Encrypted Storage** - Your credentials are encrypted locally, never stored in plain text
- **Real-time Sync** - Socket Mode keeps you in sync with Slack events

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        slack-zc (TUI)                        │
│                      (Ratatui + Tokio)                       │
└─────────────────────┬──────────────────────────────────────┘
                      │
      ┌───────────────┼───────────────┐
      │               │               │
      ▼               ▼               ▼
   ┌──────┐      ┌──────────┐    ┌────────────┐
   │Slack │      │ZeroClaw  │    │Session     │
   │ API  │      │ Gateway  │    │Storage     │
   │      │      │          │    │(AES-GCM)   │
   └──────┘      └──────────┘    └────────────┘
      │               │
      │               ▼
      │          ┌──────────────┐
      │          │LLM Provider  │
      │          │(OpenRouter/  │
      │          │ Anthropic)   │
      │          └──────────────┘
      │
      ▼
┌─────────────────────────────┐
│  Slack Workspace            │
│  (OAuth, Socket Mode, REST) │
└─────────────────────────────┘
```

**Data Flow:**
1. User types in TUI → sent through Slack API
2. Slack sends real-time events via Socket Mode
3. Messages render in TUI with full formatting
4. `/command` → forwarded to ZeroClaw gateway
5. ZeroClaw queries LLM, returns result → displayed in chat

## Installation

### Prerequisites
- Rust 1.70+ ([install here](https://rustup.rs/))
- A Slack workspace with admin permissions (to create an app)
- ZeroClaw binary (for AI features, optional but recommended)
- A CodeX/LLM account (OpenRouter, Anthropic, etc.)

### Build from Source

```bash
git clone https://github.com/ssime-git/slack-zc.git
cd slack-zc
cargo build --release
./target/release/slack-zc
```

The binary will be at `./target/release/slack-zc` (~15MB, runs anywhere).

## Configuration

### Environment Variables (Optional Quick Setup)

Create a `.env` file in the project root:

```bash
# Slack tokens from your app setup
SLACK_APP_TOKEN=xapp-1-...           # Socket Mode token (starts with xapp-)
SLACK_USER_TOKENS=xoxp-...           # User token (starts with xoxp-)

# Optional: history limits
SLACK_HISTORY_LIMIT=50               # Messages to load per channel
SLACK_HISTORY_MIN=10                 # Minimum
SLACK_HISTORY_MAX=200                # Maximum
```

On first launch without `.env`:
1. The app will start the OAuth flow
2. Authenticate in your browser
3. Enter Socket Mode token when prompted
4. Done!

### Persistent Config

Config file location: `~/.config/slack-zc/config.toml`

Auto-created on first launch with defaults:

```toml
[slack]
client_id = ""              # OAuth app ID (get from Slack)
client_secret = ""          # OAuth app secret (get from Slack)
redirect_port = 3000        # Local port for OAuth callback

[zeroclaw]
binary_path = "zeroclaw"    # Where ZeroClaw binary is installed
gateway_port = 8888         # ZeroClaw gateway port
auto_start = true           # Auto-start ZeroClaw on app launch
timeout_seconds = 30        # Timeout for ZeroClaw requests

[llm]
provider = "openrouter"     # or anthropic, openai, etc.
api_key = ""                # Your LLM API key
```

## Getting Started

### Step 1: Create a Slack App

1. Go to https://api.slack.com/apps and create a new app
2. In **Socket Mode** section, enable it and generate a token (note the `xapp-...` token)
3. In **OAuth & Permissions**, add these scopes:
   ```
   channels:read, channels:history, channels:join
   groups:read, groups:history
   im:read, im:history, mpim:read, mpim:history
   chat:write, reactions:write
   users:read, users:read.email
   files:read, team:read, connections:write
   ```
4. Install the app to your workspace
5. Keep your **Client ID** and **Client Secret** handy (from **App Credentials**)

### Step 2: Install ZeroClaw (AI Features)

ZeroClaw is optional but recommended for `/résume`, `/draft`, `/cherche` commands.

```bash
# Install via Homebrew
brew install zeroclaw

# Or build from source
git clone https://github.com/zeroclaw-labs/zeroclaw.git
cd zeroclaw
cargo install --path .
```

### Step 3: Configure ZeroClaw (First Time Only)

```bash
# Interactive setup wizard (recommended)
zeroclaw onboard --interactive

# Or quick setup with OpenRouter
zeroclaw onboard --api-key "sk-or-..." --provider openrouter
```

This creates `~/.zeroclaw/config.toml` with your setup.

### Step 4: Launch slack-zc

```bash
./target/release/slack-zc
```

If using `.env`, it will auto-authenticate. Otherwise:
1. Follow the OAuth flow in your browser
2. Paste your Socket Mode token (`xapp-...`)
3. You're in!

## Usage

### Keyboard Shortcuts

**Navigation:**
- `Tab` - Move focus between panels (sidebar, messages, input)
- `Up/Down` or `Scroll` - Scroll through messages
- `Ctrl+W` - Switch workspaces

**Messaging:**
- `Enter` - Send message
- `e` - Edit own message
- `d` - Delete own message
- `t` - Open thread
- `r` - React (then pick emoji from menu)

**Search & Discovery:**
- `Ctrl+K` - Search channels and DMs by name (type to filter)
- `j` - Jump to message timestamp
- `f` - Filter user messages in sidebar

**Mouse:**
- Click on panels to focus (sidebar, messages, input bar, agent panel)
- Drag dividers between panels to resize
- Right-click messages for context menu (reply, react, edit, delete)
- Scroll wheel to navigate

### AI Commands

Type these in any message input (ZeroClaw must be running):

- `/résume` - Summarize the conversation
- `/draft This is what I want to say` - Get help writing a message
- `/cherche keyword` - Search and analyze content

Example:
```
You: /résume last 10 messages about the project
Claude: "Here's what was discussed..."
```

### Example Workflow

```
1. Launch app → authenticate with Slack
2. Ctrl+K → search "engineering" → Enter
3. Read recent messages
4. Click input bar at bottom
5. Type: "Hey team, working on the deploy"
6. Enter to send
7. Right-click a message → React
8. Type: /draft "but we need to handle X" → Enter (Claude helps polish it)
9. Press t on your message to see replies
```

## Development

### Project Structure

```
slack-zc/
├── crates/
│   ├── tui/        # Terminal UI (Ratatui framework)
│   ├── slack/      # Slack API client & auth
│   └── agent/      # ZeroClaw integration
├── config/         # Default config
├── .env            # Environment variables (optional)
└── README.md       # This file
```

### Build & Test

```bash
# Debug build (slower, better errors)
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Check code quality
cargo fmt --check
cargo clippy
```

### How It Works

1. **TUI Layer** (Ratatui) - Renders panels, handles keyboard/mouse input
2. **Slack Layer** - OAuth flow, Socket Mode listener, REST API calls
3. **Agent Layer** - Launches ZeroClaw gateway, forwards commands via HTTP
4. **Session Layer** - Encrypts and stores tokens locally

The app runs everything in a single async Tokio runtime for low latency.

## Troubleshooting

**OAuth flow not working?**
- Check redirect URI in your Slack app matches `http://localhost:3000`
- Make sure your Client ID and Secret are correct
- If running remotely, use SSH tunnel: `ssh -L 3000:localhost:3000 user@server`

**Socket Mode connection fails?**
- Verify your `xapp-` token is valid and active in Slack app settings
- Check that Socket Mode is enabled in your Slack app

**Channels not loading?**
- Ensure `channels:read` and `channels:history` scopes are granted
- Re-authenticate by removing `~/.config/slack-zc/` and running again

**ZeroClaw not working?**
- Check ZeroClaw is installed: `zeroclaw --version`
- Ensure your LLM API key is configured: `zeroclaw auth status`
- Check gateway is running on correct port (default 8888)
- View logs: `~/.zeroclaw/logs/`

**Slow startup?**
- First launch loads all channels — this is normal
- Subsequent launches cache this data

## Known Limitations

- File uploads work but file preview is text-only (no images)
- Custom emoji are rendered as `:emoji_name:` text
- Threads can be 1-level deep (no nested replies)
- No bot integration or webhooks (you act as yourself)

## Contributing

Pull requests welcome! Before submitting:

```bash
cargo fmt              # Format code
cargo clippy           # Check for warnings
cargo test            # Run tests
```

Make sure your code:
- Follows Rust conventions (use `cargo fmt`)
- Has no clippy warnings
- Passes all tests

## License

MIT - See [LICENSE](LICENSE) file for details.

## Roadmap

- [ ] Thread replies (nested)
- [ ] Scheduled messages
- [ ] Custom themes
- [ ] Plugin system
- [ ] Mobile app (Web)
- [ ] Docker image

## Support

Found a bug? [Open an issue](https://github.com/ssime-git/slack-zc/issues).

Want to chat? DM me on Slack or reach out on GitHub.

---

Made with ❤️ for people who prefer terminal tools. Works great on Linux, macOS, and even Windows via WSL.

