use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use uuid::Uuid;

pub const FORM_TYPE_INTERVIEW_1: &str = "interview_1";
pub const FORM_TYPE_INTERVIEW_2: &str = "interview_2";
pub const FORM_TYPE_UNKNOWN: &str = "unknown";

pub fn is_known_form_type(value: &str) -> bool {
    matches!(value, FORM_TYPE_INTERVIEW_1 | FORM_TYPE_INTERVIEW_2)
}

pub const JOB_STATUS_PENDING: &str = "pending";
pub const JOB_STATUS_PROCESSING: &str = "processing";
pub const JOB_STATUS_COMPLETED: &str = "completed";
pub const JOB_STATUS_PARTIAL: &str = "partial";
pub const JOB_STATUS_FAILED: &str = "failed";
pub const CALLBACK_SKIPPED: &str = "skipped";
pub const CALLBACK_PENDING: &str = "pending";
pub const CALLBACK_DELIVERED: &str = "delivered";
pub const CALLBACK_FAILED: &str = "failed";
pub const CRITICAL_FIELD_PATHS: &[&str] = &[
    "candidate_name",
    "interview_date",
    "scheduled_start_time",
    "actual_arrival_time",
    "interview_from",
    "interview_to",
    "parameters.salary_expectation",
    "parameters.planned_start_date",
];

pub const FIELD_REVIEW_THRESHOLD: f32 = 0.75;
pub const OVERALL_REVIEW_THRESHOLD: f32 = 0.80;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InterviewFormJob {
    pub id: Uuid,
    pub status: String,
    pub source_urls: JsonValue,
    pub candidate_id: Option<Uuid>,
    pub vacancy_id: Option<i64>,
    pub form_type_hint: Option<String>,
    pub callback_url: Option<String>,
    pub callback_status: String,
    pub callback_attempts: i32,
    pub callback_last_error: Option<String>,
    pub error: Option<String>,
    pub external_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InterviewFormRecognition {
    pub id: Uuid,
    pub job_id: Uuid,
    pub candidate_id: Option<Uuid>,
    pub source_url: String,
    pub source_index: i32,
    pub image_sha256: Option<String>,
    pub form_type: String,
    pub fields: JsonValue,
    pub field_confidence: JsonValue,
    pub overall_confidence: Option<f32>,
    pub needs_review: bool,
    pub low_confidence_fields: JsonValue,
    pub warnings: JsonValue,
    pub raw_model_output: Option<JsonValue>,
    pub corrected_fields: Option<JsonValue>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillRow {
    #[serde(default)]
    pub index: i32,
    #[serde(default)]
    pub prof_soft_skills: Option<String>,
    #[serde(default)]
    pub personal_qualities: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParameterRow {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionBlock {
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecognizedForm {
    #[serde(default)]
    pub form_type: String,
    #[serde(default)]
    pub interviewers: Vec<String>,
    #[serde(default)]
    pub interviewer_position: Option<String>,
    #[serde(default)]
    pub department: Option<String>,
    #[serde(default)]
    pub division: Option<String>,
    #[serde(default)]
    pub interview_date: Option<String>,
    #[serde(default)]
    pub candidate_name: Option<String>,
    #[serde(default)]
    pub candidate_age: Option<i32>,
    #[serde(default)]
    pub position_discussed: Option<String>,
    #[serde(default)]
    pub scheduled_start_time: Option<String>,
    #[serde(default)]
    pub actual_arrival_time: Option<String>,
    #[serde(default)]
    pub interview_from: Option<String>,
    #[serde(default)]
    pub interview_to: Option<String>,
    #[serde(default)]
    pub parameters: Vec<ParameterRow>,
    #[serde(default)]
    pub strengths: Vec<SkillRow>,
    #[serde(default)]
    pub growth_areas: Vec<SkillRow>,
    #[serde(default)]
    pub comments: Option<String>,
    #[serde(default)]
    pub conclusions: Option<String>,
    #[serde(default)]
    pub hr_department_recommendation: Option<String>,
    #[serde(default)]
    pub test_results_bud: Option<String>,
    #[serde(default)]
    pub test_results_fed: Option<String>,
    #[serde(default)]
    pub extra_notes: Vec<String>,
    #[serde(default)]
    pub requester_decision: Option<DecisionBlock>,
    #[serde(default)]
    pub hr_decision: Option<DecisionBlock>,
    #[serde(default)]
    pub field_confidence: std::collections::BTreeMap<String, f32>,
    #[serde(default)]
    pub warnings: Vec<String>,
}
