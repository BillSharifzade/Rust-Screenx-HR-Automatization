use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::FromRow;

use crate::utils::text_clean::{clean_description, clean_text};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OneFVacancy {
    pub vacancy_id_1f: i64,
    pub external_vacancy_id: Option<String>,
    pub name: String,
    pub company: Option<String>,
    pub specialty: Option<String>,
    pub education: Option<String>,
    pub experience: Option<String>,
    pub age: Option<String>,
    pub gender: Option<String>,
    pub languages: Vec<String>,
    pub computer_skills: Vec<String>,
    pub professional_skills: Option<String>,
    pub personal_qualities: Option<String>,
    pub description_raw: Option<String>,
    pub description_clean: Option<String>,
    pub raw: JsonValue,
    pub data_quality: i32,
    pub content_hash: String,
    pub is_active: bool,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OneFVacancyPayload {
    #[serde(rename = "VacancyId1F", deserialize_with = "de_i64_flexible")]
    pub vacancy_id_1f: i64,

    #[serde(rename = "ExternalVacancyId", default, deserialize_with = "de_opt_string_flexible")]
    pub external_vacancy_id: Option<String>,

    #[serde(rename = "Name", default, deserialize_with = "de_opt_string_flexible")]
    pub name: Option<String>,

    #[serde(rename = "Company", default, deserialize_with = "de_opt_string_flexible")]
    pub company: Option<String>,

    #[serde(rename = "Specialty", default, deserialize_with = "de_opt_string_flexible")]
    pub specialty: Option<String>,

    #[serde(rename = "Education", default, deserialize_with = "de_opt_string_flexible")]
    pub education: Option<String>,

    #[serde(rename = "Experience", default, deserialize_with = "de_opt_string_flexible")]
    pub experience: Option<String>,

    #[serde(rename = "Age", default, deserialize_with = "de_opt_string_flexible")]
    pub age: Option<String>,

    #[serde(rename = "Gender", default, deserialize_with = "de_opt_string_flexible")]
    pub gender: Option<String>,

    #[serde(rename = "LanguageRequirements", default, deserialize_with = "de_string_vec")]
    pub language_requirements: Vec<String>,

    #[serde(rename = "ComputerKnowledgeRequirements", default, deserialize_with = "de_string_vec")]
    pub computer_knowledge_requirements: Vec<String>,

    #[serde(rename = "professionalSkills", default, deserialize_with = "de_opt_string_flexible")]
    pub professional_skills: Option<String>,

    #[serde(rename = "personalQualities", default, deserialize_with = "de_opt_string_flexible")]
    pub personal_qualities: Option<String>,

    #[serde(rename = "Description", default, deserialize_with = "de_opt_string_flexible")]
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NormalizedOneFVacancy {
    pub vacancy_id_1f: i64,
    pub external_vacancy_id: Option<String>,
    pub name: String,
    pub company: Option<String>,
    pub specialty: Option<String>,
    pub education: Option<String>,
    pub experience: Option<String>,
    pub age: Option<String>,
    pub gender: Option<String>,
    pub languages: Vec<String>,
    pub computer_skills: Vec<String>,
    pub professional_skills: Option<String>,
    pub personal_qualities: Option<String>,
    pub description_raw: Option<String>,
    pub description_clean: Option<String>,
    pub raw: JsonValue,
    pub data_quality: i32,
    pub content_hash: String,
}

impl OneFVacancyPayload {
    pub fn normalize(self, raw: JsonValue) -> NormalizedOneFVacancy {
        let name = clean_text(self.name.as_deref().unwrap_or_default());
        let company = clean_opt(self.company);
        let specialty = clean_opt(self.specialty);
        let education = clean_opt(self.education);
        let experience = clean_opt(self.experience);
        let age = clean_opt(self.age);
        let gender = clean_opt(self.gender);
        let professional_skills = clean_opt(self.professional_skills);
        let personal_qualities = clean_opt(self.personal_qualities);

        let languages = clean_list(self.language_requirements);
        let computer_skills = clean_list(self.computer_knowledge_requirements);

        let description_raw = self.description.filter(|d| !d.trim().is_empty());
        let description_clean = description_raw
            .as_deref()
            .map(clean_description)
            .filter(|d| !d.is_empty());

        let data_quality = compute_data_quality(
            &name,
            &company,
            &specialty,
            &education,
            &experience,
            &languages,
            &computer_skills,
            &professional_skills,
            &personal_qualities,
            description_clean.as_deref(),
        );

        let content_hash = content_hash(
            &name,
            &company,
            &specialty,
            &education,
            &experience,
            &age,
            &gender,
            &languages,
            &computer_skills,
            &professional_skills,
            &personal_qualities,
            description_clean.as_deref(),
        );

        NormalizedOneFVacancy {
            vacancy_id_1f: self.vacancy_id_1f,
            external_vacancy_id: clean_opt(self.external_vacancy_id),
            name,
            company,
            specialty,
            education,
            experience,
            age,
            gender,
            languages,
            computer_skills,
            professional_skills,
            personal_qualities,
            description_raw,
            description_clean,
            raw,
            data_quality,
            content_hash,
        }
    }
}

fn clean_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| clean_text(&v))
        .filter(|v| !v.is_empty())
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| clean_text(&v))
        .filter(|v| !v.is_empty())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn content_hash(
    name: &str,
    company: &Option<String>,
    specialty: &Option<String>,
    education: &Option<String>,
    experience: &Option<String>,
    age: &Option<String>,
    gender: &Option<String>,
    languages: &[String],
    computer_skills: &[String],
    professional_skills: &Option<String>,
    personal_qualities: &Option<String>,
    description_clean: Option<&str>,
) -> String {
    const SEP: &str = "\u{1f}";

    let mut sorted_languages = languages.to_vec();
    sorted_languages.sort();
    let mut sorted_computer_skills = computer_skills.to_vec();
    sorted_computer_skills.sort();

    let parts = [
        name.to_string(),
        company.clone().unwrap_or_default(),
        specialty.clone().unwrap_or_default(),
        education.clone().unwrap_or_default(),
        experience.clone().unwrap_or_default(),
        age.clone().unwrap_or_default(),
        gender.clone().unwrap_or_default(),
        sorted_languages.join(","),
        sorted_computer_skills.join(","),
        professional_skills.clone().unwrap_or_default(),
        personal_qualities.clone().unwrap_or_default(),
        description_clean.unwrap_or_default().to_string(),
    ];

    let mut hasher = Sha256::new();
    hasher.update(parts.join(SEP).as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn compute_data_quality(
    name: &str,
    company: &Option<String>,
    specialty: &Option<String>,
    education: &Option<String>,
    experience: &Option<String>,
    languages: &[String],
    computer_skills: &[String],
    professional_skills: &Option<String>,
    personal_qualities: &Option<String>,
    description_clean: Option<&str>,
) -> i32 {
    let mut score = 0;

    if !name.trim().is_empty() {
        score += 10;
    }
    if company.is_some() {
        score += 5;
    }
    if education.is_some() {
        score += 8;
    }
    if experience.is_some() {
        score += 7;
    }
    if specialty.is_some() {
        score += 5;
    }
    if !languages.is_empty() {
        score += 10;
    }
    if !computer_skills.is_empty() {
        score += 5;
    }
    if is_substantive(professional_skills.as_deref()) {
        score += 25;
    }
    if is_substantive(personal_qualities.as_deref()) {
        score += 10;
    }

    score += match description_clean.map(str::len).unwrap_or(0) {
        0 => 0,
        1..=149 => 3,
        150..=399 => 8,
        _ => 15,
    };

    score.min(100)
}

fn is_substantive(text: Option<&str>) -> bool {
    match text {
        Some(t) => t.chars().count() >= 12 && !looks_like_junk(t),
        None => false,
    }
}

fn looks_like_junk(text: &str) -> bool {
    let tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();

    if tokens.is_empty() {
        return true;
    }

    if tokens.len() >= 2 && tokens.iter().all(|t| *t == tokens[0]) {
        return true;
    }

    let long: Vec<&String> = tokens
        .iter()
        .filter(|t| t.chars().count() >= 5 && t.chars().all(char::is_alphabetic))
        .collect();
    if !long.is_empty() {
        let mash = long.iter().filter(|t| !t.chars().any(is_vowel)).count();
        if mash * 2 >= long.len() {
            return true;
        }
    }

    false
}

fn is_vowel(c: char) -> bool {
    matches!(
        c,
        'a' | 'e' | 'i' | 'o' | 'u' | 'y'
            | 'а' | 'е' | 'ё' | 'и' | 'о' | 'у' | 'ы' | 'э' | 'ю' | 'я'
    )
}

pub(crate) fn de_i64_flexible<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum IntOrString {
        Int(i64),
        Str(String),
    }

    match IntOrString::deserialize(deserializer)? {
        IntOrString::Int(i) => Ok(i),
        IntOrString::Str(s) => s
            .trim()
            .parse()
            .map_err(|_| serde::de::Error::custom(format!("expected an integer, got '{}'", s))),
    }
}

fn de_opt_string_flexible<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Scalar {
        Str(String),
        Int(i64),
        Float(f64),
        Bool(bool),
        Null,
    }

    Ok(match Option::<Scalar>::deserialize(deserializer)? {
        Some(Scalar::Str(s)) => Some(s),
        Some(Scalar::Int(i)) => Some(i.to_string()),
        Some(Scalar::Float(f)) => Some(f.to_string()),
        Some(Scalar::Bool(b)) => Some(b.to_string()),
        Some(Scalar::Null) | None => None,
    })
}

fn de_string_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ListOrOne {
        List(Vec<String>),
        One(String),
    }

    Ok(match Option::<ListOrOne>::deserialize(deserializer)? {
        Some(ListOrOne::List(v)) => v,
        Some(ListOrOne::One(s)) => vec![s],
        None => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn normalize(value: serde_json::Value) -> NormalizedOneFVacancy {
        let payload: OneFVacancyPayload = serde_json::from_value(value.clone()).unwrap();
        payload.normalize(value)
    }

    #[test]
    fn parses_a_real_record() {
        let v = normalize(json!({
            "Name": "Специалист по радости",
            "Description": "🌟 **Требования:**- Высшее образование.",
            "ExternalVacancyId": "129",
            "VacancyId1F": 29698,
            "personalQualities": "уверенный, жизнерадостный, общительный",
            "Education": "Высшее",
            "professionalSkills": "Умение радоваться каждому дню",
            "ComputerKnowledgeRequirements": ["CRM (Bitrix, AmoCRM и др.)"],
            "Age": "от 20 до 30 лет",
            "LanguageRequirements": ["Таджикский: свободный", "Русский: свободный"],
            "Experience": "3–5 лет",
            "Gender": "Мужской",
            "Specialty": "Менеджмент и бизнес: Управление персоналом",
            "Company": "AtS Тренинг Центр"
        }));

        assert_eq!(v.vacancy_id_1f, 29698);
        assert_eq!(v.external_vacancy_id.as_deref(), Some("129"));
        assert_eq!(v.languages.len(), 2);
        assert_eq!(v.description_clean.as_deref(), Some("🌟 **Требования:**\n- Высшее образование."));
    }

    #[test]
    fn hash_ignores_key_order_and_list_order() {
        let a = normalize(json!({
            "VacancyId1F": 1, "Name": "X",
            "LanguageRequirements": ["Русский: свободный", "Английский: разговорный"]
        }));
        let b = normalize(json!({
            "Name": "X", "VacancyId1F": 1,
            "LanguageRequirements": ["Английский: разговорный", "Русский: свободный"]
        }));
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn hash_changes_when_content_changes() {
        let a = normalize(json!({"VacancyId1F": 1, "Name": "X", "Education": "Высшее"}));
        let b = normalize(json!({"VacancyId1F": 1, "Name": "X", "Education": "Среднее"}));
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn scores_placeholder_records_below_real_ones() {
        let junk = normalize(json!({
            "VacancyId1F": 58588, "Name": "Angelest", "Company": "SPEY",
            "professionalSkills": "jifrjijfir\njskwjdkjkdwj",
            "personalQualities": "kfrokorko",
            "Education": "Высшее", "Experience": "1–3 года"
        }));
        let real = normalize(json!({
            "VacancyId1F": 29698, "Name": "Специалист по радости", "Company": "AtS Тренинг Центр",
            "professionalSkills": "Умение выстраивать отношения с клиентами и командой",
            "personalQualities": "уверенный, жизнерадостный, общительный, ответственный",
            "Education": "Высшее", "Experience": "3–5 лет",
            "Specialty": "Менеджмент и бизнес: Управление персоналом",
            "LanguageRequirements": ["Русский: свободный"],
            "ComputerKnowledgeRequirements": ["CRM (Bitrix, AmoCRM и др.)"]
        }));

        assert!(junk.data_quality < real.data_quality);
        assert!(junk.data_quality < 50, "got {}", junk.data_quality);
    }

    #[test]
    fn tolerates_1f_type_drift() {
        let v = normalize(json!({
            "VacancyId1F": "29502",
            "ExternalVacancyId": 205,
            "Name": "Проектный менеджер",
            "LanguageRequirements": "Русский: свободный",
            "personalQualities": null
        }));

        assert_eq!(v.vacancy_id_1f, 29502);
        assert_eq!(v.external_vacancy_id.as_deref(), Some("205"));
        assert_eq!(v.languages, vec!["Русский: свободный".to_string()]);
        assert!(v.personal_qualities.is_none());
    }
}
