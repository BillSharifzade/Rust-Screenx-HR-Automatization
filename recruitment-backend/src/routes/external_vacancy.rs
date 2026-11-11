//! 1F publishes its vacancies on job.koinotinav.tj through these endpoints.
//!
//! They call the board's API (`/api/v1/vacancies`, `X-Api-Key`). The request
//! shape of the old Selenium flow is still accepted: `email`, `password` and
//! `driver_binary` are ignored, and company / category may be given by name.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};

use crate::{
    error::{Error, Result},
    services::koinotinav_service::{BoardVacancy, ExternalCategory, ExternalCompany, VacancyFields},
    AppState,
};

const DEFAULT_CITY: &str = "Душанбе";
const STATUSES: &[&str] = &["published", "draft", "closed"];

/// Body of create and update. Every field is optional here; create checks
/// that `title` and `content` are present.
#[derive(Debug, Default, Deserialize)]
pub struct VacancyPayload {
    /// Board id, for update and delete.
    #[serde(default, alias = "id")]
    pub vacancy_id: Option<JsonValue>,
    /// 1F vacancy number. A repeated create with the same value updates the
    /// same vacancy instead of publishing a second one.
    #[serde(default, alias = "vacancy_id_1f", alias = "VacancyId1F")]
    pub external_id: Option<JsonValue>,
    #[serde(default)]
    pub title: Option<String>,
    /// Description HTML.
    #[serde(default, alias = "description_html", alias = "description")]
    pub content: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    /// Category ("Сфера") name, as listed by `GET /api/onef/vacancies/external/options`.
    #[serde(default, alias = "category")]
    pub direction: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    /// Company name, as listed by `GET /api/onef/vacancies/external/options`.
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub company_id: Option<i64>,
    #[serde(default, alias = "is_hot")]
    pub hot: Option<bool>,
    /// `published` (default on create), `draft` or `closed`.
    #[serde(default)]
    pub status: Option<String>,
}

/// `POST /api/onef/vacancies/external`: publish, or update when `external_id`
/// is already on the board.
pub async fn create_external_vacancy(
    State(state): State<AppState>,
    Json(body): Json<JsonValue>,
) -> Result<impl IntoResponse> {
    let payload = parse(body)?;
    let title = required(&payload.title, "title")?;
    let content = required(&payload.content, "content")?;
    let external_id = id_string(&payload.external_id)
        .unwrap_or_else(|| fallback_external_id(&title, &content, &payload));
    let fields = VacancyFields {
        external_id: Some(external_id),
        title: Some(title),
        description_html: Some(content),
        city: Some(text(&payload.city).unwrap_or_else(|| DEFAULT_CITY.to_string())),
        status: Some(status(&payload.status)?.unwrap_or_else(|| "published".into())),
        is_hot: Some(payload.hot.unwrap_or(false)),
        ..resolve_refs(&state, &payload).await?
    };

    let (vacancy, created) = state.koinotinav_service.upsert_vacancy(&fields).await?;
    tracing::info!(
        "Job board vacancy {} {} for 1F {:?}",
        vacancy.id,
        if created { "published" } else { "updated" },
        vacancy.external_id
    );
    Ok(reply(if created { "Vacancy published" } else { "Vacancy updated" }, &vacancy, Some(created)))
}

/// `POST /api/onef/vacancies/external/update`: change some fields of a vacancy
/// found by `vacancy_id` or `external_id`. Also links a vacancy published before
/// 1F sent numbers: pass `vacancy_id` together with the new `external_id`.
pub async fn update_external_vacancy(
    State(state): State<AppState>,
    Json(body): Json<JsonValue>,
) -> Result<impl IntoResponse> {
    let payload = parse(body)?;
    let id = board_id(&state, &payload).await?;
    let fields = VacancyFields {
        // With a board id the 1F number may be new; otherwise it is the lookup key.
        external_id: if id_string(&payload.vacancy_id).is_some() {
            id_string(&payload.external_id)
        } else {
            None
        },
        title: text(&payload.title),
        description_html: text(&payload.content),
        city: text(&payload.city),
        status: status(&payload.status)?,
        is_hot: payload.hot,
        ..resolve_refs(&state, &payload).await?
    };
    let vacancy = state
        .koinotinav_service
        .update_vacancy(id, &fields)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Job board has no vacancy {}", id)))?;
    Ok(reply("Vacancy updated", &vacancy, None))
}

/// `POST /api/onef/vacancies/external/delete`: take a vacancy off the site.
/// The board keeps it as `closed`, so candidates' links still open.
pub async fn delete_external_vacancy(
    State(state): State<AppState>,
    Json(body): Json<JsonValue>,
) -> Result<impl IntoResponse> {
    let payload = parse(body)?;
    let id = board_id(&state, &payload).await?;
    if !state.koinotinav_service.close_vacancy(id).await? {
        return Err(Error::NotFound(format!("Job board has no vacancy {}", id)));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "success": true,
            "message": "Vacancy taken off the site",
            "vacancy_id": id.to_string(),
            "status": "closed",
        })),
    ))
}

/// `GET /api/onef/vacancies/external/options`: values accepted for
/// `company`, `direction`, `city` and `status`.
pub async fn external_vacancy_options(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let board = &state.koinotinav_service;
    let companies = board.fetch_companies().await?;
    let categories = board.fetch_categories().await?;
    let mut cities: Vec<String> = board
        .fetch_vacancies()
        .await?
        .into_iter()
        .map(|v| v.city)
        .filter(|c| !c.is_empty())
        .collect();
    if !cities.iter().any(|c| c == DEFAULT_CITY) {
        cities.push(DEFAULT_CITY.to_string());
    }
    cities.sort();
    cities.dedup();
    Ok(Json(json!({
        "companies": companies.iter().map(|c| json!({"id": c.id, "name": c.title})).collect::<Vec<_>>(),
        "categories": categories,
        "cities": cities,
        "statuses": STATUSES,
    })))
}

fn parse(body: JsonValue) -> Result<VacancyPayload> {
    let body = match body {
        JsonValue::Object(mut map) => map.remove("requestBody").unwrap_or(JsonValue::Object(map)),
        other => other,
    };
    if body.get("password").is_some() {
        tracing::warn!("1F still sends the job board admin password; it is ignored and should be removed");
    }
    serde_json::from_value(body)
        .map_err(|e| Error::BadRequest(format!("Invalid request payload: {}", e)))
}

fn reply(message: &str, v: &BoardVacancy, created: Option<bool>) -> impl IntoResponse {
    let mut body = json!({
        "success": true,
        "message": message,
        "vacancy_id": v.id.to_string(),
        "external_id": v.external_id,
        "status": v.status,
        "url": v.url,
    });
    if let Some(created) = created {
        body["created"] = json!(created);
    }
    (StatusCode::ACCEPTED, Json(body))
}

async fn board_id(state: &AppState, payload: &VacancyPayload) -> Result<i64> {
    if let Some(raw) = id_string(&payload.vacancy_id) {
        return raw
            .parse()
            .map_err(|_| Error::BadRequest(format!("vacancy_id must be a number, got '{}'", raw)));
    }
    let Some(external_id) = id_string(&payload.external_id) else {
        return Err(Error::BadRequest("vacancy_id or external_id is required".into()));
    };
    state
        .koinotinav_service
        .find_vacancy_by_external_id(&external_id)
        .await?
        .map(|v| v.id)
        .ok_or_else(|| Error::NotFound(format!("Job board has no vacancy with external_id '{}'", external_id)))
}

/// Company and category ids, from the ids given or from names.
async fn resolve_refs(state: &AppState, payload: &VacancyPayload) -> Result<VacancyFields> {
    let mut fields = VacancyFields {
        company_id: payload.company_id,
        category_id: payload.category_id,
        ..Default::default()
    };
    if fields.company_id.is_none() {
        if let Some(name) = text(&payload.company) {
            let companies = state.koinotinav_service.fetch_companies().await?;
            fields.company_id = Some(company_id(&companies, &name)?);
        }
    }
    if fields.category_id.is_none() {
        if let Some(name) = text(&payload.direction) {
            let categories = state.koinotinav_service.fetch_categories().await?;
            fields.category_id = Some(category_id(&categories, &name)?);
        }
    }
    Ok(fields)
}

fn company_id(companies: &[ExternalCompany], name: &str) -> Result<i64> {
    find_by_name(companies.iter().map(|c| (c.id, c.title.as_str())), name, "company")
}

fn category_id(categories: &[ExternalCategory], name: &str) -> Result<i64> {
    find_by_name(categories.iter().map(|c| (c.id, c.name.as_str())), name, "direction")
}

fn find_by_name<'a>(
    options: impl Iterator<Item = (i64, &'a str)> + Clone,
    name: &str,
    field: &str,
) -> Result<i64> {
    let wanted = normalize(name);
    options
        .clone()
        .find(|(_, n)| normalize(n) == wanted)
        .map(|(id, _)| id)
        .ok_or_else(|| {
            let known: Vec<&str> = options.map(|(_, n)| n).collect();
            Error::BadRequest(format!(
                "Unknown {} '{}'. Known values: {}",
                field,
                name.trim(),
                known.join(", ")
            ))
        })
}

fn normalize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .replace('ё', "е")
}

fn text(value: &Option<String>) -> Option<String> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn required(value: &Option<String>, field: &str) -> Result<String> {
    text(value).ok_or_else(|| Error::BadRequest(format!("{} is required", field)))
}

/// Ids arrive as numbers or strings.
fn id_string(value: &Option<JsonValue>) -> Option<String> {
    match value {
        Some(JsonValue::Number(n)) => Some(n.to_string()),
        Some(JsonValue::String(s)) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn status(value: &Option<String>) -> Result<Option<String>> {
    match text(value).map(|s| s.to_lowercase()) {
        Some(s) if STATUSES.contains(&s.as_str()) => Ok(Some(s)),
        Some(s) => Err(Error::BadRequest(format!(
            "status must be one of {}, got '{}'",
            STATUSES.join(", "),
            s
        ))),
        None => Ok(None),
    }
}

/// Without a 1F number, the same vacancy sent twice (e.g. a retry) must still
/// land on the board once, so the key is derived from its content.
fn fallback_external_id(title: &str, content: &str, payload: &VacancyPayload) -> String {
    let mut hash = Sha256::new();
    for part in [
        title,
        content,
        payload.company.as_deref().unwrap_or(""),
        payload.direction.as_deref().unwrap_or(""),
        payload.city.as_deref().unwrap_or(""),
    ] {
        hash.update(normalize(part).as_bytes());
        hash.update([0]);
    }
    format!("screenx-{}", &hex::encode(hash.finalize())[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selenium_era_payload_is_still_accepted() {
        let p = parse(json!({
            "email": "admin@example.tj", "password": "x", "driver_binary": null,
            "title": "Бухгалтер", "content": "<p>Описание</p>",
            "city": "Душанбе", "direction": "Финансы и бухгалтерия",
            "company": "КОИНОТИ НАВ", "hot": true
        }))
        .unwrap();
        assert_eq!(p.title.as_deref(), Some("Бухгалтер"));
        assert_eq!(p.hot, Some(true));
        assert!(p.external_id.is_none());
    }

    #[test]
    fn wrapped_payload_and_1f_aliases() {
        let p = parse(json!({"requestBody": {
            "vacancy_id_1f": 10234, "title": "T", "description": "<p>d</p>", "is_hot": false
        }}))
        .unwrap();
        assert_eq!(id_string(&p.external_id).as_deref(), Some("10234"));
        assert_eq!(p.content.as_deref(), Some("<p>d</p>"));
        assert_eq!(p.hot, Some(false));

        let p = parse(json!({"vacancy_id": "99"})).unwrap();
        assert_eq!(id_string(&p.vacancy_id).as_deref(), Some("99"));
    }

    #[test]
    fn names_match_regardless_of_case_spacing_and_yo() {
        let companies = vec![
            ExternalCompany { id: 1, title: "КОИНОТИ НАВ".into(), logo: String::new() },
            ExternalCompany { id: 9, title: "JAC Motors ".into(), logo: String::new() },
            ExternalCompany { id: 13, title: "ЁВАР".into(), logo: String::new() },
        ];
        assert_eq!(company_id(&companies, " коиноти  нав ").unwrap(), 1);
        assert_eq!(company_id(&companies, "jac motors").unwrap(), 9);
        assert_eq!(company_id(&companies, "Евар").unwrap(), 13);
        let err = company_id(&companies, "Неизвестная").unwrap_err().to_string();
        assert!(err.contains("Known values: КОИНОТИ НАВ"), "{}", err);
    }

    #[test]
    fn status_is_validated() {
        assert_eq!(status(&Some(" Draft ".into())).unwrap().as_deref(), Some("draft"));
        assert_eq!(status(&None).unwrap(), None);
        assert!(status(&Some("deleted".into())).is_err());
    }

    #[test]
    fn fallback_external_id_is_stable_for_the_same_vacancy() {
        let p = VacancyPayload { company: Some("КОИНОТИ НАВ".into()), ..Default::default() };
        let a = fallback_external_id("Бухгалтер", "<p>x</p>", &p);
        let b = fallback_external_id(" бухгалтер ", "<p>x</p>", &p);
        let c = fallback_external_id("Бухгалтер", "<p>y</p>", &p);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("screenx-") && a.len() == 24);
    }
}
