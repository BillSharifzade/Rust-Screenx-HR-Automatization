# Recruitment Platform Deployment Guide

## Prerequisites

- Docker & Docker Compose installed
- PostgreSQL database (external or containerized)
- Domain with SSL (for Telegram WebApp)

## Quick Start

### 1. Clone/Upload the project

```bash
# Clone or upload to your server
cd ~/RustKernel
```

### 2. Configure environment

```bash
# Copy example env file
cp .env.example .env

# Edit with your values
nano .env
```

**Required environment variables:**
- `DATABASE_URL` - PostgreSQL connection string
- `JWT_SECRET` - Random secret for JWT tokens
- `TELEGRAM_BOT_TOKEN` - Your Telegram bot token
- `WEBAPP_URL` - Your domain (e.g., https://recruitment.example.com)

### 3. Build and deploy

```bash
# Build all services
docker compose build

# Start in background
docker compose up -d

# View logs
docker compose logs -f
```

### 4. Run database migrations

```bash
# Access the backend container
docker exec -it recruitment-backend bash

# Or run migrations from host if you have sqlx-cli
DATABASE_URL="your-db-url" sqlx migrate run
```

## Architecture

```
┌─────────────────┐     ┌─────────────────┐
│   Frontend      │────▶│    Backend      │
│   (Next.js)     │     │    (Rust)       │
│   Port: 3000    │     │   Port: 8000    │
└─────────────────┘     └─────────────────┘
                              │
                              ▼
                        ┌─────────────────┐
                        │   PostgreSQL    │
                        │   Port: 5432    │
                        └─────────────────┘
```

## Update Deployment

```bash
cd ~/RustKernel

# Pull latest code
git pull origin main

# Rebuild and restart
docker compose down
docker compose build --no-cache
docker compose up -d
```

## Reverse Proxy & TLS (Caddy)

The stack ships its own **Caddy** reverse proxy (`docker-compose.yml` → `caddy`
service, config in [`Caddyfile`](Caddyfile)). Caddy binds host ports `80`/`443`
and **automatically obtains and renews a Let's Encrypt certificate** for the
domain — Telegram requires valid HTTPS for both the Mini App and the bot webhook.

All public traffic goes to the **frontend** (`frontend:3000`). The Next.js server
proxies `/api/*` and `/uploads/*` to the backend over the internal Docker network
(`frontend/next.config.ts` rewrites → `http://backend:8080`), so only the frontend
is exposed. Do **not** route `/api` straight to the backend — the frontend's
relative-URL design (`NEXT_PUBLIC_API_URL=""`) expects everything to flow through it.

To serve a different domain, edit the site address in `Caddyfile` and recreate the
container (`docker compose up -d caddy`). Requirements:
- DNS A record for the domain → this server's public IP
- Inbound ports **80 and 443** open in the firewall (80 is needed for the ACME challenge)
- The `caddy_data` volume is persisted so certificates survive restarts (avoids
  hitting Let's Encrypt rate limits)

The `Caddyfile` also sets `request_body max_size 50MB` for CV uploads (backend
`DefaultBodyLimit` in `main.rs`).

## Troubleshooting

### Check container status
```bash
docker compose ps
```

### View logs
```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f backend
docker compose logs -f frontend
```

### Restart services
```bash
docker compose restart
```

### Clear and rebuild
```bash
docker compose down -v
docker compose build --no-cache
docker compose up -d
```
