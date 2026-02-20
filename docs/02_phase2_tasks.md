# Phase 2: ZeroClaw Commands

## Status: IN PROGRESS

## Overview

Wire agent commands (`/résume`, `/draft`, `/cherche`) to the ZeroClaw gateway webhook and display responses in both the agent panel and Slack threads.

## Tasks

### Command Parsing
- [x] Implement `/résume` command parsing ✓
- [x] Implement `/draft` command parsing ✓
- [x] Implement `/cherche` command parsing ✓
- [x] Add command argument extraction ✓

### Gateway Integration
- [x] Wire command to POST /webhook ✓
- [x] Handle webhook response ✓
- [ ] Add timeout handling

### UI Integration
- [x] Display agent responses in agent panel ✓
- [x] Post agent responses to Slack ✓
- [ ] Add loading indicator during agent processing

### Thread Support
- [x] Track thread_ts for agent messages ✓
- [x] Reply to threads instead of channels ✓

## Dependencies Graph

```
Command Parsing ✓
       ↓
Gateway Integration ✓ ───────────────┐
       ↓                          │
UI Integration ✓ ────── Thread Support
       ↓
└── Agent panel response display ✓
└── Slack reply ✓
```

## Technical Details

### Command Format
```
/résume [#channel]  - Summarize channel conversation
/draft [intent]     - Draft a response
/cherche [text]     - Search conversation history
```

### Webhook Payload
```json
{
  "command": "resume",
  "channel": "C123",
  "user": "U456",
  "message": "/resume #general"
}
```

### Agent Response Display
- Show in agent panel with timestamp
- Posted to Slack channel
- Stored in memory (last 50 responses)

## Recent Changes
- Added thread support with active_thread field
- Added send_message_to_thread to Slack API
- Updated handle_agent_command to reply to threads
- Updated process_slack_events to track thread messages

## Notes
- Phase 1 complete before starting Phase 2 ✓
- Agent commands intercepted in InputMode::AgentCommand
- Calls GatewayClient::send_to_agent()
