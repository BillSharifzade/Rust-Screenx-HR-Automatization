use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// The 7 pipeline stages, in order. The kanban renders one column per stage.
pub const RESPONSE_STAGES: &[&str] = &[
    "cv_screening",
    "phone_interview",
    "interview_1",
    "test_task",
    "presentation",
    "interview_2",
    "final_decision",
];

pub fn is_valid_stage(stage: &str) -> bool {
    RESPONSE_STAGES.contains(&stage)
}

/// A response = one candidate's application to one vacancy, progressing through the funnel.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Response {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub vacancy_id: i64,
    pub vacancy_title: Option<String>,
    pub status: String,
    pub ai_grade: Option<i32>,
    pub ai_comment: Option<String>,
    pub ai_graded_at: Option<DateTime<Utc>>,
    pub hr_comment: Option<String>,
    pub test_attempt_id: Option<Uuid>,
    pub decision: Option<String>,
    pub responded_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Denormalized row for the kanban board: a response joined with its candidate.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ResponseCard {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub candidate_name: String,
    pub candidate_email: String,
    pub candidate_phone: Option<String>,
    pub candidate_cv_url: Option<String>,
    pub telegram_id: Option<i64>,
    pub vacancy_id: i64,
    pub vacancy_title: Option<String>,
    pub status: String,
    pub ai_grade: Option<i32>,
    pub ai_comment: Option<String>,
    pub hr_comment: Option<String>,
    pub test_attempt_id: Option<Uuid>,
    pub decision: Option<String>,
    pub responded_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
