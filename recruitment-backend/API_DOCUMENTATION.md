# Recruitment Platform API Documentation

> **Version:** 1.0  
> **Last Updated:** 2026-01-08  
> **Base URL:** `https://your-domain.com` or `http://localhost:8000`

---

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Data Models](#data-models)
4. [Candidate Endpoints](#candidate-endpoints)
5. [Vacancy Endpoints](#vacancy-endpoints)
6. [Application Endpoints](#application-endpoints)
7. [Integration Endpoints](#integration-endpoints)
8. [1F Integration Webhook](#1f-integration-webhook)
9. [Error Handling](#error-handling)
10. [Webhooks](#webhooks)

---

## Overview

This API provides functionality for managing candidates, vacancies, and their interactions in a recruitment management system. The system supports:

- **Candidate Registration** - Create candidate profiles with CV upload
- **Vacancy Browsing** - Fetch available job vacancies from external sources
- **Application Management** - Candidates can apply to multiple vacancies
- **Admin Management** - View candidates, their applications, and manage the recruitment pipeline

### Rate Limiting

| API Type | Rate Limit |
|----------|------------|
| Public API | 20 requests/second |
| Integration API | 10 requests/second |

---

## Authentication

Currently, public endpoints do not require authentication. Integration endpoints may require API keys in future versions.

### Headers (Future)
```
Authorization: Bearer <api_key>
Content-Type: application/json
```

---

## Data Models

### Candidate

The core entity representing a job applicant.

```typescript
interface Candidate {
  id: string;                    // UUID v4
  telegram_id: number;           // **REQUIRED** Telegram user ID (BIGINT)
  name: string;                  // Full name (max 255 chars)
  email: string;                 // Unique email address
  phone?: string;                // Phone number (max 50 chars)
  cv_url?: string;               // Path to uploaded CV file
  dob?: string;                  // Date of birth (YYYY-MM-DD format)
  vacancy_id?: number;           // Initial vacancy applied for
  profile_data?: object;         // Additional JSON profile data
  created_at?: string;           // ISO 8601 timestamp
  updated_at?: string;           // ISO 8601 timestamp
}
```

> **⚠️ Important:** As of v1.1, `telegram_id` is **mandatory**. Registration can only occur via the Telegram bot to ensure identity verification.

**Database Schema:**
```sql
CREATE TABLE candidates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    telegram_id BIGINT UNIQUE,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    phone VARCHAR(50),
    cv_url TEXT,
    dob DATE,
    vacancy_id BIGINT,
    profile_data JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### CandidateApplication

Junction table representing a many-to-many relationship between candidates and vacancies.

```typescript
interface CandidateApplication {
  id: number;                    // Auto-incrementing ID
  candidate_id: string;          // UUID reference to candidate
  vacancy_id: number;            // External vacancy ID (BIGINT)
  created_at: string;            // ISO 8601 timestamp
}
```

**Database Schema:**
```sql
CREATE TABLE candidate_applications (
    id SERIAL PRIMARY KEY,
    candidate_id UUID NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    vacancy_id BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(candidate_id, vacancy_id)  -- Prevents duplicate applications
);
```

### ExternalVacancy

Job vacancy fetched from the external job portal (job.koinotinav.tj).

```typescript
interface ExternalVacancy {
  id: number;                    // Unique vacancy ID
  title: string;                 // Job title (may contain HTML)
  content: string;               // Full job description (HTML)
  hot: boolean;                  // Priority/featured flag
  city: string;                  // Job location city
  direction: string;             // Job category/direction
  company_id: number | null;     // Reference to company
  created_at: string;            // ISO 8601 timestamp
}
```

### ExternalCompany

Company information from external source.

```typescript
interface ExternalCompany {
  id: number;                    // Unique company ID
  title: string;                 // Company name
  logo: string;                  // Logo URL
}
```

---

## Candidate Endpoints

### 1. Register Candidate

Creates a new candidate profile with optional CV upload.

**Endpoint:** `POST /api/candidate/register`

**Content-Type:** `multipart/form-data`

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | ✅ Yes | Candidate's full name |
| `email` | string | ✅ Yes | Unique email address |
| `telegram_id` | string | ✅ Yes | **REQUIRED** - Telegram user ID (numeric string) |
| `phone` | string | No | Phone number |
| `vacancy_id` | string | No | Initial vacancy ID to apply for (numeric string) |
| `dob` | string | No | Date of birth (YYYY-MM-DD format) |
| `cv` | file | No | CV/Resume file upload |
| `profile_data` | string | No | JSON string with additional profile data |

> **Security Note:** `telegram_id` is mandatory to ensure candidates can only register through the Telegram bot, preventing unauthorized registrations.

**Example Request (cURL):**
```bash
curl -X POST "https://api.example.com/api/candidate/register" \
  -F "name=John Doe" \
  -F "email=john.doe@example.com" \
  -F "phone=+992901234567" \
  -F "telegram_id=1320166360" \
  -F "vacancy_id=142" \
  -F "dob=1995-06-15" \
  -F "cv=@/path/to/resume.pdf" \
  -F 'profile_data={"skills": ["Python", "JavaScript"], "experience_years": 5}'
```

**Success Response:**
```json
{
  "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
  "status": "success"
}
```

| Status Code | Description |
|-------------|-------------|
| `201 Created` | Candidate successfully registered |
| `400 Bad Request` | Missing required fields (name/email) |
| `500 Internal Server Error` | Server/database error |

**Notes:**
- If `vacancy_id` is provided, an automatic application entry is created in `candidate_applications`
- The `cv` file is stored in `./uploads/cv/` with a UUID prefix
- Email must be unique across all candidates

---

### 2. Get Candidate by ID

Retrieves a candidate's full profile.

**Endpoint:** `GET /api/candidate/:id`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Candidate's unique identifier |

**Example Request:**
```bash
curl "https://api.example.com/api/candidate/5dfedd06-9844-4468-807d-97e79ce2c9bc"
```

**Success Response:**
```json
{
  "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
  "telegram_id": 1320166360,
  "name": "John Doe",
  "email": "john.doe@example.com",
  "phone": "+992901234567",
  "cv_url": "./uploads/cv/abc123_resume.pdf",
  "dob": "1995-06-15",
  "vacancy_id": 142,
  "profile_data": {
    "skills": ["Python", "JavaScript"],
    "experience_years": 5
  },
  "created_at": "2026-01-08T10:30:00Z",
  "updated_at": "2026-01-08T10:30:00Z"
}
```

| Status Code | Description |
|-------------|-------------|
| `200 OK` | Candidate found |
| `404 Not Found` | Candidate does not exist |

---

### 3. Update Candidate CV

Updates the CV file for an existing candidate.

**Endpoint:** `PATCH /api/candidate/:id/cv`

**Content-Type:** `multipart/form-data`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Candidate's unique identifier |

**Request Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cv` | file | ✅ Yes | New CV/Resume file |

**Example Request:**
```bash
curl -X PATCH "https://api.example.com/api/candidate/5dfedd06-9844-4468-807d-97e79ce2c9bc/cv" \
  -F "cv=@/path/to/new_resume.pdf"
```

**Success Response:**
```json
{
  "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
  "telegram_id": 1320166360,
  "name": "John Doe",
  "email": "john.doe@example.com",
  "phone": "+992901234567",
  "cv_url": "./uploads/cv/def456_new_resume.pdf",
  "dob": "1995-06-15",
  "vacancy_id": 142,
  "profile_data": null,
  "created_at": "2026-01-08T10:30:00Z",
  "updated_at": "2026-01-08T11:45:00Z"
}
```

| Status Code | Description |
|-------------|-------------|
| `200 OK` | CV successfully updated |
| `400 Bad Request` | No CV file provided |
| `404 Not Found` | Candidate does not exist |

---

### 4. List All Candidates (Admin)

Retrieves all registered candidates.

**Endpoint:** `GET /api/integration/candidates`

**Example Request:**
```bash
curl "https://api.example.com/api/integration/candidates"
```

**Success Response:**
```json
[
  {
    "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "telegram_id": 1320166360,
    "name": "John Doe",
    "email": "john.doe@example.com",
    "phone": "+992901234567",
    "cv_url": "./uploads/cv/abc123_resume.pdf",
    "dob": "1995-06-15",
    "vacancy_id": 142,
    "profile_data": null,
    "created_at": "2026-01-08T10:30:00Z",
    "updated_at": "2026-01-08T10:30:00Z"
  },
  {
    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "telegram_id": null,
    "name": "Jane Smith",
    "email": "jane.smith@example.com",
    "phone": null,
    "cv_url": null,
    "dob": null,
    "vacancy_id": 145,
    "profile_data": null,
    "created_at": "2026-01-07T14:20:00Z",
    "updated_at": "2026-01-07T14:20:00Z"
  }
]
```

**Notes:**
- Returns candidates ordered by `created_at DESC` (newest first)
- This is an integration/admin endpoint

---

## Vacancy Endpoints

### 1. Get External Vacancies

Fetches all available job vacancies from the external source with company information.

**Endpoint:** `GET /api/external-vacancies`

> **Note:** Also available at `GET /api/integration/external-vacancies` for admin use

**Example Request:**
```bash
curl "https://api.example.com/api/external-vacancies"
```

**Success Response:**
```json
{
  "vacancies": [
    {
      "id": 142,
      "title": "<b>Senior Software Developer</b>",
      "content": "<p>We are looking for an experienced developer...</p>",
      "hot": true,
      "city": "Dushanbe",
      "direction": "IT",
      "company_id": 5,
      "created_at": "2026-01-05T09:00:00Z"
    },
    {
      "id": 145,
      "title": "Marketing Manager",
      "content": "<p>Join our marketing team...</p>",
      "hot": false,
      "city": "Khujand",
      "direction": "Marketing",
      "company_id": 3,
      "created_at": "2026-01-04T15:30:00Z"
    }
  ],
  "companies": [
    {
      "id": 5,
      "title": "Tech Corp",
      "logo": "https://example.com/logos/techcorp.png"
    },
    {
      "id": 3,
      "title": "Marketing Agency",
      "logo": "https://example.com/logos/marketing.png"
    }
  ]
}
```

**Notes:**
- Vacancies are filtered to only include those with `id >= 137`
- `title` and `content` fields may contain HTML markup
- Use `company_id` to look up company details from the `companies` array

---

## Application Endpoints

### 1. Apply for Vacancy

Records a candidate's application to a specific vacancy.

**Endpoint:** `POST /api/candidate/apply`

**Content-Type:** `application/json`

**Request Body:**
```json
{
  "candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
  "vacancy_id": 145,
  "vacancy_name": "Senior Software Developer"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `candidate_id` | UUID string | ✅ Yes | Candidate's unique identifier |
| `vacancy_id` | number | ✅ Yes | External vacancy ID |
| `vacancy_name` | string | Recommended | Vacancy title (used for 1F integration) |

**Example Request:**
```bash
curl -X POST "https://api.example.com/api/candidate/apply" \
  -H "Content-Type: application/json" \
  -d '{"candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc", "vacancy_id": 145}'
```

**Success Response:**
```json
{
  "id": 12,
  "candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
  "vacancy_id": 145,
  "created_at": "2026-01-08T12:00:00Z"
}
```

| Status Code | Description |
|-------------|-------------|
| `201 Created` | Application successfully recorded |
| `500 Internal Server Error` | Database error (may indicate duplicate application) |

**Notes:**
- A candidate **cannot** apply to the same vacancy more than once (unique constraint)
- If a duplicate application is attempted, the existing record is returned (upsert behavior)
- There is **no limit** on the number of different vacancies a candidate can apply to
- **1F Integration:** When an application is created, the system automatically sends a notification to the configured 1F webhook (if enabled)

---

### 2. Get Candidate's Applications

Retrieves all vacancy applications for a specific candidate.

**Endpoint:** `GET /api/candidate/:id/applications`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | UUID | Candidate's unique identifier |

**Example Request:**
```bash
curl "https://api.example.com/api/candidate/5dfedd06-9844-4468-807d-97e79ce2c9bc/applications"
```

**Success Response:**
```json
[
  {
    "id": 12,
    "candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "vacancy_id": 145,
    "created_at": "2026-01-08T12:00:00Z"
  },
  {
    "id": 8,
    "candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "vacancy_id": 142,
    "created_at": "2026-01-07T14:30:00Z"
  }
]
```

**Notes:**
- Returns applications ordered by `created_at DESC` (newest first)
- Use vacancy `id` to look up vacancy details from the external vacancies endpoint

---

### 3. Get Candidates for Vacancy

Retrieves all candidates who have applied to a specific vacancy.

**Endpoint:** `GET /api/vacancy/:id/candidates`

**Path Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | number | External vacancy ID |

**Example Request:**
```bash
curl "https://api.example.com/api/vacancy/145/candidates"
```

**Success Response:**
```json
[
  {
    "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "telegram_id": 1320166360,
    "name": "John Doe",
    "email": "john.doe@example.com",
    "phone": "+992901234567",
    "cv_url": "./uploads/cv/abc123_resume.pdf",
    "dob": "1995-06-15",
    "vacancy_id": 142,
    "profile_data": null,
    "created_at": "2026-01-08T10:30:00Z",
    "updated_at": "2026-01-08T10:30:00Z"
  },
  {
    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "telegram_id": null,
    "name": "Jane Smith",
    "email": "jane.smith@example.com",
    "phone": null,
    "cv_url": null,
    "dob": null,
    "vacancy_id": 145,
    "profile_data": null,
    "created_at": "2026-01-07T14:20:00Z",
    "updated_at": "2026-01-07T14:20:00Z"
  }
]
```

**Notes:**
- Returns full candidate profiles for all applicants
- Ordered by application date (`created_at DESC`)
- Useful for admin dashboards to see who applied to a specific job

---

## CV File Access

Uploaded CV files are accessible via the static file server.

**Endpoint:** `GET /uploads/cv/:filename`

**Example:**
```bash
curl "https://api.example.com/uploads/cv/abc123_resume.pdf" -o resume.pdf
```

---

## 1F Integration Webhook

When a candidate applies for a vacancy, the system can automatically notify an external "1F" service. This enables real-time integration with external HR or recruitment systems.

### Configuration

Set the following environment variable to enable 1F integration:

```bash
ONEF_WEBHOOK_URL=https://1f-service.example.com/api/applications
```

If this variable is not set, 1F integration is disabled and no webhook calls are made.

### Webhook Payload

When a candidate applies (either during registration with `vacancy_id` or via `POST /api/candidate/apply`), the following JSON payload is POSTed to the configured URL:

```json
{
  "vacancy_id": 137,
  "vacancy_name": "Senior Software Developer",
  "candidate": {
    "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "telegram_id": 1320166360,
    "fullname": "John Doe",
    "name": "John",
    "surname": "Doe",
    "email": "john.doe@example.com",
    "phone": "+992901234567",
    "dob": "1995-06-15",
    "cv_url": "https://7c82b584eac1.ngrok-free.app/uploads/cv/abc-123.pdf"
  },
  "applied_at": "2026-01-08T12:00:00+00:00"
}
```
### Webhook response example
```bash 
🚀 1F Test Webhook Server running on http://localhost:9000/webhook
   Set ONEF_WEBHOOK_URL=http://localhost:9000/webhook in .env

============================================================
🔔 RECEIVED 1F WEBHOOK
============================================================
Path: /webhook
Headers: {'content-type': 'application/json', 'x-source': 'recruitment-platform', 'accept': '*/*', 'host': 'localhost:9000', 'content-length': '385'}

Payload:
{
  "vacancy_id": 148,
  "vacancy_name": "Full Payload Test",
  "candidate": {
    "id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "telegram_id": 1320166360,
    "fullname": "John Doe",
    "name": "John",
    "surname": "Doe",
    "email": "hsdfhs@jnf.cd",
    "phone": "992992992992",
    "dob": "2005-10-12",
    "cv_url": "https://7c82b584eac1.ngrok-free.app/uploads/cv/cv_file.pdf"
  },
  "applied_at": "2026-01-08T09:46:31.418868002+00:00"
}
============================================================

127.0.0.1 - - [08/Jan/2026 14:46:31] "POST /webhook HTTP/1.1" 200 -
```
### Curl command example
```bash 
curl -X POST "http://localhost:8000/api/candidate/apply" \
  -H "Content-Type: application/json" \
  -d '{
    "candidate_id": "5dfedd06-9844-4468-807d-97e79ce2c9bc",
    "vacancy_id": 147,
    "vacancy_name": "Senior Rust Developer (Final Test)"
  }'
```
### Payload Fields

| Field | Type | Description |
|-------|------|-------------|
| `vacancy_id` | number | External vacancy ID |
| `vacancy_name` | string | Vacancy title (HTML stripped) |
| `candidate.id` | UUID string | Internal candidate identifier |
| `candidate.telegram_id` | number | **Always present** - Telegram user ID |
| `candidate.fullname` | string | Full name (combined) |
| `candidate.name` | string | First name |
| `candidate.surname` | string | Last name/Surname |
| `candidate.email` | string | Candidate's email |
| `candidate.phone` | string \| null | Phone number (if provided) |
| `candidate.dob` | string \| null | Date of birth in YYYY-MM-DD format |
| `candidate.cv_url` | string \| null | Downloadable link to the candidate's CV (built using `WEBAPP_URL`) |
| `applied_at` | string | ISO 8601 timestamp of application |

### Headers

The webhook request includes:

```
Content-Type: application/json
X-Source: recruitment-platform
```

### Expected Response

The 1F service should respond with:

```json
{
  "success": true,
  "message": "Application received"
}
```

### Error Handling

- **Timeout:** 10 seconds
- **Failure Behavior:** Webhook failures are logged but do **not** affect the application process (fire-and-forget)
- **No Retry:** Failed webhooks are not retried automatically

### TypeScript Interface

```typescript
interface OneFApplicationPayload {
  vacancy_id: number;
  vacancy_name: string;
  candidate: {
    id: string;
    telegram_id: number;
    name: string;
    email: string;
    phone: string | null;
    dob: string | null;
    cv_url: string | null;
  };
  applied_at: string;
}
```

---

## Error Handling

All errors follow a consistent format:

```json
{
  "error": "Error description message"
}
```

### Common Error Codes

| Status Code | Name | Description |
|-------------|------|-------------|
| `400` | Bad Request | Invalid input data or missing required fields |
| `404` | Not Found | Requested resource does not exist |
| `409` | Conflict | Duplicate entry (e.g., email already exists) |
| `500` | Internal Server Error | Server-side error |

---

## Webhooks

### Telegram Bot Webhook

The system supports Telegram bot integration for candidate registration and profile access.

**Endpoint:** `POST /api/webhook/telegram`

This endpoint receives updates from the Telegram Bot API.

**Behavior:**
- `/start` command: Returns candidate profile if registered, or sends registration link
- Candidate lookup is done via `telegram_id`

---

## Integration Checklist

When integrating with this API, ensure:

1. **Candidate Registration Flow:**
   - Collect required fields (name, email)
   - Handle CV file upload via multipart/form-data
   - Store returned `id` for future reference

2. **Vacancy Application Flow:**
   - Fetch vacancies from `/api/external-vacancies`
   - Display to user with company information
   - Apply via `POST /api/candidate/apply`
   - Handle duplicate application gracefully

3. **Admin Dashboard Integration:**
   - List candidates: `GET /api/integration/candidates`
   - View applicants per vacancy: `GET /api/vacancy/:id/candidates`
   - View candidate applications: `GET /api/candidate/:id/applications`

4. **File Handling:**
   - CV files are stored locally in `./uploads/cv/`
   - Access via `/uploads/cv/:filename`
   - Files are named with UUID prefix to avoid conflicts

---

## Quick Reference

| Operation | Method | Endpoint |
|-----------|--------|----------|
| Register candidate | POST | `/api/candidate/register` |
| Get candidate | GET | `/api/candidate/:id` |
| Update candidate CV | PATCH | `/api/candidate/:id/cv` |
| List all candidates | GET | `/api/integration/candidates` |
| Get vacancies | GET | `/api/external-vacancies` |
| Apply to vacancy | POST | `/api/candidate/apply` |
| Get candidate's applications | GET | `/api/candidate/:id/applications` |
| Get vacancy's applicants | GET | `/api/vacancy/:id/candidates` |

---

## Contact & Support

For API questions or integration support, contact the development team.
