use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashSet;
use tracing::info;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::onef_match::{
    age_on, parse_age_requirement, sanitize_matches, CandidateProfile, MatchBreakdown, MatchFlags,
    PROMPT_VERSION,
};
use crate::models::onef_vacancy::OneFVacancy;
use crate::services::ai_service::AIService;
use crate::services::onef_vacancy_service::OneFVacancyService;

const MIN_CV_CHARS: usize = 100;
const LOW_QUALITY_THRESHOLD: i32 = 75;
const MODEL: &str = "gpt-4o";

#[derive(Debug, Clone, Serialize)]
pub struct ScoredVacancy {
    pub vacancy_id_1f: i64,
    pub external_vacancy_id: Option<String>,
    pub name: String,
    pub company: Option<String>,
    pub score: i32,
    pub rank: i32,
    pub breakdown: MatchBreakdown,
    pub matched: Vec<String>,
    pub missing: Vec<String>,
    pub comment: String,
    pub flags: MatchFlags,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateMatchResult {
    pub candidate_id: Uuid,
    pub candidate_name: String,
    pub cached: bool,
    pub computed_at: DateTime<Utc>,
    pub catalogue_size: usize,
    pub cv_chars: usize,
    pub profile: CandidateProfile,
    pub matches: Vec<ScoredVacancy>,
}

#[derive(Clone)]
pub struct OneFMatchService {
    pool: PgPool,
    ai: AIService,
    vacancies: OneFVacancyService,
}

impl OneFMatchService {
    pub fn new(pool: PgPool, ai: AIService, vacancies: OneFVacancyService) -> Self {
        Self { pool, ai, vacancies }
    }

    pub async fn rank_for_candidate(
        &self,
        candidate_id: Uuid,
        top_n: usize,
        include_low_quality: bool,
        force_refresh: bool,
    ) -> Result<CandidateMatchResult> {
        let candidate = sqlx::query_as::<_, CandidateFacts>(
            "SELECT id, name, dob, cv_url, profile_data FROM candidates WHERE id = $1",
        )
        .bind(candidate_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Candidate {} not found", candidate_id)))?;

        let mut catalogue = self.vacancies.list(false).await?;
        if !include_low_quality {
            catalogue.retain(|v| v.data_quality >= LOW_QUALITY_THRESHOLD);
        }
        if catalogue.is_empty() {
            return Err(Error::BadRequest(
                "The 1F vacancy catalogue is empty — run a sync first".into(),
            ));
        }

        let cv_text = match candidate.cv_url.as_deref() {
            Some(path) => crate::routes::candidate_routes::extract_text_from_file(path).await,
            None => String::new(),
        };
        let cv_chars = cv_text.trim().chars().count();
        let age = candidate.dob.map(|d| age_on(d, Utc::now().date_naive()));

        let source_hash = source_hash(&candidate, &cv_text);
        let catalogue_hash = catalogue_hash(&catalogue);

        if !force_refresh {
            if let Some(cached) = self
                .load_cached(candidate_id, &source_hash, &catalogue_hash)
                .await?
            {
                let profile = self.load_profile(candidate_id).await?.unwrap_or_default();
                info!("Serving cached 1F ranking for candidate {}", candidate_id);
                return Ok(CandidateMatchResult {
                    candidate_id,
                    candidate_name: candidate.name,
                    cached: true,
                    computed_at: cached.1,
                    catalogue_size: catalogue.len(),
                    cv_chars,
                    profile,
                    matches: take_top(cached.0, top_n),
                });
            }
        }

        let profile = self
            .profile_for(&candidate, &cv_text, age, &source_hash)
            .await?;

        let raw = self.ai.rank_vacancies(&profile, &catalogue).await?;
        let allowed: HashSet<i64> = catalogue.iter().map(|v| v.vacancy_id_1f).collect();
        let ranked = sanitize_matches(&raw, &allowed);

        if ranked.len() < catalogue.len() {
            tracing::warn!(
                "Model scored {} of {} vacancies for candidate {}",
                ranked.len(),
                catalogue.len(),
                candidate_id
            );
        }

        let scored: Vec<ScoredVacancy> = ranked
            .into_iter()
            .enumerate()
            .filter_map(|(idx, m)| {
                let vacancy = catalogue.iter().find(|v| v.vacancy_id_1f == m.vacancy_id_1f)?;
                Some(ScoredVacancy {
                    vacancy_id_1f: m.vacancy_id_1f,
                    external_vacancy_id: vacancy.external_vacancy_id.clone(),
                    name: vacancy.name.clone(),
                    company: vacancy.company.clone(),
                    score: m.score,
                    rank: idx as i32 + 1,
                    breakdown: m.breakdown,
                    matched: m.matched,
                    missing: m.missing,
                    comment: m.comment,
                    flags: build_flags(vacancy, age, cv_chars),
                })
            })
            .collect();

        let computed_at = Utc::now();
        self.store(candidate_id, &scored, &source_hash, &catalogue_hash)
            .await?;

        info!(
            "Ranked {} vacancies for candidate {} (top score {})",
            scored.len(),
            candidate_id,
            scored.first().map(|s| s.score).unwrap_or(0)
        );

        Ok(CandidateMatchResult {
            candidate_id,
            candidate_name: candidate.name,
            cached: false,
            computed_at,
            catalogue_size: catalogue.len(),
            cv_chars,
            profile,
            matches: take_top(scored, top_n),
        })
    }

    async fn profile_for(
        &self,
        candidate: &CandidateFacts,
        cv_text: &str,
        age: Option<u32>,
        source_hash: &str,
    ) -> Result<CandidateProfile> {
        let existing = sqlx::query_as::<_, (String, JsonValue)>(
            "SELECT source_hash, profile FROM onef_candidate_profiles WHERE candidate_id = $1",
        )
        .bind(candidate.id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((hash, profile)) = existing {
            if hash == source_hash {
                return Ok(serde_json::from_value(profile).unwrap_or_default());
            }
        }

        let profile = self
            .ai
            .extract_candidate_profile(
                &candidate.name,
                age,
                cv_text,
                candidate.profile_data.as_ref(),
            )
            .await?;

        sqlx::query(
            "INSERT INTO onef_candidate_profiles (candidate_id, source_hash, profile, cv_chars, model) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (candidate_id) DO UPDATE SET \
                source_hash = EXCLUDED.source_hash, profile = EXCLUDED.profile, \
                cv_chars = EXCLUDED.cv_chars, model = EXCLUDED.model, extracted_at = NOW()",
        )
        .bind(candidate.id)
        .bind(source_hash)
        .bind(serde_json::to_value(&profile)?)
        .bind(cv_text.trim().chars().count() as i32)
        .bind(MODEL)
        .execute(&self.pool)
        .await?;

        Ok(profile)
    }

    async fn load_profile(&self, candidate_id: Uuid) -> Result<Option<CandidateProfile>> {
        let row = sqlx::query_as::<_, (JsonValue,)>(
            "SELECT profile FROM onef_candidate_profiles WHERE candidate_id = $1",
        )
        .bind(candidate_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|(p,)| serde_json::from_value(p).ok()))
    }

    async fn load_cached(
        &self,
        candidate_id: Uuid,
        source_hash: &str,
        catalogue_hash: &str,
    ) -> Result<Option<(Vec<ScoredVacancy>, DateTime<Utc>)>> {
        let rows = sqlx::query_as::<_, MatchRow>(
            r#"
            SELECT m.vacancy_id_1f, v.external_vacancy_id, v.name, v.company,
                   m.score, m.rank, m.breakdown, m.matched, m.missing, m.comment,
                   m.flags, m.computed_at
            FROM onef_candidate_matches m
            JOIN onef_vacancies v ON v.vacancy_id_1f = m.vacancy_id_1f
            WHERE m.candidate_id = $1 AND m.source_hash = $2
              AND m.catalogue_hash = $3 AND m.prompt_version = $4
            ORDER BY m.rank
            "#,
        )
        .bind(candidate_id)
        .bind(source_hash)
        .bind(catalogue_hash)
        .bind(PROMPT_VERSION)
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let computed_at = rows[0].computed_at;
        Ok(Some((rows.into_iter().map(Into::into).collect(), computed_at)))
    }

    async fn store(
        &self,
        candidate_id: Uuid,
        scored: &[ScoredVacancy],
        source_hash: &str,
        catalogue_hash: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Replace wholesale rather than upsert: the catalogue may have lost vacancies
        // since the last run, and stale rankings must not linger.
        sqlx::query("DELETE FROM onef_candidate_matches WHERE candidate_id = $1")
            .bind(candidate_id)
            .execute(&mut *tx)
            .await?;

        for s in scored {
            sqlx::query(
                r#"
                INSERT INTO onef_candidate_matches (
                    candidate_id, vacancy_id_1f, score, rank, breakdown, matched, missing,
                    comment, flags, model, prompt_version, catalogue_hash, source_hash
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                "#,
            )
            .bind(candidate_id)
            .bind(s.vacancy_id_1f)
            .bind(s.score)
            .bind(s.rank)
            .bind(serde_json::to_value(&s.breakdown)?)
            .bind(&s.matched)
            .bind(&s.missing)
            .bind(&s.comment)
            .bind(serde_json::to_value(&s.flags)?)
            .bind(MODEL)
            .bind(PROMPT_VERSION)
            .bind(catalogue_hash)
            .bind(source_hash)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
struct CandidateFacts {
    id: Uuid,
    name: String,
    dob: Option<chrono::NaiveDate>,
    cv_url: Option<String>,
    profile_data: Option<JsonValue>,
}

#[derive(Debug, sqlx::FromRow)]
struct MatchRow {
    vacancy_id_1f: i64,
    external_vacancy_id: Option<String>,
    name: String,
    company: Option<String>,
    score: i32,
    rank: i32,
    breakdown: Option<JsonValue>,
    matched: Vec<String>,
    missing: Vec<String>,
    comment: Option<String>,
    flags: Option<JsonValue>,
    computed_at: DateTime<Utc>,
}

impl From<MatchRow> for ScoredVacancy {
    fn from(r: MatchRow) -> Self {
        ScoredVacancy {
            vacancy_id_1f: r.vacancy_id_1f,
            external_vacancy_id: r.external_vacancy_id,
            name: r.name,
            company: r.company,
            score: r.score,
            rank: r.rank,
            breakdown: r
                .breakdown
                .and_then(|b| serde_json::from_value(b).ok())
                .unwrap_or_default(),
            matched: r.matched,
            missing: r.missing,
            comment: r.comment.unwrap_or_default(),
            flags: r
                .flags
                .and_then(|f| serde_json::from_value(f).ok())
                .unwrap_or_default(),
        }
    }
}

fn take_top(mut scored: Vec<ScoredVacancy>, top_n: usize) -> Vec<ScoredVacancy> {
    scored.truncate(top_n);
    scored
}

fn build_flags(vacancy: &OneFVacancy, candidate_age: Option<u32>, cv_chars: usize) -> MatchFlags {
    let requirement = vacancy.age.as_deref().filter(|s| !s.trim().is_empty());
    let range = requirement.and_then(parse_age_requirement);

    MatchFlags {
        age_requirement: range.and(requirement.map(str::to_string)),
        age_mismatch: match (range, candidate_age) {
            (Some(r), Some(age)) => Some(!r.contains(age)),
            _ => None,
        },
        gender_requirement: vacancy
            .gender
            .clone()
            .filter(|g| !g.to_lowercase().contains("не имеет значения")),
        data_quality: vacancy.data_quality,
        low_data_quality: vacancy.data_quality < LOW_QUALITY_THRESHOLD,
        low_confidence: cv_chars < MIN_CV_CHARS,
    }
}

fn source_hash(candidate: &CandidateFacts, cv_text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(candidate.name.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(candidate.dob.map(|d| d.to_string()).unwrap_or_default());
    hasher.update(b"\x1f");
    hasher.update(
        candidate
            .profile_data
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default(),
    );
    hasher.update(b"\x1f");
    hasher.update(cv_text.as_bytes());
    hex::encode(hasher.finalize())
}

fn catalogue_hash(vacancies: &[OneFVacancy]) -> String {
    let mut parts: Vec<String> = vacancies
        .iter()
        .map(|v| format!("{}:{}", v.vacancy_id_1f, v.content_hash))
        .collect();
    parts.sort();

    let mut hasher = Sha256::new();
    hasher.update(parts.join("\x1f").as_bytes());
    hasher.update(PROMPT_VERSION.as_bytes());
    hex::encode(hasher.finalize())
}
