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

## Nginx Reverse Proxy (Recommended)

```nginx
server {
    listen 80;
    server_name recruitment.example.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name recruitment.example.com;

    ssl_certificate /etc/letsencrypt/live/recruitment.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/recruitment.example.com/privkey.pem;

    # Frontend
    location / {
        proxy_pass http://localhost:3000;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_cache_bypass $http_upgrade;
    }

    # Backend API
    location /api/ {
        proxy_pass http://localhost:8000/api/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # CV uploads
    location /uploads/ {
        proxy_pass http://localhost:8000/uploads/;
        proxy_set_header Host $host;
    }
}
```

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
