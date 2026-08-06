use crate::error::Result;
use crate::models::response::{Response, ResponseCard};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UngradedResponse {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub vacancy_id: i64,
}

#[derive(Clone)]
pub struct ResponseService {
    pool: PgPool,
}

impl ResponseService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self) -> Result<Vec<ResponseCard>> {
        let rows = sqlx::query_as::<_, ResponseCard>(
            r#"
            SELECT
                r.id, r.candidate_id,
                c.name  AS candidate_name,
                c.email AS candidate_email,
                c.phone AS candidate_phone,
                c.cv_url AS candidate_cv_url,
                c.telegram_id,
                r.vacancy_id, r.vacancy_title, r.status,
                r.ai_grade, r.ai_comment, r.ai_graded_at, r.hr_comment,
                r.test_attempt_id, r.decision, r.responded_at, r.updated_at
            FROM responses r
            JOIN candidates c ON c.id = r.candidate_id
            ORDER BY r.responded_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: Uuid) -> Result<Option<Response>> {
        let row = sqlx::query_as::<_, Response>("SELECT * FROM responses WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn update(
        &self,
        id: Uuid,
        status: Option<String>,
        hr_comment: Option<String>,
        decision: Option<String>,
        test_attempt_id: Option<Uuid>,
    ) -> Result<Option<Response>> {
        let row = sqlx::query_as::<_, Response>(
            r#"
            UPDATE responses SET
                status          = COALESCE($2, status),
                hr_comment      = COALESCE($3, hr_comment),
                decision        = CASE
                                    WHEN $4::text IS NOT NULL THEN $4
                                    WHEN $2 IS NOT NULL AND $2 <> 'final_decision' THEN NULL
                                    ELSE decision
                                  END,
                test_attempt_id = COALESCE($5, test_attempt_id),
                updated_at      = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(hr_comment)
        .bind(decision)
        .bind(test_attempt_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn reconcile_missing(&self) -> Result<u64> {
        let res = sqlx::query(
            r#"
            INSERT INTO responses (candidate_id, vacancy_id, vacancy_title, responded_at)
            SELECT ca.candidate_id, ca.vacancy_id, NULL, COALESCE(ca.created_at, NOW())
            FROM candidate_applications ca
            LEFT JOIN responses r
              ON r.candidate_id = ca.candidate_id AND r.vacancy_id = ca.vacancy_id
            WHERE r.id IS NULL
            ON CONFLICT (candidate_id, vacancy_id) DO NOTHING
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    pub async fn claim_ungraded(&self, limit: i64) -> Result<Vec<UngradedResponse>> {
        let rows = sqlx::query_as::<_, UngradedResponse>(
            "SELECT id, candidate_id, vacancy_id FROM responses \
             WHERE ai_graded_at IS NULL ORDER BY responded_at ASC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn set_ai_result(
        &self,
        id: Uuid,
        grade: i32,
        comment: String,
        vacancy_title: Option<String>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE responses SET ai_grade = $2, ai_comment = $3, \
             vacancy_title = COALESCE($4, vacancy_title), ai_graded_at = NOW(), updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(id)
        .bind(grade)
        .bind(comment)
        .bind(vacancy_title)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_ai_failed(&self, id: Uuid, note: Option<String>) -> Result<()> {
        sqlx::query(
            "UPDATE responses SET ai_graded_at = NOW(), \
             ai_comment = COALESCE(ai_comment, $2), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
