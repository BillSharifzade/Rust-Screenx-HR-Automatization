use axum::{
    extract::{Multipart, State, Path},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::{AppState, error::Result};
use tokio::fs;
use std::path::Path as StdPath;

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterCandidateResponse {
    pub id: uuid::Uuid,
    pub status: String,
}

#[derive(Deserialize)]
pub struct ApplyVacancyRequest {
    pub candidate_id: Option<uuid::Uuid>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub telegram_id: Option<i64>,
    pub profile_data: Option<serde_json::Value>,
    
    pub vacancy_id: i64,
    pub vacancy_name: Option<String>,
}

async fn save_cv_file(filename: &str, data: &bytes::Bytes) -> Result<String> {
    let ext = StdPath::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| "bin".to_string());

    let allowed_exts = ["pdf", "doc", "docx", "txt", "rtf", "jpg", "jpeg", "png", "webp"];
    if !allowed_exts.contains(&ext.as_str()) {
        return Err(crate::error::Error::BadRequest(format!("File type .{} is not allowed", ext)));
    }

    if ext == "pdf" && !data.starts_with(b"%PDF") {
        return Err(crate::error::Error::BadRequest("Invalid PDF file content".into()));
    }
    if (ext == "jpg" || ext == "jpeg") && !data.starts_with(&[0xFF, 0xD8]) {
        return Err(crate::error::Error::BadRequest("Invalid JPEG file content".into()));
    }
    if ext == "png" && !data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Err(crate::error::Error::BadRequest("Invalid PNG file content".into()));
    }

    let upload_root = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "/app/uploads".to_string());
    let cv_dir = format!("{}/cv", upload_root);

    if let Err(e) = fs::create_dir_all(&cv_dir).await {
        tracing::error!("Failed to create upload directory {}: {}", cv_dir, e);
        return Err(crate::error::Error::Internal(format!("Storage error: {}", e)));
    }

    let file_id = uuid::Uuid::new_v4();
    let safe_filename = format!("{}.{}", file_id, ext);
    let absolute_path = format!("{}/{}", cv_dir, safe_filename);

    fs::write(&absolute_path, data).await.map_err(|e| {
        tracing::error!("Failed to write CV file at {}: {}", absolute_path, e);
        crate::error::Error::Internal(format!("Failed to save file: {}", e))
    })?;

    // Return the PUBLIC relative path for the database
    Ok(format!("uploads/cv/{}", safe_filename))
}

async fn extract_text_from_file(file_path: &str) -> String {
    let path = std::path::Path::new(file_path);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext.to_lowercase().as_str() {
        "pdf" => {
            let output = tokio::process::Command::new("pdftotext")
                .arg("-layout")
                .arg(file_path)
                .arg("-")
                .output()
                .await;

            match output {
                Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
                Err(e) => {
                    tracing::error!("Failed to run pdftotext on {}: {}", file_path, e);
                    String::new()
                }
            }
        }
        "txt" => {
            match fs::read_to_string(file_path).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to read txt file {}: {}", file_path, e);
                    String::new()
                }
            }
        }
        _ => String::new(),
    }
}

pub async fn register_candidate(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl axum::response::IntoResponse> {
    tracing::info!("Registering candidate request received");
    let mut name = String::new();
    let mut email = String::new();
    let mut phone = None;
    let mut telegram_id = None;
    let mut profile_data = None;
    let mut cv_url = None;
    let mut dob = None;
    let mut vacancy_id = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::error!("Failed to get next field: {}", e);
        crate::error::Error::BadRequest(e.to_string())
    })? {
        let field_name = field.name().unwrap_or_default().to_string();
        
        match field_name.as_str() {
            "name" => name = field.text().await.unwrap_or_default(),
            "email" => email = field.text().await.unwrap_or_default(),
            "phone" => phone = Some(field.text().await.unwrap_or_default()),
            "telegram_id" => {
                let id_str = field.text().await.unwrap_or_default();
                if let Ok(id) = id_str.parse::<i64>() {
                    telegram_id = Some(id);
                }
            },
            "vacancy_id" => {
                let id_str = field.text().await.unwrap_or_default();
                if let Ok(id) = id_str.parse::<i64>() {
                    vacancy_id = Some(id);
                }
            },
            "profile_data" => {
                let data_str = field.text().await.unwrap_or_default();
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&data_str) {
                    profile_data = Some(data);
                }
            },
            "cv" => {
                let filename = field.file_name().unwrap_or("cv.bin").to_string();
                let data = field.bytes().await.map_err(|e| {
                    tracing::error!("Failed to read CV bytes: {}", e);
                    crate::error::Error::BadRequest("Failed to read file upload".into())
                })?;

                if !data.is_empty() {
                    match save_cv_file(&filename, &data).await {
                        Ok(path) => cv_url = Some(path),
                        Err(e) => {
                             tracing::error!("CV Save Error: {:?}", e);
                             return Err(e);
                        }
                    }
                }
            },
            "dob" => {
                let dob_str = field.text().await.unwrap_or_default();
                if let Ok(d) = chrono::NaiveDate::parse_from_str(&dob_str, "%Y-%m-%d") {
                    dob = Some(d);
                }
            },
            _ => {}
        }
    }

    if name.is_empty() { return Err(crate::error::Error::BadRequest("Name is required".into())); }
    if email.is_empty() { return Err(crate::error::Error::BadRequest("Email is required".into())); }
    if phone.as_ref().map(|s| s.is_empty()).unwrap_or(true) { return Err(crate::error::Error::BadRequest("Phone number is required".into())); }
    if cv_url.is_none() { return Err(crate::error::Error::BadRequest("CV file is required".into())); }
    if dob.is_none() { return Err(crate::error::Error::BadRequest("Date of birth is required".into())); }
    if vacancy_id.is_none() { return Err(crate::error::Error::BadRequest("Vacancy selection is required".into())); }

    let telegram_id = telegram_id.ok_or_else(|| {
        crate::error::Error::BadRequest("telegram_id is required".into())
    })?;

    let candidate = state.candidate_service.create_candidate(
        Some(telegram_id),
        name.clone(),
        email.clone(),
        phone.clone(),
        cv_url.clone(),
        dob,
        vacancy_id,
        profile_data,
    ).await.map_err(|e| {
        tracing::error!("Failed to create candidate DB: {}", e);
        e
    })?;

    if let Some(vid) = vacancy_id {
        let ai_service = state.ai_service.clone();
        let koinoti_service = state.koinotinav_service.clone();
        let candidate_service = state.candidate_service.clone();
        let onef_service = state.onef_service.clone();
        
        let candidate_id = candidate.id;
        let c_name = name;
        let c_email = email;
        let c_phone = phone;
        let c_dob = dob;
        let c_cv = cv_url;
        let c_telegram_id = telegram_id;

        tokio::spawn(async move {
            let mut cv_text = String::new();
            if let Some(ref path) = c_cv {
                cv_text = extract_text_from_file(path).await;
            }
            let mut v_name = format!("Vacancy #{}", vid);
            let mut v_desc = String::new();
            
            if let Ok(Some(v)) = koinoti_service.fetch_vacancy(vid).await {
                v_name = v.title;
                v_desc = v.content;
            }

            let mut ai_rating = None;
            let mut ai_comment = None;

            if !cv_text.is_empty() || !v_desc.is_empty() {
                match ai_service.analyze_suitability(&c_name, &c_email, &cv_text, c_cv.as_deref(), &v_name, &v_desc).await {
                    Ok(suitability) => {
                        let _ = candidate_service.update_ai_suitability(candidate_id, suitability.rating, suitability.comment.clone()).await;
                        ai_rating = Some(suitability.rating);
                        ai_comment = Some(suitability.comment);
                        tracing::info!("AI Suitability Analysis completed for candidate {}", candidate_id);
                    },
                    Err(e) => {
                        tracing::error!("AI Suitability Analysis failed for candidate {}: {}", candidate_id, e);
                    }
                }
            }

            let _ = onef_service.notify_application(
                vid,
                v_name,
                candidate_id,
                c_telegram_id,
                c_name,
                c_email,
                c_phone,
                c_dob,
                c_cv,
                ai_rating,
                ai_comment,
            ).await;
        });
    }

    Ok((StatusCode::CREATED, Json(RegisterCandidateResponse {
        id: candidate.id,
        status: "success".into(),
    })))
}

pub async fn get_candidate(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    let candidate = state.candidate_service.get_candidate(id).await?;
    match candidate {
        Some(c) => Ok(Json(c)),
        None => Err(crate::error::Error::NotFound("Candidate not found".into())),
    }
}

pub async fn update_candidate_cv(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    mut multipart: Multipart,
) -> Result<impl axum::response::IntoResponse> {
    let mut cv_url = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| crate::error::Error::BadRequest(e.to_string()))? {
        if field.name() == Some("cv") {
            let filename = field.file_name().unwrap_or("cv.bin").to_string();
            let data = field.bytes().await.map_err(|e| crate::error::Error::Internal(e.to_string()))?;
            
            if !data.is_empty() {
                let path = save_cv_file(&filename, &data).await?;
                cv_url = Some(path);
                break; 
            }
        }
    }

    if let Some(path) = cv_url {
        let candidate = state.candidate_service.update_cv(id, path).await?;
        Ok(Json(candidate))
    } else {
        Err(crate::error::Error::BadRequest("No valid CV file provided".into()))
    }
}

pub async fn apply_for_vacancy(
    State(state): State<AppState>,
    Json(payload): Json<ApplyVacancyRequest>,
) -> Result<impl axum::response::IntoResponse> {
    let candidate = if let Some(id) = payload.candidate_id {
        state.candidate_service.get_candidate(id).await
            .map_err(|e| crate::error::Error::Internal(e.to_string()))?
            .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?
    } else {
        let name = payload.name.ok_or_else(|| crate::error::Error::BadRequest("name is required for new candidates".into()))?;
        let email = payload.email.ok_or_else(|| crate::error::Error::BadRequest("email is required for new candidates".into()))?;
        
        state.candidate_service.create_candidate(
            payload.telegram_id,
            name,
            email,
            payload.phone,
            None,
            None,
            Some(payload.vacancy_id),
            payload.profile_data,
        ).await?
    };
    
    let telegram_id = candidate.telegram_id.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate must have telegram_id".into())
    })?;

    let application = state.candidate_service.apply_to_vacancy(candidate.id, payload.vacancy_id).await
        .map_err(|e| crate::error::Error::Internal(e.to_string()))?;
    
    let onef_service = state.onef_service.clone();
    let c_id = candidate.id;
    let c_name = candidate.name;
    let c_email = candidate.email;
    let c_phone = candidate.phone;
    let c_dob = candidate.dob;
    let c_cv = candidate.cv_url;
    let v_id = payload.vacancy_id;
    let mut v_name = payload.vacancy_name.clone().unwrap_or_else(|| format!("Vacancy #{}", v_id));
    let koinoti_service = state.koinotinav_service.clone();
    let ai_service = state.ai_service.clone();
    let candidate_service = state.candidate_service.clone();
    
    if payload.vacancy_name.is_none() {
        if let Ok(Some(v)) = koinoti_service.fetch_vacancy(v_id).await {
            v_name = v.title;
        }
    }
    
    tokio::spawn(async move {
        let mut cv_text = String::new();
        if let Some(ref path) = c_cv {
            cv_text = extract_text_from_file(path).await;
        }
        let mut v_desc = String::new();
        if let Ok(Some(v)) = koinoti_service.fetch_vacancy(v_id).await {
            v_desc = v.content;
        }

        let mut ai_rating = None;
        let mut ai_comment = None;

        if !cv_text.is_empty() || !v_desc.is_empty() {
            match ai_service.analyze_suitability(&c_name, &c_email, &cv_text, c_cv.as_deref(), &v_name, &v_desc).await {
                Ok(suitability) => {
                    let _ = candidate_service.update_ai_suitability(c_id, suitability.rating, suitability.comment.clone()).await;
                    ai_rating = Some(suitability.rating);
                    ai_comment = Some(suitability.comment);
                    tracing::info!("AI Suitability Analysis completed for candidate {} (re-application)", c_id);
                },
                Err(e) => {
                    tracing::error!("AI Suitability Analysis failed for candidate {} (re-application): {}", c_id, e);
                }
            }
        }

        let _ = onef_service.notify_application(
            v_id,
            v_name,
            c_id,
            telegram_id,
            c_name,
            c_email,
            c_phone,
            c_dob,
            c_cv,
            ai_rating,
            ai_comment,
        ).await;
    });

    Ok((StatusCode::CREATED, Json(application)))
}

pub async fn get_candidate_applications(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    let applications = state.candidate_service.get_candidate_applications(id).await?;
    Ok(Json(applications))
}

pub async fn get_candidates_for_vacancy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl axum::response::IntoResponse> {
    let candidates = state.candidate_service.get_vacancy_candidates(id).await?;
    Ok(Json(candidates))
}

pub async fn analyze_candidate_suitability(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    tracing::info!("Analyzing suitability for candidate: {}", id);
    let candidate = state.candidate_service.get_candidate(id).await?
        .ok_or_else(|| {
            tracing::error!("Candidate not found: {}", id);
            crate::error::Error::NotFound("Candidate not found".into())
        })?;
    
    let vid = candidate.vacancy_id.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate has no associated vacancy".into())
    })?;

    let mut cv_text = String::new();
    if let Some(ref path) = candidate.cv_url {
        cv_text = extract_text_from_file(path).await;
    }

    let is_scanned = !cv_text.is_empty() && cv_text.trim().len() < 100;
    let cv_info = if is_scanned {
        format!("[NOTE: The candidate's CV appears to be a scanned image. Extracted text is very sparse: '{}'. Please evaluate based on this and basic profile info.]", cv_text.trim())
    } else {
        cv_text.clone()
    };

    tracing::info!("Suitability analysis for {}. CV text len: {}. Scanned suspected: {}", id, cv_text.len(), is_scanned);

    let vacancy = state.koinotinav_service.fetch_vacancy(vid).await?
        .ok_or_else(|| {
            tracing::error!("Could not find details for vacancy ID: {}", vid);
            crate::error::Error::NotFound(format!("Vacancy #{} not found on the job portal", vid))
        })?;

    let v_name = vacancy.title;
    let v_desc = vacancy.content;

    if v_desc.trim().is_empty() {
        return Err(crate::error::Error::BadRequest("Vacancy description is empty, cannot perform analysis".into()));
    }

    let v_name_clean = v_name.replace("<h1>", "").replace("</h1>", "").replace("<strong>", "").replace("</strong>", "").replace("<span>", "").replace("</span>", "");
    let v_desc_clean = v_desc.replace("<p>", "\n").replace("</p>", "").replace("<br>", "\n").replace("<li>", "- ").replace("</li>", "");
    
    tracing::info!("Analyzing suitability for '{}' against vacancy: '{}'", candidate.name, v_name_clean);

    let suitability = state.ai_service.analyze_suitability(
        &candidate.name,
        &candidate.email,
        &cv_info,
        candidate.cv_url.as_deref(),
        &v_name_clean,
        &v_desc_clean
    ).await?;

    let updated = state.candidate_service.update_ai_suitability(id, suitability.rating, suitability.comment).await?;

    Ok(Json(updated))
}

#[derive(Debug, Serialize)]
pub struct HistoryItem {
    pub event_type: String,
    pub title: String,
    pub description: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

pub async fn get_candidate_history(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    let mut history: Vec<HistoryItem> = Vec::new();
    
    let candidate = state.candidate_service.get_candidate(id).await
        .map_err(|e| crate::error::Error::Internal(e.to_string()))?
        .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".to_string()))?;
    
    history.push(HistoryItem {
        event_type: "registration".to_string(),
        title: "candidate_profile.event_registered".to_string(),
        description: Some(candidate.email.clone()),
        timestamp: candidate.created_at.unwrap_or_else(chrono::Utc::now),
        status: Some("candidate_profile.status_completed".to_string()),
        metadata: None,
    });

    if let (Some(created), Some(updated)) = (candidate.created_at, candidate.updated_at) {
        if updated.signed_duration_since(created).num_minutes() > 1 {
            history.push(HistoryItem {
                event_type: "profile_update".to_string(),
                title: "candidate_profile.event_update".to_string(),
                description: None,
                timestamp: updated,
                status: Some("candidate_profile.status_completed".to_string()),
                metadata: None,
            });
        }
    }
    
    let applications = state.candidate_service.get_candidate_applications(id).await
        .map_err(|e| crate::error::Error::Internal(e.to_string()))?;
    for app in applications {
        history.push(HistoryItem {
            event_type: "application".to_string(),
            title: "candidate_profile.event_applied".to_string(),
            description: Some(app.vacancy_id.to_string()),
            timestamp: app.created_at.unwrap_or_else(chrono::Utc::now),
            status: Some("candidate_profile.status_submitted".to_string()),
            metadata: None,
        });
    }
    
    let attempt_svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempts = attempt_svc.list_attempts(
        None, 
        Some(candidate.email.clone()),
        None,
        1,
        100
    ).await?;
    
    for attempt in attempts.0 {
        let status_key = match attempt.status.as_str() {
            "pending" => "dashboard.invites.statuses.pending",
            "in_progress" => "dashboard.invites.statuses.in_progress",
            "completed" => if attempt.passed.unwrap_or(false) { "candidate_profile.status_passed" } else { "candidate_profile.status_failed" },
            "timeout" => "dashboard.invites.statuses.timeout",
            "escaped" => "dashboard.invites.statuses.escaped",
            "needs_review" => "dashboard.invites.statuses.needs_review",
            _ => "dashboard.invites.statuses.pending",
        };
        
        let desc = if let Some(score) = attempt.percentage {
            Some(format!("{:.1}", score))
        } else {
            None
        };
        
        history.push(HistoryItem {
            event_type: "test_attempt".to_string(),
            title: "candidate_profile.event_test".to_string(),
            description: desc,
            timestamp: attempt.created_at.unwrap_or_else(chrono::Utc::now),
            status: Some(status_key.to_string()),
            metadata: Some(serde_json::json!({
                "attempt_id": attempt.id,
                "test_id": attempt.test_id,
                "passed": attempt.passed,
                "score": attempt.score,
                "percentage": attempt.percentage,
                "raw_status": attempt.status,
            })),
        });
    }
    
    history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(Json(history))
}

#[axum::debug_handler]
pub async fn update_candidate_status(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl axum::response::IntoResponse> {
    let status = payload["status"].as_str().ok_or_else(|| {
        crate::error::Error::BadRequest("Status is required".into())
    })?.to_string();

    let updated = state.candidate_service.update_status(id, status.clone()).await?;

    // Push to OneF
    let onef = state.onef_service.clone();
    tokio::spawn(async move {
        let _ = onef.notify_candidate_status(id, status).await;
    });

    Ok(Json(updated))
}

#[axum::debug_handler]
pub async fn share_candidate_grade_to_onef(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    let candidate = state.candidate_service.get_candidate(id).await?
        .ok_or_else(|| {
            crate::error::Error::NotFound("Candidate not found".into())
        })?;
    
    let grade = candidate.ai_rating.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate has no AI grade yet. Please run analyze-suitability first.".into())
    })?;

    state.onef_service.notify_grade(id, grade).await.map_err(|e| {
        crate::error::Error::Internal(format!("Failed to share grade with 1F: {}", e))
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Grade shared with 1F",
        "grade": grade
    })))
}

pub async fn delete_candidate(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<impl axum::response::IntoResponse> {
    state.candidate_service.delete_candidate(id).await.map_err(|e| {
        tracing::error!("Failed to delete candidate {}: {}", id, e);
        crate::error::Error::Internal(e.to_string())
    })?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
