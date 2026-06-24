use crate::{error::Result, models::response::is_valid_stage, AppState};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

/// Run the existing AI suitability analysis for one response.
/// Returns (grade 0-100, comment, cleaned vacancy title). Used by the background grader.
pub async fn grade_one(
    state: &AppState,
    candidate_id: Uuid,
    vacancy_id: i64,
) -> Result<(i32, String, Option<String>)> {
    let candidate = state
        .candidate_service
        .get_candidate(candidate_id)
        .await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    let mut cv_text = String::new();
    if let Some(ref path) = candidate.cv_url {
        cv_text = crate::routes::candidate_routes::extract_text_from_file(path).await;
    }
    let is_scanned = !cv_text.is_empty() && cv_text.trim().len() < 100;
    let cv_info = if is_scanned {
        format!("[NOTE: The candidate's CV appears to be a scanned image. Extracted text is very sparse: '{}'. Please evaluate based on this and basic profile info.]", cv_text.trim())
    } else {
        cv_text.clone()
    };

    let vacancy = state
        .koinotinav_service
        .fetch_vacancy(vacancy_id)
        .await?
        .ok_or_else(|| crate::error::Error::NotFound(format!("Vacancy #{} not found", vacancy_id)))?;

    let v_name_clean = vacancy
        .title
        .replace("<h1>", "").replace("</h1>", "")
        .replace("<strong>", "").replace("</strong>", "")
        .replace("<span>", "").replace("</span>", "");
    let v_desc_clean = vacancy
        .content
        .replace("<p>", "\n").replace("</p>", "")
        .replace("<br>", "\n").replace("<li>", "- ").replace("</li>", "");

    if v_desc_clean.trim().is_empty() {
        return Err(crate::error::Error::BadRequest("Vacancy description is empty".into()));
    }

    let suitability = state
        .ai_service
        .analyze_suitability(
            &candidate.name,
            &candidate.email,
            &cv_info,
            candidate.cv_url.as_deref(),
            &v_name_clean,
            &v_desc_clean,
        )
        .await?;

    Ok((suitability.rating, suitability.comment, Some(v_name_clean.trim().to_string())))
}

/// GET /api/integration/responses — kanban feed (all responses + candidate info).
pub async fn list_responses(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let cards = state.response_service.list().await?;
    Ok(Json(serde_json::json!({
        "stages": crate::models::response::RESPONSE_STAGES,
        "items": cards,
    })))
}

pub async fn get_response(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let r = state
        .response_service
        .get(id)
        .await?
        .ok_or_else(|| crate::error::Error::NotFound("Response not found".into()))?;
    Ok(Json(r))
}

#[derive(Debug, Deserialize)]
pub struct UpdateResponsePayload {
    /// Target pipeline stage (one of RESPONSE_STAGES).
    pub status: Option<String>,
    pub hr_comment: Option<String>,
    /// "accepted" | "rejected" — only meaningful at final_decision.
    pub decision: Option<String>,
    pub test_attempt_id: Option<Uuid>,
}

/// PATCH /api/integration/responses/:id — move stage / set HR comment / decision.
pub async fn update_response(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateResponsePayload>,
) -> Result<impl IntoResponse> {
    if let Some(ref s) = payload.status {
        if !is_valid_stage(s) {
            return Err(crate::error::Error::BadRequest(format!(
                "Invalid stage '{}'. Expected one of: {}",
                s,
                crate::models::response::RESPONSE_STAGES.join(", ")
            )));
        }
    }
    if let Some(ref d) = payload.decision {
        if d != "accepted" && d != "rejected" {
            return Err(crate::error::Error::BadRequest(
                "decision must be 'accepted' or 'rejected'".into(),
            ));
        }
    }

    let updated = state
        .response_service
        .update(id, payload.status, payload.hr_comment, payload.decision, payload.test_attempt_id)
        .await?
        .ok_or_else(|| crate::error::Error::NotFound("Response not found".into()))?;
    Ok(Json(updated))
}
