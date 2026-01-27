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

    let upload_dir = "./uploads/cv";
    fs::create_dir_all(upload_dir).await.map_err(|e| crate::error::Error::Internal(e.to_string()))?;

    let file_id = uuid::Uuid::new_v4();
    let safe_filename = format!("{}.{}", file_id, ext);
    let file_path = format!("{}/{}", upload_dir, safe_filename);

    fs::write(&file_path, data).await.map_err(|e| {
        tracing::error!("Failed to write CV file: {}", e);
        crate::error::Error::Internal(format!("Failed to save file: {}", e))
    })?;

    Ok(file_path)
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

            if !cv_text.is_empty() || !v_desc.is_empty() {
                match ai_service.analyze_suitability(&c_name, &c_email, &cv_text, &v_name, &v_desc).await {
                    Ok(suitability) => {
                        let _ = candidate_service.update_ai_suitability(candidate_id, suitability.rating, suitability.comment).await;
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

        if !cv_text.is_empty() && !v_desc.is_empty() {
            match ai_service.analyze_suitability(&c_name, &c_email, &cv_text, &v_name, &v_desc).await {
                Ok(suitability) => {
                    let _ = candidate_service.update_ai_suitability(c_id, suitability.rating, suitability.comment).await;
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

    let mut v_name = format!("Vacancy #{}", vid);
    let mut v_desc = String::new();
    
    if let Ok(Some(v)) = state.koinotinav_service.fetch_vacancy(vid).await {
        v_name = v.title;
        v_desc = v.content;
    }

    let v_name_clean = v_name.replace("<h1>", "").replace("</h1>", "").replace("<strong>", "").replace("</strong>", "").replace("<span>", "").replace("</span>", "");
    let v_desc_clean = v_desc.replace("<p>", "\n").replace("</p>", "").replace("<br>", "\n").replace("<li>", "- ").replace("</li>", "");
    
    tracing::info!("Vacancy: '{}'. Desc len: {}", v_name_clean, v_desc_clean.len());

    let suitability = state.ai_service.analyze_suitability(
        &candidate.name,
        &candidate.email,
        &cv_info,
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
        title: "Registered".to_string(),
        description: Some(format!("Candidate registered with email {}", candidate.email)),
        timestamp: candidate.created_at.unwrap_or_else(chrono::Utc::now),
        status: Some("completed".to_string()),
        metadata: None,
    });

    if let (Some(created), Some(updated)) = (candidate.created_at, candidate.updated_at) {
        if updated.signed_duration_since(created).num_minutes() > 1 {
            history.push(HistoryItem {
                event_type: "profile_update".to_string(),
                title: "Profile Updated".to_string(),
                description: Some("Candidate profile or CV was updated".to_string()),
                timestamp: updated,
                status: Some("completed".to_string()),
                metadata: None,
            });
        }
    }
    
    let applications = state.candidate_service.get_candidate_applications(id).await
        .map_err(|e| crate::error::Error::Internal(e.to_string()))?;
    for app in applications {
        history.push(HistoryItem {
            event_type: "application".to_string(),
            title: "Applied for vacancy".to_string(),
            description: Some(format!("Vacancy ID: {}", app.vacancy_id)),
            timestamp: app.created_at.unwrap_or_else(chrono::Utc::now),
            status: Some("submitted".to_string()),
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
        let status_display = match attempt.status.as_str() {
            "pending" => "Pending",
            "in_progress" => "In Progress",
            "completed" => if attempt.passed.unwrap_or(false) { "Passed" } else { "Failed" },
            "timeout" => "Timed Out",
            "escaped" => "Left Page",
            "needs_review" => "Needs Review",
            _ => &attempt.status,
        };
        
        let desc = if let Some(score) = attempt.percentage {
            Some(format!("Score: {:.1}%", score))
        } else {
            None
        };
        
        history.push(HistoryItem {
            event_type: "test_attempt".to_string(),
            title: format!("Test: {}", attempt.status),
            description: desc,
            timestamp: attempt.created_at.unwrap_or_else(chrono::Utc::now),
            status: Some(status_display.to_string()),
            metadata: Some(serde_json::json!({
                "attempt_id": attempt.id,
                "test_id": attempt.test_id,
                "passed": attempt.passed,
                "score": attempt.score,
                "percentage": attempt.percentage,
            })),
        });
    }
    
    history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    
    Ok(Json(history))
}
