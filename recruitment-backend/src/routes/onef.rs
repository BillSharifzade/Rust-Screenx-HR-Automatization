use crate::{
    error::Result,
    AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct OneFSendMessageRequest {
    pub candidate_id: Uuid,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct OneFUpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct OneFCreateInviteRequest {
    pub candidate_id: Uuid,
    pub test_id: Uuid,
    pub expires_in_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OneFTestSummary {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub test_type: String,
    pub duration_minutes: i32,
    pub passing_score: f64,
    pub is_active: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct OneFChatMessage {
    pub id: Uuid,
    pub direction: String,
    pub text: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_read: bool,
}

#[derive(Debug, Serialize)]
pub struct OneFDashboardStats {
    pub candidates_total: i64,
    pub candidates_new_today: i64,
    pub active_vacancies: i64,
    pub test_attempts_pending: i64,
    pub recruitment_funnel: RecruitmentFunnel,
}

#[derive(Debug, Serialize)]
pub struct RecruitmentFunnel {
    pub registered: i64,
    pub applied: i64,
    pub test_started: i64,
    pub test_completed: i64,
    pub hired: i64,
}

#[derive(Debug, Serialize)]
pub struct OneFCandidateResponse {
    pub id: Uuid,
    pub telegram_id: Option<i64>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub cv_url: Option<String>,
    pub status: String,
    pub ai_rating: Option<i32>,
    pub ai_comment: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn send_message(
    State(state): State<AppState>,
    Json(payload): Json<OneFSendMessageRequest>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(payload.candidate_id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    let telegram_id = candidate.telegram_id.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate has no linked Telegram account".into())
    })?;

    let config = crate::config::get_config();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_bot_token);
    let client = reqwest::Client::new();
    
    let telegram_body = json!({
        "chat_id": telegram_id,
        "text": payload.text,
    });

    let resp = client.post(&url)
        .json(&telegram_body)
        .send()
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Telegram request failed: {}", e)))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(crate::error::Error::Internal(format!("Telegram API error: {}", err_text)));
    }

    let create_msg = crate::models::message::CreateMessage {
        candidate_id: candidate.id,
        telegram_id,
        direction: "outbound".to_string(),
        text: payload.text,
    };
    
    let _ = state.message_service.create(create_msg).await?;

    Ok(StatusCode::OK)
}

pub async fn get_chat_history(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let messages = state.message_service.get_by_candidate(candidate_id).await?;
    
    let _ = state.message_service.mark_as_read(candidate_id).await;
    
    let onef_messages: Vec<OneFChatMessage> = messages.into_iter().map(|m| OneFChatMessage {
        id: m.id,
        direction: m.direction,
        text: m.text,
        created_at: m.created_at,
        is_read: m.read_at.is_some(),
    }).collect();

    Ok(Json(onef_messages))
}

pub async fn get_unread_count(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let count = state.message_service.total_unread_count().await?;
    Ok(Json(json!({ "unread_count": count })))
}

pub async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    
    let total_candidates_map = state.candidate_service.get_status_counts().await?;
    let total_candidates: i64 = total_candidates_map.values().sum();
    let history = state.candidate_service.get_history_counts().await?;
    let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let candidates_new_today = history.iter()
        .find(|(date, _)| *date == today_str)
        .map(|(_, count)| *count)
        .unwrap_or(0);

    let internal_vacancies = state.vacancy_service.list_published(50).await?.len() as i64;
    let external_vacancies = state.koinotinav_service.fetch_vacancies().await.map(|v| v.len() as i64).unwrap_or(0);
    let active_vacancies = internal_vacancies + external_vacancies;
    
    let attempts_status = state.attempt_service.get_status_distribution().await?;
    let test_attempts_pending = *attempts_status.get("pending").unwrap_or(&0);
    let test_completed = *attempts_status.get("completed").unwrap_or(&0) + *attempts_status.get("passed").unwrap_or(&0) + *attempts_status.get("failed").unwrap_or(&0);

    let funnel = RecruitmentFunnel {
        registered: total_candidates,
        applied: total_candidates,
        test_started: *attempts_status.get("in_progress").unwrap_or(&0) + test_completed,
        test_completed,
        hired: *total_candidates_map.get("accepted").unwrap_or(&0),
    };

    let stats = OneFDashboardStats {
        candidates_total: total_candidates,
        candidates_new_today,
        active_vacancies,
        test_attempts_pending,
        recruitment_funnel: funnel,
    };

    Ok(Json(stats))
}

pub async fn update_candidate_status(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
    Json(payload): Json<OneFUpdateStatusRequest>,
) -> Result<impl IntoResponse> {
    let _updated = state.candidate_service.update_status(candidate_id, payload.status.clone()).await?;
    

    Ok(Json(json!({ 
        "id": candidate_id, 
        "status": payload.status,
        "updated_at": chrono::Utc::now()
    })))
}

pub async fn get_candidate(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(candidate_id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    let response = OneFCandidateResponse {
        id: candidate.id,
        telegram_id: candidate.telegram_id,
        name: candidate.name,
        email: candidate.email,
        phone: candidate.phone,
        cv_url: candidate.cv_url,
        status: candidate.status,
        ai_rating: candidate.ai_rating,
        ai_comment: candidate.ai_comment,
        created_at: candidate.created_at,
    };

    Ok(Json(response))
}

pub async fn get_candidate_attempts(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(candidate_id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, total) = svc.list_attempts(None, Some(candidate.email), None, 1, 100).await?;

    Ok(Json(json!({
        "items": items,
        "total": total
    })))
}

pub async fn get_test_attempt(
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempt = svc.get_attempt_by_id(attempt_id).await?;
    
    let test = state.test_service.get_test_by_id(attempt.test_id).await?;

    Ok(Json(json!({
        "attempt": attempt,
        "test_title": test.title,
        "test_type": test.test_type
    })))
}

pub async fn list_candidates(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let candidates = state.candidate_service.list_candidates().await?;
    
    let response: Vec<OneFCandidateResponse> = candidates.into_iter().map(|c| OneFCandidateResponse {
        id: c.id,
        telegram_id: c.telegram_id,
        name: c.name,
        email: c.email,
        phone: c.phone,
        cv_url: c.cv_url,
        status: c.status,
        ai_rating: c.ai_rating,
        ai_comment: c.ai_comment,
        created_at: c.created_at,
    }).collect();

    Ok(Json(response))
}

pub async fn list_attempts_filter(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let status = params.get("status").cloned();
    let email = params.get("email").cloned();
    let page = params.get("page").and_then(|v| v.parse().ok()).unwrap_or(1);
    let limit = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(50);

    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, total) = svc.list_attempts(None, email, status, page, limit).await?;

    Ok(Json(json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit
    })))
}

pub async fn list_all_attempts(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, _) = svc.list_attempts(None, None, None, 1, 1000).await?;

    Ok(Json(items))
}

fn strip_html_tags(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut inside_tag = false;
    for c in input.chars() {
        if c == '<' {
            inside_tag = true;
        } else if c == '>' {
            inside_tag = false;
        } else if !inside_tag {
            output.push(c);
        }
    }
    output.trim().to_string()
}

pub async fn list_vacancies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let mut combined = Vec::new();

    if let Ok(internal) = state.vacancy_service.list_published(100).await {
        for v in internal {
            combined.push(serde_json::json!({
                "id": v.id.to_string(),
                "title": v.title,
                "company": v.company,
                "location": v.location,
                "status": v.status,
                "source": "internal",
                "created_at": v.created_at,
            }));
        }
    }

    if let Ok(external) = state.koinotinav_service.fetch_vacancies().await {
        for v in external {
            combined.push(serde_json::json!({
                "id": v.id.to_string(),
                "title": strip_html_tags(&v.title),
                "company": v.direction, 
                "location": v.city,
                "status": "published",
                "source": "external",
                "created_at": v.created_at,
            }));
        }
    }

    Ok(Json(combined))
}

pub async fn get_vacancy(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse> {
    if let Ok(uuid) = Uuid::parse_str(&id_str) {
        if let Ok(vacancy) = state.vacancy_service.get_by_id(uuid).await {
            return Ok(Json(serde_json::to_value(vacancy).unwrap()));
        }
    }

    if let Ok(ext_id) = id_str.parse::<i64>() {
        if let Ok(Some(ext_v)) = state.koinotinav_service.fetch_vacancy(ext_id).await {
            return Ok(Json(json!({
                "id": ext_v.id.to_string(),
                "title": strip_html_tags(&ext_v.title),
                "company": ext_v.direction,
                "location": ext_v.city,
                "description": ext_v.content,
                "status": "published",
                "source": "external",
                "created_at": ext_v.created_at,
            })));
        }
    }

    Err(crate::error::Error::NotFound("Vacancy not found".into()))
}

pub async fn list_tests(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let result = state.test_service.list_tests(
        1,
        100,
        Some(crate::services::test_service::TestFilter {
            is_active: Some(true),
            created_by: None,
            search: None,
        })
    ).await?;

    let tests: Vec<OneFTestSummary> = result.tests.into_iter().map(|t| {
        use rust_decimal::prelude::ToPrimitive;
        OneFTestSummary {
            id: t.id,
            title: t.title,
            description: t.description,
            test_type: t.test_type,
            duration_minutes: t.duration_minutes,
            passing_score: t.passing_score.to_f64().unwrap_or(0.0),
            is_active: t.is_active.unwrap_or(true),
            created_at: t.created_at,
        }
    }).collect();

    Ok(Json(json!({
        "items": tests,
        "total": result.total
    })))
}


pub async fn list_candidate_statuses(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!([
        { "id": "new", "label": "New" },
        { "id": "reviewing", "label": "Reviewing" },
        { "id": "test_assigned", "label": "Test Assigned" },
        { "id": "test_completed", "label": "Test Completed" },
        { "id": "interview", "label": "Interview" },
        { "id": "accepted", "label": "Accepted" },
        { "id": "rejected", "label": "Rejected" }
    ])))
}

pub async fn list_test_statuses(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!([
        { "id": "pending", "label": "Pending (Invite Sent)" },
        { "id": "in_progress", "label": "In Progress" },
        { "id": "completed", "label": "Completed (Waiting for Grading)" },
        { "id": "needs_review", "label": "Needs Manual Review" },
        { "id": "passed", "label": "Passed" },
        { "id": "failed", "label": "Failed" },
        { "id": "timeout", "label": "Timed Out" },
        { "id": "escaped", "label": "Escaped (Left Test)" }
    ])))
}

pub async fn create_test_invite(
    State(state): State<AppState>,
    Json(payload): Json<OneFCreateInviteRequest>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(payload.candidate_id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;
    let test = state.test_service.get_test_by_id(payload.test_id).await?;
    let expires_in_hours = payload.expires_in_hours.unwrap_or_else(|| {
        if test.duration_minutes > 0 && test.test_type == "presentation" {
            (test.duration_minutes / 60) as i64
        } else {
            48
        }
    });

    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let result = svc.create_invite(
        payload.test_id,
        crate::services::attempt_service::InviteCandidate {
            external_id: candidate.telegram_id.map(|id| id.to_string()),
            name: candidate.name.clone(),
            email: candidate.email.clone(),
            telegram_id: candidate.telegram_id,
            phone: candidate.phone.clone(),
        },
        expires_in_hours,
        Some(json!({ "source": "onef" })),
    ).await?;
    if let Some(telegram_id) = candidate.telegram_id {
        let config = crate::config::get_config();
        let webapp_url = &config.webapp_url;
        let bot_token = &config.telegram_bot_token;

        let message_text = if test.test_type == "presentation" {
            let themes_count = test.presentation_themes
                .as_ref()
                .and_then(|t| t.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!(
                "Вам назначена презентация: {}\n\nКоличество тем: {}\nСрок выполнения: {} часов\n\nНажмите кнопку ниже, чтобы просмотреть задание.",
                test.title, themes_count, expires_in_hours
            )
        } else {
            format!(
                "Вам назначен тест: {}\n\nНажмите кнопку ниже, чтобы начать прохождение теста.",
                test.title
            )
        };

        let reply_markup = json!({
            "inline_keyboard": [[
                {
                    "text": "Профиль",
                    "web_app": { "url": webapp_url }
                }
            ]]
        });

        let telegram_body = json!({
            "chat_id": telegram_id,
            "text": message_text,
            "reply_markup": reply_markup,
        });

        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let client = reqwest::Client::new();
        if let Err(e) = client.post(&url).json(&telegram_body).send().await {
            tracing::warn!("Failed to send Telegram notification for OneF invite: {}", e);
        } else {
            tracing::info!("OneF invite: Telegram notification sent to chat_id: {}", telegram_id);
        }
    }

    let notif = crate::services::notification_service::NotificationService::new(
        state.pool.clone(),
        crate::config::get_config().telegram_bot_webhook_url.clone(),
    );
    let assigned = crate::dto::webhook_dto::TestAssignedWebhook {
        event: "test_assigned".to_string(),
        attempt_id: result.attempt_id,
        candidate: crate::dto::webhook_dto::WebhookCandidate {
            name: candidate.name.clone(),
            telegram_id: candidate.telegram_id,
        },
        test: crate::dto::webhook_dto::WebhookTest {
            title: test.title.clone(),
        },
        access_token: result.access_token.clone(),
        expires_at: result.expires_at,
    };
    let payload_json = serde_json::to_value(&assigned)?;
    let _ = notif.enqueue_webhook("test_assigned", &payload_json).await;

    let config = crate::config::get_config();
    Ok((StatusCode::CREATED, Json(json!({
        "attempt_id": result.attempt_id,
        "access_token": result.access_token,
        "test_url": format!("{}/test/{}", config.webapp_url, result.access_token),
        "expires_at": result.expires_at,
        "status": result.status,
        "candidate_name": candidate.name,
        "test_title": test.title,
    }))))
}
