use crate::models::question::{Question, QuestionDetails, QuestionType};
use serde_json::Value as JsonValue;

pub struct GradingService;

impl GradingService {
    pub fn grade_mcq_only(
        questions: &[Question],
        answers: &[JsonValue],
    ) -> (i32, i32, Vec<JsonValue>, bool) {
        let mut total_max_points: i32 = 0;
        let mut earned_points: i32 = 0;
        let mut graded: Vec<JsonValue> = Vec::new();
        let mut needs_review = false;

        for (idx, q) in questions.iter().enumerate() {
            total_max_points += q.points;
            let question_id = q.id.max((idx as i32) + 1);
            let ans = answers.iter().find(|a| {
                a.get("question_id").and_then(|v| v.as_i64()) == Some(question_id as i64)
            });

            let candidate_answer = ans.and_then(|a| a.get("answer").cloned()).unwrap_or(serde_json::json!(null));
            
            match q.question_type {
                QuestionType::MultipleChoice => {
                    let mut points_earned = 0;
                    let mut is_correct = false;
                    let mut correct_val = serde_json::json!(null);
                    let mut candidate_val = candidate_answer.clone();

                    if let QuestionDetails::MultipleChoice(ref mc) = q.details {
                        if let Some(option) = mc.options.get(mc.correct_answer as usize) {
                            correct_val = serde_json::json!(option);
                        }

                        let given_idx_opt = candidate_answer.as_i64()
                            .or_else(|| {
                                if candidate_answer.is_object() {
                                    candidate_answer.get("selected").and_then(|v| v.as_i64())
                                } else {
                                    None
                                }
                            });

                        if let Some(given_idx) = given_idx_opt {
                            if let Some(option) = mc.options.get(given_idx as usize) {
                                candidate_val = serde_json::json!(option);
                            }

                            if given_idx as i32 == mc.correct_answer {
                                points_earned = q.points;
                                is_correct = true;
                            }
                        }
                    }

                    earned_points += points_earned;
                    graded.push(serde_json::json!({
                        "question_id": question_id,
                        "question_text": q.question,
                        "type": "multiple_choice",
                        "candidate_answer": candidate_val,
                        "correct_answer": correct_val,
                        "points_earned": points_earned,
                        "max_points": q.points,
                        "is_correct": is_correct,
                    }));
                }
                QuestionType::ShortAnswer => {
                    needs_review = true;
                    graded.push(serde_json::json!({
                        "question_id": question_id,
                        "question_text": q.question,
                        "type": "short_answer",
                        "candidate_answer": candidate_answer,
                        "correct_answer": "Manual review required",
                        "points_earned": 0,
                        "max_points": q.points,
                        "is_correct": false,
                        "needs_review": true,
                    }));
                }
                _ => {
                    graded.push(serde_json::json!({
                        "question_id": question_id,
                        "question_text": q.question,
                        "type": format!("{:?}", q.question_type).to_lowercase(),
                        "candidate_answer": candidate_answer,
                        "correct_answer": "Auto-grading not supported for this type",
                        "points_earned": 0,
                        "max_points": q.points,
                        "is_correct": false,
                    }));
                }
            }
        }

        (earned_points, total_max_points, graded, needs_review)
    }
}
