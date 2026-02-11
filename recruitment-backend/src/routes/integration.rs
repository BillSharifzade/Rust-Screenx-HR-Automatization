use crate::{
    dto::integration_dto::{
        CreateTestPayload, EnqueueAiJobPayload, GenerateAiTestPayload,
        GenerateVacancyDescriptionPayload, UpdateTestPayload, GradePresentationPayload,
        SendMessagePayload, CandidateStatusSync, DashboardStats,
    },
    error::Result,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value as JsonValue};
use std::time::Duration;
use uuid::Uuid;
use validator::Validate;

#[axum::debug_handler]
pub async fn create_test(
    State(state): State<AppState>,
    Json(payload): Json<CreateTestPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;

    let user = sqlx::query!("SELECT id FROM users LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Failed to fetch user: {}", e)))?;

    let created_by = match user {
        Some(u) => u.id,
        None => {
             let new_id = Uuid::new_v4();
             sqlx::query!(
                "INSERT INTO users (id, external_id, name, email, role) VALUES ($1, $2, $3, $4, $5)",
                new_id,
                "system_default",
                "System Admin",
                "admin@example.com",
                "admin"
             )
             .execute(&state.pool)
             .await
             .map_err(|e| crate::error::Error::Internal(format!("Failed to create default user: {}", e)))?;
             new_id
        }
    };

    let test = state.test_service.create_test(payload, created_by).await?;

    let response = json!({
        "id": test.id,
        "external_id": test.external_id,
        "title": test.title,
        "created_at": test.created_at,
    });

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn get_test_by_id(
    State(state): State<AppState>,
    axum::extract::Path(test_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse> {
    let test = state.test_service.get_test_by_id(test_id).await?;
    Ok(Json(test))
}

#[axum::debug_handler]
pub async fn update_test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTestPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    let test = state.test_service.update_test(id, payload).await?;
    let response = json!({
        "status": "success",
        "test": test,
    });
    Ok(Json(response))
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(default)]
pub struct ListTestsQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub is_active: Option<bool>,
    pub search: Option<String>,
}

pub async fn list_tests(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<ListTestsQuery>,
) -> Result<impl IntoResponse> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);

    let filter = crate::services::test_service::TestFilter {
        is_active: query.is_active,
        created_by: None,
        search: query.search,
    };

    let result = state
        .test_service
        .list_tests(page, per_page, Some(filter))
        .await?;
    Ok(Json(result))
}

pub async fn delete_test(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    state.test_service.delete_test(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateInviteRequest {
    pub test_id: Uuid,
    pub candidate: InviteCandidateDto,
    pub expires_in_hours: i64,
    pub send_notification: Option<bool>,
    pub notification_method: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize)]
pub struct InviteCandidateDto {
    pub external_id: Option<String>,
    pub name: String,
    pub email: String,
    pub telegram_id: Option<i64>,
    pub phone: Option<String>,
}

#[axum::debug_handler]
pub async fn create_test_invite(
    State(state): State<AppState>,
    Json(payload): Json<CreateInviteRequest>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let candidate_name = payload.candidate.name.clone();
    let result = svc
        .create_invite(
            payload.test_id,
            crate::services::attempt_service::InviteCandidate {
                external_id: payload.candidate.external_id,
                name: candidate_name.clone(),
                email: payload.candidate.email,
                telegram_id: payload.candidate.telegram_id,
                phone: payload.candidate.phone,
            },
            payload.expires_in_hours,
            payload.metadata,
        )
        .await?;

    let test = state.test_service.get_test_by_id(payload.test_id).await?;

    let notif = crate::services::notification_service::NotificationService::new(
        state.pool.clone(),
        crate::config::get_config().telegram_bot_webhook_url.clone(),
    );
    let assigned = crate::dto::webhook_dto::TestAssignedWebhook {
        event: "test_assigned".to_string(),
        attempt_id: result.attempt_id,
        candidate: crate::dto::webhook_dto::WebhookCandidate {
            name: candidate_name,
            telegram_id: payload.candidate.telegram_id,
        },
        test: crate::dto::webhook_dto::WebhookTest {
            title: test.title.clone(),
        },
        access_token: result.access_token.clone(),
        expires_at: result.expires_at,
    };
    let payload_json = serde_json::to_value(&assigned)?;
    let _ = notif
        .enqueue_webhook("test_assigned", &payload_json)
        .await?;

    if let Some(telegram_id) = payload.candidate.telegram_id {
        let config = crate::config::get_config();
        let webapp_url = &config.webapp_url;
        let bot_token = &config.telegram_bot_token;
        
        let message_text = if test.test_type == "presentation" {
            let themes_count = test.presentation_themes
                .as_ref()
                .and_then(|t| t.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let deadline_hours = test.duration_minutes / 60;
            format!(
                "Вам назначена презентация: {}\n\nКоличество тем: {}\nСрок выполнения: {} часов\n\nНажмите кнопку ниже, чтобы просмотреть задание.",
                test.title,
                themes_count,
                deadline_hours
            )
        } else {
            format!(
                "Вам назначен тест: {}\n\nНажмите кнопку ниже, чтобы начать прохождение теста.",
                test.title
            )
        };
        
        let reply_markup = serde_json::json!({
            "inline_keyboard": [[
                {
                    "text": "Профиль",
                    "web_app": { "url": webapp_url }
                }
            ]]
        });
        
        let telegram_body = serde_json::json!({
            "chat_id": telegram_id,
            "text": message_text,
            "reply_markup": reply_markup,
        });
        
        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let client = reqwest::Client::new();
        if let Err(e) = client.post(&url).json(&telegram_body).send().await {
            tracing::warn!("Failed to send Telegram notification: {}", e);
        } else {
            tracing::info!("Telegram notification sent to chat_id: {}", telegram_id);
        }
    }

    let audit = crate::services::audit_service::AuditService::new(state.pool.clone());
    let _ = audit
        .log(
            None,
            "create_invite",
            "test_attempt",
            result.attempt_id,
            Some(serde_json::json!({"test_id": payload.test_id})),
            None,
            None,
        )
        .await?;

    let config = crate::config::get_config();
    let response = json!({
        "attempt_id": result.attempt_id,
        "access_token": result.access_token,
        "test_url": format!("{}/test/{}", config.webapp_url, result.access_token),
        "expires_at": result.expires_at,
        "status": result.status,
    });
    Ok((StatusCode::CREATED, Json(response)))
}

#[axum::debug_handler]
pub async fn list_test_invites(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, _total) = svc
        .list_attempts(None, None, None, 1, 100)
        .await?;
    
    let invites: Vec<serde_json::Value> = items.iter().map(|a| {
        serde_json::json!({
            "id": a.id,
            "test_id": a.test_id,
            "candidate_email": a.candidate_email,
            "candidate_name": a.candidate_name,
            "status": a.status,
            "created_at": a.created_at,
            "expires_at": a.expires_at,
        })
    }).collect();
    
    Ok(Json(serde_json::json!({ "items": invites })))
}

pub async fn get_test_attempt_by_id(
    State(state): State<AppState>,
    axum::extract::Path(attempt_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempt = svc.get_attempt_by_id(attempt_id).await?;
    let test = state.test_service.get_test_by_id(attempt.test_id).await?;
    let resp = serde_json::json!({
        "id": attempt.id,
        "test": {
            "id": test.id,
            "title": test.title,
            "test_type": test.test_type,
        },
        "candidate": {
            "external_id": attempt.candidate_external_id,
            "name": attempt.candidate_name,
            "email": attempt.candidate_email,
            "telegram_id": attempt.candidate_telegram_id,
            "phone": attempt.candidate_phone,
        },
        "status": attempt.status,
        "score": attempt.score,
        "max_score": attempt.max_score,
        "percentage": attempt.percentage,
        "passed": attempt.passed,
        "started_at": attempt.started_at,
        "completed_at": attempt.completed_at,
        "time_spent_seconds": attempt.time_spent_seconds,
        "graded_answers": attempt.graded_answers,
        "presentation_submission_link": attempt.presentation_submission_link,
        "presentation_submission_file_path": attempt.presentation_submission_file_path,
        "presentation_grade": attempt.presentation_grade,
        "presentation_grade_comment": attempt.presentation_grade_comment,
        "metadata": attempt.metadata,
    });
    Ok(Json(resp))
}

#[derive(Debug, serde::Deserialize, Default)]
#[serde(default)]
pub struct ListAttemptsQuery {
    pub test_id: Option<Uuid>,
    pub candidate_email: Option<String>,
    pub status: Option<String>,
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn list_test_attempts(
    State(state): State<AppState>,
    Query(q): Query<ListAttemptsQuery>,
) -> Result<impl IntoResponse> {
    let page = q.page.unwrap_or(1);
    let limit = q.limit.unwrap_or(20).clamp(1, 100);
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, total) = svc
        .list_attempts(q.test_id, q.candidate_email, q.status, page, limit)
        .await?;
    let total_pages = ((total as f64) / (limit as f64)).ceil() as i64;
    let resp = serde_json::json!({
        "items": items,
        "total": total,
        "page": page,
        "limit": limit,
        "total_pages": total_pages,
    });
    Ok(Json(resp))
}

#[axum::debug_handler]
pub async fn generate_ai_test(
    State(state): State<AppState>,
    Json(payload): Json<GenerateAiTestPayload>,
) -> Result<impl IntoResponse> {
    let cfg = crate::config::get_config();
    let num_q = payload.num_questions.unwrap_or(6).min(cfg.max_ai_questions);
    let skills: Vec<String> = payload.skills.clone().unwrap_or_default();

    let ai_future = state.ai_service.generate_test(
        &payload.profession,
        &skills,
        num_q,
    );

    let gen_output = match tokio::time::timeout(Duration::from_secs(300), ai_future).await {
        Ok(Ok(val)) => val,
        _ => {
            tracing::warn!("AI generation failed or timed out");
            crate::services::ai_service::GenerationOutput {
                questions: vec![],
                logs: vec!["Timeout or fatal error in generate_test".to_string()],
            }
        }
    };
    let questions_val = serde_json::to_value(&gen_output.questions)?;

    if payload.persist.unwrap_or(false) {
        let create_questions = state.ai_service.to_create_questions(&gen_output.questions);
        let user = sqlx::query!("SELECT id FROM users LIMIT 1")
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to fetch user: {}", e)))?;

        let created_by = match user {
            Some(u) => u.id,
            None => {
                 let new_id = Uuid::new_v4();
                 sqlx::query!(
                    "INSERT INTO users (id, external_id, name, email, role) VALUES ($1, $2, $3, $4, $5)",
                    new_id,
                    "system_default",
                    "System Admin",
                    "admin@example.com",
                    "admin"
                 )
                 .execute(&state.pool)
                 .await
                 .map_err(|e| crate::error::Error::Internal(format!("Failed to create default user: {}", e)))?;
                 new_id
            }
        };

        let test_payload = CreateTestPayload {
            title: payload
                .title
                .unwrap_or_else(|| format!("AI {} Test", payload.profession)),
            external_id: None,
            description: payload.description,
            instructions: None,
            questions: Some(create_questions),
            duration_minutes: payload.duration_minutes.unwrap_or(45),
            passing_score: payload.passing_score.unwrap_or(70.0),
            shuffle_questions: Some(false),
            shuffle_options: Some(false),
            show_results_immediately: Some(false),
            test_type: Some("question_based".to_string()),
            presentation_themes: None,
            presentation_extra_info: None,
        };

        let test = state
            .test_service
            .create_test(test_payload, created_by)
            .await?;
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "questions": questions_val, "test_id": test.id })),
        )
            .into_response())
    } else {
        Ok((
            StatusCode::OK,
            Json(serde_json::json!({ "questions": questions_val })),
        )
            .into_response())
    }
}

#[axum::debug_handler]
pub async fn generate_vacancy_description(
    State(state): State<AppState>,
    Json(payload): Json<GenerateVacancyDescriptionPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    let description = state
        .ai_service
        .generate_vacancy_description(&payload)
        .await?;
    Ok(Json(serde_json::json!({ "description": description })))
}

#[utoipa::path(
    post,
    path = "/api/integration/ai-jobs",
    request_body = EnqueueAiJobPayload,
    responses(
        (status = 202, description = "AI job enqueued successfully", body = Json<serde_json::Value>),
        (status = 400, description = "Invalid request payload"),
        (status = 500, description = "Internal server error"),
    ),
)]
pub async fn enqueue_ai_job(
    State(state): State<AppState>,
    Json(payload): Json<EnqueueAiJobPayload>,
) -> Result<impl IntoResponse> {
    let cfg = crate::config::get_config();
    let num_q = payload.num_questions.unwrap_or(6).min(cfg.max_ai_questions);
    let queue = crate::services::queue_service::AiQueueService::new(state.pool.clone());
    let job_payload: JsonValue = serde_json::json!({
        "profession": payload.profession,
        "cv_summary": payload.cv_summary.unwrap_or_default(),
        "skills": payload.skills.unwrap_or_default(),
        "num_questions": num_q,
        "created_by_sub": "local_dev_user",
        "created_by_role": "admin",
    });
    let id = queue
        .enqueue(
            job_payload,
            payload.persist.unwrap_or(false),
            payload.title,
            payload.description,
            payload.duration_minutes,
            payload.passing_score,
        )
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"job_id": id})),
    ))
}

#[utoipa::path(
    get,
    path = "/api/integration/ai-jobs/{id}",
    params(
        ("id" = Uuid, Path, description = "AI Job ID")
    ),
    responses(
        (status = 200, description = "AI job status retrieved successfully", body = Json<serde_json::Value>),
        (status = 404, description = "Job not found"),
    ),
)]
pub async fn get_ai_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let queue = crate::services::queue_service::AiQueueService::new(state.pool.clone());
    let job = queue.get(id).await?;
    Ok(Json(job))
}

#[axum::debug_handler]
pub async fn generate_test_spec(
    State(state): State<AppState>,
    Json(payload): Json<crate::dto::integration_dto::SpecGenerateTestPayload>,
) -> Result<impl IntoResponse> {
    let cfg = crate::config::get_config();
    let num_q = payload.question_count.min(cfg.max_ai_questions);
    let skills = payload.topics.clone();
    let title = format!("{} Assessment", payload.position);

    let ai_future = state.ai_service.generate_test(
        &payload.position,
        &skills,
        num_q,
    );
    let gen_output = match tokio::time::timeout(std::time::Duration::from_secs(300), ai_future).await
    {
        Ok(Ok(v)) => v,
        _ => {
            tracing::warn!("AI generation failed or timed out for spec route");
            crate::services::ai_service::GenerationOutput {
                questions: vec![],
                logs: vec!["Timeout or fatal error".to_string()],
            }
        }
    };

    let created_by = Uuid::parse_str("2cd84131-6e83-4c98-91ba-f9b9a5f0a06c").unwrap();

    let create_payload = crate::dto::integration_dto::CreateTestPayload {
        title: title.clone(),
        external_id: None,
        description: Some(format!("Generated for position: {}", payload.position)),
        instructions: None,
        questions: Some(state.ai_service.to_create_questions(&gen_output.questions)),
        duration_minutes: payload.duration_minutes.unwrap_or(90),
        passing_score: 70.0,
        shuffle_questions: Some(false),
        shuffle_options: Some(false),
        show_results_immediately: Some(false),
        test_type: Some("question_based".to_string()),
        presentation_themes: None,
        presentation_extra_info: None,
    };
    let test = state
        .test_service
        .create_test(create_payload, created_by)
        .await?;

    let resp = json!({
        "id": test.id,
        "title": test.title,
        "questions": test.questions,
        "created_at": test.created_at,
    });
    Ok((StatusCode::CREATED, Json(resp)))
}

#[axum::debug_handler]
pub async fn send_message(
    State(state): State<AppState>,
    Json(payload): Json<SendMessagePayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    
    let candidate = if let Some(cid) = payload.candidate_id {
        state.candidate_service.get_candidate(cid).await?
            .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?
    } else if let Some(tid) = payload.telegram_id {
        state.candidate_service.get_by_telegram_id(tid).await?
            .ok_or_else(|| crate::error::Error::NotFound("Candidate not found".into()))?
    } else {
        return Err(crate::error::Error::BadRequest("Either candidate_id or telegram_id must be provided".into()));
    };

    let telegram_id = candidate.telegram_id.ok_or_else(|| {
        crate::error::Error::BadRequest("Candidate has no associated Telegram ID".into())
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
        .map_err(|e| crate::error::Error::Internal(format!("Failed to send to Telegram: {}", e)))?;

    if !resp.status().is_success() {
        let err_text = resp.text().await.unwrap_or_default();
        return Err(crate::error::Error::Internal(format!("Telegram API error: {}", err_text)));
    }

    let create_msg = crate::models::message::CreateMessage {
        candidate_id: candidate.id,
        telegram_id,
        direction: "outbound".to_string(),
        text: payload.text.clone(),
    };
    let _ = state.message_service.create(create_msg).await;

    Ok(Json(json!({ "status": "sent" })))
}

#[axum::debug_handler]
pub async fn get_chat_messages(
    State(state): State<AppState>,
    Path(candidate_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let messages = state.message_service.get_by_candidate(candidate_id).await?;
    let _ = state.message_service.mark_as_read(candidate_id).await;
    Ok(Json(messages))
}

#[axum::debug_handler]
pub async fn get_unread_count(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let count = state.message_service.total_unread_count().await?;
    Ok(Json(json!({ "unread_count": count })))
}

pub async fn sync_candidate_statuses(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let statuses = sqlx::query_as!(
        CandidateStatusSync,
        r#"
        SELECT DISTINCT ON (c.id)
            c.id,
            c.telegram_id::text as external_id,
            c.name as "name!",
            c.email as "email!",
            COALESCE(ta.status, 'pending') as "status!",
            COALESCE(ta.updated_at, c.updated_at, NOW()) as "last_updated!"
        FROM candidates c
        LEFT JOIN test_attempts ta ON c.email = ta.candidate_email
        ORDER BY c.id, ta.updated_at DESC NULLS LAST
        "#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(statuses))
}

pub async fn get_dashboard_stats(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let candidates_status = state.candidate_service.get_status_counts().await?;
    let total_candidates: i64 = candidates_status.values().sum();
    let unread_messages = state.message_service.total_unread_count().await?;
    let tests_list = state.test_service.list_tests(
        1, 
        1, 
        Some(crate::services::test_service::TestFilter {
            is_active: Some(true),
            created_by: None,
            search: None,
        })
    ).await?;
    let active_tests = tests_list.total;
    let internal_active_vacancies = match state.vacancy_service.list_published(1000).await {
        Ok(v) => v.len() as i64,
        Err(e) => {
            tracing::error!("Failed to fetch vacancies for dashboard: {:?}", e);
            0
        }
    };

    let external_vacancies = match state.koinotinav_service.fetch_vacancies().await {
        Ok(v) => v.len() as i64,
        Err(e) => {
            tracing::error!("Failed to fetch external vacancies for dashboard: {:?}", e);
            0
        }
    };

    let total_active_vacancies = internal_active_vacancies + external_vacancies;

    let candidates_history = state.candidate_service.get_history_counts().await?;
    let attempts_status = state.attempt_service.get_status_distribution().await?;

    let stats = DashboardStats {
        total_candidates,
        unread_messages,
        active_tests,
        active_vacancies: total_active_vacancies,
        candidates_by_status: candidates_status,
        candidates_history,
        attempts_status,
    };

    Ok(Json(stats))
}

pub async fn delete_test_invite(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    svc.delete_attempt(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_candidates(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let candidates = state.candidate_service.list_candidates().await?;
    Ok(Json(candidates))
}

#[axum::debug_handler]
pub async fn grade_presentation(
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<GradePresentationPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;

    let user = sqlx::query!("SELECT id FROM users LIMIT 1")
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Failed to fetch user: {}", e)))?;
        
    let graded_by = match user {
        Some(u) => u.id,
        None => Uuid::nil(),
    };

    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempt = svc
        .grade_presentation(attempt_id, payload.grade, payload.comment.clone(), graded_by)
        .await?;

    if let Some(telegram_id) = attempt.candidate_telegram_id {
        let test = state.test_service.get_test_by_id(attempt.test_id).await?;
        let config = crate::config::get_config();
        
        let message_text = format!(
            "Ваша презентация по тесту \"{}\" проверена!\n\nОценка: {}/100\nКомментарий: {}\n\nВы можете посмотреть подробности и оценку в профиле, нажав кнопку 'История активности'.\n\nОкончательное решение мы вам объявим немного позже.",
            test.title,
            payload.grade,
            payload.comment.unwrap_or_else(|| "Без комментария".to_string())
        );

        let reply_markup = serde_json::json!({
            "inline_keyboard": [[
                {
                    "text": "Профиль",
                    "web_app": { "url": config.webapp_url }
                }
            ]]
        });

        let telegram_body = serde_json::json!({
            "chat_id": telegram_id,
            "text": message_text,
            "reply_markup": reply_markup,
        });

        let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_bot_token);
        let client = reqwest::Client::new();
        tokio::spawn(async move {
            if let Err(e) = client.post(&url).json(&telegram_body).send().await {
                tracing::warn!("Failed to send grading notification: {}", e);
            }
        });
    }

    Ok(Json(attempt))
}

#[axum::debug_handler]
pub async fn grade_test_answer(
    State(state): State<AppState>,
    Path(attempt_id): Path<Uuid>,
    Json(payload): Json<crate::dto::integration_dto::GradeAnswerPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let attempt = svc.grade_answer(attempt_id, payload.question_id, payload.is_correct).await?;

    if attempt.status == "completed" {
         if let Some(telegram_id) = attempt.candidate_telegram_id {
            let state_clone = state.clone();
            let attempt_clone = attempt.clone();
            tokio::spawn(async move {
                let test = state_clone.test_service.get_test_by_id(attempt_clone.test_id).await;
                 if let Ok(test) = test {
                    let config = crate::config::get_config();
                    let message_text = format!(
                        "Ваш тест \"{}\" проверен!\n\nРезультат: {}%\n\nВы можете посмотреть подробности и оценку в профиле, нажав кнопку 'История активности'.\n\nОкончательное решение мы вам объявим немного позже.",
                        test.title,
                        attempt_clone.percentage.unwrap_or_default()
                    );

                    let reply_markup = serde_json::json!({
                        "inline_keyboard": [[
                            {
                                "text": "Профиль",
                                "web_app": { "url": config.webapp_url }
                            }
                        ]]
                    });

                    let telegram_body = serde_json::json!({
                        "chat_id": telegram_id,
                        "text": message_text,
                        "reply_markup": reply_markup,
                    });

                    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_bot_token);
                    let client = reqwest::Client::new();
                     if let Err(e) = client.post(&url).json(&telegram_body).send().await {
                         tracing::warn!("Failed to send grading notification: {}", e);
                     }
                 }
            });
        }
    }

    Ok(Json(attempt))
}

#[derive(serde::Deserialize)]
pub struct PollQuery {
    #[serde(default = "default_since")]
    since: chrono::DateTime<chrono::Utc>,
}

fn default_since() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() - chrono::Duration::minutes(5)
}

#[axum::debug_handler]
pub async fn poll_notifications(
    State(state): State<AppState>,
    Query(query): Query<PollQuery>,
) -> Result<impl IntoResponse> {
    let new_candidates = sqlx::query!(
        "SELECT id, name, created_at FROM candidates WHERE created_at > $1 ORDER BY created_at DESC",
        query.since
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Failed to fetch candidates: {}", e)))?;

    let total_new_candidates = sqlx::query!(
        "SELECT COUNT(*) as count FROM candidates WHERE status = 'new'"
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Failed to count new candidates: {}", e)))?
    .count.unwrap_or(0);

    let total_needs_review_attempts = sqlx::query!(
        "SELECT COUNT(*) as count FROM test_attempts WHERE status = 'needs_review'"
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Failed to count review attempts: {}", e)))?
    .count.unwrap_or(0);

    let updated_attempts = sqlx::query!(
        r#"
        SELECT id, candidate_name, status, started_at, completed_at, test_id 
        FROM test_attempts 
        WHERE (started_at > $1) OR (completed_at > $1) OR (status = 'needs_review' AND updated_at > $1)
        ORDER BY updated_at DESC
        "#,
        query.since
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| crate::error::Error::Internal(format!("Failed to fetch attempts: {}", e)))?;

    Ok(Json(serde_json::json!({
        "candidates": new_candidates.iter().map(|c| serde_json::json!({
            "type": "candidate",
            "id": c.id,
            "name": c.name,
            "created_at": c.created_at
        })).collect::<Vec<_>>(),
        "attempts": updated_attempts.iter().map(|a| serde_json::json!({
            "type": "attempt",
            "id": a.id,
            "candidate_name": a.candidate_name,
            "status": a.status,
            "test_id": a.test_id,
            "timestamp": a.completed_at.or(a.started_at)
        })).collect::<Vec<_>>(),
        "counts": {
            "candidates": total_new_candidates,
            "attempts": total_needs_review_attempts
        }
    })))
}



pub async fn list_all_tests(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let result = state.test_service.list_tests(1, 1000, None).await?;
    Ok(Json(result.tests))
}



pub async fn list_attempts_for_review(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let svc = crate::services::attempt_service::AttemptService::new(state.pool.clone());
    let (items, _total) = svc
        .list_attempts(None, None, Some("needs_review".to_string()), 1, 100)
        .await?;
    
    Ok(Json(items))
}




