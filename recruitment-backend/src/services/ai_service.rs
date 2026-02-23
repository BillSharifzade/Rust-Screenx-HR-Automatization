use crate::dto::integration_dto::{CreateQuestion, GenerateVacancyDescriptionPayload};
use crate::error::Result;
use crate::models::question::{
    MultipleChoiceDetails, Question, QuestionDetails, QuestionType,
    ShortAnswerDetails,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::seq::SliceRandom;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationOutput {
    pub questions: Vec<Question>,
    pub logs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CandidateSuitability {
    pub rating: i32,
    pub comment: String,
}

#[derive(Clone)]
pub struct AIService {
    client: Client,
    api_key: String,
}

impl AIService {
    pub fn new(api_key: String, client: Client) -> Self {
        Self { client, api_key }
    }

    pub async fn generate_test(
        &self,
        profession: &str,
        skills: &[String],
        num_questions: usize,
    ) -> Result<GenerationOutput> {
        let mut logs: Vec<String> = vec![];
        logs.push(format!("Starting GPT-4o generation for {} questions.", num_questions));

        let system_prompt = r#"You are a Senior Technical Recruiter and Engineering Manager. 
Your task is to generate a comprehensive technical assessment test in RUSSIAN language (Cyrillic).
The output must be a valid JSON object containing a 'questions' array.

Rules:
1. Generate exactly the requested number of questions.
2. Mix 'multiple_choice' (approx 60%) and 'short_answer' (approx 40%) types.
3. Questions should be non-trivial, practical, and test deep understanding.
4. All text (questions, options, explanations) MUST be in Russian.
5. Avoid "All of the above" or "None of the above" options.
6. CRITICAL: For multiple choice questions, VARY the correct_answer index. Do NOT always use 0.
   - Distribute correct answers across all positions (0, 1, 2, 3) roughly equally.
   - The correct answer should match the actual correct option's position.
"#;

        let user_schema = serde_json::json!({
            "profession": profession,
            "skills": skills,
            "required_count": num_questions,
            "schema_example": {
                "questions": [
                    {
                        "type": "multiple_choice",
                        "question": "Russian text here...",
                        "options": ["Option 1", "Option 2", "Option 3", "Option 4"],
                        "correct_answer": 2, // index - VARY THIS! Don't always use 0
                        "explanation": "Why option at index 2 is correct..."
                    },
                    {
                        "type": "short_answer",
                        "question": "Russian text...",
                        "min_words": 50,
                        "expected_keywords": ["keyword1", "keyword2"]
                    }
                ]
            }
        });

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": serde_json::to_string(&user_schema).unwrap()}
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0.8
        });

        logs.push("Sending request to OpenAI...".to_string());
        let response_json = self.chat_openai(payload).await?;
        logs.push("Response received. Parsing and sanitizing...".to_string());
        let questions = self.sanitize_questions(&response_json, num_questions);
        logs.push(format!("Finalized {} questions.", questions.len()));

        Ok(GenerationOutput {
            questions,
            logs,
        })
    }

    pub async fn generate_vacancy_description(
        &self,
        payload: &GenerateVacancyDescriptionPayload,
    ) -> Result<String> {
        let system_prompt = "You are an expert HR Copywriter. Write an engaging, professional vacancy description in RUSSIAN language (strictly, even if user context is in another language). \
            Return a JSON object with a single field 'description'. \
            Use emoji bullets, clear structure, and an enthusiastic tone.".to_string();

        let user_data = serde_json::json!({
            "title": payload.title,
            "company": payload.company,
            "details": payload
        });

        let ai_payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": serde_json::to_string(&user_data).unwrap()}
            ],
            "response_format": { "type": "json_object" }
        });

        match self.chat_openai(ai_payload).await {
            Ok(resp) => {
                if let Some(desc) = resp.get("description").and_then(|v| v.as_str()) {
                    return Ok(desc.trim().to_string());
                }
            }
            Err(e) => tracing::error!("Vacancy generation failed: {:?}", e),
        }

        Ok(self.fallback_vacancy_description(payload))
    }

    pub async fn analyze_suitability(
        &self,
        candidate_name: &str,
        candidate_email: &str,
        cv_text: &str,
        cv_file_path: Option<&str>,
        vacancy_title: &str,
        vacancy_description: &str,
    ) -> Result<CandidateSuitability> {
        let raw_text = cv_text.replace("[NOTE: The candidate's CV appears to be a scanned image. Extracted text is very sparse: '", "")
                             .replace("'. Please evaluate based on this and basic profile info.]", "");
        
        let text_extraction_failed = raw_text.trim().len() < 100;
        tracing::info!("Suitability check: raw_text len={}, failed={}", raw_text.trim().len(), text_extraction_failed);
        
        if text_extraction_failed {
            if let Some(path) = cv_file_path {
                tracing::info!("Triggering Vision API fallback for {}", path);
                match self.analyze_suitability_with_vision(
                    candidate_name,
                    candidate_email,
                    path,
                    vacancy_title,
                    vacancy_description,
                ).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        tracing::error!("Vision analysis failed, falling back to text: {:?}", e);
                    }
                }
            } else {
                tracing::warn!("Text extraction failed but no file path provided for vision fallback");
            }
        }

        let system_prompt = r#"You are a Critical and Unbiased Senior HR Specialist. 
        Your task is to strictly evaluate how well a candidate's CV matches a specific vacancy.

        Evaluation Rules:
        1. BE STRICT. If the candidate's core profession is fundamentally different from the vacancy (e.g., IT developer applying for Legal role, or Doctor applying for Accountant), the rating MUST be extremely low (0-10%).
        2. TRANSFERABLE SKILLS ARE NOT ENOUGH for professional roles. Do not give a high rating just because someone is 'organized' or 'fast learner' if they lack the required professional background/education.
        3. Mandatory requirements: If the vacancy requires a specific license, education, or years of experience which the candidate clearly lacks, deduct points heavily.
        4. Rating Scale:
           - 0-30%: Fundamental mismatch / Lack of core experience.
           - 31-60%: Some overlap but lacks key professional requirements.
           - 61-80%: Strong match, lacks some minor details or specific domain experience.
           - 81-100%: Perfect or nearly perfect matching background.

        Return JSON: { "rating": <0-100>, "comment": "<brutally honest and concise explanation in Russian>" }. 
        Always respond in Russian language strictly. Ignore any English in the CV and provide your comment ONLY in Russian."#;

        let user_content = format!(
            "Candidate: {} ({})\n\nVacancy: {}\n{}\n\nCV Content:\n{}",
            candidate_name, candidate_email, vacancy_title, vacancy_description, cv_text
        );

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "response_format": { "type": "json_object" }
        });

        let resp = self.chat_openai(payload).await?;
        let suitability: CandidateSuitability = serde_json::from_value(resp)?;
        Ok(suitability)
    }

    async fn analyze_suitability_with_vision(
        &self,
        candidate_name: &str,
        candidate_email: &str,
        cv_file_path: &str,
        vacancy_title: &str,
        vacancy_description: &str,
    ) -> Result<CandidateSuitability> {
        tracing::info!("Using Vision API to analyze CV: {}", cv_file_path);
        
        let images = self.extract_images_from_cv(cv_file_path).await?;
        
        if images.is_empty() {
            return Err(anyhow::anyhow!("No images could be extracted from CV").into());
        }

        let system_prompt = r#"You are a Critical Senior HR Specialist. 
        Analyze the candidate's CV (provided as images) against the vacancy requirements.
        
        STRICT RULES:
        1. If the candidate's profession in the CV is fundamentally different from the vacancy, rate 0-15%.
        2. Focus on hard technical/professional skills and education.
        3. Ignore generic soft skills if professional background is missing.

        Return JSON: { "rating": <0-100>, "comment": "<brutally honest evaluation in Russian>" }. 
        Always respond in Russian language strictly. Do NOT use English even if the CV is in English."#;

        let mut content: Vec<JsonValue> = vec![
            serde_json::json!({
                "type": "text",
                "text": format!(
                    "Candidate: {} ({})\n\nVacancy: {}\n{}\n\nPlease analyze the CV images below and evaluate the candidate's suitability for this position.",
                    candidate_name, candidate_email, vacancy_title, vacancy_description
                )
            })
        ];

        for (i, image_base64) in images.iter().take(3).enumerate() {
            tracing::info!("Adding CV page {} to vision request", i + 1);
            content.push(serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", image_base64),
                    "detail": "high"
                }
            }));
        }

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": content}
            ],
            "response_format": { "type": "json_object" },
            "max_tokens": 1000
        });

        let resp = self.chat_openai(payload).await?;
        let suitability: CandidateSuitability = serde_json::from_value(resp)?;
        
        tracing::info!("Vision-based CV analysis complete. Rating: {}", suitability.rating);
        Ok(suitability)
    }

    async fn extract_images_from_cv(&self, file_path: &str) -> Result<Vec<String>> {
        let path = std::path::Path::new(file_path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        match ext.to_lowercase().as_str() {
            "pdf" => {
                self.pdf_to_images(file_path).await
            }
            "jpg" | "jpeg" | "png" | "webp" => {
                let data = fs::read(file_path).await?;
                Ok(vec![BASE64.encode(&data)])
            }
            "doc" | "docx" | "rtf" | "odt" => {
                let temp_dir = format!("/tmp/cv_topdf_{}", uuid::Uuid::new_v4());
                fs::create_dir_all(&temp_dir).await?;

                let output = Command::new("libreoffice")
                    .arg("--headless")
                    .arg("--norestore")
                    .arg("--convert-to")
                    .arg("pdf")
                    .arg("--outdir")
                    .arg(&temp_dir)
                    .arg(file_path)
                    .output()
                    .await;

                match output {
                    Ok(out) => {
                        if !out.status.success() {
                            let _ = fs::remove_dir_all(&temp_dir).await;
                            return Err(anyhow::anyhow!(
                                "LibreOffice PDF conversion failed: {}",
                                String::from_utf8_lossy(&out.stderr)
                            ).into());
                        }
                    }
                    Err(e) => {
                        let _ = fs::remove_dir_all(&temp_dir).await;
                        return Err(anyhow::anyhow!("Failed to run libreoffice: {}", e).into());
                    }
                }

                let mut pdf_path = None;
                let mut entries = fs::read_dir(&temp_dir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let p = entry.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("pdf") {
                        pdf_path = Some(p);
                        break;
                    }
                }

                let result = if let Some(pdf) = pdf_path {
                    self.pdf_to_images(pdf.to_str().unwrap_or("")).await
                } else {
                    Err(anyhow::anyhow!("LibreOffice produced no PDF output").into())
                };

                let _ = fs::remove_dir_all(&temp_dir).await;
                result
            }
            _ => {
                Err(anyhow::anyhow!("Unsupported file format for vision: {}", ext).into())
            }
        }
    }

    async fn pdf_to_images(&self, pdf_path: &str) -> Result<Vec<String>> {
        let temp_dir = format!("/tmp/cv_images_{}", uuid::Uuid::new_v4());
        fs::create_dir_all(&temp_dir).await?;

        let output = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg("150")
            .arg(pdf_path)
            .arg(format!("{}/page", temp_dir))
            .output()
            .await;

        match output {
            Ok(out) => {
                if !out.status.success() {
                    tracing::error!("pdftoppm failed: {}", String::from_utf8_lossy(&out.stderr));
                    let _ = fs::remove_dir_all(&temp_dir).await;
                    return Err(anyhow::anyhow!("PDF conversion failed").into());
                }
            }
            Err(e) => {
                tracing::error!("Failed to run pdftoppm: {}", e);
                let _ = fs::remove_dir_all(&temp_dir).await;
                return Err(anyhow::anyhow!("pdftoppm not available").into());
            }
        }

        let mut image_files = Vec::new();
        let mut entries = fs::read_dir(&temp_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|e| e.to_str()) == Some("png") {
                image_files.push(entry_path);
            }
        }

        image_files.sort_by_key(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string());

        let mut images = Vec::new();
        for img_path in image_files {
            if let Ok(data) = fs::read(&img_path).await {
                tracing::info!("Adding image to vision processing: {:?}", img_path);
                images.push(BASE64.encode(&data));
            }
        }

        let _ = fs::remove_dir_all(&temp_dir).await;
        Ok(images)
    }

    async fn chat_openai(&self, payload: JsonValue) -> Result<JsonValue> {
        let res = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&payload)
            .timeout(Duration::from_secs(120))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenAI API Error {}: {}", status, text).into());
        }

        let body: JsonValue = res.json().await?;
        
        body.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid OpenAI response format").into())
    }

    pub fn sanitize_questions(&self, raw: &JsonValue, num_questions: usize) -> Vec<Question> {
        let mut questions = Vec::new();
        
        let arr_val = if let Some(arr) = raw.get("questions").and_then(|a| a.as_array()) {
            arr.clone()
        } else if let Some(arr) = raw.as_array() {
            arr.clone()
        } else {
            vec![]
        };

        let mut rng = rand::thread_rng();

        for (idx, val) in arr_val.iter().enumerate() {
            if let Ok(mut q) = self.coerce_question(val, &mut rng) {
                q.id = (idx as i32) + 1;
                
                match &mut q.details {
                    QuestionDetails::MultipleChoice(mc) => {
                        if mc.options.len() < 2 { continue; }
                        if mc.correct_answer < 0 || mc.correct_answer as usize >= mc.options.len() {
                            mc.correct_answer = 0;
                        }
                    }
                    QuestionDetails::ShortAnswer(sa) => {
                        if sa.min_words.is_none() { sa.min_words = Some(40); }
                    }
                    _ => {}
                }
                questions.push(q);
            }
        }
        
        if questions.len() > num_questions {
            questions.truncate(num_questions);
        }
        
        questions
    }

    fn coerce_question(&self, v: &JsonValue, rng: &mut impl rand::Rng) -> Result<Question> {
        let type_str = v.get("type").and_then(|s| s.as_str()).unwrap_or("multiple_choice");
        let question_text = v.get("question").and_then(|s| s.as_str()).unwrap_or("Empty question").to_string();
        
        let details = match type_str {
            "multiple_choice" => {
                let mut options: Vec<String> = v.get("options")
                    .and_then(|o| o.as_array())
                    .map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
                    .unwrap_or_default();
                
                let mut correct = v.get("correct_answer").and_then(|i| i.as_i64()).unwrap_or(0) as i32;
                let explanation = v.get("explanation").and_then(|s| s.as_str()).map(|s| s.to_string());
                
                if !options.is_empty() && correct >= 0 && (correct as usize) < options.len() {
                    let correct_option = options[correct as usize].clone();
                    options.shuffle(rng);
                    correct = options.iter().position(|o| o == &correct_option).unwrap_or(0) as i32;
                }
                
                QuestionDetails::MultipleChoice(MultipleChoiceDetails {
                    options,
                    correct_answer: correct,
                    explanation,
                })
            },
            "short_answer" | "code" => { 
                 let min = v.get("min_words").and_then(|i| i.as_i64()).map(|i| i as i32);
                 let keys = v.get("expected_keywords").and_then(|a| a.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect());
                 
                 QuestionDetails::ShortAnswer(ShortAnswerDetails {
                     min_words: min,
                     expected_keywords: keys,
                     ai_grading: true
                 })
            },
            _ => return Err(anyhow::anyhow!("Unknown type").into()),
        };

        Ok(Question {
            id: 0,
            question_type: match type_str {
                "multiple_choice" => QuestionType::MultipleChoice,
                "code" => QuestionType::Code,
                _ => QuestionType::ShortAnswer,
            },
            question: question_text,
            points: 10,
            details,
        })
    }
    
    pub fn to_create_questions(&self, questions: &[Question]) -> Vec<CreateQuestion> {
        questions.iter().map(|q| CreateQuestion {
            question_type: q.question_type.clone(),
            question: q.question.clone(),
            points: q.points,
            details: q.details.clone(),
        }).collect()
    }

    fn fallback_vacancy_description(&self, payload: &GenerateVacancyDescriptionPayload) -> String {
        format!(
            "{} at {}. \n\nWe are looking for a professional with: {}.\n\nApply now!",
            payload.title, payload.company, payload.professional_skills.clone().unwrap_or_default()
        )
    }
}

