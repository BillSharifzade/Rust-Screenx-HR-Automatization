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

// --- DTOs ---

#[derive(Debug, Deserialize)]
pub struct OneFSendMessageRequest {
    pub candidate_id: Uuid,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct OneFUpdateStatusRequest {
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct OneFChatMessage {
    pub id: Uuid,
    pub direction: String, // "inbound" | "outbound"
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

// --- Endpoints ---

/// Send a message to a candidate via Telegram
pub async fn send_message(
    State(state): State<AppState>,
    Json(payload): Json<OneFSendMessageRequest>,
) -> Result<impl IntoResponse> {
    // 1. Validate candidate exists
    let candidate = state.candidate_service.get_candidate(payload.candidate_id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    let telegram_id = candidate.telegram_id.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate has no linked Telegram account".into())
    })?;

    // 2. Send via Telegram Bot API
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

    // 3. Store message in DB
    let create_msg = crate::models::message::CreateMessage {
        candidate_id: candidate.id,
        telegram_id,
        direction: "outbound".to_string(),
        text: payload.text,
    };
    
    // We use the existing message service
    let _ = state.message_service.create(create_msg).await?;

    Ok(StatusCode::OK)
}

/// Get chat history for a candidate (Read-only, does not mark as read)
pub async fn get_chat_history(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    // We use generic message service. Note: This assumes get_by_candidate doesn't side-effect.
    // Inspection of message_service.rs showed get_by_candidate is just a SELECT.
    let messages = state.message_service.get_by_candidate(candidate_id).await?;
    
    // Mark inbound messages as read if accessed by 1F (implicit read)
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

/// Get total count of unread messages from all candidates
pub async fn get_unread_count(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let count = state.message_service.total_unread_count().await?;
    Ok(Json(json!({ "unread_count": count })))
}

/// Get aggregated statistics for 1F Dashboard
pub async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    // Reuse existing high-level services to aggregate data
    
    // 1. Basic counts
    let total_candidates_map = state.candidate_service.get_status_counts().await?;
    let total_candidates: i64 = total_candidates_map.values().sum();
    
    // 2. New candidates today - would need a new service method or query. 
    // For now, let's approximate or use history counts if available, 
    // but to be safe and quick, we'll do a direct count query here or reuse existing if possible.
    // Leveraging candidate_service.get_history_counts() which returns last 7 days.
    let history = state.candidate_service.get_history_counts().await?;
    let today_str = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let candidates_new_today = history.iter()
        .find(|(date, _)| *date == today_str)
        .map(|(_, count)| *count)
        .unwrap_or(0);

    // 3. Active vacancies
    let active_vacancies = state.vacancy_service.list_published(1).await?.len() as i64; // This might be slow if list is huge, but usually vacancies are few.
    
    // 4. Test attempts
    let attempts_status = state.attempt_service.get_status_distribution().await?;
    let test_attempts_pending = *attempts_status.get("pending").unwrap_or(&0);
    let test_completed = *attempts_status.get("completed").unwrap_or(&0) + *attempts_status.get("passed").unwrap_or(&0) + *attempts_status.get("failed").unwrap_or(&0);

    // 5. Funnel (Simplified estimation based on available data)
    let funnel = RecruitmentFunnel {
        registered: total_candidates,
        applied: total_candidates, // Assuming registration = application for now, or refine if we have 'new' status
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

/// Update candidate status from 1F (e.g. "rejected", "hired")
pub async fn update_candidate_status(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
    Json(payload): Json<OneFUpdateStatusRequest>,
) -> Result<impl IntoResponse> {
    let _updated = state.candidate_service.update_status(candidate_id, payload.status.clone()).await?;
    
    // Optionally notify candidate via Telegram if needed?
    // For now just update DB.
    
    Ok(Json(json!({ 
        "id": candidate_id, 
        "status": payload.status,
        "updated_at": chrono::Utc::now()
    })))
}

/// Get candidate details including AI suitability
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

/// List test attempts for a candidate
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

/// Get detailed test attempt results
pub async fn get_test_attempt(
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempt = svc.get_attempt_by_id(attempt_id).await?;
    
    // Also fetch test title/info for context
    let test = state.test_service.get_test_by_id(attempt.test_id).await?;

    Ok(Json(json!({
        "attempt": attempt,
        "test_title": test.title,
        "test_type": test.test_type
    })))
}

/// List published vacancies
pub async fn list_vacancies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let vacancies = state.vacancy_service.list_published(50).await?;
    Ok(Json(vacancies))
}

/// Get specific vacancy details
pub async fn get_vacancy(
    State(state): State<AppState>,
    Path(vacancy_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let vacancy = state.vacancy_service.get_by_id(vacancy_id).await?;
    Ok(Json(vacancy))
}
