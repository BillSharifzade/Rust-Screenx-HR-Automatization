use crate::error::{Error, Result};
use crate::models::interview_form::*;
use crate::services::ai_service::AIService;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::imageops::FilterType;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;

const MAX_IMAGE_EDGE: u32 = 2048;

const JPEG_QUALITY: u8 = 88;

const PROBE_EDGE: u32 = 512;
const PROBE_QUALITY: u8 = 70;

const MAX_IMAGES_PER_JOB: usize = 10;

const CALLBACK_MAX_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct RecognizeRequest {
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_urls: Option<Vec<String>>,
    #[serde(default)]
    pub candidate_id: Option<Uuid>,
    #[serde(default)]
    pub vacancy_id: Option<i64>,
    #[serde(default)]
    pub form_type: Option<String>,
    #[serde(default)]
    pub callback_url: Option<String>,
    #[serde(default)]
    pub external_ref: Option<String>,
}

impl RecognizeRequest {
    pub fn urls(&self) -> Vec<String> {
        let raw = match (&self.image_urls, &self.image_url) {
            (Some(list), _) if !list.is_empty() => list.clone(),
            (_, Some(single)) => vec![single.clone()],
            _ => Vec::new(),
        };

        let mut seen = std::collections::HashSet::new();
        raw.into_iter()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty())
            .filter(|u| seen.insert(u.clone()))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    #[serde(flatten)]
    pub job: InterviewFormJob,
    pub forms: Vec<InterviewFormRecognition>,
}

#[derive(Clone)]
pub struct InterviewFormService {
    pool: PgPool,
    ai_service: AIService,
    client: Client,
}

impl InterviewFormService {
    pub fn new(pool: PgPool, ai_service: AIService, client: Client) -> Self {
        Self { pool, ai_service, client }
    }

    pub async fn create_job(&self, req: &RecognizeRequest) -> Result<InterviewFormJob> {
        let urls = req.urls();
        if urls.is_empty() {
            return Err(Error::BadRequest(
                "Provide image_url or a non-empty image_urls array".into(),
            ));
        }
        if urls.len() > MAX_IMAGES_PER_JOB {
            return Err(Error::BadRequest(format!(
                "At most {} images per request, got {}",
                MAX_IMAGES_PER_JOB,
                urls.len()
            )));
        }
        for url in &urls {
            validate_url(url)?;
        }
        if let Some(cb) = req.callback_url.as_deref() {
            validate_url(cb)?;
        }
        if let Some(hint) = req.form_type.as_deref() {
            if !is_known_form_type(hint) {
                return Err(Error::BadRequest(format!(
                    "Unknown form_type \"{}\"; expected {} or {}",
                    hint, FORM_TYPE_INTERVIEW_1, FORM_TYPE_INTERVIEW_2
                )));
            }
        }

        let callback_status = if req.callback_url.is_some() {
            CALLBACK_PENDING
        } else {
            CALLBACK_SKIPPED
        };

        let job = sqlx::query_as::<_, InterviewFormJob>(
            r#"
            INSERT INTO interview_form_jobs
                (status, source_urls, candidate_id, vacancy_id, form_type_hint,
                 callback_url, callback_status, external_ref)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(JOB_STATUS_PENDING)
        .bind(json!(urls))
        .bind(req.candidate_id)
        .bind(req.vacancy_id)
        .bind(req.form_type.as_deref())
        .bind(req.callback_url.as_deref())
        .bind(callback_status)
        .bind(req.external_ref.as_deref())
        .fetch_one(&self.pool)
        .await?;

        Ok(job)
    }

    pub fn spawn(&self, job_id: Uuid) {
        let service = self.clone();
        tokio::spawn(async move {
            if let Err(e) = service.run_job(job_id).await {
                tracing::error!("Interview form job {} failed: {:?}", job_id, e);
                let _ = sqlx::query(
                    "UPDATE interview_form_jobs
                        SET status = $2, error = $3, finished_at = NOW()
                      WHERE id = $1",
                )
                .bind(job_id)
                .bind(JOB_STATUS_FAILED)
                .bind(e.to_string())
                .execute(&service.pool)
                .await;
            }
        });
    }

    async fn run_job(&self, job_id: Uuid) -> Result<()> {
        let job = sqlx::query_as::<_, InterviewFormJob>(
            "UPDATE interview_form_jobs
                SET status = $2, started_at = NOW()
              WHERE id = $1
          RETURNING *",
        )
        .bind(job_id)
        .bind(JOB_STATUS_PROCESSING)
        .fetch_one(&self.pool)
        .await?;

        let urls: Vec<String> = serde_json::from_value(job.source_urls.clone()).unwrap_or_default();

        let mut succeeded = 0usize;
        let mut failures: Vec<String> = Vec::new();

        for (index, url) in urls.iter().enumerate() {
            match self.process_one(&job, url, index as i32).await {
                Ok(_) => succeeded += 1,
                Err(e) => {
                    tracing::warn!("Interview form {} of job {} failed: {:?}", index, job_id, e);
                    failures.push(format!("{}: {}", url, e));
                }
            }
        }

        let status = match (succeeded, failures.is_empty()) {
            (0, _) => JOB_STATUS_FAILED,
            (_, true) => JOB_STATUS_COMPLETED,
            _ => JOB_STATUS_PARTIAL,
        };

        sqlx::query(
            "UPDATE interview_form_jobs
                SET status = $2, error = $3, finished_at = NOW()
              WHERE id = $1",
        )
        .bind(job_id)
        .bind(status)
        .bind((!failures.is_empty()).then(|| failures.join("; ")))
        .execute(&self.pool)
        .await?;

        if job.callback_url.is_some() {
            self.deliver_callback(job_id).await;
        }

        Ok(())
    }

    async fn process_one(
        &self,
        job: &InterviewFormJob,
        url: &str,
        index: i32,
    ) -> Result<InterviewFormRecognition> {
        let raw_bytes = self.fetch_image(url).await?;

        let (page, probe, mut warnings) =
            tokio::task::spawn_blocking(move || decode_and_orient(&raw_bytes))
                .await
                .map_err(|e| Error::Internal(format!("image worker panicked: {}", e)))??;

        let rotation = match self
            .ai_service
            .detect_page_rotation(&BASE64.encode(&probe))
            .await
        {
            Ok(degrees) => degrees,
            Err(e) => {
                let guess = fallback_rotation(&page);
                tracing::warn!(
                    "Rotation probe failed ({:?}); assuming {} degrees for {}",
                    e,
                    guess,
                    url
                );
                warnings.push(format!(
                    "Page orientation was guessed ({} degrees), not detected",
                    guess
                ));
                guess
            }
        };

        let (jpeg, sha) = tokio::task::spawn_blocking(move || finalize_image(page, rotation))
            .await
            .map_err(|e| Error::Internal(format!("image worker panicked: {}", e)))??;

        let encoded = BASE64.encode(&jpeg);
        let (form, raw_output) = self
            .ai_service
            .recognize_interview_form(&encoded, job.form_type_hint.as_deref())
            .await?;

        warnings.extend(form.warnings.iter().cloned());

        if let Some(hint) = job.form_type_hint.as_deref() {
            if form.form_type != hint {
                warnings.push(format!(
                    "Caller expected {} but the printed page title reads {}",
                    hint, form.form_type
                ));
            }
        }

        let assessment = assess(&form, &warnings);

        let mut fields = serde_json::to_value(&form)?;
        if let Some(obj) = fields.as_object_mut() {
            obj.remove("field_confidence");
            obj.remove("warnings");
        }

        let saved = sqlx::query_as::<_, InterviewFormRecognition>(
            r#"
            INSERT INTO interview_form_recognitions
                (job_id, candidate_id, source_url, source_index, image_sha256, form_type,
                 fields, field_confidence, overall_confidence, needs_review,
                 low_confidence_fields, warnings, raw_model_output)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(job.id)
        .bind(job.candidate_id)
        .bind(url)
        .bind(index)
        .bind(&sha)
        .bind(&form.form_type)
        .bind(fields)
        .bind(json!(form.field_confidence))
        .bind(assessment.overall)
        .bind(assessment.needs_review)
        .bind(json!(assessment.low_confidence_fields))
        .bind(json!(warnings))
        .bind(raw_output)
        .fetch_one(&self.pool)
        .await?;

        Ok(saved)
    }

    async fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        let res = self
            .client
            .get(url)
            .timeout(Duration::from_secs(60))
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(Error::BadRequest(format!(
                "Could not download {} ({})",
                url,
                res.status()
            )));
        }

        if let Some(len) = res.content_length() {
            if len as usize > MAX_IMAGE_BYTES {
                return Err(Error::BadRequest(format!(
                    "{} is {} bytes, over the {} byte limit",
                    url, len, MAX_IMAGE_BYTES
                )));
            }
        }

        let bytes = res.bytes().await?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(Error::BadRequest(format!(
                "{} is {} bytes, over the {} byte limit",
                url,
                bytes.len(),
                MAX_IMAGE_BYTES
            )));
        }
        if bytes.is_empty() {
            return Err(Error::BadRequest(format!("{} returned an empty body", url)));
        }

        Ok(bytes.to_vec())
    }

    async fn deliver_callback(&self, job_id: Uuid) {
        let Ok(result) = self.get_job(job_id).await else {
            return;
        };
        let Some(callback_url) = result.job.callback_url.clone() else {
            return;
        };

        let payload = match serde_json::to_value(&result) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("Could not serialise callback for job {}: {:?}", job_id, e);
                return;
            }
        };

        let mut last_error = String::new();
        for attempt in 1..=CALLBACK_MAX_ATTEMPTS {
            let response = self
                .client
                .post(&callback_url)
                .json(&payload)
                .timeout(Duration::from_secs(30))
                .send()
                .await;

            match response {
                Ok(res) if res.status().is_success() => {
                    let _ = sqlx::query(
                        "UPDATE interview_form_jobs
                            SET callback_status = $2, callback_attempts = $3, callback_last_error = NULL
                          WHERE id = $1",
                    )
                    .bind(job_id)
                    .bind(CALLBACK_DELIVERED)
                    .bind(attempt as i32)
                    .execute(&self.pool)
                    .await;
                    return;
                }
                Ok(res) => last_error = format!("HTTP {}", res.status()),
                Err(e) => last_error = e.to_string(),
            }

            if attempt < CALLBACK_MAX_ATTEMPTS {
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
        }

        tracing::warn!(
            "Callback to {} for job {} failed after {} attempts: {}",
            callback_url,
            job_id,
            CALLBACK_MAX_ATTEMPTS,
            last_error
        );
        let _ = sqlx::query(
            "UPDATE interview_form_jobs
                SET callback_status = $2, callback_attempts = $3, callback_last_error = $4
              WHERE id = $1",
        )
        .bind(job_id)
        .bind(CALLBACK_FAILED)
        .bind(CALLBACK_MAX_ATTEMPTS as i32)
        .bind(last_error)
        .execute(&self.pool)
        .await;
    }


    pub async fn get_job(&self, job_id: Uuid) -> Result<JobResult> {
        let job = sqlx::query_as::<_, InterviewFormJob>(
            "SELECT * FROM interview_form_jobs WHERE id = $1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Interview form job {} not found", job_id)))?;

        let forms = sqlx::query_as::<_, InterviewFormRecognition>(
            "SELECT * FROM interview_form_recognitions
              WHERE job_id = $1
           ORDER BY source_index",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(JobResult { job, forms })
    }

    pub async fn get_recognition(&self, id: Uuid) -> Result<InterviewFormRecognition> {
        sqlx::query_as::<_, InterviewFormRecognition>(
            "SELECT * FROM interview_form_recognitions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Recognized form {} not found", id)))
    }

    pub async fn list_review_queue(
        &self,
        candidate_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<InterviewFormRecognition>> {
        let rows = sqlx::query_as::<_, InterviewFormRecognition>(
            "SELECT * FROM interview_form_recognitions
              WHERE needs_review
                AND reviewed_at IS NULL
                AND ($1::uuid IS NULL OR candidate_id = $1)
           ORDER BY created_at DESC
              LIMIT $2",
        )
        .bind(candidate_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn apply_review(
        &self,
        id: Uuid,
        corrected_fields: Option<JsonValue>,
        reviewed_by: Option<Uuid>,
    ) -> Result<InterviewFormRecognition> {
        let updated = sqlx::query_as::<_, InterviewFormRecognition>(
            "UPDATE interview_form_recognitions
                SET corrected_fields = COALESCE($2, corrected_fields),
                    reviewed_by = $3,
                    reviewed_at = NOW(),
                    needs_review = FALSE
              WHERE id = $1
          RETURNING *",
        )
        .bind(id)
        .bind(corrected_fields)
        .bind(reviewed_by)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Recognized form {} not found", id)))?;

        Ok(updated)
    }
}

fn validate_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)
        .map_err(|e| Error::BadRequest(format!("Invalid URL \"{}\": {}", url, e)))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        other => Err(Error::BadRequest(format!(
            "Unsupported URL scheme \"{}\"; only http and https are fetched",
            other
        ))),
    }
}

struct Assessment {
    overall: Option<f32>,
    needs_review: bool,
    low_confidence_fields: Vec<String>,
}

fn assess(form: &RecognizedForm, warnings: &[String]) -> Assessment {
    let mut low: Vec<String> = form
        .field_confidence
        .iter()
        .filter(|(_, score)| **score < FIELD_REVIEW_THRESHOLD)
        .map(|(path, _)| path.clone())
        .collect();
    low.sort();

    let overall = if form.field_confidence.is_empty() {
        None
    } else {
        let sum: f32 = form.field_confidence.values().sum();
        Some(sum / form.field_confidence.len() as f32)
    };

    let mut needs_review = false;

    // An unrecognised template means we do not know what we just read.
    if !is_known_form_type(&form.form_type) {
        needs_review = true;
    }

    // No scores at all is not a clean bill of health.
    match overall {
        None => needs_review = true,
        Some(value) if value < OVERALL_REVIEW_THRESHOLD => needs_review = true,
        _ => {}
    }

    // A filled-in critical field must come with a good score of its own. A
    // blank one is fine — plenty of these boxes are legitimately left empty.
    for path in CRITICAL_FIELD_PATHS {
        if !has_value_at(form, path) {
            continue;
        }
        match form.field_confidence.get(*path) {
            Some(score) if *score >= FIELD_REVIEW_THRESHOLD => {}
            Some(_) => needs_review = true,
            None => {
                needs_review = true;
                low.push((*path).to_string());
            }
        }
    }

    if !warnings.is_empty() {
        needs_review = true;
    }

    low.dedup();

    Assessment { overall, needs_review, low_confidence_fields: low }
}

fn has_value_at(form: &RecognizedForm, path: &str) -> bool {
    if let Some(key) = path.strip_prefix("parameters.") {
        return form
            .parameters
            .iter()
            .any(|p| p.key.as_deref() == Some(key) && non_empty(p.value.as_deref()));
    }

    match path {
        "candidate_name" => non_empty(form.candidate_name.as_deref()),
        "interview_date" => non_empty(form.interview_date.as_deref()),
        "scheduled_start_time" => non_empty(form.scheduled_start_time.as_deref()),
        "actual_arrival_time" => non_empty(form.actual_arrival_time.as_deref()),
        "interview_from" => non_empty(form.interview_from.as_deref()),
        "interview_to" => non_empty(form.interview_to.as_deref()),
        _ => false,
    }
}

fn non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

fn decode_and_orient(bytes: &[u8]) -> Result<(image::DynamicImage, Vec<u8>, Vec<String>)> {
    let mut warnings = Vec::new();

    let decoded = image::load_from_memory(bytes)
        .map_err(|e| Error::BadRequest(format!("Not a readable image: {}", e)))?;

    let oriented = match exif_orientation(bytes) {
        Some(orientation) => apply_orientation(decoded, orientation),
        None => decoded,
    };

    if oriented.width() < 800 || oriented.height() < 800 {
        warnings.push(format!(
            "Low resolution source ({}x{}); handwriting may not be legible",
            oriented.width(),
            oriented.height()
        ));
    }

    // Downscale once, here. A 12-megapixel Lanczos pass is by far the most
    // expensive step, and everything downstream — the probe, the rotation, the
    // JPEG we upload — works off this copy.
    let page = if oriented.width().max(oriented.height()) > MAX_IMAGE_EDGE {
        oriented.resize(MAX_IMAGE_EDGE, MAX_IMAGE_EDGE, FilterType::Lanczos3)
    } else {
        oriented
    };

    let probe = encode_jpeg(
        &page.resize(PROBE_EDGE, PROBE_EDGE, FilterType::Triangle).into_rgb8(),
        PROBE_QUALITY,
    )?;

    Ok((page, probe, warnings))
}

fn finalize_image(page: image::DynamicImage, rotation_cw: u32) -> Result<(Vec<u8>, String)> {
    let upright = match rotation_cw {
        90 => page.rotate90(),
        180 => page.rotate180(),
        270 => page.rotate270(),
        _ => page,
    };

    let out = encode_jpeg(&upright.into_rgb8(), JPEG_QUALITY)?;
    let sha = hex::encode(Sha256::digest(&out));

    Ok((out, sha))
}

fn fallback_rotation(page: &image::DynamicImage) -> u32 {
    if page.width() > page.height() {
        90
    } else {
        0
    }
}

fn encode_jpeg(image: &image::RgbImage, quality: u8) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
        .encode_image(image)
        .map_err(|e| Error::Internal(format!("Could not re-encode image: {}", e)))?;
    Ok(out)
}

fn exif_orientation(bytes: &[u8]) -> Option<u32> {
    let mut cursor = std::io::Cursor::new(bytes);
    let exif = exif::Reader::new().read_from_container(&mut cursor).ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?
        .value
        .get_uint(0)
}

fn apply_orientation(img: image::DynamicImage, orientation: u32) -> image::DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn form_with(confidence: &[(&str, f32)]) -> RecognizedForm {
        RecognizedForm {
            form_type: FORM_TYPE_INTERVIEW_1.to_string(),
            candidate_name: Some("Шарипова Фируза".into()),
            interview_date: Some("2026-07-02".into()),
            field_confidence: confidence
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<BTreeMap<_, _>>(),
            ..Default::default()
        }
    }

    #[test]
    fn clean_read_of_a_known_template_passes() {
        let form = form_with(&[("candidate_name", 0.96), ("interview_date", 0.94)]);
        let assessment = assess(&form, &[]);
        assert!(!assessment.needs_review);
        assert!(assessment.low_confidence_fields.is_empty());
    }

    #[test]
    fn a_shaky_critical_field_forces_review() {
        let form = form_with(&[("candidate_name", 0.95), ("interview_date", 0.40)]);
        let assessment = assess(&form, &[]);
        assert!(assessment.needs_review);
        assert!(assessment
            .low_confidence_fields
            .contains(&"interview_date".to_string()));
    }

    #[test]
    fn a_filled_critical_field_with_no_score_forces_review() {
        let form = form_with(&[("candidate_name", 0.99)]);
        let assessment = assess(&form, &[]);
        assert!(assessment.needs_review);
        assert!(assessment
            .low_confidence_fields
            .contains(&"interview_date".to_string()));
    }

    #[test]
    fn a_blank_critical_field_is_not_held_against_the_sheet() {
        let mut form = form_with(&[("candidate_name", 0.97), ("interview_date", 0.95)]);
        form.interview_to = None;
        let assessment = assess(&form, &[]);
        assert!(!assessment.needs_review);
    }

    #[test]
    fn an_unreadable_title_forces_review() {
        let mut form = form_with(&[("candidate_name", 0.99), ("interview_date", 0.99)]);
        form.form_type = FORM_TYPE_UNKNOWN.to_string();
        assert!(assess(&form, &[]).needs_review);
    }

    #[test]
    fn a_page_level_warning_forces_review() {
        let form = form_with(&[("candidate_name", 0.99), ("interview_date", 0.99)]);
        let assessment = assess(&form, &["Glare across the comments block".to_string()]);
        assert!(assessment.needs_review);
    }

    #[test]
    fn salary_is_treated_as_critical_when_present() {
        let mut form = form_with(&[
            ("candidate_name", 0.99),
            ("interview_date", 0.99),
            ("parameters.salary_expectation", 0.5),
        ]);
        form.parameters.push(ParameterRow {
            key: Some("salary_expectation".into()),
            label: "ЗП (минимальный/ожидаемый)".into(),
            value: Some("На ИС 10.000 после 12.000".into()),
        });
        assert!(assess(&form, &[]).needs_review);
    }

    #[test]
    fn urls_dedupe_and_prefer_the_array() {
        let req = RecognizeRequest {
            image_url: Some("https://example.test/a.jpg".into()),
            image_urls: Some(vec![
                "https://example.test/b.jpg".into(),
                " https://example.test/b.jpg ".into(),
                "https://example.test/c.jpg".into(),
            ]),
            candidate_id: None,
            vacancy_id: None,
            form_type: None,
            callback_url: None,
            external_ref: None,
        };
        assert_eq!(
            req.urls(),
            vec![
                "https://example.test/b.jpg".to_string(),
                "https://example.test/c.jpg".to_string()
            ]
        );
    }

    #[test]
    fn sample_photos_decode_and_fit_the_model_budget() {
        let Ok(entries) = std::fs::read_dir("assets") else {
            eprintln!("skipping: no assets/ directory");
            return;
        };

        let mut checked = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
                continue;
            }

            let bytes = std::fs::read(&path).expect("readable sample");
            let (page, probe, _warnings) =
                decode_and_orient(&bytes).unwrap_or_else(|e| panic!("{:?}: {}", path, e));

            let probe_img = image::load_from_memory(&probe).expect("probe decodes");
            assert!(
                probe_img.width().max(probe_img.height()) <= PROBE_EDGE,
                "{:?} probe is {}x{}",
                path,
                probe_img.width(),
                probe_img.height()
            );

            let rotation = fallback_rotation(&page);
            let (jpeg, sha) =
                finalize_image(page, rotation).unwrap_or_else(|e| panic!("{:?}: {}", path, e));

            let out = image::load_from_memory(&jpeg).expect("re-encoded output decodes");
            assert!(
                out.width().max(out.height()) <= MAX_IMAGE_EDGE,
                "{:?} came out {}x{}, over the {}px budget",
                path,
                out.width(),
                out.height(),
                MAX_IMAGE_EDGE
            );  
            assert!(
                out.height() > out.width(),
                "{:?} came out landscape ({}x{})",
                path,
                out.width(),
                out.height()
            );
            assert_eq!(sha.len(), 64);
            checked += 1;
        }

        assert!(checked > 0, "assets/ held no .jpg samples");
        eprintln!("normalised {} sample photos", checked);
    }

    #[tokio::test]
    async fn live_recognition_reads_a_sideways_sheet() {
        if std::env::var("RUN_LIVE_OCR").is_err() {
            eprintln!("skipping: set RUN_LIVE_OCR=1 to run");
            return;
        }
        let Ok(api_key) = std::env::var("OPENAI_API_KEY") else {
            eprintln!("skipping: no OPENAI_API_KEY");
            return;
        };
        let path = std::path::Path::new("assets/20260820_095058.jpg");
        if !path.exists() {
            eprintln!("skipping: sample photo not present");
            return;
        }

        let ai = AIService::new(
            api_key,
            std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            Client::new(),
        );

        let bytes = std::fs::read(path).unwrap();
        let (page, probe, _) = decode_and_orient(&bytes).unwrap();

        let rotation = ai.detect_page_rotation(&BASE64.encode(&probe)).await.unwrap();
        eprintln!("detected rotation: {} degrees", rotation);
        assert_eq!(rotation, 90, "this sheet is stored on its side");

        let (jpeg, _sha) = finalize_image(page, rotation).unwrap();
        let (form, _raw) = ai
            .recognize_interview_form(&BASE64.encode(&jpeg), None)
            .await
            .unwrap();

        eprintln!("{}", serde_json::to_string_pretty(&form).unwrap());

        assert_eq!(form.form_type, FORM_TYPE_INTERVIEW_1);
        assert!(
            form.candidate_name.as_deref().unwrap_or("").contains("аидов"),
            "candidate_name was {:?}",
            form.candidate_name
        );
        assert_eq!(form.interview_date.as_deref(), Some("2026-08-17"));
        assert_eq!(form.scheduled_start_time.as_deref(), Some("11:00"));
    }

    #[test]
    fn rotation_is_only_guessed_for_landscape_frames() {
        let portrait = image::DynamicImage::new_rgb8(800, 1200);
        let landscape = image::DynamicImage::new_rgb8(1200, 800);
        assert_eq!(fallback_rotation(&portrait), 0);
        assert_eq!(fallback_rotation(&landscape), 90);
    }

    #[tokio::test]
    async fn db_roundtrip_over_every_statement() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("skipping: no DATABASE_URL");
            return;
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("connect");
        let svc = InterviewFormService::new(pool.clone(), dummy_ai(), Client::new());

        // create_job
        let req = RecognizeRequest {
            image_url: None,
            image_urls: Some(vec![
                "https://files.test/a.jpg".into(),
                "https://files.test/b.jpg".into(),
            ]),
            candidate_id: None,
            vacancy_id: Some(1042),
            form_type: Some(FORM_TYPE_INTERVIEW_1.into()),
            callback_url: Some("https://1f.test/hook".into()),
            external_ref: Some("1F-DOC-8871".into()),
        };
        let job = svc.create_job(&req).await.expect("create_job");
        assert_eq!(job.status, JOB_STATUS_PENDING);
        assert_eq!(job.callback_status, CALLBACK_PENDING);
        assert_eq!(job.vacancy_id, Some(1042));

        let form = RecognizedForm {
            form_type: FORM_TYPE_INTERVIEW_1.to_string(),
            candidate_name: Some("Саидов Самиуллох".into()),
            interview_date: Some("2026-08-17".into()),
            conclusions: Some("Есть своя строит. Ко.".into()),
            extra_notes: vec!["ЗП: 2мес. - 20000, 3/4 - 25000".into()],
            ..Default::default()
        };
        let assessment = assess(&form, &["glare".to_string()]);
        let mut fields = serde_json::to_value(&form).unwrap();
        fields.as_object_mut().unwrap().remove("field_confidence");
        fields.as_object_mut().unwrap().remove("warnings");

        let saved = sqlx::query_as::<_, InterviewFormRecognition>(
            r#"
            INSERT INTO interview_form_recognitions
                (job_id, candidate_id, source_url, source_index, image_sha256, form_type,
                 fields, field_confidence, overall_confidence, needs_review,
                 low_confidence_fields, warnings, raw_model_output)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING *
            "#,
        )
        .bind(job.id)
        .bind(job.candidate_id)
        .bind("https://files.test/a.jpg")
        .bind(0i32)
        .bind("a".repeat(64))
        .bind(&form.form_type)
        .bind(fields)
        .bind(json!(form.field_confidence))
        .bind(assessment.overall)
        .bind(assessment.needs_review)
        .bind(json!(assessment.low_confidence_fields))
        .bind(json!(["glare"]))
        .bind(json!({"raw": "output"}))
        .fetch_one(&pool)
        .await
        .expect("recognition INSERT");

        assert!(saved.needs_review, "no confidence scores => must be flagged");
        assert_eq!(saved.fields["conclusions"], "Есть своя строит. Ко.");

        sqlx::query(
            "UPDATE interview_form_jobs
                SET status = $2, error = $3, finished_at = NOW()
              WHERE id = $1",
        )
        .bind(job.id)
        .bind(JOB_STATUS_PARTIAL)
        .bind(Some("https://files.test/b.jpg: boom".to_string()))
        .execute(&pool)
        .await
        .expect("job status UPDATE");

        let result = svc.get_job(job.id).await.expect("get_job");
        assert_eq!(result.job.status, JOB_STATUS_PARTIAL);
        assert_eq!(result.forms.len(), 1);
        assert!(serde_json::to_value(&result).is_ok(), "callback body serialises");

        svc.get_recognition(saved.id).await.expect("get_recognition");

        let queue = svc.list_review_queue(None, 50).await.expect("review queue");
        assert!(queue.iter().any(|f| f.id == saved.id));

        sqlx::query(
            "UPDATE interview_form_jobs
                SET callback_status = $2, callback_attempts = $3, callback_last_error = $4
              WHERE id = $1",
        )
        .bind(job.id)
        .bind(CALLBACK_FAILED)
        .bind(CALLBACK_MAX_ATTEMPTS as i32)
        .bind("HTTP 502".to_string())
        .execute(&pool)
        .await
        .expect("callback UPDATE");

        let reviewed = svc
            .apply_review(
                saved.id,
                Some(json!({"candidate_name": "Саидов С. Д."})),
                None,
            )
            .await
            .expect("apply_review");
        assert!(!reviewed.needs_review);
        assert!(reviewed.reviewed_at.is_some());
        assert_eq!(reviewed.corrected_fields.unwrap()["candidate_name"], "Саидов С. Д.");
        assert_eq!(
            reviewed.fields["candidate_name"], "Саидов Самиуллох",
            "the model's own reading must survive the correction"
        );

        let after = svc.list_review_queue(None, 50).await.unwrap();
        assert!(!after.iter().any(|f| f.id == saved.id), "leaves the queue");

        svc.list_review_queue(Some(Uuid::new_v4()), 10)
            .await
            .expect("filtered review queue");

        sqlx::query("DELETE FROM interview_form_jobs WHERE id = $1")
            .bind(job.id)
            .execute(&pool)
            .await
            .unwrap();
    }

    fn dummy_ai() -> AIService {
        AIService::new(
            "unused".into(),
            "https://api.openai.com/v1".into(),
            Client::new(),
        )
    }

    #[test]
    fn only_http_urls_are_fetched() {
        assert!(validate_url("https://example.test/a.jpg").is_ok());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("not a url").is_err());
    }
}
