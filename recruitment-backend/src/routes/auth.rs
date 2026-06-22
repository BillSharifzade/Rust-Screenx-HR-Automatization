use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;

use crate::middleware::auth::{mint_token, Claims};
use crate::models::user::AdminUser;
use crate::utils::crypto::{hash_password, verify_password};
use crate::AppState;

/// Columns selected into [`AdminUser`] everywhere in this module.
const USER_COLS: &str = "id, name, email, role, is_active, must_change_password, \
    password_hash, last_login_at, created_at, updated_at";

const ALLOWED_ROLES: [&str; 3] = ["hr", "manager", "admin"];
const TOKEN_TTL_HOURS: i64 = 12;
const MIN_PASSWORD_LEN: usize = 8;

type ApiResult = Result<Json<JsonValue>, (StatusCode, Json<JsonValue>)>;

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<JsonValue>) {
    (status, Json(json!({ "error": msg })))
}

fn db_err(e: sqlx::Error) -> (StatusCode, Json<JsonValue>) {
    tracing::error!(error = ?e, "auth db error");
    err(StatusCode::INTERNAL_SERVER_ERROR, "database_error")
}

/// Best-effort client IP, honoring the reverse-proxy headers (OpenResty/Caddy).
fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn validate_role(role: &str) -> Result<(), (StatusCode, Json<JsonValue>)> {
    if ALLOWED_ROLES.contains(&role) {
        Ok(())
    } else {
        Err(err(StatusCode::BAD_REQUEST, "invalid_role"))
    }
}

fn validate_password(pw: &str) -> Result<(), (StatusCode, Json<JsonValue>)> {
    if pw.chars().count() < MIN_PASSWORD_LEN {
        Err(err(StatusCode::BAD_REQUEST, "password_too_short"))
    } else {
        Ok(())
    }
}

fn validate_email(email: &str) -> Result<(), (StatusCode, Json<JsonValue>)> {
    let ok = email.len() >= 3 && email.contains('@') && !email.contains(' ');
    if ok {
        Ok(())
    } else {
        Err(err(StatusCode::BAD_REQUEST, "invalid_email"))
    }
}

// ---------------------------------------------------------------------------
// Login + session
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let email = req.email.trim().to_lowercase();
    let ip = client_ip(&headers);
    let key = format!("{}|{}", ip, email);

    // Brute-force lockout check.
    if let Err(retry_after) = state.login_guard.check(&key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": "too_many_attempts",
                "retry_after": retry_after,
            })),
        );
    }

    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "SELECT {USER_COLS} FROM users WHERE lower(email) = $1 AND is_active = true"
    ))
    .bind(&email)
    .fetch_optional(&state.pool)
    .await;

    let user = match user {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = ?e, "login query failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database_error" })),
            );
        }
    };

    let valid = user
        .as_ref()
        .and_then(|u| u.password_hash.as_deref())
        .map(|hash| verify_password(&req.password, hash).unwrap_or(false))
        .unwrap_or(false);

    if !valid {
        let remaining = state.login_guard.record_failure(&key);
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": "invalid_credentials",
                "attempts_remaining": remaining,
            })),
        );
    }

    let user = user.expect("validated user exists");
    state.login_guard.record_success(&key);

    // Stamp last login (best-effort).
    let _ = sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
        .bind(user.id)
        .execute(&state.pool)
        .await;

    let token = match mint_token(&user.id.to_string(), &user.role, TOKEN_TTL_HOURS) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = ?e, "token minting failed");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "token_error" })),
            );
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "token": token,
            "must_change_password": user.must_change_password,
            "user": user,
        })),
    )
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult {
    let id = Uuid::parse_str(&claims.sub)
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "invalid_token"))?;
    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "SELECT {USER_COLS} FROM users WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| err(StatusCode::NOT_FOUND, "user_not_found"))?;
    Ok(Json(json!(user)))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Lets the logged-in user rotate their own password (used to clear the
/// forced "must change password" state on the seeded admin).
pub async fn change_my_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> ApiResult {
    validate_password(&req.new_password)?;
    let id = Uuid::parse_str(&claims.sub)
        .map_err(|_| err(StatusCode::UNAUTHORIZED, "invalid_token"))?;

    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "SELECT {USER_COLS} FROM users WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| err(StatusCode::NOT_FOUND, "user_not_found"))?;

    let ok = user
        .password_hash
        .as_deref()
        .map(|h| verify_password(&req.current_password, h).unwrap_or(false))
        .unwrap_or(false);
    if !ok {
        return Err(err(StatusCode::UNAUTHORIZED, "invalid_current_password"));
    }

    let new_hash = hash_password(&req.new_password)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "hash_error"))?;
    sqlx::query(
        "UPDATE users SET password_hash = $2, must_change_password = false WHERE id = $1",
    )
    .bind(id)
    .bind(&new_hash)
    .execute(&state.pool)
    .await
    .map_err(db_err)?;

    Ok(Json(json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------------
// User management (admin only — gated by require_admin middleware)
// ---------------------------------------------------------------------------

pub async fn list_users(State(state): State<AppState>) -> ApiResult {
    let users = sqlx::query_as::<_, AdminUser>(&format!(
        "SELECT {USER_COLS} FROM users ORDER BY created_at ASC"
    ))
    .fetch_all(&state.pool)
    .await
    .map_err(db_err)?;
    Ok(Json(json!(users)))
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub role: String,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub must_change_password: bool,
}

fn default_true() -> bool {
    true
}

pub async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> ApiResult {
    let name = req.name.trim().to_string();
    let email = req.email.trim().to_lowercase();
    if name.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "name_required"));
    }
    validate_email(&email)?;
    validate_role(&req.role)?;
    validate_password(&req.password)?;

    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users WHERE lower(email) = $1")
        .bind(&email)
        .fetch_one(&state.pool)
        .await
        .map_err(db_err)?;
    if exists > 0 {
        return Err(err(StatusCode::CONFLICT, "email_exists"));
    }

    let hash = hash_password(&req.password)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "hash_error"))?;

    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "INSERT INTO users (external_id, name, email, role, password_hash, is_active, must_change_password) \
         VALUES (NULL, $1, $2, $3, $4, $5, $6) RETURNING {USER_COLS}"
    ))
    .bind(&name)
    .bind(&email)
    .bind(&req.role)
    .bind(&hash)
    .bind(req.is_active)
    .bind(req.must_change_password)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    Ok(Json(json!(user)))
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> ApiResult {
    let email = match &req.email {
        Some(e) => {
            let e = e.trim().to_lowercase();
            validate_email(&e)?;
            // Reject if the email belongs to a different user.
            let clash = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM users WHERE lower(email) = $1 AND id <> $2",
            )
            .bind(&e)
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(db_err)?;
            if clash > 0 {
                return Err(err(StatusCode::CONFLICT, "email_exists"));
            }
            Some(e)
        }
        None => None,
    };

    if let Some(role) = &req.role {
        validate_role(role)?;
    }

    // Guard against demoting/deactivating the last remaining active admin.
    if req.role.as_deref().map(|r| r != "admin").unwrap_or(false)
        || req.is_active == Some(false)
    {
        guard_last_admin(&state, id).await?;
    }

    let name = req.name.map(|n| n.trim().to_string());

    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "UPDATE users SET \
            name = COALESCE($2, name), \
            email = COALESCE($3, email), \
            role = COALESCE($4, role), \
            is_active = COALESCE($5, is_active) \
         WHERE id = $1 RETURNING {USER_COLS}"
    ))
    .bind(id)
    .bind(name)
    .bind(email)
    .bind(req.role)
    .bind(req.is_active)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| err(StatusCode::NOT_FOUND, "user_not_found"))?;

    Ok(Json(json!(user)))
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub new_password: String,
    #[serde(default = "default_true")]
    pub must_change_password: bool,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<ResetPasswordRequest>,
) -> ApiResult {
    validate_password(&req.new_password)?;
    let hash = hash_password(&req.new_password)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "hash_error"))?;

    let user = sqlx::query_as::<_, AdminUser>(&format!(
        "UPDATE users SET password_hash = $2, must_change_password = $3 \
         WHERE id = $1 RETURNING {USER_COLS}"
    ))
    .bind(id)
    .bind(&hash)
    .bind(req.must_change_password)
    .fetch_optional(&state.pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| err(StatusCode::NOT_FOUND, "user_not_found"))?;

    Ok(Json(json!(user)))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult {
    guard_last_admin(&state, id).await?;

    let affected = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(db_err)?
        .rows_affected();
    if affected == 0 {
        return Err(err(StatusCode::NOT_FOUND, "user_not_found"));
    }
    Ok(Json(json!({ "status": "deleted" })))
}

/// Returns an error if `id` is the last active admin (prevents lockout).
async fn guard_last_admin(
    state: &AppState,
    id: Uuid,
) -> Result<(), (StatusCode, Json<JsonValue>)> {
    let is_target_admin = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE id = $1 AND role = 'admin' AND is_active = true",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if is_target_admin == 0 {
        return Ok(());
    }

    let active_admins = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE role = 'admin' AND is_active = true",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(db_err)?;

    if active_admins <= 1 {
        return Err(err(StatusCode::BAD_REQUEST, "cannot_remove_last_admin"));
    }
    Ok(())
}
