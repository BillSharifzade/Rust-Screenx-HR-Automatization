# RustKernel Project Analysis

## Overview
**RustKernel** is a high-performance recruitment and candidate management platform designed with a hybrid architecture. It serves as a central hub integrating an HR system ("Первая Форма"), Telegram (Bot & Mini App), and an external job portal (Koinoti Nav).

## Architecture

### Backend (`/recruitment-backend`)
- **Core**: Rust (2021 edition) using **Axum 0.7** for the web server.
- **Database**: PostgreSQL with **SQLx** for type-safe async queries.
- **Authentication**: JWT-based auth with Argon2 password hashing.
- **Async Runtime**: Tokio.
- **API Style**: RESTful API with OpenAPI documentation (via `utoipa`).
- **Integrations**:
  - **Telegram**: Webhook-based bot interaction and Mini App support.
  - **AI**: OpenAI/OpenRouter integration for automated test generation and candidate analysis.
  - **Browser Automation**: Spawns Python scripts (`vacancy_creation.py`) using Selenium/Chromium for interactions with the Koinoti Nav admin panel.

### Frontend (`/frontend`)
- **Framework**: **Next.js 16.0.1** (React 19).
- **Styling**: Tailwind CSS v4 with `tailwindcss-animate`.
- **State Management**: TanStack Query (React Query).
- **UI Library**: Radix UI primitives + bespoke components (Shadcn-like).
- **Charts**: Recharts for dashboard analytics.

## Key Workflows
1.  **Vacancy Management**: 
    - Internal vacancies are synced or created manually.
    - External vacancies on Koinoti Nav are managed via Selenium automation triggered by the Rust backend.
2.  **Candidate Assessment**:
    - **Test Generation**: AI-generated tests based on job descriptions/skills.
    - **Test Taking**: Public-facing, token-secured test runner for candidates.
    - **Grading**: Automated MCQ grading + manual review for presentation tasks.
3.  **Communication**:
    - Integrated messaging system enabling recruiters to chat with candidates via Telegram.
    - Automated notifications for deadlines, invites, and grading results.

## Infrastructure
- **Docker**: Full containerization (`Dockerfile` for backend includes Python environment).
- **Migrations**: SQLx migrations for database schema management.

## Project Health
- The codebase uses modern, type-safe practices (Rust + TypeScript).
- The "First Form" integration appears to be a key business requirement.
- The hybrid Rust/Python approach for Selenium automation adds complexity but allows reusing the robust Python Selenium ecosystem.
