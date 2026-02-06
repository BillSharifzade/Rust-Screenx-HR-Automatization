# 1F Integration Documentation

**Version:** 1.1  
**Target Audience:** 1F Developers / Integrators

---

## 1. Overview
This integration consists of two API sets:
1.  **V2 Dedicated API (Recommended)**: Specifically designed for OneF, providing properly formatted data arrays and simplified interactions.
2.  **V1 Internal API (Legacy)**: Shared with the internal Recruitment Frontend. Can be used for advanced data syncing if needed.

All endpoints are hosted at: `https://your-recruitment-domain.com`

---

## 2. Webhooks (Real-Time Push)

Enable real-time updates by configuring `ONEF_WEBHOOK_URL`.

### New Application
Triggered when a candidate applies.
```json
{
  "requestBody": {
    "event_type": "new_application",
    "vacancy_id": 123,
    "candidate": {
      "id": "uuid",
      "name": "John Doe",
      "telegram_id": 123456789
    }
  }
}
```

### New Message
Triggered when a candidate sends a message via Telegram.
```json
{
  "requestBody": {
    "event_type": "new_message",
    "candidate_id": "uuid",
    "telegram_id": 123456789,
    "text": "Hello!",
    "received_at": "2024-02-06T10:00:00Z"
  }
}
```

---

## 3. V2: Dedicated OneF API (Recommended)

### 3.1 Dashboard Stats
Optimized dashboard metrics object.

*   **Endpoint:** `GET /api/onef/dashboard`
*   **Response:**
```json
{
  "candidates_total": 150,
  "candidates_new_today": 5,
  "active_vacancies": 12,
  "test_attempts_pending": 8,
  "recruitment_funnel": {
    "registered": 150,
    "applied": 145,
    "test_started": 100,
    "test_completed": 85,
    "hired": 5
  }
}
```

### 3.2 Chat History
Returns a flat list of messages suitable for a chat UI. Automatically marks inbound messages as read.

*   **Endpoint:** `GET /api/onef/messages/{candidate_id}`
*   **Response:**
```json
[
  {
    "id": "uuid",
    "direction": "inbound",
    "text": "I am interested",
    "created_at": "2024-02-06T10:00:00Z",
    "is_read": true
  },
  {
    "id": "uuid",
    "direction": "outbound",
    "text": "Great, let's schedule a call",
    "created_at": "2024-02-06T10:05:00Z",
    "is_read": true
  }
]
```

### 3.3 Send Message
Send a message from 1F to the candidate (via Telegram).

*   **Endpoint:** `POST /api/onef/messages`
*   **Payload:**
```json
{
  "candidate_id": "uuid",
  "text": "Hello, are you free?"
}
```

### 3.4 Unread Count
Total unread messages for notification badges.

*   **Endpoint:** `GET /api/onef/messages/unread`
*   **Response:** `{"unread_count": 3}`

### 3.5 Get Candidate Details
Retrieves full candidate information, including AI suitability scores.

*   **Endpoint:** `GET /api/onef/candidates/{id}`
*   **Response:**
```json
{
  "id": "uuid",
  "telegram_id": 123456789,
  "name": "John Doe",
  "email": "john@example.com",
  "phone": "+123456789",
  "cv_url": "uploads/cv/uuid.pdf",
  "status": "new",
  "ai_rating": 85,
  "ai_comment": "Strong technical skills...",
  "created_at": "2024-02-06T10:00:00Z"
}
```

### 3.7 Update Candidate Status
Sync recruiting decisions from 1F to the platform.

*   **Endpoint:** `POST /api/onef/candidates/{id}/status`
*   **Payload:**
```json
{
  "status": "accepted"
}
```
*Statuses:* `new`, `reviewing`, `accepted`, `rejected`.

### 3.8 List Vacancies
Get all published vacancies.

*   **Endpoint:** `GET /api/onef/vacancies`

### 3.9 Get Vacancy Details
*   **Endpoint:** `GET /api/onef/vacancies/{id}`

### 3.10 List Candidate Test Attempts
Retrieves all test invitations and results for a candidate.

*   **Endpoint:** `GET /api/onef/candidates/{id}/attempts`
*   **Response:**
```json
{
  "items": [...],
  "total": 2
}
```

### 3.11 Get Detailed Attempt Results
Fetch the full report of a specific test attempt (score, answers, etc.).

*   **Endpoint:** `GET /api/onef/attempts/{id}`

### 3.12 Trigger AI Analysis
Manually trigger a fresh AI suitability analysis for a candidate.

*   **Endpoint:** `POST /api/onef/candidates/{id}/analyze`
*   **Response:** Returns the updated candidate object with new `ai_rating` and `ai_comment`.




---

## 4. V1: Internal API (Legacy/Advanced)

These endpoints mirror the internal dashboard functionality. Use them if you need raw data structures matching the Frontend.

### 4.1 Dashboard Stats (Detailed)
Returns raw hashmaps for charts.

*   **Endpoint:** `GET /api/integration/dashboard/stats`
*   **Response:**
```json
{
  "total_candidates": 150,
  "unread_messages": 5,
  "active_tests": 10,
  "active_vacancies": 12,
  "candidates_by_status": {
    "new": 10,
    "accepted": 5,
    "rejected": 2
  },
  "attempts_status": {
    "pending": 5,
    "completed": 20
  },
  "candidates_history": [
    ["2024-02-01", 3],
    ["2024-02-02", 5]
  ]
}
```

### 4.2 Sync Candidate Statuses
Bulk fetch of all candidates and their current test statuses.

*   **Endpoint:** `GET /api/integration/candidates/statuses`
*   **Response:**
```json
[
  {
    "id": "uuid",
    "external_id": "123456789",
    "name": "John Doe",
    "email": "john@example.com",
    "status": "pending",
    "last_updated": "2024-02-06T10:00:00Z"
  }
]
```

### 4.3 Send Message (Flexible)
Allows sending by Telegram ID directly.

*   **Endpoint:** `POST /api/integration/messages`
*   **Payload:**
```json
{
  "candidate_id": "uuid", 
  // OR
  "telegram_id": 123456789,
  "text": "Message content"
}
```

### 4.4 Get Chat Messages (Raw)
Returns raw database message objects.

*   **Endpoint:** `GET /api/integration/messages/{candidate_id}`

### 4.5 Unread Count
*   **Endpoint:** `GET /api/integration/messages/unread`
*   **Response:** `{"unread_count": 5}`
