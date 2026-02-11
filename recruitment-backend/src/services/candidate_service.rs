use crate::models::candidate::{Candidate, CandidateApplication};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use anyhow::Result;

#[derive(Clone)]
pub struct CandidateService {
    pool: PgPool,
}

impl CandidateService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_by_telegram_id(&self, telegram_id: i64) -> Result<Option<Candidate>> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            SELECT id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at,
            (SELECT COUNT(*) FROM messages m WHERE m.candidate_id = candidates.id AND m.read_at IS NULL AND m.direction = 'inbound') as unread_messages
            FROM candidates 
            WHERE telegram_id = $1
            "#,
            telegram_id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn get_candidate(&self, id: uuid::Uuid) -> Result<Option<Candidate>> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            SELECT id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at,
            (SELECT COUNT(*) FROM messages m WHERE m.candidate_id = candidates.id AND m.read_at IS NULL AND m.direction = 'inbound') as unread_messages
            FROM candidates 
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn get_by_email(&self, email: &str) -> Result<Option<Candidate>> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            SELECT id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at,
            (SELECT COUNT(*) FROM messages m WHERE m.candidate_id = candidates.id AND m.read_at IS NULL AND m.direction = 'inbound') as unread_messages
            FROM candidates 
            WHERE email = $1
            "#,
            email
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn create_candidate(
        &self,
        telegram_id: Option<i64>,
        name: String,
        email: String,
        phone: Option<String>,
        cv_url: Option<String>,
        dob: Option<chrono::NaiveDate>,
        vacancy_id: Option<i64>,
        profile_data: Option<JsonValue>,
    ) -> Result<Candidate> {
        if let Some(tg_id) = telegram_id {
            let exists = sqlx::query!("SELECT id FROM candidates WHERE telegram_id = $1", tg_id)
                .fetch_optional(&self.pool)
                .await?;
            if exists.is_some() {
                return Err(anyhow::anyhow!("A candidate with this Telegram ID already exists."));
            }
        }

        let exists_email = sqlx::query!("SELECT id FROM candidates WHERE email = $1", email)
            .fetch_optional(&self.pool)
            .await?;
        if exists_email.is_some() {
            return Err(anyhow::anyhow!("A candidate with this email address already exists."));
        }

        if let Some(ref ph) = phone {
            if !ph.is_empty() {
                let exists_phone = sqlx::query!("SELECT id FROM candidates WHERE phone = $1", ph)
                    .fetch_optional(&self.pool)
                    .await?;
                if exists_phone.is_some() {
                    return Err(anyhow::anyhow!("A candidate with this phone number already exists."));
                }
            }
        }

        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            INSERT INTO candidates (telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'new')
            RETURNING id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at, 0::bigint as "unread_messages!"
            "#,
            telegram_id,
            name,
            email,
            phone,
            cv_url,
            dob,
            vacancy_id,
            profile_data
        )
        .fetch_one(&self.pool)
        .await?;
        if let Some(vid) = vacancy_id {
            let _ = self.apply_to_vacancy(candidate.id, vid).await;
        }

        Ok(candidate)
    }

    pub async fn update_cv(&self, id: uuid::Uuid, cv_url: String) -> Result<Candidate> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            UPDATE candidates
            SET cv_url = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at, 0::bigint as "unread_messages!"
            "#,
            cv_url,
            id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn list_candidates(&self) -> Result<Vec<Candidate>> {
        let candidates = sqlx::query_as!(
            Candidate,
            r#"
            SELECT id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at,
            (SELECT COUNT(*) FROM messages m WHERE m.candidate_id = candidates.id AND m.read_at IS NULL AND m.direction = 'inbound') as unread_messages
            FROM candidates 
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(candidates)
    }

    pub async fn apply_to_vacancy(&self, candidate_id: uuid::Uuid, vacancy_id: i64) -> Result<CandidateApplication> {
        let application = sqlx::query_as!(
            CandidateApplication,
            r#"
            INSERT INTO candidate_applications (candidate_id, vacancy_id)
            VALUES ($1, $2)
            ON CONFLICT (candidate_id, vacancy_id) DO UPDATE SET vacancy_id = EXCLUDED.vacancy_id -- dummy update to return row
            RETURNING id, candidate_id, vacancy_id, created_at
            "#,
            candidate_id,
            vacancy_id
        )
        .fetch_one(&self.pool)
        .await?;
        
        Ok(application)
    }

    pub async fn get_candidate_applications(&self, candidate_id: uuid::Uuid) -> Result<Vec<CandidateApplication>> {
        let applications = sqlx::query_as!(
            CandidateApplication,
            r#"SELECT id, candidate_id, vacancy_id, created_at FROM candidate_applications WHERE candidate_id = $1 ORDER BY created_at DESC"#,
            candidate_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(applications)
    }

    pub async fn get_vacancy_candidates(&self, vacancy_id: i64) -> Result<Vec<Candidate>> {
        let candidates = sqlx::query_as!(
            Candidate,
            r#"
            SELECT c.id, c.telegram_id, c.name, c.email, c.phone, c.cv_url, c.dob, c.vacancy_id, c.profile_data, c.ai_rating, c.ai_comment, c.status, c.created_at, c.updated_at,
            (SELECT COUNT(*) FROM messages m WHERE m.candidate_id = c.id AND m.read_at IS NULL AND m.direction = 'inbound') as unread_messages
            FROM candidates c
            JOIN candidate_applications ca ON c.id = ca.candidate_id
            WHERE ca.vacancy_id = $1
            ORDER BY ca.created_at DESC
            "#,
            vacancy_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(candidates)
    }

    pub async fn update_ai_suitability(&self, id: uuid::Uuid, rating: i32, comment: String) -> Result<Candidate> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            UPDATE candidates
            SET ai_rating = $1, ai_comment = $2, updated_at = NOW()
            WHERE id = $3
            RETURNING id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at, 0::bigint as "unread_messages!"
            "#,
            rating,
            comment,
            id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn update_status(&self, id: uuid::Uuid, status: String) -> Result<Candidate> {
        let candidate = sqlx::query_as!(
            Candidate,
            r#"
            UPDATE candidates
            SET status = $1, updated_at = NOW()
            WHERE id = $2
            RETURNING id, telegram_id, name, email, phone, cv_url, dob, vacancy_id, profile_data, ai_rating, ai_comment, status, created_at, updated_at, 0::bigint as "unread_messages!"
            "#,
            status,
            id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(candidate)
    }

    pub async fn get_status_counts(&self) -> Result<std::collections::HashMap<String, i64>> {
        let rows = sqlx::query!(
            r#"
            SELECT status, COUNT(*) as count
            FROM candidates
            GROUP BY status
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut counts = std::collections::HashMap::new();
        for row in rows {
            counts.insert(row.status, row.count.unwrap_or(0));
        }
        Ok(counts)
    }

    pub async fn get_history_counts(&self) -> Result<Vec<(String, i64)>> {
        let rows = sqlx::query!(
            r#"
            SELECT TO_CHAR(created_at, 'YYYY-MM-DD') as "date!", COUNT(*) as "count!"
            FROM candidates
            WHERE created_at > NOW() - INTERVAL '7 days'
            GROUP BY TO_CHAR(created_at, 'YYYY-MM-DD')
            ORDER BY "date!"
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.date, r.count)).collect())
    }
}
