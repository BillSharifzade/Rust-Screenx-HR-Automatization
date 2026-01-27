use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use chrono::Utc;
use url::Url;
use serde_json::json;
use validator::Validate;

use crate::dto::public_dto::{
    GetTestByTokenResponse, SaveAnswerRequest, SaveAnswerResponse, StartTestResponse,
    StatusResponse, SubmitTestRequest, SubmitTestResponse,
};
use crate::services::attempt_service::AttemptService;
use crate::services::audit_service::AuditService;
use crate::services::notification_service::NotificationService;
use crate::AppState;

#[axum::debug_handler]
pub async fn get_test_by_token(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> crate::error::Result<Response> {
    let svc = AttemptService::new(state.pool.clone());
    let (attempt, test) = svc.get_attempt_and_test_by_token(&token).await?;
    if attempt.expires_at <= Utc::now() {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "test_expired",
                "message": "This test invitation has expired"
            })),
        )
            .into_response());
    }

    let questions: Vec<crate::models::question::Question> =
        serde_json::from_value(test.questions.clone()).unwrap_or_default();
    let response = GetTestByTokenResponse {
        test: crate::dto::public_dto::PublicTestSummary {
            title: test.title,
            description: test.description,
            instructions: test.instructions,
            duration_minutes: test.duration_minutes,
            total_questions: questions.len(),
            passing_score: test.passing_score.to_string().parse::<f64>().unwrap_or(0.0),
            test_type: Some(test.test_type),
            presentation_themes: test.presentation_themes,
            presentation_extra_info: test.presentation_extra_info,
        },
        attempt: crate::dto::public_dto::PublicAttemptSummary {
            id: attempt.id,
            status: attempt.status,
            expires_at: attempt.expires_at,
            candidate_name: attempt.candidate_name,
            candidate_external_id: attempt.candidate_external_id,
        },
    };
    Ok(Json(response).into_response())
}

#[axum::debug_handler]
pub async fn start_test(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> crate::error::Result<Response> {
    tracing::info!("Starting test for token: {}", token);
    let svc = AttemptService::new(state.pool.clone());
    let (attempt, _test) = svc.get_attempt_and_test_by_token(&token).await?;
    
    tracing::info!("Found attempt: {:?}, expires_at: {:?}, now: {:?}", attempt.id, attempt.expires_at, Utc::now());

    if attempt.expires_at <= Utc::now() {
        tracing::warn!("Test expired for token: {}", token);
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "test_expired",
                "message": "This test invitation has expired"
            })),
        )
            .into_response());
    }
    match svc.start_attempt_by_token(&token).await {
        Ok(updated) => {
             tracing::info!("Test started successfully: {:?}", updated.id);
             let response = StartTestResponse {
                attempt_id: updated.id,
                status: updated.status,
                started_at: updated.started_at.unwrap_or(Utc::now()),
                expires_at: updated.expires_at,
                questions: updated.questions_snapshot,
            };
            Ok(Json(response).into_response())
        },
        Err(e) => {
            tracing::error!("Failed to start test: {:?}", e);
            Err(e)
        }
    }
}

#[axum::debug_handler]
pub async fn save_answer(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<SaveAnswerRequest>,
) -> crate::error::Result<Response> {
    req.validate()?;
    let svc = AttemptService::new(state.pool.clone());
    let (attempt, _test) = svc.get_attempt_and_test_by_token(&token).await?;
    if attempt.expires_at <= Utc::now() {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "test_expired",
                "message": "This test invitation has expired"
            })),
        )
            .into_response());
    }
    let question_id = req.question_id;
    let ts = svc.save_answer_by_token(&token, req).await?;
    Ok(Json(SaveAnswerResponse {
        saved: true,
        question_id,
        timestamp: ts,
    })
    .into_response())
}

#[axum::debug_handler]
pub async fn submit_presentation(
    State(state): State<AppState>,
    Path(token): Path<String>,
    mut multipart: axum::extract::Multipart,
) -> crate::error::Result<Response> {
    let svc = AttemptService::new(state.pool.clone());
    let (attempt_init, _test) = svc.get_attempt_and_test_by_token(&token).await?;

    if attempt_init.status == "completed" {
         return Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "already_completed",
                "message": "Presentation has already been submitted"
            })),
        ).into_response());
    }

    let mut presentation_link: Option<String> = None;
    let mut file_path: Option<String> = None;
    let allowed_extensions = ["pdf", "pptx", "ppt", "key"];

    while let Some(field) = multipart.next_field().await.map_err(crate::error::Error::Multipart)? {
        let name = field.name().unwrap_or("").to_string();
        if name == "presentation_link" {
            let data = field.text().await.map_err(crate::error::Error::Multipart)?;
            let trimmed = data.trim();
            if !trimmed.is_empty() {
                match Url::parse(trimmed) {
                    Ok(url) => {
                        if url.scheme() != "http" && url.scheme() != "https" {
                            return Ok((
                                StatusCode::BAD_REQUEST,
                                Json(json!({
                                    "error": "invalid_url_scheme",
                                    "message": "Only HTTP and HTTPS links are allowed"
                                })),
                            ).into_response());
                        }
                        presentation_link = Some(trimmed.to_string());
                    },
                    Err(_) => {
                        return Ok((
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "error": "invalid_url",
                                "message": "The provided link is not a valid URL"
                            })),
                        ).into_response());
                    }
                }
            }
        } else if name == "file" {
            let filename = field.file_name().unwrap_or("presentation").to_string();
            let data = field.bytes().await.map_err(crate::error::Error::Multipart)?;
            
            if !data.is_empty() {
                let extension = std::path::Path::new(&filename)
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();

                if !allowed_extensions.contains(&extension.as_str()) {
                    return Ok((
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "invalid_file_type",
                            "message": format!("File type not allowed. Allowed: {}", allowed_extensions.join(", "))
                        })),
                    ).into_response());
                }

                let upload_dir = "uploads/presentations";
                tokio::fs::create_dir_all(upload_dir).await.map_err(crate::error::Error::Io)?;
                let file_id = uuid::Uuid::new_v4();
                let saved_filename = format!("{}.{}", file_id, extension);
                let path = format!("{}/{}", upload_dir, saved_filename);
                tokio::fs::write(&path, data).await.map_err(crate::error::Error::Io)?;
                file_path = Some(path);
            }
        }
    }

    if presentation_link.is_none() && file_path.is_none() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "empty_submission",
                "message": "Please provide either a link or a file for your presentation"
            })),
        ).into_response());
    }

    let attempt = svc.submit_presentation_by_token(&token, presentation_link, file_path).await?;
    
    let notif = NotificationService::new(
        state.pool.clone(),
        crate::config::get_config().telegram_bot_webhook_url.clone(),
    );
    
    if let Ok(test) = state.test_service.get_test_by_id(attempt.test_id).await {
        let completed = json!({
            "event": "presentation_submitted",
            "attempt_id": attempt.id,
            "candidate": {
                "name": attempt.candidate_name.clone(),
                "telegram_id": attempt.candidate_telegram_id,
            },
            "test": {
                "title": test.title.clone(),
            },
            "submission_link": attempt.presentation_submission_link,
            "has_file": attempt.presentation_submission_file_path.is_some(),
        });
        let _ = notif.enqueue_webhook("presentation_submitted", &completed).await;
    }

    Ok(Json(json!({ 
        "status": "completed",
        "message": "Presentation submitted successfully" 
    })).into_response())
}

#[axum::debug_handler]
pub async fn submit_test(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(req): Json<SubmitTestRequest>,
) -> crate::error::Result<Response> {
    tracing::info!("Submitting test for token: {}, answers count: {}", token, req.answers.len());
    let svc = AttemptService::new(state.pool.clone());
    let (attempt0, _test) = svc.get_attempt_and_test_by_token(&token).await?;

    if attempt0.expires_at <= Utc::now() {
        tracing::warn!("Submission failed: Test expired for token: {}", token);
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": "test_expired",
                "message": "This test invitation has expired"
            })),
        )
            .into_response());
    }

    if attempt0.status == "completed" {
         tracing::warn!("Submission failed: Test already completed for token: {}", token);
         return Ok((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "already_completed",
                "message": "Test has already been submitted"
            })),
        )
            .into_response());
    }

    let (attempt, score, max_score, percentage, passed) =
        svc.submit_attempt_by_token(&token, req).await?;

    tracing::info!("Test graded: id={}, score={}, percentage={}, passed={}", attempt.id, score, percentage, passed);

    let notif = NotificationService::new(
        state.pool.clone(),
        crate::config::get_config().telegram_bot_webhook_url.clone(),
    );
    
    match state.test_service.get_test_by_id(attempt.test_id).await {
        Ok(test) => {
            let completed = crate::dto::webhook_dto::TestCompletedWebhook {
                event: "test_completed".to_string(),
                attempt_id: attempt.id,
                candidate: crate::dto::webhook_dto::WebhookCandidate {
                    name: attempt.candidate_name.clone(),
                    telegram_id: attempt.candidate_telegram_id,
                },
                test: crate::dto::webhook_dto::WebhookTest {
                    title: test.title.clone(),
                },
                score,
                percentage,
                passed,
            };
            let payload_json = serde_json::to_value(&completed)?;
            if let Err(e) = notif.enqueue_webhook("test_completed", &payload_json).await {
                tracing::error!("Failed to enqueue webhook: {:?}", e);
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch test for notification: {:?}", e);
        }
    }

    let audit = AuditService::new(state.pool.clone());
    let _ = audit
        .log(
            None,
            "submit_attempt",
            "test_attempt",
            attempt.id,
            Some(serde_json::json!({"score": score, "percentage": percentage, "passed": passed})),
            None,
            None,
        )
        .await?;

    let resp = SubmitTestResponse {
        attempt_id: attempt.id,
        status: attempt.status,
        score,
        max_score,
        percentage,
        passed,
        show_results: false,
        message: "Test submitted successfully. Results have been sent to HR.".to_string(),
    };
    tracing::info!("Test submission successful for token: {}", token);
    Ok(Json(resp).into_response())
}


#[axum::debug_handler]
pub async fn get_status(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> crate::error::Result<Response> {
    let svc = AttemptService::new(state.pool.clone());
    let (attempt, test) = svc.get_attempt_and_test_by_token(&token).await?;
    let total_questions: i32 = match serde_json::from_value::<Vec<serde_json::Value>>(
        attempt.questions_snapshot.clone(),
    ) {
        Ok(v) => v.len() as i32,
        Err(_) => 0,
    };
    let answered: i32 = match attempt.answers.clone() {
        Some(v) => serde_json::from_value::<Vec<serde_json::Value>>(v)
            .map(|a| a.len() as i32)
            .unwrap_or(0),
        None => 0,
    };
    let time_remaining = attempt.started_at.map(|started| {
        let end = started + chrono::Duration::minutes(test.duration_minutes as i64);
        let now = Utc::now();
        (end - now).num_seconds().max(0) as i32
    });
    let resp = StatusResponse {
        status: attempt.status,
        started_at: attempt.started_at,
        time_remaining_seconds: time_remaining,
        questions_answered: Some(answered),
        total_questions: Some(total_questions),
    };
    Ok(Json(resp).into_response())
}

#[axum::debug_handler]
pub async fn heartbeat(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> crate::error::Result<Response> {
    let svc = AttemptService::new(state.pool.clone());
    svc.heartbeat(&token).await?;
    Ok(StatusCode::OK.into_response())
}