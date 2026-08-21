# 1F Integration Documentation

**Version:** 1.3  
**Last Updated:** 2026-08-21  
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
      "cv_url": "https://recruit.work/uploads/cv_123.pdf",
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
  "test_id": "uuid"
}
```
*   **Response:** Details of the created invitation, including the `test_url`.

#### Filter Test Attempts
*   **Endpoint:** `GET /attempts_filter`
*   **Query Parameters:** `status`, `email`, `page`, `limit`
*   **Description:** Filterable list of all test attempts with pagination payload.

#### List All Attempts
*   **Endpoint:** `GET /attempts`
*   **Description:** Simple robust query without complex query params, directly emits all test attempts.

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

### 3.7 AI Vacancy Matching

Ranks the whole 1F vacancy catalogue for one candidate. Self-contained: it keeps its own
synced copy of 1F's vacancies and does not touch the platform's `/vacancies` records or
the application pipeline.

**Where the data comes from.** The platform pulls vacancies from
`GET http://192.168.1.47/app/v1.2/api/publications/action/getVacancies` on a timer
(only that host serves it — the others return 404). Candidate data is read from the
platform's own database; nothing about the candidate comes from 1F.

#### Rank Vacancies For A Candidate
*   **Endpoint:** `POST /matching/candidate`
*   **Payload:** `{ "candidate_id": "uuid", "top_n": 5 }` — bare or wrapped in `requestBody`.
*   **`candidate_id` is the platform UUID 1F already has**: the `candidate.id` from
    `new_application`, and the `candidate_id` in every later webhook.
*   Cold calls run two AI requests and take 5-15 seconds — **use a timeout of 60s or more**.
    Repeat calls are cached and instant.

```json
{
  "candidate_id": "a109320e-...",
  "candidate_name": "Собиров Суннатулло",
  "scoreable": true,
  "cached": false,
  "catalogue_size": 5,
  "cv_chars": 0,
  "profile": { "education_level": "Высшее", "total_experience_years": 7.0, "...": "..." },
  "matches": [
    {
      "vacancy_id_1f": 29502,
      "external_vacancy_id": "205",
      "name": "Проектный менеджер",
      "company": "BYD",
      "score": 80,
      "rank": 1,
      "breakdown": { "education": 90, "experience": 85, "specialty": 80, "...": 0 },
      "matched": ["Высшее образование", "Опыт 3–5 лет"],
      "missing": ["Таджикский язык"],
      "unknown": ["computer_skills"],
      "comment": "Сильное соответствие по специальности и опыту.",
      "flags": {
        "age_requirement": "от 20 до 30 лет",
        "age_mismatch": false,
        "gender_requirement": "Мужской",
        "data_quality": 60,
        "low_data_quality": true,
        "low_confidence": false
      }
    }
  ]
}
```

**Four things to handle on the 1F side:**

1. **Check `scoreable` before reading scores.** When a CV yields no professional
   information the platform returns `scoreable: false`, a Russian `reason`, and an empty
   `matches` array — rather than inventing numbers from nothing.
2. **Age and gender never affect the score.** They appear in `flags` only, for a human to
   weigh. `age_mismatch` is computed from the candidate's date of birth; it is absent when
   the vacancy states no age requirement or no date of birth is on file. Gender is echoed
   from the vacancy and never compared, because the platform stores no candidate gender.
3. **`unknown` lists dimensions scored 50 for lack of data**, so a neutral 50 can be told
   apart from a genuinely mediocre 50.
4. **`low_data_quality`** means that vacancy's requirement text was placeholder or junk
   when synced. Treat its score with caution.

`cv_chars: 0` does not mean the CV was empty — image CVs and unreadable PDFs are processed
with Vision instead, and still produce a full profile.

#### Catalogue Inspection
*   `GET /matching/vacancies` — the synced catalogue plus `last_synced_at`. Add
    `?include_inactive=true` to include vacancies 1F has withdrawn (kept, not deleted).
*   `GET /matching/vacancies/{vacancy_id_1f}` — one vacancy, by 1F's internal id.
*   `POST /matching/sync` — force a refresh instead of waiting for the timer. Use after
    publishing or editing a vacancy in 1F. Returns counts of what changed. An empty
    response from 1F is treated as a fault and leaves the catalogue untouched.

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
