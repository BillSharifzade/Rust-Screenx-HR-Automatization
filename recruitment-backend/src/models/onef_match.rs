use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;

use crate::models::onef_vacancy::de_i64_flexible;

pub const PROMPT_VERSION: &str = "match-v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CandidateProfile {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub education_level: Option<String>,
    #[serde(default)]
    pub education_detail: Option<String>,
    #[serde(default)]
    pub total_experience_years: Option<f32>,
    #[serde(default)]
    pub current_role: Option<String>,
    #[serde(default)]
    pub specialties: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub computer_skills: Vec<String>,
    #[serde(default)]
    pub professional_skills: Vec<String>,
    #[serde(default)]
    pub personal_qualities: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchBreakdown {
    #[serde(default)]
    pub education: i32,
    #[serde(default)]
    pub experience: i32,
    #[serde(default)]
    pub professional_skills: i32,
    #[serde(default)]
    pub languages: i32,
    #[serde(default)]
    pub computer_skills: i32,
    #[serde(default)]
    pub specialty: i32,
    #[serde(default)]
    pub personal_qualities: i32,
}

impl MatchBreakdown {
    fn clamp(&mut self) {
        for v in [
            &mut self.education,
            &mut self.experience,
            &mut self.professional_skills,
            &mut self.languages,
            &mut self.computer_skills,
            &mut self.specialty,
            &mut self.personal_qualities,
        ] {
            *v = (*v).clamp(0, 100);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VacancyMatch {
    #[serde(deserialize_with = "de_i64_flexible")]
    pub vacancy_id_1f: i64,
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub breakdown: MatchBreakdown,
    #[serde(default)]
    pub matched: Vec<String>,
    #[serde(default)]
    pub missing: Vec<String>,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchFlags {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_requirement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_mismatch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender_requirement: Option<String>,
    pub data_quality: i32,
    pub low_data_quality: bool,
    pub low_confidence: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgeRange {
    pub min: Option<u32>,
    pub max: Option<u32>,
}

impl AgeRange {
    pub fn contains(&self, age: u32) -> bool {
        self.min.map_or(true, |m| age >= m) && self.max.map_or(true, |m| age <= m)
    }
}

pub fn parse_age_requirement(raw: &str) -> Option<AgeRange> {
    let lower = raw.to_lowercase();
    if lower.contains("не имеет значения") {
        return None;
    }

    let numbers = extract_numbers(&lower);
    if numbers.is_empty() {
        return None;
    }

    let has_from = lower.contains("от");
    let has_to = lower.contains("до");

    Some(match (has_from, has_to) {
        (true, true) if numbers.len() >= 2 => AgeRange {
            min: Some(numbers[0]),
            max: Some(numbers[1]),
        },
        (true, false) => AgeRange {
            min: Some(numbers[0]),
            max: None,
        },
        (false, true) => AgeRange {
            min: None,
            max: Some(numbers[0]),
        },
        _ if numbers.len() >= 2 => AgeRange {
            min: Some(numbers[0]),
            max: Some(numbers[1]),
        },
        _ => AgeRange {
            min: Some(numbers[0]),
            max: None,
        },
    })
}

fn extract_numbers(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            current.push(c);
        } else if !current.is_empty() {
            if let Ok(n) = current.parse() {
                out.push(n);
            }
            current.clear();
        }
    }
    if let Ok(n) = current.parse() {
        out.push(n);
    }
    out
}

pub fn age_on(dob: NaiveDate, today: NaiveDate) -> u32 {
    let mut age = today.year() - dob.year();
    if (today.month(), today.day()) < (dob.month(), dob.day()) {
        age -= 1;
    }
    age.max(0) as u32
}

pub fn sanitize_matches(raw: &JsonValue, allowed: &HashSet<i64>) -> Vec<VacancyMatch> {
    let items = raw
        .get("matches")
        .and_then(JsonValue::as_array)
        .or_else(|| raw.as_array())
        .cloned()
        .unwrap_or_default();

    let mut seen = HashSet::new();

pub fn sanitize_matches(raw: &JsonValue, allowed: &HashSet<i64>) -> Vec<VacancyMatch> {
    let items = raw
        .get("matches")
        .and_then(JsonValue::as_array)
        .or_else(|| raw.as_array())
        .cloned()
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let mut out: Vec<VacancyMatch> = Vec::with_capacity(items.len());

    for item in items {
        let Ok(mut m) = serde_json::from_value::<VacancyMatch>(item) else {
            continue;
        };
        if !allowed.contains(&m.vacancy_id_1f) || !seen.insert(m.vacancy_id_1f) {
            continue;
        }
        m.score = m.score.clamp(0, 100);
        m.breakdown.clamp();
        m.matched.retain(|s| !s.trim().is_empty());
        m.missing.retain(|s| !s.trim().is_empty());
        out.push(m);
    }

    out.sort_by(|a, b| b.score.cmp(&a.score).then(a.vacancy_id_1f.cmp(&b.vacancy_id_1f)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_the_age_forms_1f_actually_sends() {
        assert_eq!(
            parse_age_requirement("от 18 до 25 лет"),
            Some(AgeRange { min: Some(18), max: Some(25) })
        );
        assert_eq!(
            parse_age_requirement("от 25 лет"),
            Some(AgeRange { min: Some(25), max: None })
        );
        assert_eq!(
            parse_age_requirement("до 40 лет"),
            Some(AgeRange { min: None, max: Some(40) })
        );
        assert_eq!(parse_age_requirement("не имеет значения"), None);
        assert_eq!(parse_age_requirement(""), None);
    }

    #[test]
    fn age_range_bounds_are_inclusive() {
        let r = parse_age_requirement("от 20 до 30 лет").unwrap();
        assert!(r.contains(20) && r.contains(30) && r.contains(25));
        assert!(!r.contains(19) && !r.contains(31));
    }

    #[test]
    fn computes_age_accounting_for_birthday_not_yet_passed() {
        let dob = NaiveDate::from_ymd_opt(2000, 6, 15).unwrap();
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2026, 6, 14).unwrap()), 25);
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()), 26);
        assert_eq!(age_on(dob, NaiveDate::from_ymd_opt(2026, 8, 21).unwrap()), 26);
    }

    #[test]
    fn drops_vacancies_the_model_invented() {
        let allowed: HashSet<i64> = [29698, 29502].into_iter().collect();
        let raw = json!({"matches": [
            {"vacancy_id_1f": 29698, "score": 80},
            {"vacancy_id_1f": 11111, "score": 99},
            {"vacancy_id_1f": 29502, "score": 40}
        ]});
        let out = sanitize_matches(&raw, &allowed);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].vacancy_id_1f, 29698);
        assert_eq!(out[1].vacancy_id_1f, 29502);
    }

    #[test]
    fn clamps_scores_and_drops_duplicates() {
        let allowed: HashSet<i64> = [1, 2].into_iter().collect();
        let raw = json!({"matches": [
            {"vacancy_id_1f": 1, "score": 250, "breakdown": {"education": -30}},
            {"vacancy_id_1f": 1, "score": 10},
            {"vacancy_id_1f": "2", "score": -5}
        ]});
        let out = sanitize_matches(&raw, &allowed);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].score, 100);
        assert_eq!(out[0].breakdown.education, 0);
        assert_eq!(out[1].score, 0);
        assert_eq!(out[1].vacancy_id_1f, 2);
    }

    #[test]
    fn accepts_a_bare_array_too() {
        let allowed: HashSet<i64> = [7].into_iter().collect();
        let out = sanitize_matches(&json!([{"vacancy_id_1f": 7, "score": 55}]), &allowed);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].score, 55);
    }
}
