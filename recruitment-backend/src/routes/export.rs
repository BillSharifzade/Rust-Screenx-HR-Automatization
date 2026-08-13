use axum::{
    extract::{Path, Query, State},
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

#[derive(Debug, Deserialize)]
pub struct TestPdfQuery {
    pub lang: Option<String>,
}

pub async fn export_test_pdf(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Query(query): Query<TestPdfQuery>,
) -> Result<impl IntoResponse> {
    let test = state.test_service.get_test_by_id(id).await?;
    let lang = query.lang.as_deref().unwrap_or("ru");
    let buffer = crate::services::pdf_service::PdfService::generate_test_pdf(&test, lang)?;

    let stem = slugify(&test.title);
    let filename = format!("{}_{}.pdf", stem, chrono::Utc::now().format("%Y%m%d"));
    let encoded = percent_encode(&format!("{}.pdf", test.title.trim()));
    let disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        filename, encoded
    );

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        buffer,
    ))
}

pub async fn export_all_tests_pdf(
    State(state): State<AppState>,
    Query(query): Query<TestPdfQuery>,
) -> Result<impl IntoResponse> {
    const CHUNK: i64 = 200;

    let mut tests = Vec::new();
    let mut page = 1;
    loop {
        let result = state.test_service.list_tests(page, CHUNK, None).await?;
        let fetched = result.tests.len() as i64;
        tests.extend(result.tests);
        if fetched < CHUNK || page >= result.total_pages {
            break;
        }
        page += 1;
    }

    let lang = query.lang.as_deref().unwrap_or("ru");
    let buffer = crate::services::pdf_service::PdfService::generate_tests_catalogue_pdf(&tests, lang)?;

    let date = chrono::Utc::now().format("%Y-%m-%d");
    let filename = format!("tests_catalogue_{}.pdf", chrono::Utc::now().format("%Y%m%d"));
    let title = if lang.eq_ignore_ascii_case("en") {
        "Test catalogue"
    } else {
        "Каталог тестов"
    };
    let encoded = percent_encode(&format!("{} {}.pdf", title, date));
    let disposition = format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        filename, encoded
    );

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        buffer,
    ))
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;

    for c in input.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('_');
            last_dash = true;
        }
    }

    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "test".to_string()
    } else {
        trimmed.chars().take(60).collect()
    }
}

fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

pub async fn export_candidate(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse> {
    let candidate = state.candidate_service.get_candidate(id).await?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?;

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
