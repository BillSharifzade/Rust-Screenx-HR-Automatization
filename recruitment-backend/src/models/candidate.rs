use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Candidate {
    pub id: Uuid,
    pub telegram_id: Option<i64>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub cv_url: Option<String>,
    pub dob: Option<chrono::NaiveDate>,
    pub vacancy_id: Option<i64>,
    pub profile_data: Option<JsonValue>,
    pub ai_rating: Option<i32>,
    pub ai_comment: Option<String>,
    pub status: String,
    pub unread_messages: Option<i64>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub event_type: String,
    pub title: String,
    pub description: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub status: Option<String>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CandidateApplication {
    pub id: i32,
    pub candidate_id: Uuid,
    pub vacancy_id: i64,
    pub created_at: Option<DateTime<Utc>>,
}
