use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use uuid::Uuid;
use validator::Validate;

use crate::{
    dto::vacancy_dto::{
        CreateVacancyPayload, UpdateVacancyPayload, VacancyListQuery, VacancyListResponse,
        VacancyPublicListResponse, VacancyPublicQuery, VacancyPublicSummary, VacancyResponse,
    },
    error::Result,
    AppState,
};

#[utoipa::path(
    post,
    path = "/api/integration/vacancies",
    request_body = CreateVacancyPayload,
    responses(
        (status = 201, description = "Vacancy created successfully", body = Json<VacancyResponse>),
        (status = 400, description = "Invalid payload")
    )
)]
#[axum::debug_handler]
pub async fn create_vacancy(
    State(state): State<AppState>,
    Json(payload): Json<CreateVacancyPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    let vacancy = state.vacancy_service.create(payload).await?;
    Ok((StatusCode::CREATED, Json(VacancyResponse::from(vacancy))))
}

#[utoipa::path(
    patch,
    path = "/api/integration/vacancies/{id}",
    params(
        ("id" = Uuid, Path, description = "Vacancy ID")
    ),
    request_body = UpdateVacancyPayload,
    responses(
        (status = 200, description = "Vacancy updated successfully", body = Json<VacancyResponse>),
        (status = 400, description = "Invalid payload"),
        (status = 404, description = "Vacancy not found")
    )
)]
#[axum::debug_handler]
pub async fn update_vacancy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateVacancyPayload>,
) -> Result<impl IntoResponse> {
    payload.validate()?;
    let vacancy = state.vacancy_service.update(id, payload).await?;
    Ok(Json(VacancyResponse::from(vacancy)))
}

#[utoipa::path(
    delete,
    path = "/api/integration/vacancies/{id}",
    params(
        ("id" = Uuid, Path, description = "Vacancy ID")
    ),
    responses(
        (status = 204, description = "Vacancy deleted successfully"),
        (status = 404, description = "Vacancy not found")
    )
)]
#[axum::debug_handler]
pub async fn delete_vacancy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    state.vacancy_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/integration/vacancies",
    params(
        ("page" = Option<i64>, Query, description = "Page number"),
        ("per_page" = Option<i64>, Query, description = "Items per page"),
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("company" = Option<String>, Query, description = "Filter by company"),
        ("search" = Option<String>, Query, description = "Search query")
    ),
    responses(
        (status = 200, description = "List of vacancies", body = Json<VacancyListResponse>)
    )
)]
#[axum::debug_handler]
pub async fn list_vacancies(
    State(state): State<AppState>,
    Query(query): Query<VacancyListQuery>,
) -> Result<impl IntoResponse> {
    let result = state.vacancy_service.list(query).await?;
    Ok(Json(VacancyListResponse::from(result)))
}

#[utoipa::path(
    get,
    path = "/api/integration/vacancies/{id}",
    params(
        ("id" = Uuid, Path, description = "Vacancy ID")
    ),
    responses(
        (status = 200, description = "Vacancy found", body = Json<VacancyResponse>),
        (status = 404, description = "Vacancy not found")
    )
)]
#[axum::debug_handler]
pub async fn get_vacancy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let vacancy = state.vacancy_service.get_by_id(id).await?;
    Ok(Json(VacancyResponse::from(vacancy)))
}

#[utoipa::path(
    get,
    path = "/api/public/vacancies",
    params(
        ("page" = Option<i64>, Query, description = "Page number, 1-based"),
        ("per_page" = Option<i64>, Query, description = "Items per page"),
        ("limit" = Option<i64>, Query, description = "Deprecated alias for per_page")
    ),
    responses(
        (status = 200, description = "List of public vacancies", body = Json<VacancyPublicListResponse>)
    )
)]
#[axum::debug_handler]
pub async fn list_public_vacancies(
    State(state): State<AppState>,
    Query(query): Query<VacancyPublicQuery>,
) -> Result<impl IntoResponse> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query
        .per_page
        .or(query.limit)
        .unwrap_or(20)
        .clamp(1, 100);
    let offset = (page - 1) * per_page;

    let (items, total) = state
        .vacancy_service
        .list_published(per_page, offset)
        .await?;
    let summaries: Vec<VacancyPublicSummary> = items.into_iter().map(Into::into).collect();
    let total_pages = ((total as f64) / (per_page as f64)).ceil() as i64;

    Ok(Json(VacancyPublicListResponse {
        items: summaries,
        total,
        page,
        per_page,
        total_pages,
    }))
}

#[utoipa::path(
    get,
    path = "/api/public/vacancies/{id}",
    params(
        ("id" = Uuid, Path, description = "Vacancy ID")
    ),
    responses(
        (status = 200, description = "Public vacancy found", body = Json<VacancyResponse>),
        (status = 404, description = "Vacancy not found")
    )
)]
#[axum::debug_handler]
pub async fn get_public_vacancy(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let vacancy = state.vacancy_service.get_by_id(id).await?;
    if vacancy.status != "published" {
        return Err(crate::error::Error::Unauthorized(
            "Vacancy not published".into(),
        ));
    }
    Ok(Json(VacancyResponse::from(vacancy)))
}
