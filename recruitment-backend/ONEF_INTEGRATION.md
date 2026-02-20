# 1F Integration Documentation

**Version:** 1.2  
**Last Updated:** 2026-02-11  
**Target Audience:** 1F Developers / Integrators

---

## 1. Overview
The OneF (1F) integration allows for real-time synchronization of recruitment data, candidate communication via Telegram, and management of technical assessments.

All dedicated OneF endpoints are hosted at: `https://your-recruitment-domain.com/api/onef/`

### Authentication
Currently, the OneF API routes are protected by a rate-limiting layer. Ensure your requests do not exceed the configured `INTEGRATION_RPS` (default: 10 requests per second). Authenticated access via API keys is planned for future versions.

---

## 2. Webhooks (Real-Time Push)

Enable real-time updates by configuring the `ONEF_WEBHOOK_URL` environment variable. All webhooks are sent as `POST` requests with a standard JSON wrapper.

### Standard Request Wrapper
```json
{
  "requestBody": {
    "event_type": "string",
    ...
  }
}
```

### 2.1 New Application (`new_application`)
Triggered when a candidate applies for a vacancy (internal or external).
```json
{
  "requestBody": {
    "event_type": "new_application",
    "vacancy_id": 123,
    "vacancy_name": "Senior Rust Developer",
    "applied_at": "2026-02-11T12:00:00Z",
    "candidate": {
      "id": "uuid",
      "telegram_id": 123456789,
      "fullname": "John Doe",
      "name": "John",
      "surname": "Doe",
      "email": "john@example.com",
      "phone": "+123456789",
      "dob": "1990-01-01",
      "cv_file_base64": "JVBERi0xLjQKJc...",
      "cv_filename": "cv.pdf",
      "ai_rating": 85,
      "ai_comment": "Strong match based on experience."
    }
  }
}
```

### 2.2 New Message (`new_message`)
Triggered when a candidate sends a message via the Telegram bot.
```json
{
  "requestBody": {
    "event_type": "new_message",
    "candidate_id": "uuid",
    "telegram_id": 123456789,
    "text": "Hello, I have a question about the test.",
    "received_at": "2026-02-11T12:05:00Z"
  }
}
```

### 2.3 Candidate Status Changed (`candidate_status_changed`)
Triggered when an HR manager updates a candidate's status.
```json
{
  "requestBody": {
    "event_type": "candidate_status_changed",
    "candidate_id": "uuid",
    "status": "reviewing",
    "updated_at": "2026-02-11T12:10:00Z"
  }
}
```

### 2.4 Test Status Changed (`test_status_changed`)
Triggered when a candidate starts or submits a test.
```json
{
  "requestBody": {
    "event_type": "test_status_changed",
    "attempt_id": "uuid",
    "candidate_id": "uuid",
    "test_id": "uuid",
    "status": "in_progress", 
    "score": 85.0,
    "max_score": 100.0,
    "percentage": 85.0,
    "passed": true,
    "updated_at": "2026-02-11T12:15:00Z"
  }
}
```
*Note: `score`, `max_score`, `percentage`, and `passed` are only present when the status is `completed`, `passed`, or `failed`.*

### 2.5 Grade Shared (`grade_shared`)
Triggered when a grade is manually shared with OneF.
```json
{
  "requestBody": {
    "event_type": "grade_shared",
    "candidate_id": "uuid",
    "grade": 90,
    "shared_at": "2026-02-11T12:20:00Z"
  }
}
```

---

## 3. V2: Dedicated OneF API Endpoints

### 3.1 Dashboard Stats
*   **Endpoint:** `GET /dashboard`
*   **Description:** Retrieves high-level recruitment metrics.
*   **Response:**
```json
{
  "candidates_total": 1500,
  "candidates_new_today": 12,
  "active_vacancies": 45,
  "test_attempts_pending": 82,
  "recruitment_funnel": {
    "registered": 1500,
    "applied": 1420,
    "test_started": 800,
    "test_completed": 650,
    "hired": 52
  }
}
```

### 3.2 Candidate Management

#### List Candidates
*   **Endpoint:** `GET /candidates`
*   **Description:** Retrieves a list of all candidates.
*   **Response:** Array of Candidate objects.

#### Get Candidate Details
*   **Endpoint:** `GET /candidates/{id}`
*   **Description:** Retrieves full profile information for a specific candidate.

#### Update Candidate Status
*   **Endpoint:** `POST /candidates/{id}/status`
*   **Payload:** `{ "status": "reviewing" }`
*   **Response:** `{ "id": "uuid", "status": "reviewing", "updated_at": "..." }`

#### Trigger AI Analysis
*   **Endpoint:** `POST /candidates/{id}/analyze`
*   **Description:** Manually triggers a new AI suitability analysis.
*   **Response:** Updated Candidate object.

---

### 3.3 Chat & Communication

#### Get Chat History
*   **Endpoint:** `GET /messages/{candidate_id}`
*   **Description:** Returns all messages for a candidate. Automatically marks inbound messages as read.
*   **Response:**
```json
[
  {
    "id": "uuid",
    "direction": "inbound",
    "text": "Hello!",
    "created_at": "...",
    "is_read": true
  }
]
```

#### Send Message
*   **Endpoint:** `POST /messages`
*   **Description:** Sends a message to the candidate via Telegram.
*   **Payload:**
```json
{
  "candidate_id": "uuid",
  "text": "Your interview is scheduled for tomorrow."
}
```

#### Global Unread Count
*   **Endpoint:** `GET /messages/unread`
*   **Response:** `{ "unread_count": 5 }`

---

### 3.4 Tests & Invitations

#### List Active Tests
*   **Endpoint:** `GET /tests`
*   **Description:** Lists all tests available for assignment.

#### Create Test Invitation
*   **Endpoint:** `POST /invites`
*   **Payload:**
```json
{
  "candidate_id": "uuid",
  "test_id": "uuid",
  "expires_in_hours": 48
}
```
*   **Response:** Details of the created invitation, including the `test_url`.

#### List All Attempts
*   **Endpoint:** `GET /attempts`
*   **Query Parameters:** `status`, `email`, `page`, `limit`
*   **Description:** Filterable list of all test attempts.

#### Get Candidate Attempts
*   **Endpoint:** `GET /candidates/{id}/attempts`
*   **Description:** Lists all test attempts for a specific candidate.

#### Get Detailed Attempt Result
*   **Endpoint:** `GET /attempts/{id}`
*   **Description:** Retrieves full results for a specific attempt, including answers.

---

### 3.5 Vacancies

#### List Vacancies
*   **Endpoint:** `GET /vacancies`
*   **Description:** Lists all published vacancies.

#### Get Vacancy Details
*   **Endpoint:** `GET /vacancies/{id}`

---

### 3.6 Dictionaries

#### Candidate Statuses
*   **Endpoint:** `GET /dictionaries/candidate-statuses`
*   **Response:** `[{ "id": "new", "label": "New" }, ...]`

#### Test Statuses
*   **Endpoint:** `GET /dictionaries/test-statuses`
*   **Response:** `[{ "id": "pending", "label": "Pending (Invite Sent)" }, ...]`

---

## 4. Status Reference Table

### Candidate Statuses
- `new`: Just registered/applied.
- `reviewing`: CV is being reviewed by HR.
- `test_assigned`: Candidate has pending test invitations.
- `test_completed`: Candidate finished all assigned tests.
- `interview`: Candidate invited for an interview.
- `accepted`: Candidate passed and is hired.
- `rejected`: Candidate did not pass the selection process.

### Test Attempt Statuses
- `pending`: Invitation sent, link not yet accessed.
- `in_progress`: Candidate has started the test.
- `completed`: Test submitted, waiting for manual grading if needed.
- `needs_review`: MCQ finished, but contains open questions requiring manual review.
- `passed`: Score above threshold.
- `failed`: Score below threshold.
- `timeout`: Test closed automatically due to time limit.
- `escaped`: Candidate left the test session (heartbeat lost).
