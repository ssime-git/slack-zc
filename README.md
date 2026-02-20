# slack-zc

Terminal Slack client with native ZeroClaw AI agent integration.

## Status

- **Phase 1: TUI Fonctionnel + Bridge Bootstrap** - COMPLETED ✓
- **Phase 2: ZeroClaw Commands** - COMPLETED ✓
- **Phase 3: Polish** - COMPLETED ✓
- **Phase 4: Docker** - PENDING

## Features

- Terminal-based Slack client using Ratatui
- Multi-workspace support
- ZeroClaw AI agent integration (`/résume`, `/draft`, `/cherche`)
- Mouse support (click, scroll, drag resize)
- AES-GCM encrypted session storage
- Real-time message updates via Socket Mode

## Installation

```bash
cargo build --release
```

## Onboarding (Manual Testing)

### Prerequisites

1. **Slack App Configuration**
   - Create a new Slack app at https://api.slack.com/apps
   - Enable **Socket Mode** with a unique token (prefix: `xapp-`)
   - Add the following **OAuth Scopes** (user token = xoxp-):
     - `channels:read`, `channels:history`, `channels:join`
     - `groups:read`, `groups:history`
     - `mpim:read`, `mpim:history`
     - `im:read`, `im:history`
     - `chat:write`, `reactions:write`
     - `users:read`, `users:read.email`
     - `files:read`, `files:write`
     - `team:read`
     - `connections:write` (for Socket Mode)
   - Subscribe to **App Events** (not Bot Events - you act as yourself):
     - `message.channels`, `message.groups`, `message.im`, `message.mpim`
     - `reaction_added`, `reaction_removed`, `typing`
   - Install the app to your workspace

2. **ZeroClaw Binary** (optional, for Phase 2 features)
   - Place `zeroclaw` binary in PATH or configure `binary_path`

3. **Remote SSH Setup**
   - If testing remotely, create an SSH tunnel:
     ```bash
     # On your LOCAL machine
     ssh -L 3000:localhost:3000 user@your-remote-server
     ```
   - Or use ngrok: `ngrok http 3000` (then update redirect_uri in Slack app)
   - **Custom port**: Set `redirect_port` in config.toml, update Slack app redirect_uri accordingly

### First Launch

```bash
./target/release/slack-zc
```

The app will start the onboarding wizard:

1. **Welcome Screen** - Press any key to continue
2. **OAuth Flow** - Opens browser for Slack authentication
3. **Socket Mode** - Enter your Socket Mode token (starts with `xapp-`)
4. **Channel Selection** - Choose which channels to join
5. **Main Interface** - You're ready to use slack-zc!

### Testing Checklist

- [ ] OAuth authentication completes successfully
- [ ] Channels load and display correctly
- [ ] Messages appear in real-time (Socket Mode)
- [ ] Sending messages works
- [ ] Emoji reactions work
- [ ] Thread replies work (`t` to open thread)
- [ ] Message editing (`e`) and deletion (`d`)
- [ ] File uploads/downloads
- [ ] ZeroClaw commands (`/résume`, `/draft`, `/cherche`)
- [ ] Multi-workspace switching (`Ctrl+W`)
- [ ] Channel search (`Ctrl+K`)
- [ ] User filter (`f` key)
- [ ] Jump to timestamp (`g` key)

### Troubleshooting

- **OAuth flow**:
  1. Configure redirect URI in Slack app: `http://localhost:3000` (local) or `https://your-ngrok-url` (remote)
  2. Generate OAuth URL in app, open in browser
  3. After auth, browser redirects to localhost with `?code=XXX`
  4. Copy the code from browser URL and paste in TUI
- **Remote SSH without tunnel**: Use ngrok (`ngrok http 3000`), update Slack app redirect_uri to your ngrok URL
- **Socket Mode not connecting**: Check your `xapp-` token is valid and app is installed
- **Channels not loading**: Ensure `channels:history` and `channels:read` scopes are granted

## Configuration

Create `~/.config/slack-zc/config.toml`:

```toml
[slack]
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_port = 3000

[zeroclaw]
binary_path = "zeroclaw"
gateway_port = 8080
auto_start = true
```

## Usage

```bash
./target/release/slack-zc
```

### Keybindings

- `Ctrl+Q` - Quit
- `Ctrl+W` - Switch workspace
- `Ctrl+K` - Channel search
- `Alt+↑/↓` - Navigate channels
- `?` - Help
- `/` - Agent command mode
- `@zeroclaw` - Agent mention mode

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        slack-zc binary                      │
├─────────────────┬─────────────────┬─────────────────────────┤
│    crates/tui   │   crates/slack  │      crates/agent      │
│  ┌───────────┐  │  ┌───────────┐  │  ┌───────────────────┐  │
│  │  Ratatui  │──│──│  Slack    │  │  │   ZeroClaw       │  │
│  │    TUI    │  │  │    API    │  │  │   Gateway Client │  │
│  └───────────┘  │  └───────────┘  │  └───────────────────┘  │
│  ┌───────────┐  │  ┌───────────┐  │  ┌───────────────────┐  │
│  │ Onboarding│──│──│   OAuth   │  │  │  Command Parser   │  │
│  │  Wizard   │  │  │   Flow    │  │  │  (/résume, etc)  │  │
│  └───────────┘  │  └───────────┘  └───────────────────────┘  │
│  ┌───────────┐  │  ┌───────────┐                             │
│  │   Input   │──│──│  Socket   │                             │
│  │  Handler  │  │  │   Mode    │                             │
│  └───────────┘  │  └───────────┘                             │
└─────────────────┴─────────────────┴─────────────────────────┘
        │                 │                    │
        ▼                 ▼                    ▼
   Terminal UI      Slack API           ZeroClaw Gateway
                 (Socket Mode)          (WebSocket/HTTP)
```

## Launch Command

```bash
./target/release/slack-zc
```

Or for development:

```bash
cargo run --release
```

## License

MIT
