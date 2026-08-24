use crate::dto::integration_dto::{CreateQuestion, GenerateVacancyDescriptionPayload};
use crate::error::Result;
use crate::models::interview_form::RecognizedForm;
use crate::models::onef_match::CandidateProfile;
use crate::models::onef_vacancy::OneFVacancy;
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

const GENERATION_BATCH_SIZE: usize = 25;

const MIN_EXTRACTABLE_CV_CHARS: usize = 100;

const CANDIDATE_PROFILE_PROMPT: &str = r#"You are an HR analyst extracting a structured profile from a candidate's CV.

Rules:
1. Extract ONLY what the source states. Never infer, embellish or invent experience.
2. Leave a field null or empty when the CV does not cover it. An empty field is correct and useful; a guessed one is harmful.
3. `education_level` must use one of exactly these values when determinable: "Высшее", "Неоконченное высшее", "Среднее специальное", "Среднее". Otherwise null.
4. `total_experience_years` is a number — total professional experience in years. If the CV lists dated jobs, sum them. Null ONLY if there is nothing to compute from.
5. `languages` entries use the form "Язык: уровень" (e.g. "Русский: свободный"), levels being родной/свободный/профессиональный/разговорный/базовый. Map any wording onto that scale.
6. `specialties` must name the professional FIELD in the form "Категория: Подкатегория", matching how vacancies are classified (e.g. "Менеджмент и бизнес: Менеджмент", "Информационные технологии: Разработка ПО", "Гуманитарные и социальные: Психология"). Give 1-3. This is the field of work, NOT a list of technologies.
7. `professional_skills` is what the person can DO (e.g. "проектирование REST API", "управление командой"). `computer_skills` is the tools and technologies they use (e.g. "Docker", "Bitrix24"). Keep them separate — never put tools in professional_skills.
8. `summary` is 2-3 sentences in Russian describing who this candidate professionally is.

Return JSON:
{"summary": "", "education_level": null, "education_detail": null, "total_experience_years": null,
 "current_role": null, "specialties": [], "languages": [], "computer_skills": [],
 "professional_skills": [], "personal_qualities": []}"#;
const PAGE_ROTATION_PROMPT: &str = r#"You are shown one photograph of a printed A4 form lying on a desk.

Decide how far the PAGE must be turned CLOCKWISE for its printed text to read normally, left to right, with the heading at the top.

  0   — already upright
  90  — the page is on its side and its top edge currently points LEFT
  180 — the page is upside down
  270 — the page is on its side and its top edge currently points RIGHT

Judge this from the printed text and the page layout, not from the shape of the photo. You do not need to read the words — the direction the lines of type run is enough.

Answer with only: {"rotation": 0}"#;

const INTERVIEW_FORM_PROMPT: &str = r#"You transcribe handwritten HR interview evaluation sheets used by ГК «КОИНОТИ НАВ» into structured JSON.

This is TRANSCRIPTION, not interpretation. Copy what is written. Do not summarise, expand abbreviations, translate, or tidy grammar.

## Which template is it

Read the printed title line at the top of the page.

- "Форма оценки соискателя в ходе проведения 1-го личного собеседования" -> form_type "interview_1"
- "Форма оценки соискателя в ходе проведения 2-го личного собеседования" -> form_type "interview_2"
- Title unreadable or a different document -> form_type "unknown", and say so in `warnings`.

## interview_1

Header: Интервьюер (one or two people — split on the comma into separate array entries), Дата интервью, Ф.И.О. соискателя, Обсуждаемая должность, Установленное время начала интервью, Фактическое время прихода соискателя, Продолжительность собеседования: с ___ до ___.

The «Параметры / Информация» table has PRE-PRINTED row labels. Emit one `parameters` entry per printed row, in page order, `key` set to the canonical slug below, `label` copied as printed:

  Откуда узнали о вакансии                              -> vacancy_source
  Знания о нашей Компании                               -> company_knowledge
  Место жительства  /  Место жительства/Место рождения  -> residence_birthplace
  Состав семьи                                          -> family
  Основные ценности                                     -> core_values
  Цели личные/профессиональные                          -> goals
  Родные/знакомые, работающие в ГК «КОИНОТИ НАВ»        -> relatives_in_group
  ЗП (минимальный/ожидаемый)                            -> salary_expectation
  Дедлайн выполнения Самостоятельной Работы             -> self_work_deadline
  Планируемая дата выхода                               -> planned_start_date
  Готовность работать за границей                       -> ready_to_work_abroad

Two templates are in circulation and rows differ between them. Emit only the rows actually printed on THIS sheet. A printed row left blank by hand gets `value: null` — that is a correct answer, not a failure.

Then two 3-row grids, each with columns "Prof & Soft skills" and "Личные Качества":
`strengths` from «1-3 сильных качества соискателя», `growth_areas` from «1-3 точки роста соискателя». Keep the printed row number in `index`. Skip rows where both cells are empty.

Footer: `comments` from «Комментарии и общие впечатления о соискателе» (transcribe in full, keep the writer's own numbered list and line breaks), `hr_department_recommendation` from «Рекомендации ДЧР», `test_results_bud` from «Итоги тестов БЮД», `test_results_fed` from «Итоги тестов ФЭД».

The older revision of this template also prints a «Выводы:» box under the comments -> `conclusions`. It is a separate box: never fold it into `comments`, and leave it null on sheets that do not print it.

## interview_2

Header additionally has Должность (of the interviewer), Департамент and Отдел -> `interviewer_position`, `department`, `division`.

Its «Параметры / Информация» row labels are HANDWRITTEN and differ on every sheet (e.g. «Причина переезда», «Критерии выбора Ко.», «Интересы», «СТОП-ФАКТОРЫ», «Знание Англ-го», «Опыт работы», «Лог-е мышление»). For this template set `key` to null and copy the handwritten label into `label`.

Two decision blocks, each with Ф.И.О., Должность and a comment box:
«Комментарии и решение со стороны Инициатора заявки» -> `requester_decision`
«Комментарии и решение со стороны Специалиста HR Департамента» -> `hr_decision`

`strengths` and `growth_areas` do not exist on this template — leave them empty.

## Reading rules

1. NEVER invent. If a box is blank, the value is null. If ink is present but you genuinely cannot read it, transcribe your best attempt and lower that field's confidence — do not substitute a plausible-looking word.
2. Handwriting routinely OVERFLOWS its cell: it runs into the row below, past the right edge, over printed rules, and into the margins. Attribute overflowing text to the row where the line STARTS. Text written outside every box — dates of birth, salary ladders like "ЗП: 2мес. - 20000, 3/4 - 25000, с 5-го 30000", target departments — goes into `extra_notes`, one entry per note.
3. The script is Russian cursive with Latin fragments (Python, MS Office, KPI, P&L, Scrum, Agile, Somon.tj, FinTech, HR) and Tajik proper nouns (Душанбе, Худжанд, Спартак, Айни, "101 мкр"). Keep Latin in Latin and Cyrillic in Cyrillic — never transliterate between them.
4. Keep in-house abbreviations exactly as written: Ко., ГК, ДЧР, БЮД, ФЭД, ИС / МС (испытательный срок), ЗП, рук-во, упр., МСФО. Do not expand them.
5. `interview_date` -> ISO "YYYY-MM-DD". The year is printed as "202" with only the LAST DIGIT handwritten, so "202" + "6" means 2026. If the date is incomplete, return null and add a warning.
6. All times -> "HH:MM", 24-hour. «Продолжительность собеседования: с X до Y» gives `interview_from` and `interview_to`; the "до" half is often left blank -> null.
7. `candidate_age` only if a number of years is actually written on the sheet (some have e.g. "47 лет" in the margin). Otherwise null.
8. Salaries, times and dates carry the highest risk of misreading. Re-check every digit and be conservative with their confidence scores.
9. Transcribe struck-through text only if the replacement is unclear; otherwise take the correction and note it in `warnings`.

## Confidence

`field_confidence` maps a field path to 0.0-1.0 reflecting how legible that value was, NOT how plausible it looks. Use dotted paths: "candidate_name", "interview_date", "parameters.salary_expectation" (canonical key when there is one, otherwise "parameters.3" by zero-based position), "strengths.0.prof_soft_skills", "comments", "hr_decision.comment". Score every field you filled in. A clearly printed or crisply written value is 0.95+; a confident cursive read is 0.8-0.9; a plausible-but-ambiguous read is 0.4-0.7; a guess is below 0.4.

Put page-level problems in `warnings`: blur, glare, shadow, a photocopy with washed-out contrast, a cropped edge, a page photographed at an angle, or a title you could not read.

Return ONLY this JSON object:

{"form_type":"interview_1","interviewers":[],"interviewer_position":null,"department":null,"division":null,
 "interview_date":null,"candidate_name":null,"candidate_age":null,"position_discussed":null,
 "scheduled_start_time":null,"actual_arrival_time":null,"interview_from":null,"interview_to":null,
 "parameters":[{"key":null,"label":"","value":null}],
 "strengths":[{"index":1,"prof_soft_skills":null,"personal_qualities":null}],
 "growth_areas":[{"index":1,"prof_soft_skills":null,"personal_qualities":null}],
 "comments":null,"conclusions":null,"hr_department_recommendation":null,"test_results_bud":null,"test_results_fed":null,
 "extra_notes":[],
 "requester_decision":{"full_name":null,"position":null,"comment":null},
 "hr_decision":{"full_name":null,"position":null,"comment":null},
 "field_confidence":{},"warnings":[]}"#;

const MAX_PARALLEL_BATCHES: usize = 8;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct PipelineAdvice {
    pub stage: String,
    pub summary: String,
    pub recommendation: String,
    pub score: i32,
    #[serde(default)]
    pub advice: Vec<String>,
    #[serde(default)]
    pub suggested_questions: Vec<String>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
}

pub fn normalize_pipeline_advice(mut advice: PipelineAdvice, stage: &str) -> PipelineAdvice {
    advice.stage = stage.to_string();
    advice.score = advice.score.clamp(0, 100);
    advice.recommendation = match advice.recommendation.trim().to_lowercase().as_str() {
        "proceed" | "hold" | "reject" => advice.recommendation.trim().to_lowercase(),
        _ => "hold".to_string(),
    };
    advice
}

#[derive(Clone)]
pub struct AIService {
    client: Client,
    api_key: String,
    api_base: String,
}

impl AIService {
    pub fn new(api_key: String, api_base: String, client: Client) -> Self {
        Self { client, api_key, api_base }
    }

    pub async fn generate_test(
        &self,
        profession: &str,
        skills: &[String],
        num_questions: usize,
    ) -> Result<GenerationOutput> {
        let mut logs: Vec<String> = vec![];
        logs.push(format!("Starting GPT-4o generation for {} questions.", num_questions));

        let total_batches = num_questions.div_ceil(GENERATION_BATCH_SIZE).max(1);
        logs.push(format!(
            "Planning {} batch(es) of up to {} questions.",
            total_batches, GENERATION_BATCH_SIZE
        ));

        let mut collected: Vec<(usize, Vec<Question>)> = Vec::new();
        let mut failures: Vec<String> = Vec::new();
        let mut next_batch = 0usize;

        while next_batch < total_batches {
            let window = (total_batches - next_batch).min(MAX_PARALLEL_BATCHES);
            let mut tasks = tokio::task::JoinSet::new();

            for offset in 0..window {
                let index = next_batch + offset;
                let produced = num_questions.saturating_sub(index * GENERATION_BATCH_SIZE);
                let count = produced.min(GENERATION_BATCH_SIZE);
                if count == 0 {
                    continue;
                }

                let service = self.clone();
                let profession = profession.to_string();
                let skills = skills.to_vec();
                tasks.spawn(async move {
                    let result = service
                        .generate_question_batch(&profession, &skills, count, index, total_batches)
                        .await;
                    (index, result)
                });
            }

            while let Some(joined) = tasks.join_next().await {
                match joined {
                    Ok((index, Ok(questions))) => collected.push((index, questions)),
                    Ok((index, Err(e))) => {
                        failures.push(format!("batch {}: {}", index + 1, e));
                        tracing::warn!("AI generation batch {} failed: {}", index + 1, e);
                    }
                    Err(e) => failures.push(format!("join error: {}", e)),
                }
            }

            next_batch += window;
        }

        collected.sort_by_key(|(index, _)| *index);

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut questions: Vec<Question> = Vec::new();
        for (_, batch) in collected {
            for question in batch {
                let key = question
                    .question
                    .to_lowercase()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if key.is_empty() || !seen.insert(key) {
                    continue;
                }
                questions.push(question);
                if questions.len() == num_questions {
                    break;
                }
            }
            if questions.len() == num_questions {
                break;
            }
        }

        if questions.is_empty() {
            let detail = if failures.is_empty() {
                "no questions returned".to_string()
            } else {
                failures.join("; ")
            };
            return Err(anyhow::anyhow!("AI generation produced no questions: {}", detail).into());
        }

        for (idx, question) in questions.iter_mut().enumerate() {
            question.id = (idx as i32) + 1;
        }

        if !failures.is_empty() {
            logs.push(format!("{} batch(es) failed: {}", failures.len(), failures.join("; ")));
        }
        logs.push(format!("Finalized {} questions.", questions.len()));

        Ok(GenerationOutput {
            questions,
            logs,
        })
    }

    async fn generate_question_batch(
        &self,
        profession: &str,
        skills: &[String],
        count: usize,
        batch_index: usize,
        total_batches: usize,
    ) -> Result<Vec<Question>> {
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
7. This request is one slice of a larger exam assembled from several slices.
   Cover a distinct part of the topic space for your slice and never repeat a
   question that an adjacent slice would obviously produce.
"#;

        let user_schema = serde_json::json!({
            "profession": profession,
            "skills": skills,
            "required_count": count,
            "slice_index": batch_index + 1,
            "slice_total": total_batches,
            "slice_instruction": format!(
                "Produce slice {} of {}. Progress from foundational to advanced across slices, and keep every question unique within the whole exam.",
                batch_index + 1,
                total_batches
            ),
            "schema_example": {
                "questions": [
                    {
                        "type": "multiple_choice",
                        "question": "Russian text here...",
                        "options": ["Option 1", "Option 2", "Option 3", "Option 4"],
                        "correct_answer": 2,
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

        let response_json = self.chat_openai(payload).await?;
        Ok(self.sanitize_questions(&response_json, count))
    }

    pub async fn generate_vacancy_description(
        &self,
        payload: &GenerateVacancyDescriptionPayload,
    ) -> Result<String> {
        let system_prompt = "You are an expert HR Copywriter. Write an engaging, professional vacancy description in RUSSIAN language (strictly, even if user context is in another language). \
            Return a JSON object with a single field 'description'. \
            Use emoji bullets, clear structure, and an enthusiastic tone. \
            IMPORTANT: Do NOT include any application instructions or bot links at the end — those will be appended automatically.".to_string();

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

        let bot_cta = "\n\n📲 Для подачи заявки обязательно напишите нашему боту в Telegram: @koinot_dhr_bot";

        match self.chat_openai(ai_payload).await {
            Ok(resp) => {
                if let Some(desc) = resp.get("description").and_then(|v| v.as_str()) {
                    return Ok(format!("{}{}", desc.trim(), bot_cta));
                }
            }
            Err(e) => tracing::error!("Vacancy generation failed: {:?}", e),
        }

        Ok(format!("{}{}", self.fallback_vacancy_description(payload), bot_cta))
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

    pub async fn advise_pipeline_stage(
        &self,
        stage: &str,
        candidate: &JsonValue,
        vacancy: &JsonValue,
        history: &JsonValue,
        language: &str,
    ) -> Result<PipelineAdvice> {
        let lang = if language.trim().is_empty() { "ru" } else { language.trim() };

        let system_prompt = format!(
            r#"You are an expert recruitment co-pilot embedded in an HR pipeline (the "Первая Форма"/1F system).
A candidate moves through a multi-stage hiring funnel. The stages, in order, are:
1. cv_screening    — CV uploaded, AI screening score + insights.
2. phone_interview — short phone screen; HR records a summary.
3. interview_1     — first face-to-face interview; HR comments, culture & competency notes.
4. test_task       — take-home test assignment; status, HR comment, percentage score.
5. presentation    — presentation/case analysis (ПЗГ); score, strengths, weaknesses, conclusion.
6. interview_2     — final interview with the lead/manager; hard-skills assessment, manager comment.
7. final_decision  — hiring decision; outcome and reasoning.

You are given the candidate, the vacancy, and `history`: every stage that has ALREADY been filled in.
Your job is to reason about the WHOLE funnel so far and produce guidance for the CURRENT stage only.

Rules:
- Be aware of all prior stages: connect the dots (e.g. a concern raised on the phone screen should
  drive the questions you suggest for interview_1).
- `suggested_questions` MUST be tailored to this candidate using prior-stage data — never generic.
  If the current stage is not an interview (e.g. final_decision), suggested_questions may be empty.
- `recommendation` is exactly one of: "proceed", "hold", "reject".
- `score` is an integer 0-100 reflecting overall fit/confidence to advance.
- Be honest and specific; surface gaps and red flags in `risk_flags`.
- Write ALL human-readable text (summary, advice, suggested_questions, risk_flags) in {lang} language.

Return ONLY a JSON object with this exact shape:
{{"stage": string, "summary": string, "recommendation": "proceed|hold|reject", "score": 0-100,
  "advice": [string], "suggested_questions": [string], "risk_flags": [string]}}"#,
            lang = lang
        );

        let user_data = serde_json::json!({
            "current_stage": stage,
            "candidate": candidate,
            "vacancy": vacancy,
            "history": history,
        });

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": serde_json::to_string(&user_data).unwrap()}
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0.4
        });

        let resp = self.chat_openai(payload).await?;
        let advice: PipelineAdvice = serde_json::from_value(resp)?;
        Ok(normalize_pipeline_advice(advice, stage))
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


    pub async fn extract_candidate_profile(
        &self,
        name: &str,
        age: Option<u32>,
        cv_text: &str,
        cv_file_path: Option<&str>,
        profile_data: Option<&JsonValue>,
    ) -> Result<CandidateProfile> {
        if cv_text.trim().chars().count() < MIN_EXTRACTABLE_CV_CHARS {
            if let Some(path) = cv_file_path {
                tracing::info!("CV text too sparse ({} chars); using Vision for {}", cv_text.trim().chars().count(), path);
                match self.extract_candidate_profile_with_vision(name, age, path, profile_data).await {
                    Ok(profile) => return Ok(profile),
                    Err(e) => tracing::error!("Vision profile extraction failed, falling back to text: {:?}", e),
                }
            }
        }

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": CANDIDATE_PROFILE_PROMPT},
                {"role": "user", "content": self.profile_user_content(name, age, cv_text, profile_data)}
            ],
            "response_format": { "type": "json_object" }
        });

        let resp = self.chat_json_retrying(payload, "candidate profile extraction").await?;
        Ok(serde_json::from_value(resp).unwrap_or_default())
    }

    async fn extract_candidate_profile_with_vision(
        &self,
        name: &str,
        age: Option<u32>,
        cv_file_path: &str,
        profile_data: Option<&JsonValue>,
    ) -> Result<CandidateProfile> {
        let images = self.extract_images_from_cv(cv_file_path).await?;
        if images.is_empty() {
            return Err(anyhow::anyhow!("No images could be extracted from CV").into());
        }

        let mut content: Vec<JsonValue> = vec![serde_json::json!({
            "type": "text",
            "text": format!(
                "{}\n\nРезюме приложено изображениями ниже. Извлеки профиль строго из них.",
                self.profile_user_content(name, age, "", profile_data)
            )
        })];

        for (i, image_base64) in images.iter().take(3).enumerate() {
            tracing::info!("Adding CV page {} to vision profile request", i + 1);
            content.push(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:image/png;base64,{}", image_base64), "detail": "high" }
            }));
        }

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": CANDIDATE_PROFILE_PROMPT},
                {"role": "user", "content": content}
            ],
            "response_format": { "type": "json_object" },
            "max_tokens": 1500
        });

        let resp = self.chat_json_retrying(payload, "vision profile extraction").await?;
        Ok(serde_json::from_value(resp).unwrap_or_default())
    }

    fn profile_user_content(
        &self,
        name: &str,
        age: Option<u32>,
        cv_text: &str,
        profile_data: Option<&JsonValue>,
    ) -> String {
        let mut user_content = format!("Кандидат: {}\n", name);
        if let Some(age) = age {
            user_content.push_str(&format!("Возраст: {}\n", age));
        }
        if let Some(extra) = profile_data {
            user_content.push_str(&format!("Анкета: {}\n", extra));
        }
        if !cv_text.trim().is_empty() {
            user_content.push_str(&format!("\nТекст резюме:\n{}", cv_text));
        }
        user_content
    }

    pub async fn rank_vacancies(
        &self,
        profile: &CandidateProfile,
        vacancies: &[OneFVacancy],
    ) -> Result<JsonValue> {
        let system_prompt = r#"You are a critical, unbiased senior HR specialist ranking vacancies for one candidate.

You receive one candidate profile and a list of vacancies. Score EVERY vacancy in the list.

Scoring rules:
1. BE STRICT and comparative. These vacancies are ranked against each other — spread the scores. If everything lands in the 60-75 band you have failed the task.
2. A fundamentally different profession is a fundamental mismatch (0-20), no matter how transferable the soft skills. An accountant is not a fit for a designer role.
3. Missing a stated mandatory requirement (education level, licence, years of experience) costs heavily.
4. Judge only on: specialty fit, professional skills, education, experience, languages, computer skills, personal qualities. The candidate's specialty fit and professional skills matter most; personal qualities matter least.
4a. Compare specialty by MEANING, not wording. The candidate profile and the vacancy may phrase the same field differently ("Разработка ПО" vs "Информационные технологии: Разработка ПО"); treat those as a match. Only a genuinely different profession is a mismatch.
4b. A null or empty candidate field means UNKNOWN, not zero. Never penalise a dimension you have no data on: score it 50, and list that dimension's name in `unknown`. Score 0 only when you have evidence the candidate genuinely lacks the requirement. Judge the candidate on what is known.
5. NEVER consider age or gender. They are handled outside this scoring and must not influence any number or comment.
6. Some vacancies contain placeholder or nonsense requirement text (random characters, a word repeated, "test test test"). Do NOT invent a match for those — score the affected dimension low and say plainly in the comment that the vacancy has no usable requirements.
7. Judge only from the profile given. Never assume unstated experience.

Score bands: 0-30 fundamental mismatch. 31-60 partial overlap, key requirements missing. 61-80 strong fit with gaps. 81-100 excellent fit.

`matched` and `missing` list concrete requirements, not generalities. `unknown` lists dimension names scored 50 for lack of data (one of: education, experience, professional_skills, languages, computer_skills, specialty, personal_qualities). `comment` is 1-2 sentences, in Russian, blunt and specific; if anything is in `unknown`, say what was missing from the CV.

Return JSON:
{"matches": [{"vacancy_id_1f": 0, "score": 0,
  "breakdown": {"education": 0, "experience": 0, "professional_skills": 0, "languages": 0,
                "computer_skills": 0, "specialty": 0, "personal_qualities": 0},
  "matched": [], "missing": [], "unknown": [], "comment": ""}]}"#;

        let user_content = format!(
            "ПРОФИЛЬ КАНДИДАТА:\n{}\n\nВАКАНСИИ ({} шт.):\n{}",
            serde_json::to_string_pretty(profile).unwrap_or_default(),
            vacancies.len(),
            render_vacancies(vacancies)
        );

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_content}
            ],
            "response_format": { "type": "json_object" }
        });

        self.chat_json_retrying(payload, "vacancy ranking").await
    }

    async fn chat_json_retrying(&self, payload: JsonValue, label: &str) -> Result<JsonValue> {
        match self.chat_openai(payload.clone()).await {
            Ok(v) => Ok(v),
            Err(e) => {
                tracing::warn!("{} failed ({:?}); retrying once", label, e);
                self.chat_openai(payload).await
            }
        }
    }

    async fn chat_openai(&self, payload: JsonValue) -> Result<JsonValue> {
        let res = self.client
            .post(format!("{}/chat/completions", self.api_base))
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


    pub async fn detect_page_rotation(&self, probe_base64: &str) -> Result<u32> {
        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": PAGE_ROTATION_PROMPT},
                {"role": "user", "content": [
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", probe_base64),
                        "detail": "low"
                    }}
                ]}
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0,
            "max_tokens": 50
        });

        let raw = self.chat_json_retrying(payload, "page rotation probe").await?;

        let rotation = raw
            .get("rotation")
            .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
            .unwrap_or(0);

        Ok(match rotation {
            90 | 180 | 270 => rotation as u32,
            _ => 0,
        })
    }

    pub async fn recognize_interview_form(
        &self,
        image_base64: &str,
        form_type_hint: Option<&str>,
    ) -> Result<(RecognizedForm, JsonValue)> {
        let mut instruction = String::from(
            "Транскрибируй прикреплённый бланк оценки соискателя. Верни JSON строго по схеме.",
        );
        if let Some(hint) = form_type_hint {
            instruction.push_str(&format!(
                "\n\nОтправитель ожидает шаблон \"{}\". Проверь это по печатному заголовку страницы: \
                 если заголовок говорит другое, доверяй заголовку и добавь предупреждение в warnings.",
                hint
            ));
        }

        let payload = serde_json::json!({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": INTERVIEW_FORM_PROMPT},
                {"role": "user", "content": [
                    {"type": "text", "text": instruction},
                    {"type": "image_url", "image_url": {
                        "url": format!("data:image/jpeg;base64,{}", image_base64),
                        "detail": "high"
                    }}
                ]}
            ],
            "response_format": { "type": "json_object" },
            "temperature": 0,
            "max_tokens": 4000
        });

        let raw = self
            .chat_json_retrying(payload, "interview form recognition")
            .await?;

        let form: RecognizedForm = serde_json::from_value(sanitize_interview_form_json(raw.clone()))
            .map_err(|e| {
                anyhow::anyhow!("Interview form JSON did not match the expected shape: {}", e)
            })?;

        Ok((form, raw))
    }
}

fn sanitize_interview_form_json(mut raw: JsonValue) -> JsonValue {
    let Some(obj) = raw.as_object_mut() else {
        return serde_json::json!({});
    };

    for key in [
        "interviewers",
        "parameters",
        "strengths",
        "growth_areas",
        "extra_notes",
        "warnings",
    ] {
        match obj.get(key) {
            Some(JsonValue::Array(_)) => {}
            _ => {
                obj.insert(key.to_string(), JsonValue::Array(Vec::new()));
            }
        }
    }

    if let Some(JsonValue::Array(items)) = obj.get_mut("interviewers") {
        let split: Vec<JsonValue> = items
            .iter()
            .filter_map(|v| v.as_str())
            .flat_map(|s| s.split(',').map(str::trim).map(str::to_string))
            .filter(|s| !s.is_empty())
            .map(JsonValue::String)
            .collect();
        *items = split;
    }

    for key in ["requester_decision", "hr_decision"] {
        if !matches!(obj.get(key), Some(JsonValue::Object(_))) {
            obj.insert(key.to_string(), JsonValue::Null);
        }
    }

    if let Some(age) = obj.get("candidate_age") {
        let parsed = match age {
            JsonValue::Number(n) => n.as_i64(),
            JsonValue::String(s) => s
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<i64>()
                .ok(),
            _ => None,
        };
        obj.insert(
            "candidate_age".to_string(),
            match parsed.filter(|v| (14..=99).contains(v)) {
                Some(v) => JsonValue::from(v),
                None => JsonValue::Null,
            },
        );
    }

    let confidence = obj
        .get("field_confidence")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .filter_map(|(k, v)| {
                    let score = match v {
                        JsonValue::Number(n) => n.as_f64(),
                        JsonValue::String(s) => s.trim().parse::<f64>().ok(),
                        _ => None,
                    }?;
                    Some((k.clone(), JsonValue::from(score.clamp(0.0, 1.0))))
                })
                .collect::<serde_json::Map<_, _>>()
        })
        .unwrap_or_default();
    obj.insert("field_confidence".to_string(), JsonValue::Object(confidence));

    raw
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_full_advice_from_model_json() {
        let raw = serde_json::json!({
            "stage": "interview_1",
            "summary": "Сильный бэкенд, но завышенные зарплатные ожидания.",
            "recommendation": "proceed",
            "score": 72,
            "advice": ["Уточнить вилку на старте интервью"],
            "suggested_questions": ["Расскажите про опыт с очередями сообщений?"],
            "risk_flags": ["Ожидания по зарплате выше бюджета"]
        });
        let advice: PipelineAdvice = serde_json::from_value(raw).expect("must deserialize");
        assert_eq!(advice.recommendation, "proceed");
        assert_eq!(advice.score, 72);
        assert_eq!(advice.suggested_questions.len(), 1);
        assert_eq!(advice.risk_flags.len(), 1);
    }

    #[test]
    fn deserializes_with_missing_optional_arrays() {
        let raw = serde_json::json!({
            "stage": "final_decision",
            "summary": "Все этапы пройдены.",
            "recommendation": "proceed",
            "score": 88
        });
        let advice: PipelineAdvice = serde_json::from_value(raw).expect("must deserialize");
        assert!(advice.advice.is_empty());
        assert!(advice.suggested_questions.is_empty());
        assert!(advice.risk_flags.is_empty());
    }

    #[test]
    fn normalize_clamps_score_and_overrides_stage() {
        let advice = PipelineAdvice {
            stage: "garbage_from_model".into(),
            summary: "s".into(),
            recommendation: "proceed".into(),
            score: 250,
            advice: vec![],
            suggested_questions: vec![],
            risk_flags: vec![],
        };
        let out = normalize_pipeline_advice(advice, "interview_2");
        assert_eq!(out.stage, "interview_2", "stage must be our label, not the model's");
        assert_eq!(out.score, 100, "score must clamp to 100");
    }

    #[test]
    fn normalize_clamps_negative_score() {
        let advice = PipelineAdvice {
            stage: "x".into(),
            summary: "s".into(),
            recommendation: "reject".into(),
            score: -40,
            advice: vec![],
            suggested_questions: vec![],
            risk_flags: vec![],
        };
        let out = normalize_pipeline_advice(advice, "cv_screening");
        assert_eq!(out.score, 0);
        assert_eq!(out.recommendation, "reject");
    }

    #[test]
    fn normalize_coerces_unknown_recommendation_to_hold() {
        for bad in ["maybe", "PROCEED ", "", "yes", "Reject"] {
            let advice = PipelineAdvice {
                stage: "x".into(),
                summary: "s".into(),
                recommendation: bad.into(),
                score: 50,
                advice: vec![],
                suggested_questions: vec![],
                risk_flags: vec![],
            };
            let out = normalize_pipeline_advice(advice, "phone_interview");
            let expected = match bad.trim().to_lowercase().as_str() {
                "proceed" | "hold" | "reject" => bad.trim().to_lowercase(),
                _ => "hold".to_string(),
            };
            assert_eq!(out.recommendation, expected, "input was {:?}", bad);
        }
    }
}

fn render_vacancies(vacancies: &[OneFVacancy]) -> String {
    const DESCRIPTION_BUDGET: usize = 600;
    const CATALOGUE_BUDGET: usize = 120_000;

    let render = |include_description: bool| -> String {
        vacancies
            .iter()
            .map(|v| {
                let mut block = format!(
                    "---\nvacancy_id_1f: {}\nname: {}\ncompany: {}\nspecialty: {}\neducation: {}\nexperience: {}\nlanguages: {}\ncomputer_skills: {}\nprofessional_skills: {}\npersonal_qualities: {}",
                    v.vacancy_id_1f,
                    v.name,
                    v.company.as_deref().unwrap_or("-"),
                    v.specialty.as_deref().unwrap_or("-"),
                    v.education.as_deref().unwrap_or("-"),
                    v.experience.as_deref().unwrap_or("-"),
                    if v.languages.is_empty() { "-".to_string() } else { v.languages.join("; ") },
                    if v.computer_skills.is_empty() { "-".to_string() } else { v.computer_skills.join("; ") },
                    v.professional_skills.as_deref().unwrap_or("-"),
                    v.personal_qualities.as_deref().unwrap_or("-"),
                );
                if include_description {
                    if let Some(d) = v.description_clean.as_deref() {
                        let truncated: String = d.chars().take(DESCRIPTION_BUDGET).collect();
                        block.push_str(&format!("\ndescription: {}", truncated));
                    }
                }
                block
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let with_descriptions = render(true);
    if with_descriptions.chars().count() <= CATALOGUE_BUDGET {
        with_descriptions
    } else {
        tracing::info!(
            "Catalogue of {} vacancies exceeds the prompt budget; ranking on structured fields only",
            vacancies.len()
        );
        render(false)
    }
}
