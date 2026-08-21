use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{error::Result, AppState};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub include_inactive: bool,
}

pub async fn list_onef_vacancies(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse> {
    let items = state
        .onef_vacancy_service
        .list(query.include_inactive)
        .await?;
    let (active, total, last_synced_at) = state.onef_vacancy_service.stats().await?;

    Ok(Json(serde_json::json!({
        "active_count": active,
        "total_count": total,
        "last_synced_at": last_synced_at,
        "count": items.len(),
        "items": items,
    })))
}

pub async fn get_onef_vacancy(
    State(state): State<AppState>,
    Path(vacancy_id_1f): Path<i64>,
) -> Result<impl IntoResponse> {
    let vacancy = state
        .onef_vacancy_service
        .get(vacancy_id_1f)
        .await?
        .ok_or_else(|| {
            crate::error::Error::NotFound(format!(
                "Vacancy {} is not in the 1F catalogue",
                vacancy_id_1f
            ))
        })?;
    Ok(Json(vacancy))
}

pub async fn sync_onef_vacancies(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let summary = state.onef_vacancy_service.sync().await?;
    Ok(Json(summary))
}

#[derive(Debug, Deserialize)]
pub struct MatchRequest {
    #[serde(alias = "candidateId", alias = "CandidateId", alias = "id")]
    pub candidate_id: Uuid,
    #[serde(default = "default_top_n", alias = "TopN", alias = "topN")]
    pub top_n: usize,
    #[serde(default = "default_true")]
    pub include_low_quality: bool,
    #[serde(default)]
    pub force_refresh: bool,
}

fn default_top_n() -> usize {
    5
}

fn default_true() -> bool {
    true
}

pub async fn match_candidate(
    State(state): State<AppState>,
    Json(body): Json<JsonValue>,
) -> Result<impl IntoResponse> {
    let payload = match body {
        JsonValue::Object(mut map) => map.remove("requestBody").unwrap_or(JsonValue::Object(map)),
        other => other,
    };

    let request: MatchRequest = serde_json::from_value(payload).map_err(|e| {
        crate::error::Error::BadRequest(format!("Invalid request payload: {}", e))
    })?;

    let result = state
        .onef_match_service
        .rank_for_candidate(
            request.candidate_id,
            request.top_n.clamp(1, 50),
            request.include_low_quality,
            request.force_refresh,
        )
        .await?;

    Ok(Json(result))
}
