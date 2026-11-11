//! Imports candidate applications left on job.koinotinav.tj into the responses
//! board, so HR reviews them next to the Telegram ones.
//!
//! Each application becomes a candidate (matched by email, then phone) plus a
//! response for its vacancy; the AI grading worker picks the response up like
//! any other. Imported application ids are recorded in `koinotinav_applications`
//! and never imported twice, even if HR later deletes the candidate.
//!
//! Once graded, each one is forwarded to 1F as a `new_application` event, the
//! same one the Telegram bot sends. Delivery is recorded and retried.

use std::collections::HashSet;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;
use crate::services::koinotinav_service::{KoinotinavService, PortalApplication};
use crate::services::onef_service::{
    OneFApplicationPayload, OneFCandidateInfo, OneFService, SOURCE_JOB_SITE,
};

/// `responses.vacancy_id` for a general application not tied to a vacancy.
pub const NO_VACANCY_ID: i64 = 0;
const NO_VACANCY_TITLE: &str = "Без вакансии (анкета с сайта)";
/// Re-read this far behind the newest imported application, so one that was
/// saved on the board with a slightly older timestamp is not missed.
const SINCE_OVERLAP_HOURS: i64 = 24;
/// 1F gets the event with the AI grade; if grading has not finished by then,
/// it is sent without one.
const ONEF_GRADE_WAIT_MINUTES: i64 = 10;
const ONEF_RETRY_MINUTES: i64 = 15;
const ONEF_MAX_ATTEMPTS: i32 = 12;

#[derive(Clone)]
pub struct PortalApplicationService {
    pool: PgPool,
    board: KoinotinavService,
    onef: OneFService,
    webapp_url: String,
}

#[derive(Debug, Default)]
pub struct ForwardReport {
    pub sent: usize,
    pub failed: usize,
}

#[derive(sqlx::FromRow)]
struct PendingForward {
    response_id: Uuid,
    vacancy_id: i64,
    vacancy_title: Option<String>,
    application_created_at: DateTime<Utc>,
    ai_grade: Option<i32>,
    ai_comment: Option<String>,
    candidate_id: Uuid,
    telegram_id: Option<i64>,
    name: String,
    email: String,
    phone: Option<String>,
    dob: Option<NaiveDate>,
    cv_url: Option<String>,
    city: Option<String>,
    about: Option<String>,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub fetched: usize,
    pub imported: usize,
    pub failed: usize,
}

impl PortalApplicationService {
    pub fn new(pool: PgPool, board: KoinotinavService, onef: OneFService, webapp_url: String) -> Self {
        Self {
            pool,
            board,
            onef,
            webapp_url: webapp_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.board.has_api_key()
    }

    /// Sends graded, not yet delivered applications to 1F, oldest first.
    ///
    /// 1F rejects an application whose vacancy it does not know ("Задача 0 не
    /// существует"), so one waits until 1F's catalogue (`onef_vacancies`, synced
    /// every few minutes) links its vacancy, then goes out on the next pass.
    /// Without a fresh catalogue everything is tried as is.
    pub async fn forward_to_onef(&self) -> Result<ForwardReport> {
        let mut report = ForwardReport::default();
        if !self.onef.is_enabled() {
            return Ok(report);
        }
        let mut pending: Vec<PendingForward> = sqlx::query_as(
            "SELECT DISTINCT ON (k.response_id) \
                k.response_id, k.vacancy_id, r.vacancy_title, k.application_created_at, \
                r.ai_grade, r.ai_comment, c.id AS candidate_id, c.telegram_id, c.name, c.email, \
                c.phone, c.dob, c.cv_url, \
                c.profile_data->>'city' AS city, c.profile_data->>'about' AS about \
             FROM koinotinav_applications k \
             JOIN responses r ON r.id = k.response_id \
             JOIN candidates c ON c.id = r.candidate_id \
             WHERE k.onef_sent_at IS NULL \
               AND k.onef_attempts < $1 \
               AND (k.onef_last_attempt_at IS NULL \
                    OR k.onef_last_attempt_at < NOW() - make_interval(mins => $2)) \
               AND (r.ai_graded_at IS NOT NULL \
                    OR k.imported_at < NOW() - make_interval(mins => $3)) \
               AND (NOT EXISTS (SELECT 1 FROM onef_vacancies \
                                WHERE last_seen_at > NOW() - INTERVAL '1 day') \
                    OR EXISTS (SELECT 1 FROM onef_vacancies o \
                               WHERE o.is_active AND o.external_vacancy_id = k.vacancy_id::text)) \
             ORDER BY k.response_id, k.application_created_at",
        )
        .bind(ONEF_MAX_ATTEMPTS)
        .bind(ONEF_RETRY_MINUTES as i32)
        .bind(ONEF_GRADE_WAIT_MINUTES as i32)
        .fetch_all(&self.pool)
        .await?;
        pending.sort_by_key(|p| p.application_created_at);

        for p in pending {
            let response_id = p.response_id;
            let outcome = self.onef.deliver_application(self.onef_payload(p)).await;
            let error = outcome.as_ref().err().cloned();
            sqlx::query(
                "UPDATE koinotinav_applications SET \
                    onef_sent_at = CASE WHEN $2::text IS NULL THEN NOW() END, \
                    onef_attempts = onef_attempts + 1, \
                    onef_last_attempt_at = NOW(), \
                    onef_last_error = $2 \
                 WHERE response_id = $1 AND onef_sent_at IS NULL",
            )
            .bind(response_id)
            .bind(&error)
            .execute(&self.pool)
            .await?;
            match error {
                None => report.sent += 1,
                Some(e) => {
                    report.failed += 1;
                    tracing::warn!("Response {} not delivered to 1F: {}", response_id, e);
                }
            }
        }
        Ok(report)
    }

    fn onef_payload(&self, p: PendingForward) -> OneFApplicationPayload {
        let fullname = p.name.trim().to_string();
        let (first, last) = match fullname.split_once(' ') {
            Some((first, rest)) => (first.to_string(), rest.trim().to_string()),
            None => (fullname.clone(), String::new()),
        };
        OneFApplicationPayload {
            event_type: "new_application".into(),
            vacancy_id: p.vacancy_id,
            vacancy_name: p
                .vacancy_title
                .unwrap_or_else(|| format!("Vacancy #{}", p.vacancy_id)),
            candidate: OneFCandidateInfo {
                id: p.candidate_id.to_string(),
                telegram_id: p.telegram_id,
                fullname,
                name: first,
                surname: last,
                email: p.email,
                phone: p.phone,
                dob: p.dob.map(|d| d.format("%Y-%m-%d").to_string()),
                cv_url: p.cv_url.map(|path| format!("{}/{}", self.webapp_url, path.trim_start_matches('/'))),
                ai_rating: p.ai_grade,
                // Without a grade the comment is only "AI grading unavailable";
                // the Telegram flow sends nothing in that case either.
                ai_comment: p.ai_grade.and(p.ai_comment),
                city: p.city.filter(|s| !s.trim().is_empty()),
                about: p.about.filter(|s| !s.trim().is_empty()),
            },
            // Same shape as the Telegram flow's `Utc::now().to_rfc3339()`
            // ("…:10.075543123+00:00"). 1F's script cannot parse whole seconds
            // ("…:10+00:00"): "Can not parse string 1793776090 to date with time".
            applied_at: p
                .application_created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, false),
            source: Some(SOURCE_JOB_SITE),
        }
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
