use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use recruitment_backend::services::queue_service::AiQueueService;
use recruitment_backend::{
    config::{get_config, init_config},
    database::pool::create_pool,
    routes, AppState,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    init_config()?;
    let config = get_config();

    let pool = create_pool().await?;
    
    let _ = sqlx::query("DELETE FROM _sqlx_migrations WHERE version >= 20260120150000 AND version < 20260120160000")
        .execute(&pool)
        .await;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let app_state = AppState::new(pool);

    {
        let bot_token = config.telegram_bot_token.clone();
        let target_webhook_url = format!("{}/api/webhook/telegram", config.webapp_url);
        
        info!("Checking Telegram webhook status...");
        
        match reqwest::get(format!("https://api.telegram.org/bot{}/getWebhookInfo", bot_token)).await {
            Ok(resp) => {
                if let Ok(info) = resp.json::<serde_json::Value>().await {
                    let current_url = info["result"]["url"].as_str().unwrap_or("");
                    
                    if current_url == target_webhook_url {
                        info!("Telegram webhook is already up to date: {}", current_url);
                    } else {
                        info!("Updating Telegram webhook: {} -> {}", current_url, target_webhook_url);
                        let set_url = format!(
                            "https://api.telegram.org/bot{}/setWebhook?url={}",
                            bot_token, target_webhook_url
                        );
                        if let Ok(set_resp) = reqwest::get(&set_url).await {
                            if set_resp.status().is_success() {
                                info!("Telegram webhook registered successfully");
                            } else {
                                tracing::warn!("Failed to register Telegram webhook: {:?}", set_resp.status());
                            }
                        }
                    }
                }
            }
            Err(e) => tracing::warn!("Could not check Telegram webhook status: {:?}", e),
        }
    }

    {
        let state = app_state.clone();
        tokio::spawn(async move {
            let queue = AiQueueService::new(state.pool.clone());
            loop {
                match queue.run_once(&state).await {
                    Ok(true) => {
                    }
                    Ok(false) => {
                        tokio::time::sleep(Duration::from_millis(750)).await;
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "AI queue worker error");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    {
        let state = app_state.clone();
        tokio::spawn(async move {
            let notif =
                recruitment_backend::services::notification_service::NotificationService::new(
                    state.pool.clone(),
                    recruitment_backend::config::get_config()
                        .telegram_bot_webhook_url
                        .clone(),
                );
            loop {
                match notif.run_once().await {
                    Ok(true) => {}
                    Ok(false) => {
                        tokio::time::sleep(Duration::from_millis(1000)).await;
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "Webhook worker error");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        });
    }

    {
        let state = app_state.clone();
        tokio::spawn(async move {
            let attempt_svc = recruitment_backend::services::attempt_service::AttemptService::new(state.pool.clone());
            let notif = recruitment_backend::services::notification_service::NotificationService::new(
                state.pool.clone(),
                recruitment_backend::config::get_config().telegram_bot_webhook_url.clone(),
            );
            loop {
                if let Err(e) = attempt_svc.check_deadlines(&notif).await {
                    tracing::error!("Deadline checker error: {:?}", e);
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    let base_routes = Router::new().route("/health", get(routes::health::health));

    let integration_api = Router::new()
        .route(
            "/api/integration/test-invites",
            get(routes::integration::list_test_invites).post(routes::integration::create_test_invite),
        )
        .route(
            "/api/integration/tests",
            get(routes::integration::list_tests).post(routes::integration::create_test),
        )
        .route(
            "/api/integration/tests/:id",
            get(routes::integration::get_test_by_id)
                .patch(routes::integration::update_test)
                .delete(routes::integration::delete_test),
        )


        .route(
            "/api/integration/tests/generate",
            post(routes::integration::generate_test_spec),
        )
        .route(
            "/api/integration/tests/generate-ai",
            post(routes::integration::generate_ai_test),
        )
        .route(
            "/api/integration/vacancies/description",
            post(routes::integration::generate_vacancy_description),
        )
        .route(
            "/api/integration/vacancies",
            get(routes::vacancy::list_vacancies).post(routes::vacancy::create_vacancy),
        )
        .route(
            "/api/integration/external-vacancies",
            get(routes::koinotinav::list_external_vacancies),
        )
        .route(
            "/api/onef/vacancies/external",
            post(routes::external_vacancy::create_external_vacancy),
        )
        .route(
            "/api/onef/vacancies/external/delete",
            post(routes::external_vacancy::delete_external_vacancy),
        )
        .route(
            "/api/integration/vacancies/:id",
            get(routes::vacancy::get_vacancy)
                .patch(routes::vacancy::update_vacancy)
                .delete(routes::vacancy::delete_vacancy),
        )
        .route(
            "/api/integration/ai-jobs",
            post(routes::integration::enqueue_ai_job),
        )
        .route(
            "/api/integration/ai-jobs/:id",
            get(routes::integration::get_ai_job),
        )
        .route(
            "/api/integration/test-attempts/:id",
            get(routes::integration::get_test_attempt_by_id)
                .delete(routes::integration::delete_test_invite),
        )
        .route(
            "/api/integration/test-attempts/:id/grade",
            post(routes::integration::grade_presentation),
        )
        .route(
            "/api/integration/test-attempts/:id/grade-answer",
            post(routes::integration::grade_test_answer),
        )
        .route(
            "/api/integration/test-attempts",
            get(routes::integration::list_test_attempts),
        )
        .route(
            "/api/integration/candidates",
            get(routes::integration::list_candidates),
        )
        .route(
            "/api/integration/candidates/:id",
            axum::routing::delete(routes::candidate_routes::delete_candidate),
        )
        .route(
            "/api/integration/candidates/:id/status",
            post(routes::candidate_routes::update_candidate_status),
        )
        .route(
            "/api/integration/analyze-suitability/:id",
            post(routes::candidate_routes::analyze_candidate_suitability),
        )
        .route(
            "/api/integration/candidates/:id/onef-grade",
            post(routes::candidate_routes::share_candidate_grade_to_onef),
        )
        .route(
            "/api/integration/tests/all",
            get(routes::integration::list_all_tests),
        )
        .route(
            "/api/integration/candidates/statuses",
            get(routes::integration::sync_candidate_statuses),
        )
        .route(
            "/api/integration/test-attempts/needs-review",
            get(routes::integration::list_attempts_for_review),
        )
        .route(
            "/api/integration/messages",
            post(routes::integration::send_message),
        )
        .route(
            "/api/integration/messages/:candidate_id",
            get(routes::integration::get_chat_messages),
        )
        .route(
            "/api/integration/messages/unread",
            get(routes::integration::get_unread_count),
        )

        .route(
            "/api/integration/notifications/poll",
            get(routes::integration::poll_notifications),
        )
        .route(
            "/api/integration/dashboard/stats",
            get(routes::integration::get_dashboard_stats),
        )
        .route(
            "/api/integration/candidates/:id/export",
            get(routes::export::export_candidate),
        )
        .route(
            "/api/integration/candidates/export",
            post(routes::export::export_candidates_bulk),
        )

        .layer(axum::middleware::from_fn_with_state(
            recruitment_backend::middleware::rate_limit::new_rps_state(config.integration_rps),
            recruitment_backend::middleware::rate_limit::rps_middleware,
        ));

    let public_api = Router::new()
        .route(
            "/api/public/tests/:token",
            get(routes::public::get_test_by_token),
        )
        .route(
            "/api/public/tests/:token/start",
            post(routes::public::start_test),
        )
        .route(
            "/api/public/tests/:token/answer",
            axum::routing::patch(routes::public::save_answer),
        )
        .route(
            "/api/public/tests/:token/submit",
            post(routes::public::submit_test),
        )
        .route(
            "/api/public/tests/:token/submit-presentation",
            post(routes::public::submit_presentation),
        )
        .route(
            "/api/public/tests/:token/status",
            get(routes::public::get_status),
        )
        .route(
            "/api/public/tests/:token/heartbeat",
            post(routes::public::heartbeat),
        )
        .route(
            "/api/public/tests/:token/report-violation",
            post(routes::public::report_violation),
        )
        .route(
            "/api/public/vacancies",
            get(routes::vacancy::list_public_vacancies),
        )
        .route(
            "/api/public/vacancies/:id",
            get(routes::vacancy::get_public_vacancy),
        )
        .route(
            "/api/webhook/telegram",
            post(routes::telegram::handle_webhook),
        )
        .route(
            "/api/candidate/register",
            post(routes::candidate_routes::register_candidate),
        )
        .route(
            "/api/candidate/:id",
            get(routes::candidate_routes::get_candidate),
        )
        .route(
            "/api/candidate/:id/cv",
            axum::routing::patch(routes::candidate_routes::update_candidate_cv),
        )
        .route(
            "/api/candidate/apply",
            post(routes::candidate_routes::apply_for_vacancy),
        )
        .route(
            "/api/candidate/:id/applications",
            get(routes::candidate_routes::get_candidate_applications),
        )
        .route(
            "/api/vacancy/:id/candidates",
            get(routes::candidate_routes::get_candidates_for_vacancy),
        )
        .route(
            "/api/candidate/:id/history",
            get(routes::candidate_routes::get_candidate_history),
        )
        .route(
            "/api/external-vacancies",
            get(routes::koinotinav::list_external_vacancies),
        )
        .layer(axum::middleware::from_fn_with_state(
            recruitment_backend::middleware::rate_limit::new_rps_state(config.public_rps),
            recruitment_backend::middleware::rate_limit::rps_middleware,
        ));

    let onef_api = Router::new()
        .route(
            "/api/onef/messages",
            post(routes::onef::send_message),
        )
        .route(
            "/api/onef/messages/:candidate_id",
            get(routes::onef::get_chat_history),
        )
        .route(
            "/api/onef/messages/unread",
            get(routes::onef::get_unread_count),
        )
        .route(
            "/api/onef/dashboard",
            get(routes::onef::get_dashboard_stats),
        )
        .route(
            "/api/onef/vacancies",
            get(routes::onef::list_vacancies),
        )
        .route(
            "/api/onef/vacancies/:id",
            get(routes::onef::get_vacancy),
        )
        .route(
            "/api/onef/candidates",
            get(routes::onef::list_candidates),
        )
        .route(
            "/api/onef/candidates/:id",
            get(routes::onef::get_candidate),
        )
        .route(
            "/api/onef/candidates/:id/attempts",
            get(routes::onef::get_candidate_attempts),
        )
        .route(
            "/api/onef/attempts_filter",
            get(routes::onef::list_attempts_filter),
        )
        .route(
            "/api/onef/attempts",
            get(routes::onef::list_all_attempts),
        )
        .route(
            "/api/onef/attempts/:id",
            get(routes::onef::get_test_attempt),
        )
        .route(
            "/api/onef/candidates/:id/status",
            post(routes::onef::update_candidate_status),
        )
        .route(
            "/api/onef/candidates/:id/analyze",
            post(routes::candidate_routes::analyze_candidate_suitability),
        )
        .route(
            "/api/onef/tests",
            get(routes::onef::list_tests),
        )
        .route(
            "/api/onef/invites",
            post(routes::onef::create_test_invite),
        )
        .route(
            "/api/onef/dictionaries/candidate-statuses",
            get(routes::onef::list_candidate_statuses),
        )
        .route(
            "/api/onef/dictionaries/test-statuses",
            get(routes::onef::list_test_statuses),
        )
        .layer(axum::middleware::from_fn_with_state(
            recruitment_backend::middleware::rate_limit::new_rps_state(config.integration_rps),
            recruitment_backend::middleware::rate_limit::rps_middleware,
        ));

    let upload_path = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "/app/uploads".to_string());
    info!("Serving uploads from: {}", upload_path);

    let app = base_routes
        .merge(integration_api)
        .merge(public_api)
        .merge(onef_api)
        .nest_service("/uploads", tower_http::services::ServeDir::new(upload_path))
        .with_state(app_state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024));

    let addr: SocketAddr = config.server_address.parse()?;
    info!("Server listening on {}", addr);
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
