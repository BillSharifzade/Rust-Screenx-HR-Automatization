use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub telegram_id: i64,
    pub direction: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessage {
    pub candidate_id: Uuid,
    pub telegram_id: i64,
    pub direction: String,
    pub text: String,
}
