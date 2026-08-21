use reqwest::Client;
use serde::Serialize;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::models::onef_vacancy::{NormalizedOneFVacancy, OneFVacancy, OneFVacancyPayload};

const PATH_GET_VACANCIES: &str = "/action/getVacancies";

#[derive(Debug, Clone, Default, Serialize)]
pub struct OneFVacancySyncSummary {
    pub fetched: usize,
    pub skipped: usize,
    pub inserted: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub deactivated: u64,
}

#[derive(Clone)]
pub struct OneFVacancyService {
    pool: PgPool,
    client: Client,
    base_url: Option<String>,
}

impl OneFVacancyService {
    pub fn new(pool: PgPool, base_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("Failed to create HTTP client for the 1F vacancy catalogue");

        match &base_url {
            Some(url) => info!("1F vacancy catalogue reading from {}{}", url, PATH_GET_VACANCIES),
            None => info!("1F vacancy catalogue disabled (no ONEF_READ_BASE_URL / ONEF_BASE_URLS)"),
        }

        Self { pool, client, base_url }
    }

    pub fn is_enabled(&self) -> bool {
        self.base_url.is_some()
    }

    pub async fn fetch_remote(&self) -> Result<(Vec<NormalizedOneFVacancy>, usize)> {
        let base = self.base_url.as_ref().ok_or_else(|| {
            Error::BadRequest("1F vacancy sync is not configured (set ONEF_READ_BASE_URL)".into())
        })?;

        let url = format!("{}{}", base, PATH_GET_VACANCIES);
        let response = self.client.get(&url).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Internal(format!(
                "1F getVacancies returned {} from {}: {}",
                status,
                url,
                body.chars().take(200).collect::<String>()
            )));
        }

        let items = unwrap_list(response.json::<JsonValue>().await?, &url)?;

        // Parse record by record: one malformed vacancy must not cost us the sync.
        let mut parsed = Vec::with_capacity(items.len());
        let mut skipped = 0;
        for item in items {
            match serde_json::from_value::<OneFVacancyPayload>(item.clone()) {
                Ok(payload) => parsed.push(payload.normalize(item)),
                Err(e) => {
                    skipped += 1;
                    warn!("Skipping unparseable 1F vacancy record: {} — {}", e, preview(&item));
                }
            }
        }

        Ok((parsed, skipped))
    }

    pub async fn sync(&self) -> Result<OneFVacancySyncSummary> {
        let (vacancies, skipped) = self.fetch_remote().await?;

        let mut summary = OneFVacancySyncSummary {
            fetched: vacancies.len() + skipped,
            skipped,
            ..Default::default()
        };

        if vacancies.is_empty() {
            warn!("1F returned no vacancies; keeping the existing catalogue untouched");
            return Ok(summary);
        }

        let known: HashMap<i64, String> =
            sqlx::query_as::<_, (i64, String)>("SELECT vacancy_id_1f, content_hash FROM onef_vacancies")
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .collect();

        let mut tx = self.pool.begin().await?;

        for vacancy in &vacancies {
            match known.get(&vacancy.vacancy_id_1f) {
                None => summary.inserted += 1,
                Some(hash) if *hash == vacancy.content_hash => summary.unchanged += 1,
                Some(_) => summary.updated += 1,
            }
            upsert(&mut tx, vacancy).await?;
        }

        let seen: Vec<i64> = vacancies.iter().map(|v| v.vacancy_id_1f).collect();
        summary.deactivated = sqlx::query(
            "UPDATE onef_vacancies SET is_active = FALSE \
             WHERE is_active = TRUE AND vacancy_id_1f <> ALL($1)",
        )
        .bind(&seen)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;

        info!(
            "1F vacancy sync: {} fetched, {} inserted, {} updated, {} unchanged, {} deactivated, {} skipped",
            summary.fetched,
            summary.inserted,
            summary.updated,
            summary.unchanged,
            summary.deactivated,
            summary.skipped
        );

        Ok(summary)
    }

    pub async fn list(&self, include_inactive: bool) -> Result<Vec<OneFVacancy>> {
        let rows = sqlx::query_as::<_, OneFVacancy>(
            "SELECT * FROM onef_vacancies WHERE ($1 OR is_active) ORDER BY vacancy_id_1f DESC",
        )
        .bind(include_inactive)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, vacancy_id_1f: i64) -> Result<Option<OneFVacancy>> {
        let row = sqlx::query_as::<_, OneFVacancy>(
            "SELECT * FROM onef_vacancies WHERE vacancy_id_1f = $1",
        )
        .bind(vacancy_id_1f)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
        }

    pub async fn stats(&self) -> Result<(i64, i64, Option<chrono::DateTime<chrono::Utc>>)> {
        let row = sqlx::query_as::<_, (i64, i64, Option<chrono::DateTime<chrono::Utc>>)>(
            "SELECT COUNT(*) FILTER (WHERE is_active), COUNT(*), MAX(last_seen_at) FROM onef_vacancies",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}

async fn upsert(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    v: &NormalizedOneFVacancy,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO onef_vacancies (
            vacancy_id_1f, external_vacancy_id, name, company, specialty, education,
            experience, age, gender, languages, computer_skills, professional_skills,
            personal_qualities, description_raw, description_clean, raw, data_quality,
            content_hash, is_active, last_seen_at, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
            $18, TRUE, NOW(), NOW(), NOW()
        )
        ON CONFLICT (vacancy_id_1f) DO UPDATE SET
            external_vacancy_id = EXCLUDED.external_vacancy_id,
            name                = EXCLUDED.name,
            company             = EXCLUDED.company,
            specialty           = EXCLUDED.specialty,
            education           = EXCLUDED.education,
            experience          = EXCLUDED.experience,
            age                 = EXCLUDED.age,
            gender              = EXCLUDED.gender,
            languages           = EXCLUDED.languages,
            computer_skills     = EXCLUDED.computer_skills,
            professional_skills = EXCLUDED.professional_skills,
            personal_qualities  = EXCLUDED.personal_qualities,
            description_raw     = EXCLUDED.description_raw,
            description_clean   = EXCLUDED.description_clean,
            raw                 = EXCLUDED.raw,
            data_quality        = EXCLUDED.data_quality,
            content_hash        = EXCLUDED.content_hash,
            is_active           = TRUE,
            last_seen_at        = NOW(),
            -- Only a real content change moves updated_at, so phase 2 can use it
            -- to decide what needs re-embedding.
            updated_at          = CASE
                                    WHEN onef_vacancies.content_hash IS DISTINCT FROM EXCLUDED.content_hash
                                    THEN NOW()
                                    ELSE onef_vacancies.updated_at
                                  END
        "#,
    )
    .bind(v.vacancy_id_1f)
    .bind(&v.external_vacancy_id)
    .bind(&v.name)
    .bind(&v.company)
    .bind(&v.specialty)
    .bind(&v.education)
    .bind(&v.experience)
    .bind(&v.age)
    .bind(&v.gender)
    .bind(&v.languages)
    .bind(&v.computer_skills)
    .bind(&v.professional_skills)
    .bind(&v.personal_qualities)
    .bind(&v.description_raw)
    .bind(&v.description_clean)
    .bind(&v.raw)
    .bind(v.data_quality)
    .bind(&v.content_hash)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn unwrap_list(value: JsonValue, url: &str) -> Result<Vec<JsonValue>> {
    match value {
        JsonValue::Array(items) => Ok(items),
        JsonValue::Object(map) => map
            .get("requestBody")
            .or_else(|| map.get("data"))
            .or_else(|| map.get("items"))
            .and_then(JsonValue::as_array)
            .cloned()
            .ok_or_else(|| {
                Error::Internal(format!(
                    "1F getVacancies at {} returned an object with no recognisable list",
                    url
                ))
            }),
        other => Err(Error::Internal(format!(
            "1F getVacancies at {} returned {} instead of a list",
            url,
            kind_of(&other)
        ))),
    }
}

fn kind_of(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "a boolean",
        JsonValue::Number(_) => "a number",
        JsonValue::String(_) => "a string",
        JsonValue::Array(_) => "an array",
        JsonValue::Object(_) => "an object",
    }
}

fn preview(value: &JsonValue) -> String {
    value.to_string().chars().take(160).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_a_bare_array_or_a_wrapped_one() {
        assert_eq!(unwrap_list(json!([{"a": 1}]), "u").unwrap().len(), 1);
        assert_eq!(unwrap_list(json!({"requestBody": [{"a": 1}]}), "u").unwrap().len(), 1);
        assert!(unwrap_list(json!({"unexpected": 1}), "u").is_err());
        assert!(unwrap_list(json!("nope"), "u").is_err());
    }
}
