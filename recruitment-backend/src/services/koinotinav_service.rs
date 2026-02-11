use crate::error::Result;
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};

fn deserialize_bool_flexible<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(i64),
        String(String),
    }

    match BoolOrInt::deserialize(deserializer)? {
        BoolOrInt::Bool(b) => Ok(b),
        BoolOrInt::Int(i) => Ok(i != 0),
        BoolOrInt::String(s) => match s.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(serde::de::Error::custom(format!("Invalid boolean string: {}", s))),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalVacancy {
    pub id: i64,
    pub title: String,
    pub content: String,
    #[serde(deserialize_with = "deserialize_bool_flexible")]
    pub hot: bool,
    pub city: String,
    pub direction: String,
    pub company_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCompany {
    pub id: i64,
    pub title: String,
    pub logo: String,
}

#[derive(Clone)]
pub struct KoinotinavService {
    client: Client,
    base_url: String,
}

impl KoinotinavService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://job.koinotinav.tj".to_string(),
        }
    }

    pub async fn fetch_vacancies(&self) -> Result<Vec<ExternalVacancy>> {
        let url = format!("{}/api/vacancies", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await?;
        let vacancies = response
            .json::<Vec<ExternalVacancy>>()
            .await?;
        Ok(vacancies.into_iter().filter(|v| v.id >= 137).collect())
    }

    pub async fn fetch_vacancy(&self, id: i64) -> Result<Option<ExternalVacancy>> {
        let url = format!("{}/api/vacancies/{}", self.base_url, id);
        tracing::info!("Fetching single vacancy details from: {}", url);
        
        let response = self
            .client
            .get(&url)
            .send()
            .await?;
        
        if response.status() == reqwest::StatusCode::OK {
            let content_type = response.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            if content_type.contains("application/json") {
                match response.json::<ExternalVacancy>().await {
                    Ok(v) => return Ok(Some(v)),
                    Err(e) => tracing::warn!("Failed to parse single vacancy JSON: {}. Falling back to list.", e),
                }
            } else {
                tracing::warn!("Vacancy endpoint returned non-JSON content ({}). Falling back to list scan.", content_type);
            }
        } else if response.status() == reqwest::StatusCode::NOT_FOUND {
            tracing::warn!("Vacancy {} not found via direct API. Falling back to list scan.", id);
        } else {
            tracing::warn!("Vacancy API returned status {}. Falling back to list scan.", response.status());
        }

        tracing::info!("Scanning full vacancy list for ID {}", id);
        let all_vacancies = self.fetch_vacancies().await?;
        Ok(all_vacancies.into_iter().find(|v| v.id == id))
    }

    pub async fn fetch_companies(&self) -> Result<Vec<ExternalCompany>> {
        let url = format!("{}/api/companies", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await?;
        let companies = response
            .json::<Vec<ExternalCompany>>()
            .await?;
        Ok(companies)
    }
}
