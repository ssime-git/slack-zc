# Phase 4: Docker

## Status: PENDING

## Overview

Containerization and distribution via Docker. Single image with no external dependencies for end users.

## Tasks

### Docker Image
- [ ] Create Dockerfile (depends on: None)
- [ ] Multi-stage build (rust:1.82-alpine builder → alpine:3.20 runtime) (depends on: Dockerfile)
- [ ] Embed ZeroClaw install script (depends on: Dockerfile)
- [ ] Test Docker image locally (depends on: Multi-stage build)

### Docker Compose
- [ ] docker-compose.yml for local dev (depends on: Docker Image)
- [ ] Volume mounts for config (depends on: Docker Compose)
- [ ] Environment variable support (depends on: Docker Compose)

### CI/CD
- [ ] GitHub Actions workflow (depends on: Docker Image)
- [ ] Build and push on tag (depends on: GitHub Actions)
- [ ] Release workflow (depends on: CI/CD)

### Distribution
- [ ] Publish to GitHub Container Registry (depends on: CI/CD)
- [ ] Add imagePullPolicy documentation (depends on: Distribution)
- [ ] Create installation guide (depends on: Distribution)

## Dependencies Graph

```
Dockerfile
      ↓
Multi-stage Build
      ↓
Embed ZeroClaw
      ↓
Test Docker Image ────────────────────┐
      ↓                              │
Docker Compose                       │
      ↓                              │
Volume Mounts ──────────────────────┤
      ↓                              │
Environment Variables ───────────────┤
      ↓                              │
GitHub Actions ─────────────────────┤
      ↓                              │
Build on Tag ───────────────────────┤
      ↓                              │
Release Workflow ───────────────────┤
      ↓                              │
GitHub Container Registry ──────────┤
      ↓                              │
Installation Guide ─────────────────┘
```

## Technical Details

### Dockerfile Structure
```dockerfile
FROM rust:1.82-alpine AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM alpine:3.20
RUN curl -LsSf https://raw.githubusercontent.com/zeroclaw-labs/zeroclaw/main/scripts/install.sh | sh
COPY --from=builder /app/target/release/slack-zc /usr/local/bin/
ENTRYPOINT ["slack-zc"]
```

### docker-compose.yml
```yaml
services:
  slack-zc:
    image: slack-zc:latest
    volumes:
      - ~/.config/slack-zc:/config
    environment:
      - TERM=xterm-256color
```

## Notes
- Phase 3 should be complete before Dockerization
- Single image, no external dependencies
- ZeroClaw installed at runtime
