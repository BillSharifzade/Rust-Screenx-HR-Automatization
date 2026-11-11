//! Imports candidate applications left on job.koinotinav.tj into the responses
//! board, so HR reviews them next to the Telegram ones.
//!
//! Each application becomes a candidate (matched by email, then phone) plus a
//! response for its vacancy; the AI grading worker picks the response up like
//! any other. Imported application ids are recorded in `koinotinav_applications`
//! and never imported twice, even if HR later deletes the candidate.

use std::collections::HashSet;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;
use crate::services::koinotinav_service::{KoinotinavService, PortalApplication};

/// `responses.vacancy_id` for a general application not tied to a vacancy.
pub const NO_VACANCY_ID: i64 = 0;
const NO_VACANCY_TITLE: &str = "Без вакансии (анкета с сайта)";
/// Re-read this far behind the newest imported application, so one that was
/// saved on the board with a slightly older timestamp is not missed.
const SINCE_OVERLAP_HOURS: i64 = 24;

#[derive(Clone)]
pub struct PortalApplicationService {
    pool: PgPool,
    board: KoinotinavService,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub fetched: usize,
    pub imported: usize,
    pub failed: usize,
}

impl PortalApplicationService {
    pub fn new(pool: PgPool, board: KoinotinavService) -> Self {
        Self { pool, board }
    }

    pub fn is_enabled(&self) -> bool {
        self.board.has_api_key()
    }

    pub async fn sync(&self) -> Result<SyncReport> {
        let newest: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT MAX(application_created_at) FROM koinotinav_applications")
                .fetch_one(&self.pool)
                .await?;
        let since = newest.map(|t| t - Duration::hours(SINCE_OVERLAP_HOURS));
        let mut applications = self.board.fetch_applications(since).await?;

        let ids: Vec<i64> = applications.iter().map(|a| a.id).collect();
        let known: HashSet<i64> = sqlx::query_scalar(
            "SELECT application_id FROM koinotinav_applications WHERE application_id = ANY($1)",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .collect();

        let mut report = SyncReport {
            fetched: applications.len(),
            ..Default::default()
        };
        applications.retain(|a| !known.contains(&a.id));
        applications.sort_by_key(|a| (received_at(a), a.id));
        for app in &applications {
            match self.import(app).await {
                Ok(()) => report.imported += 1,
                Err(e) => {
                    report.failed += 1;
                    tracing::warn!("Job board application {} not imported: {}", app.id, e);
                }
            }
        }
        Ok(report)
    }

    async fn import(&self, app: &PortalApplication) -> Result<()> {
        let name = clean(&app.name).unwrap_or_else(|| format!("Кандидат с сайта #{}", app.id));
        // `candidates.email` is required and unique.
        let email = clean(&app.email)
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| format!("job-koinotinav-{}@no-email.invalid", app.id));
        let phone = clean(&app.phone);
        let dob = app
            .birthdate
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d.get(..10).unwrap_or(d), "%Y-%m-%d").ok());
        let received = received_at(app);
        let (vacancy_id, vacancy_title) = match &app.vacancy {
            Some(v) => (v.id, clean(&v.title)),
            None => (NO_VACANCY_ID, Some(NO_VACANCY_TITLE.to_string())),
        };

        // Fetched before the transaction: if the database step fails, the
        // application stays unrecorded and the next run tries again.
        let cv_url = match &app.resume {
            Some(resume) => self.store_resume(app.id, resume.name.as_deref()).await,
            None => None,
        };

        let mut tx = self.pool.begin().await?;
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM candidates \
             WHERE lower(email) = $1 OR ($2::text IS NOT NULL AND phone = $2) \
             ORDER BY (lower(email) = $1) DESC, created_at ASC LIMIT 1",
        )
        .bind(&email)
        .bind(&phone)
        .fetch_optional(&mut *tx)
        .await?;

        let candidate_id = match existing {
            Some(id) => {
                // The CV sent with this application is the most recent one.
                sqlx::query(
                    "UPDATE candidates SET cv_url = COALESCE($2, cv_url), \
                     phone = COALESCE(phone, $3), dob = COALESCE(dob, $4), \
                     vacancy_id = CASE WHEN $5 > 0 THEN $5 ELSE vacancy_id END, \
                     updated_at = NOW() WHERE id = $1",
                )
                .bind(id)
                .bind(&cv_url)
                .bind(&phone)
                .bind(dob)
                .bind(vacancy_id)
                .execute(&mut *tx)
                .await?;
                id
            }
            None => {
                let profile = json!({
                    "source": "job.koinotinav.tj",
                    "portal_application_id": app.id,
                    "form": app.source,
                    "city": app.city,
                    "about": app.about,
                    "locale": app.locale,
                    "consent": app.consent,
                });
                sqlx::query_scalar(
                    "INSERT INTO candidates (name, email, phone, cv_url, dob, vacancy_id, profile_data, status, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, 'new', $8) RETURNING id",
                )
                .bind(&name)
                .bind(&email)
                .bind(&phone)
                .bind(&cv_url)
                .bind(dob)
                .bind((vacancy_id > 0).then_some(vacancy_id))
                .bind(profile)
                .bind(received)
                .fetch_one(&mut *tx)
                .await?
            }
        };

        if vacancy_id > 0 {
            sqlx::query(
                "INSERT INTO candidate_applications (candidate_id, vacancy_id, created_at) \
                 VALUES ($1, $2, $3) ON CONFLICT (candidate_id, vacancy_id) DO NOTHING",
            )
            .bind(candidate_id)
            .bind(vacancy_id)
            .bind(received)
            .execute(&mut *tx)
            .await?;
        }

        // A repeat application to the same vacancy keeps the existing card and
        // its stage; HR sees it once.
        let response_id: Uuid = sqlx::query_scalar(
            "INSERT INTO responses (candidate_id, vacancy_id, vacancy_title, responded_at) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (candidate_id, vacancy_id) \
             DO UPDATE SET vacancy_title = COALESCE(responses.vacancy_title, EXCLUDED.vacancy_title) \
             RETURNING id",
        )
        .bind(candidate_id)
        .bind(vacancy_id)
        .bind(&vacancy_title)
        .bind(received)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO koinotinav_applications \
             (application_id, candidate_id, response_id, vacancy_id, application_created_at) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(app.id)
        .bind(candidate_id)
        .bind(response_id)
        .bind(vacancy_id)
        .bind(received)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        tracing::info!(
            "Imported job board application {} for vacancy {} as response {}",
            app.id,
            vacancy_id,
            response_id
        );
        Ok(())
    }

    /// Saves the resume next to the Telegram CVs. A file that cannot be fetched
    /// or is of a type we do not accept is skipped; the application is still imported.
    async fn store_resume(&self, application_id: i64, file_name: Option<&str>) -> Option<String> {
        let file_name = file_name.filter(|n| !n.trim().is_empty()).unwrap_or("resume.pdf");
        let bytes = match self.board.fetch_resume(application_id).await {
            Ok(Some(bytes)) if !bytes.is_empty() => bytes,
            Ok(_) => return None,
            Err(e) => {
                tracing::warn!("Resume of job board application {} not fetched: {}", application_id, e);
                return None;
            }
        };
        match crate::routes::candidate_routes::save_cv_file(file_name, &bytes).await {
            Ok(path) => Some(path),
            Err(e) => {
                tracing::warn!("Resume of job board application {} not saved: {}", application_id, e);
                None
            }
        }
    }
}

fn clean(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|s| !s.is_empty())
}

fn received_at(app: &PortalApplication) -> DateTime<Utc> {
    app.created_at
        .as_deref()
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}
