use crate::error::Result;
use crate::models::test::Test;
use crate::models::test_attempt::TestAttempt;
use crate::utils::token::generate_access_token;
use crate::dto::public_dto::{SaveAnswerRequest, SubmitTestRequest};
use crate::models::question::Question;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AttemptService {
    pool: PgPool,
}

impl AttemptService {
    pub fn new(pool: PgPool) -> Self { Self { pool } }

    pub async fn create_invite(
        &self,
        test_id: Uuid,
        candidate: InviteCandidate,
        expires_in_hours: i64,
        metadata: Option<serde_json::Value>,
    ) -> Result<CreateInviteResult> {
        let pending_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM test_attempts WHERE candidate_email = $1 AND status = 'pending'"#
        )
        .bind(&candidate.email)
        .fetch_one(&self.pool)
        .await?;

        if pending_count > 0 {
            return Err(crate::error::Error::BadRequest(
                "Candidate already has a pending test invitation. They must start or complete existing tests before receiving new invitations.".to_string()
            ));
        }

        let test = sqlx::query_as!(
            Test,
            r#"SELECT 
                id, external_id, title, description, instructions, 
                questions as "questions: serde_json::Value",
                duration_minutes,
                passing_score as "passing_score: rust_decimal::Decimal",
                max_attempts, shuffle_questions, shuffle_options, show_results_immediately,
                created_by, is_active, 
                test_type, presentation_themes as "presentation_themes: serde_json::Value",
                presentation_extra_info,
                created_at, updated_at
            FROM tests WHERE id = $1"#,
            test_id
        )
        .fetch_one(&self.pool)
        .await?;

        let access_token = generate_access_token(32);
        let expires_at: DateTime<Utc> = Utc::now() + Duration::hours(expires_in_hours);

        let mut questions_snapshot = test.questions.clone();
        if test.test_type == "presentation" {
            questions_snapshot = json!({
                "test_type": "presentation",
                "themes": test.presentation_themes,
                "extra_info": test.presentation_extra_info
            });
        }

        let attempt = sqlx::query_as::<_, TestAttempt>(
            r#"
            INSERT INTO test_attempts (
                test_id, candidate_external_id, candidate_name, candidate_email, candidate_telegram_id, candidate_phone,
                access_token, expires_at, questions_snapshot, answers, score, max_score, percentage, passed,
                started_at, completed_at, time_spent_seconds, status, ip_address, user_agent, tab_switches, suspicious_activity, metadata
            ) VALUES (
                $1, $2, $3, $4, $5, $6,
                $7, $8, $9, NULL, NULL, NULL, NULL, NULL,
                NULL, NULL, NULL, 'pending', NULL, NULL, 0, NULL, $10
            )
            RETURNING *
            "#
        )
        .bind(test.id)
        .bind(candidate.external_id)
        .bind(candidate.name)
        .bind(candidate.email)
        .bind(candidate.telegram_id)
        .bind(candidate.phone)
        .bind(access_token.clone())
        .bind(expires_at)
        .bind(questions_snapshot)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?;

        Ok(CreateInviteResult {
            attempt_id: attempt.id,
            access_token: attempt.access_token,
            expires_at,
            status: attempt.status,
        })
    }

    pub async fn get_attempt_and_test_by_token(&self, token: &str) -> Result<(TestAttempt, Test)> {
        let attempt = sqlx::query_as::<_, TestAttempt>(
            r#"SELECT * FROM test_attempts WHERE access_token = $1"#
        )
        .bind(token)
        .fetch_one(&self.pool)
        .await?;

        let test = sqlx::query_as!(
            Test,
            r#"SELECT 
                id, external_id, title, description, instructions, 
                questions as "questions: serde_json::Value",
                duration_minutes,
                passing_score as "passing_score: rust_decimal::Decimal",
                max_attempts, shuffle_questions, shuffle_options, show_results_immediately,
                created_by, is_active, 
                test_type, presentation_themes as "presentation_themes: serde_json::Value",
                presentation_extra_info,
                created_at, updated_at
            FROM tests WHERE id = $1"#,
            attempt.test_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((attempt, test))
    }

    pub async fn start_attempt_by_token(&self, token: &str) -> Result<TestAttempt> {
        let (attempt, test) = self.get_attempt_and_test_by_token(token).await?;

        let now = Utc::now();
        let expires_candidate = now + Duration::minutes(test.duration_minutes as i64);
        let new_expires = if expires_candidate < attempt.expires_at { expires_candidate } else { attempt.expires_at };

        let updated = sqlx::query_as::<_, TestAttempt>(
            r#"
            UPDATE test_attempts
            SET status = 'in_progress', started_at = COALESCE(started_at, $1), expires_at = $2
            WHERE access_token = $3
            RETURNING *
            "#
        )
        .bind(now)
        .bind(new_expires)
        .bind(token)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    pub async fn save_answer_by_token(&self, token: &str, req: SaveAnswerRequest) -> Result<DateTime<Utc>> {
        let (mut attempt, _test) = self.get_attempt_and_test_by_token(token).await?;
        let timestamp = Utc::now();

        sqlx::query!(
            r#"INSERT INTO answer_logs (attempt_id, question_id, answer_value, time_spent_seconds) VALUES ($1, $2, $3, $4)"#,
            attempt.id,
            req.question_id,
            req.answer,
            req.time_spent_seconds
        )
        .execute(&self.pool)
        .await?;

        let mut answers: Vec<serde_json::Value> = match attempt.answers.take() {
            Some(v) => serde_json::from_value(v).unwrap_or_default(),
            None => Vec::new(),
        };

        let new_item = json!({
            "question_id": req.question_id,
            "answer": req.answer,
            "time_spent": req.time_spent_seconds,
            "marked_for_review": req.marked_for_review.unwrap_or(false),
            "answered_at": timestamp,
        });

        if let Some(pos) = answers.iter().position(|a| a.get("question_id").and_then(|v| v.as_i64()) == Some(req.question_id as i64)) {
            answers[pos] = new_item;
        } else {
            answers.push(new_item);
        }

        let answers_json = serde_json::to_value(answers)?;
        sqlx::query!(
            r#"UPDATE test_attempts SET answers = $1, updated_at = NOW() WHERE id = $2"#,
            answers_json,
            attempt.id
        )
        .execute(&self.pool)
        .await?;

        Ok(timestamp)
    }

    pub async fn submit_attempt_by_token(&self, token: &str, req: SubmitTestRequest) -> Result<(TestAttempt, f64, f64, f64, bool)> {
        let (attempt, test) = self.get_attempt_and_test_by_token(token).await?;

        let status = req.status.clone().unwrap_or_else(|| "completed".to_string());

        let answers_json = serde_json::to_value(&req.answers)?;
        sqlx::query!(
            r#"UPDATE test_attempts SET answers = $1 WHERE id = $2"#,
            answers_json,
            attempt.id
        )
        .execute(&self.pool)
        .await?;

        let questions: Vec<Question> = serde_json::from_value(test.questions.clone()).unwrap_or_default();
        let answers: Vec<serde_json::Value> = serde_json::from_value(answers_json.clone()).unwrap_or_default();
        let (earned_points, total_max_points, graded_answers, needs_review) = crate::services::grading_service::GradingService::grade_mcq_only(&questions, &answers);
        
        let mut final_status = status.clone();
        if needs_review && final_status == "completed" {
            final_status = "needs_review".to_string();
        }

        let score_f = earned_points as f64;
        let max_score_f = total_max_points as f64;
        let percentage = if max_score_f > 0.0 { (score_f / max_score_f) * 100.0 } else { 0.0 };
        let passing_threshold = test.passing_score.to_string().parse::<f64>().unwrap_or(0.0);
        let passed = percentage >= passing_threshold;

        let graded_json = serde_json::to_value(graded_answers)?;
        let now = Utc::now();
        let score_dec = Decimal::from_f64(score_f).unwrap_or_else(|| Decimal::new(0, 0));
        let max_score_dec = Decimal::from_f64(max_score_f).unwrap_or_else(|| Decimal::new(0, 0));
        let percentage_dec = Decimal::from_f64(percentage).unwrap_or_else(|| Decimal::new(0, 0));

        let updated = sqlx::query_as::<_, TestAttempt>(
            r#"
            UPDATE test_attempts
            SET status = $8, completed_at = $1, 
                time_spent_seconds = ROUND(EXTRACT(EPOCH FROM ($1 - started_at)))::integer,
                score = $2, max_score = $3, percentage = $4, passed = $5, graded_answers = $6
            WHERE id = $7
            RETURNING *
            "#
        )
        .bind(now)
        .bind(score_dec)
        .bind(max_score_dec)
        .bind(percentage_dec)
        .bind(passed)
        .bind(graded_json)
        .bind(attempt.id)
        .bind(final_status)
        .fetch_one(&self.pool)
        .await?;

        Ok((updated, score_f, max_score_f, percentage, passed))
    }

    pub async fn submit_presentation_by_token(
        &self,
        token: &str,
        presentation_link: Option<String>,
        file_path: Option<String>,
    ) -> Result<TestAttempt> {
        let (attempt, _test) = self.get_attempt_and_test_by_token(token).await?;

        let now = Utc::now();
        let updated = sqlx::query_as::<_, TestAttempt>(
            r#"
            UPDATE test_attempts
            SET status = 'needs_review', 
                completed_at = $1, 
                time_spent_seconds = ROUND(EXTRACT(EPOCH FROM ($1 - started_at)))::integer,
                presentation_submission_link = $2,
                presentation_submission_file_path = $3,
                updated_at = NOW()
            WHERE id = $4
            RETURNING *
            "#
        )
        .bind(now)
        .bind(presentation_link)
        .bind(file_path)
        .bind(attempt.id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }


    pub async fn get_attempt_by_id(&self, attempt_id: Uuid) -> Result<TestAttempt> {
        let attempt = sqlx::query_as::<_, TestAttempt>(
            r#"SELECT * FROM test_attempts WHERE id = $1"#
        )
        .bind(attempt_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(attempt)
    }

    pub async fn list_attempts(
        &self,
        test_id: Option<Uuid>,
        candidate_email: Option<String>,
        status: Option<String>,
        page: i64,
        limit: i64,
    ) -> Result<(Vec<TestAttempt>, i64)> {
        let offset = (page - 1) * limit;
        let rows = sqlx::query_as::<_, TestAttempt>(
            r#"
            SELECT * FROM test_attempts
            WHERE ($1::uuid IS NULL OR test_id = $1)
              AND ($2::text IS NULL OR candidate_email = $2)
              AND ($3::text IS NULL OR status = $3)
            ORDER BY created_at DESC
            LIMIT $4 OFFSET $5
            "#
        )
        .bind(test_id)
        .bind(candidate_email.clone())
        .bind(status.clone())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM test_attempts
               WHERE ($1::uuid IS NULL OR test_id = $1)
                 AND ($2::text IS NULL OR candidate_email = $2)
                 AND ($3::text IS NULL OR status = $3)"#,
            test_id,
            candidate_email,
            status
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((rows, total))
    }

    pub async fn delete_attempt(&self, attempt_id: Uuid) -> Result<()> {
        let attempt = self.get_attempt_by_id(attempt_id).await?;
        if attempt.status != "pending" {
            return Err(crate::error::Error::BadRequest(format!(
                "Cannot delete invitation with status '{}'. Only 'pending' invitations can be removed.",
                attempt.status
            )));
        }

        sqlx::query!(
            r#"DELETE FROM test_attempts WHERE id = $1"#,
            attempt_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
    pub async fn grade_presentation(
        &self,
        attempt_id: Uuid,
        grade: f64,
        comment: Option<String>,
        graded_by: Uuid,
    ) -> Result<TestAttempt> {
        let attempt = sqlx::query_as::<_, TestAttempt>(
            r#"
            UPDATE test_attempts
            SET 
                presentation_grade = $2,
                presentation_grade_comment = $3,
                graded_by = $4,
                graded_at = NOW(),
                score = $2,
                max_score = 100.0,
                percentage = $2,
                passed = ($2 >= t.passing_score),
                status = 'completed'
            FROM tests t
            WHERE test_attempts.id = $1 AND test_attempts.test_id = t.id
            RETURNING test_attempts.*
            "#
        )
        .bind(attempt_id)
        .bind(Decimal::from_f64_retain(grade).unwrap_or_default())
        .bind(comment)
        .bind(graded_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(attempt)
    }

    pub async fn grade_answer(&self, attempt_id: Uuid, question_id: i32, is_correct: bool) -> Result<TestAttempt> {
        let attempt = self.get_attempt_by_id(attempt_id).await?;
        let graded_val = attempt.graded_answers.clone().unwrap_or_else(|| serde_json::json!([]));
        let mut graded_answers: Vec<serde_json::Value> = serde_json::from_value(graded_val).unwrap_or_default();
        
        let mut found = false;
        let mut total_score = Decimal::new(0, 0);
        let mut max_score = Decimal::new(0, 0);

        for ans in graded_answers.iter_mut() {
            let q_id = ans.get("question_id").and_then(|v| v.as_i64());
            if q_id == Some(question_id as i64) {
                let max_pts = ans.get("max_points").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let earned_pts = if is_correct { max_pts } else { 0 };
                
                ans["points_earned"] = serde_json::json!(earned_pts);
                ans["is_correct"] = serde_json::json!(is_correct);
                ans["needs_review"] = serde_json::json!(false);
                found = true;
            }
            
            total_score += Decimal::from(ans.get("points_earned").and_then(|v| v.as_i64()).unwrap_or(0));
            max_score += Decimal::from(ans.get("max_points").and_then(|v| v.as_i64()).unwrap_or(0));
        }

        if !found {
            return Err(crate::error::Error::NotFound("Question answer not found in attempt".into()));
        }

        let percentage = if max_score > Decimal::ZERO { 
            (total_score / max_score) * Decimal::new(100, 0) 
        } else { 
            Decimal::ZERO 
        };
        
        let still_needs_review = graded_answers.iter().any(|ans| {
            ans.get("needs_review").and_then(|v| v.as_bool()).unwrap_or(false)
        });

        let test = sqlx::query!("SELECT passing_score FROM tests WHERE id = $1", attempt.test_id)
            .fetch_one(&self.pool)
            .await?;
        
        let passing_threshold = test.passing_score;
        let passed = percentage >= passing_threshold;

        let status = if still_needs_review { "needs_review" } else { "completed" };

        let updated = sqlx::query_as::<_, TestAttempt>(
            r#"
            UPDATE test_attempts
            SET status = $2, graded_answers = $3, score = $4, max_score = $5, percentage = $6, passed = $7, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#
        )
        .bind(attempt_id)
        .bind(status)
        .bind(serde_json::to_value(graded_answers)?)
        .bind(total_score)
        .bind(max_score)
        .bind(percentage)
        .bind(passed)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    pub async fn heartbeat(&self, token: &str) -> Result<()> {
        let now = Utc::now();
        sqlx::query!(
            "UPDATE test_attempts SET last_heartbeat_at = $1 WHERE access_token = $2",
            now,
            token
        )
        .execute(&self.pool)
        .await
        .map_err(|e| crate::error::Error::Internal(format!("Failed to update heartbeat: {}", e)))?;
        Ok(())
    }

    pub async fn check_deadlines(&self, notification_service: &crate::services::notification_service::NotificationService) -> Result<()> {
        let now = Utc::now();

        let warning_threshold = now + Duration::hours(1);
        let warnings = sqlx::query_as::<_, TestAttempt>(
            r#"
            SELECT ta.* 
            FROM test_attempts ta
            JOIN tests t ON ta.test_id = t.id
            WHERE ta.status = 'in_progress' 
              AND t.test_type = 'presentation'
              AND ta.expires_at <= $1
              AND ta.deadline_notified = FALSE
            "#
        )
        .bind(warning_threshold)
        .fetch_all(&self.pool)
        .await?;

        for attempt in warnings {
            let payload = json!({
                "event": "deadline_warning",
                "attempt_id": attempt.id,
                "candidate_name": attempt.candidate_name,
                "candidate_telegram_id": attempt.candidate_telegram_id,
                "expires_at": attempt.expires_at,
            });
            if let Err(e) = notification_service.enqueue_webhook("deadline_warning", &payload).await {
                tracing::error!("Failed to enqueue deadline warning: {:?}", e);
            } else {
                sqlx::query!("UPDATE test_attempts SET deadline_notified = TRUE WHERE id = $1", attempt.id)
                    .execute(&self.pool)
                    .await?;
            }
        }

        sqlx::query!(
            r#"
            UPDATE test_attempts
            SET status = 'timeout', 
                completed_at = expires_at,
                updated_at = NOW(),
                score = COALESCE(score, 0),
                max_score = COALESCE(max_score, 0),
                percentage = COALESCE(percentage, 0),
                passed = FALSE
            WHERE status IN ('pending', 'in_progress')
              AND expires_at <= $1
            "#,
            now
        )
        .execute(&self.pool)
        .await?;

        let abandon_threshold = now - Duration::minutes(2);
        sqlx::query!(
            r#"
            UPDATE test_attempts ta
            SET status = 'escaped',
                completed_at = $1,
                updated_at = $1
            FROM tests t
            WHERE ta.test_id = t.id
              AND ta.status = 'in_progress'
              AND t.test_type != 'presentation'
              AND ta.last_heartbeat_at IS NOT NULL
              AND ta.last_heartbeat_at < $2
            "#,
            now,
            abandon_threshold
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Report an anti-cheat violation (tab/window switch).
    /// Returns (current_tab_switches, terminated: bool).
    /// On the 2nd violation the test is auto-failed.
    pub async fn report_violation(&self, token: &str, violation_type: &str) -> Result<(i32, bool)> {
        let (attempt, _test) = self.get_attempt_and_test_by_token(token).await?;

        // Only act on in-progress attempts
        if attempt.status != "in_progress" {
            let current = attempt.tab_switches.unwrap_or(0);
            return Ok((current, attempt.status == "escaped"));
        }

        let new_count = attempt.tab_switches.unwrap_or(0) + 1;
        let now = Utc::now();

        // Build suspicious_activity log
        let mut activity: Vec<serde_json::Value> = attempt
            .suspicious_activity
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        activity.push(json!({
            "type": violation_type,
            "tab_switches": new_count,
            "timestamp": now.to_rfc3339(),
        }));

        let activity_json = serde_json::to_value(&activity)?;

        const MAX_VIOLATIONS: i32 = 2;
        let terminated = new_count >= MAX_VIOLATIONS;

        if terminated {
            // Auto-fail: set score to 0, status to 'escaped'
            sqlx::query!(
                r#"
                UPDATE test_attempts
                SET tab_switches = $1,
                    suspicious_activity = $2,
                    status = 'escaped',
                    completed_at = $3,
                    score = 0,
                    max_score = COALESCE(max_score, 0),
                    percentage = 0,
                    passed = FALSE,
                    updated_at = $3
                WHERE access_token = $4
                "#,
                new_count,
                activity_json,
                now,
                token
            )
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to terminate attempt: {}", e)))?;

            tracing::warn!(
                "Anti-cheat: Test auto-failed for token={} after {} tab switches",
                token,
                new_count
            );
        } else {
            // Just increment the counter
            sqlx::query!(
                r#"
                UPDATE test_attempts
                SET tab_switches = $1,
                    suspicious_activity = $2,
                    updated_at = $3
                WHERE access_token = $4
                "#,
                new_count,
                activity_json,
                now,
                token
            )
            .execute(&self.pool)
            .await
            .map_err(|e| crate::error::Error::Internal(format!("Failed to record violation: {}", e)))?;

            tracing::info!(
                "Anti-cheat: Violation #{} recorded for token={}",
                new_count,
                token
            );
        }

        Ok((new_count, terminated))
    }

    pub async fn get_status_distribution(&self) -> Result<std::collections::HashMap<String, i64>> {
        let rows = sqlx::query!(
            r#"SELECT status as "status!", COUNT(*) as "count!" FROM test_attempts GROUP BY status"#
        )
        .fetch_all(&self.pool)
        .await?;

        let mut map = std::collections::HashMap::new();
        for row in rows {
            map.insert(row.status, row.count);
        }
        Ok(map)
    }
}

#[derive(Debug, Clone)]
pub struct InviteCandidate {
    pub external_id: Option<String>,
    pub name: String,
    pub email: String,
    pub telegram_id: Option<i64>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateInviteResult {
    pub attempt_id: Uuid,
    pub access_token: String,
    pub expires_at: DateTime<Utc>,
    pub status: String,
}
