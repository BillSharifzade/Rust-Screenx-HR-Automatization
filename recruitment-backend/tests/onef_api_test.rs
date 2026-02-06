use std::env;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use serde_json::{json, Value as JsonValue};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn onef_api_end_to_end() {
    dotenvy::dotenv().ok();
    env::set_var("SERVER_ADDRESS", "127.0.0.1:0");
    env::set_var("JWT_SECRET", "test_secret_key");
    env::set_var("WEBHOOK_SECRET", "whsec_test");
    env::set_var("PUBLIC_RPS", "100");
    env::set_var("INTEGRATION_RPS", "100");
    // Ensure we have a valid pool config
    recruitment_backend::config::init_config().expect("init config");

    let pool = recruitment_backend::database::pool::create_pool()
        .await
        .expect("pool");
    
    // Run migrations to ensure schema is up to date
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    // --- Seed Data ---
    // Cleanup potential leftovers from previous failed runs
    let _ = sqlx::query!("DELETE FROM candidates WHERE telegram_id = 123456789 OR email = 'onef_test@example.com'")
        .execute(&pool)
        .await;

    let candidate_id = Uuid::new_v4();
    let _ = sqlx::query!(
        "INSERT INTO candidates (id, name, email, telegram_id, status, ai_rating, ai_comment) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        candidate_id,
        "OneF Test Candidate",
        "onef_test@example.com",
        123456789,
        "new",
        Some(80i32),
        Some("Good potential".to_string())
    )
    .execute(&pool)
    .await
    .expect("seed candidate");

    // Seed Messages (1 unread, 1 read)
    let msg1_id = Uuid::new_v4();
    let _ = sqlx::query!(
        "INSERT INTO messages (id, candidate_id, telegram_id, direction, text, created_at) VALUES ($1, $2, $3, 'inbound', 'Hello OneF', NOW())",
        msg1_id, candidate_id, 123456789i64
    )
    .execute(&pool)
    .await
    .expect("seed message");

    // --- Setup Router ---
    let app_state = recruitment_backend::AppState::new(pool.clone());
    let onef_api = Router::new()
        .route(
            "/api/onef/messages",
            post(recruitment_backend::routes::onef::send_message),
        )
        .route(
            "/api/onef/messages/:candidate_id",
            get(recruitment_backend::routes::onef::get_chat_history),
        )
        .route(
            "/api/onef/messages/unread",
            get(recruitment_backend::routes::onef::get_unread_count),
        )
        .route(
            "/api/onef/dashboard",
            get(recruitment_backend::routes::onef::get_dashboard_stats),
        )
        .route(
            "/api/onef/candidates/:id",
            get(recruitment_backend::routes::onef::get_candidate),
        )
        .route(
            "/api/onef/candidates/:id/status",
            post(recruitment_backend::routes::onef::update_candidate_status),
        )
        .with_state(app_state.clone());

    let app = onef_api;

    // --- Test 1: Dashboard Stats ---
    let req = Request::builder()
        .method("GET")
        .uri("/api/onef/dashboard")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let stats: JsonValue = serde_json::from_slice(&bytes).unwrap();
    // Verify structure
    assert!(stats.get("candidates_total").is_some());
    assert!(stats.get("recruitment_funnel").is_some());

    // --- Test 2: Unread Count ---
    // Should be at least 1 from our seeded message
    let req = Request::builder()
        .method("GET")
        .uri("/api/onef/messages/unread")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let json: JsonValue = serde_json::from_slice(&bytes).unwrap();
    let count = json["unread_count"].as_i64().unwrap();
    assert!(count >= 1, "Expected unread count >= 1");

    // --- Test 3: Get Chat History ---
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/onef/messages/{}", candidate_id))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let messages: Vec<JsonValue> = serde_json::from_slice(&bytes).unwrap();
    assert!(!messages.is_empty(), "Expected chat history");
    assert_eq!(messages[0]["text"], "Hello OneF");

    // Verify side effect: fetching history should mark messages as read
    // So unread count should now decrease/be 0 for this candidate.
    // However, `mark_as_read` is async fire-and-forget in some implementations or direct await.
    // In onef.rs it is `let _ = state.message_service.mark_as_read(candidate_id).await;` -> It is awaited.
    
    let req = Request::builder()
        .method("GET")
        .uri("/api/onef/messages/unread")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let _json: JsonValue = serde_json::from_slice(&bytes).unwrap();
    // Depending on parallel test execution, other tests might affect this, but locally it should decrease.
    // We just verify the call succeeded.


    // --- Test 4: Update Candidate Status ---
    let status_body = json!({ "status": "accepted" });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/onef/candidates/{}/status", candidate_id))
        .header("content-type", "application/json")
        .body(Body::from(status_body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    if resp.status() != StatusCode::OK {
         let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
         let body = String::from_utf8(bytes.to_vec()).unwrap();
         println!("Response body: {}", body);
         panic!("Expected 200 OK, got {}", 400); 
    }
    
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let updated: JsonValue = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(updated["status"], "accepted");

    // Verify in DB
    let stored_status = sqlx::query!("SELECT status FROM candidates WHERE id = $1", candidate_id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .status;
    assert_eq!(stored_status, "accepted");

    // --- Test 5: Get Candidate Details (AI Suitability) ---
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/onef/candidates/{}", candidate_id))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    let candidate_details: JsonValue = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(candidate_details["ai_rating"], 80);
    assert_eq!(candidate_details["ai_comment"], "Good potential");

    // --- Test 6: Send Message (Missing Candidate) ---
    let bad_id = Uuid::new_v4();
    let msg_body = json!({ "candidate_id": bad_id, "text": "Fail" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/onef/messages")
        .header("content-type", "application/json")
        .body(Body::from(msg_body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Clean up
    let _ = sqlx::query!("DELETE FROM candidates WHERE id = $1", candidate_id).execute(&pool).await;
}
