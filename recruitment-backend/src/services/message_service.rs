use sqlx::PgPool;
use uuid::Uuid;
use crate::error::Result;
use crate::models::message::{Message, CreateMessage};

#[derive(Clone)]
pub struct MessageService {
    pool: PgPool,
}

impl MessageService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Save a new message (inbound or outbound)
    pub async fn create(&self, msg: CreateMessage) -> Result<Message> {
        let message = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (candidate_id, telegram_id, direction, text)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#
        )
        .bind(msg.candidate_id)
        .bind(msg.telegram_id)
        .bind(&msg.direction)
        .bind(&msg.text)
        .fetch_one(&self.pool)
        .await?;

        Ok(message)
    }

    /// Get all messages for a candidate, ordered by time
    pub async fn get_by_candidate(&self, candidate_id: Uuid) -> Result<Vec<Message>> {
        let messages = sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM messages
            WHERE candidate_id = $1
            ORDER BY created_at ASC
            "#
        )
        .bind(candidate_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }

    /// Mark all unread inbound messages as read for a candidate
    pub async fn mark_as_read(&self, candidate_id: Uuid) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE messages
            SET read_at = NOW()
            WHERE candidate_id = $1 AND direction = 'inbound' AND read_at IS NULL
            "#
        )
        .bind(candidate_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get count of unread messages for a candidate
    pub async fn unread_count(&self, candidate_id: Uuid) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM messages
            WHERE candidate_id = $1 AND direction = 'inbound' AND read_at IS NULL
            "#
        )
        .bind(candidate_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0)
    }

    /// Get total unread count across all candidates (for admin dashboard)
    pub async fn total_unread_count(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM messages
            WHERE direction = 'inbound' AND read_at IS NULL
            "#
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0)
    }
}
