use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub role: Option<String>,
}

pub fn mint_token(
    user_id: &str,
    role: &str,
    ttl_hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = (chrono::Utc::now() + chrono::Duration::hours(ttl_hours)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        role: Some(role.to_string()),
    };
    let secret = crate::config::get_config().jwt_secret.clone();
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

pub async fn require_admin(req: Request, next: Next) -> Response {
    require_roles(req, next, &["admin"]).await
}

pub async fn require_bearer_auth(mut req: Request, next: Next) -> Response {
    let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"missing_authorization"})),
        )
            .into_response();
    };
    let Ok(auth_str) = auth_header.to_str() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"bad_authorization"})),
        )
            .into_response();
    };
    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unsupported_scheme"})),
        )
            .into_response();
    };

    let config = crate::config::get_config();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => {
            req.extensions_mut().insert(data.claims);
            next.run(req).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_token"})),
        )
            .into_response(),
    }
}

pub async fn require_hr_or_admin(mut req: Request, next: Next) -> Response {
    let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"missing_authorization"})),
        )
            .into_response();
    };
    let Ok(auth_str) = auth_header.to_str() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"bad_authorization"})),
        )
            .into_response();
    };
    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unsupported_scheme"})),
        )
            .into_response();
    };

    let config = crate::config::get_config();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => {
            let role = data.claims.role.clone().unwrap_or_default();
            let allowed = ["admin", "hr"];
            if !allowed.iter().any(|r| r.eq_ignore_ascii_case(&role)) {
                return (StatusCode::FORBIDDEN, Json(json!({"error":"forbidden"}))).into_response();
            }
            req.extensions_mut().insert(data.claims);
            next.run(req).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_token"})),
        )
            .into_response(),
    }
}

pub async fn require_roles(mut req: Request, next: Next, allowed: &[&str]) -> Response {
    let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"missing_authorization"})),
        )
            .into_response();
    };
    let Ok(auth_str) = auth_header.to_str() else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"bad_authorization"})),
        )
            .into_response();
    };
    let Some(token) = auth_str.strip_prefix("Bearer ") else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"unsupported_scheme"})),
        )
            .into_response();
    };

    let config = crate::config::get_config();
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    match decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    ) {
        Ok(data) => {
            let role = data.claims.role.clone().unwrap_or_default();
            if !allowed.is_empty() && !allowed.iter().any(|r| r.eq_ignore_ascii_case(&role)) {
                return (StatusCode::FORBIDDEN, Json(json!({"error":"forbidden"}))).into_response();
            }
            req.extensions_mut().insert(data.claims);
            next.run(req).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"invalid_token"})),
        )
            .into_response(),
    }
}
