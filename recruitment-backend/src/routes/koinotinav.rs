use axum::{
    extract::State,
    response::{IntoResponse, Json},
};

use crate::{
    error::Result,
    AppState,
};

#[axum::debug_handler]
pub async fn list_external_vacancies(
    State(state): State<AppState>,
) -> Result<impl IntoResponse> {
    let vacancies = state.koinotinav_service.fetch_vacancies().await?;
    let companies = state.koinotinav_service.fetch_companies().await?;

    Ok(Json(serde_json::json!({
        "vacancies": vacancies,
        "companies": companies
    })))
}
