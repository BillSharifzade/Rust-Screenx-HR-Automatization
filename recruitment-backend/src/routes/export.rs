use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use crate::{AppState, error::Result};

#[derive(Debug, Deserialize)]
pub struct BulkExportRequest {
    pub candidate_ids: Option<Vec<uuid::Uuid>>,
}

/// Export a single candidate as XLSX
pub async fn export_candidate(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

    // Prepare data for export
    let vacancies = state.koinotinav_service.fetch_vacancies().await.unwrap_or_default();
    let mut vacancy_map = HashMap::new();
    for v in vacancies {
        vacancy_map.insert(v.id, v.title);
    }

    let mut history_map = HashMap::new();
    let history = state.candidate_service.get_candidate_history(candidate.id).await?;
    history_map.insert(candidate.id, history);

    let buffer = crate::services::export_service::ExportService::generate_candidates_xlsx(
        &[candidate.clone()],
        &vacancy_map,
        &history_map
    )?;
    let filename = format!("candidate_{}_{}.xlsx",
        candidate.name.replace(' ', "_"),
        chrono::Utc::now().format("%Y%m%d")
    );
    let disposition = format!("attachment; filename=\"{}\"", filename);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        buffer,
    ))
}

/// Export multiple or all candidates as XLSX
pub async fn export_candidates_bulk(
    State(state): State<AppState>,
    Json(payload): Json<BulkExportRequest>,
) -> Result<impl IntoResponse> {
    let candidates = if let Some(ids) = payload.candidate_ids {
        if ids.is_empty() {
            state.candidate_service.list_candidates().await?
        } else {
            let all = state.candidate_service.list_candidates().await?;
            all.into_iter().filter(|c| ids.contains(&c.id)).collect()
        }
    } else {
        state.candidate_service.list_candidates().await?
    };

    // Prepare data for export
    let vacancies = state.koinotinav_service.fetch_vacancies().await.unwrap_or_default();
    let mut vacancy_map = HashMap::new();
    for v in vacancies {
        vacancy_map.insert(v.id, v.title);
    }

    let mut history_map = HashMap::new();
    for c in &candidates {
        if let Ok(h) = state.candidate_service.get_candidate_history(c.id).await {
            history_map.insert(c.id, h);
        }
    }

    let buffer = crate::services::export_service::ExportService::generate_candidates_xlsx(
        &candidates,
        &vacancy_map,
        &history_map
    )?;
    let filename = format!("candidates_export_{}.xlsx",
        chrono::Utc::now().format("%Y%m%d_%H%M")
    );
    let disposition = format!("attachment; filename=\"{}\"", filename);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        buffer,
    ))
}
