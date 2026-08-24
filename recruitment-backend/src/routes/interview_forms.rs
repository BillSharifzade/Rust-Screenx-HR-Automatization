use crate::{
    error::{Error, Result},
    models::interview_form::{FORM_TYPE_INTERVIEW_1, FORM_TYPE_INTERVIEW_2},
    services::interview_form_service::RecognizeRequest,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use uuid::Uuid;

pub async fn recognize(
    State(state): State<AppState>,
    Json(payload): Json<RecognizeRequest>,
) -> Result<impl IntoResponse> {
    let job = state.interview_form_service.create_job(&payload).await?;
    state.interview_form_service.spawn(job.id);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "job_id": job.id,
            "status": job.status,
            "images": payload.urls().len(),
            "callback_status": job.callback_status,
            "poll_url": format!("/api/onef/interview-forms/jobs/{}", job.id),
        })),
    ))
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let result = state.interview_form_service.get_job(job_id).await?;
    Ok(Json(result))
}

pub async fn get_result(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let form = state.interview_form_service.get_recognition(id).await?;
    Ok(Json(form))
}

/// The transcription as a PDF laid out like the paper sheet it came from.
pub async fn get_result_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let form = state.interview_form_service.get_recognition(id).await?;
    let buffer = crate::services::interview_form_pdf::InterviewFormPdf::render(&form)?;

    let stem = form
        .source_url
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem).or(Some(name)))
        .map(|stem| {
            stem.chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| form.id.to_string());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.pdf\"", stem),
            ),
        ],
        buffer,
    ))
}

pub async fn review_queue(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    let candidate_id = match params.get("candidate_id") {
        Some(raw) => Some(
            Uuid::parse_str(raw)
                .map_err(|_| Error::BadRequest(format!("Invalid candidate_id \"{}\"", raw)))?,
        ),
        None => None,
    };

    let limit = params
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50);

    let forms = state
        .interview_form_service
        .list_review_queue(candidate_id, limit)
        .await?;

    Ok(Json(json!({ "count": forms.len(), "forms": forms })))
}

#[derive(Debug, Deserialize)]
pub struct ReviewRequest {
    #[serde(default)]
    pub corrected_fields: Option<JsonValue>,
    #[serde(default)]
    pub reviewed_by: Option<Uuid>,
}

pub async fn submit_review(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReviewRequest>,
) -> Result<impl IntoResponse> {
    if let Some(fields) = payload.corrected_fields.as_ref() {
        if !fields.is_object() {
            return Err(Error::BadRequest(
                "corrected_fields must be a JSON object shaped like `fields`".into(),
            ));
        }
    }

    let form = state
        .interview_form_service
        .apply_review(id, payload.corrected_fields, payload.reviewed_by)
        .await?;

    Ok(Json(form))
}

pub async fn schema() -> impl IntoResponse {
    Json(json!({
        "form_types": {
            FORM_TYPE_INTERVIEW_1: {
                "title": "Форма оценки соискателя в ходе проведения 1-го личного собеседования",
                "parameter_keys": [
                    "vacancy_source",
                    "company_knowledge",
                    "residence_birthplace",
                    "family",
                    "core_values",
                    "goals",
                    "relatives_in_group",
                    "salary_expectation",
                    "self_work_deadline",
                    "planned_start_date",
                    "ready_to_work_abroad"
                ],
                "notes": "Parameter labels are pre-printed, so every row carries a canonical `key`. \
                          Two revisions of this template are in circulation and rows differ between \
                          them, so only the rows printed on the sheet come back."
            },
            FORM_TYPE_INTERVIEW_2: {
                "title": "Форма оценки соискателя в ходе проведения 2-го личного собеседования",
                "parameter_keys": [],
                "notes": "Parameter labels are handwritten and differ per interviewer, so rows come \
                          back with `key: null` and the label transcribed verbatim. Carries the two \
                          decision blocks instead of the strengths/growth grids."
            }
        },
        "critical_fields": crate::models::interview_form::CRITICAL_FIELD_PATHS,
        "thresholds": {
            "field": crate::models::interview_form::FIELD_REVIEW_THRESHOLD,
            "overall": crate::models::interview_form::OVERALL_REVIEW_THRESHOLD
        },
        "job_statuses": ["pending", "processing", "completed", "partial", "failed"],
        "callback_statuses": ["skipped", "pending", "delivered", "failed"]
    }))
}
