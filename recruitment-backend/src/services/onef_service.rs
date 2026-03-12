use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFRequestWrapper {
    #[serde(rename = "requestBody")]
    pub request_body: OneFApplicationPayload,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFApplicationPayload {
    pub event_type: String,
    pub vacancy_id: i64,
    pub vacancy_name: String,
    pub candidate: OneFCandidateInfo,
    pub applied_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFGradePayload {
    pub candidate_id: uuid::Uuid,
    pub grade: i32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFCandidateInfo {
    pub id: String,
    pub telegram_id: i64,
    pub fullname: String,
    pub name: String,
    pub surname: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cv_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_rating: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_comment: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFWebhookResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OneFTestStatusEventData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFTestStatusPayload {
    pub candidate_id: uuid::Uuid,
    pub test_id: uuid::Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vacancy_id: Option<i64>,
    pub test_status: String,
    pub event_date: String,
    pub event_data: OneFTestStatusEventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFCandidateStatusPayload {
    pub event_type: String,
    pub candidate_id: uuid::Uuid,
    pub status: String,
    pub updated_at: String,
}

const PATH_CANDIDATE_RESPONSE: &str = "/action/candidateResponse";
const PATH_POST_TEST_STATUS: &str = "/action/postTestStatus";
const PATH_RECEIVE_MESSAGE: &str = "/action/receivemessage";

#[derive(Clone)]
pub struct OneFService {
    client: Client,
    base_urls: Vec<String>,
}

impl OneFService {
    pub fn new(base_urls: Vec<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client for 1F service");

        if base_urls.is_empty() {
            info!("1F integration disabled (no ONEF_BASE_URLS / ONEF_WEBHOOK_URL configured)");
        } else {
            for url in &base_urls {
                info!("1F integration enabled, base URL: {}", url);
            }
        }

        Self { client, base_urls }
    }

    pub fn is_enabled(&self) -> bool {
        !self.base_urls.is_empty()
    }

    pub async fn notify_application(
        &self,
        vacancy_id: i64,
        vacancy_name: String,
        candidate_id: uuid::Uuid,
        telegram_id: i64,
        name: String,
        email: String,
        phone: Option<String>,
        dob: Option<chrono::NaiveDate>,
        cv_url: Option<String>,
        ai_rating: Option<i32>,
        ai_comment: Option<String>,
    ) -> Result<(), String> {
        if self.base_urls.is_empty() {
            return Ok(());
        }

        let fullname = name.trim().to_string();
        let name_parts: Vec<&str> = fullname.split_whitespace().collect();
        let (first_name, last_name) = match name_parts.len() {
            0 => ("".to_string(), "".to_string()),
            1 => (name_parts[0].to_string(), "".to_string()),
            _ => {
                let first = name_parts[0].to_string();
                let rest = name_parts[1..].join(" ");
                (first, rest)
            }
        };

        let full_cv_url = cv_url.map(|path| {
            let config = crate::config::get_config();
            format!("{}/{}", config.webapp_url, path)
        });

        let payload = OneFApplicationPayload {
            event_type: "new_application".to_string(),
            vacancy_id,
            vacancy_name,
            candidate: OneFCandidateInfo {
                id: candidate_id.to_string(),
                telegram_id,
                fullname,
                name: first_name,
                surname: last_name,
                email,
                phone,
                dob: dob.map(|d| d.format("%Y-%m-%d").to_string()),
                cv_url: full_cv_url,
                ai_rating,
                ai_comment,
            },
            applied_at: chrono::Utc::now().to_rfc3339(),
        };

        let wrapper = OneFRequestWrapper {
            request_body: payload,
        };

        info!(
            "Sending application to 1F: candidate {} applied for vacancy {} → {} target(s)",
            candidate_id, vacancy_id, self.base_urls.len()
        );

        let urls: Vec<String> = self.base_urls.iter()
            .map(|base| format!("{}{}", base, PATH_CANDIDATE_RESPONSE))
            .collect();

        let body = serde_json::to_value(&wrapper)
            .map_err(|e| format!("Serialization error: {}", e))?;

        self.fan_out_post(&urls, &body, "new_application").await;
        Ok(())
    }

    pub async fn notify_grade(
        &self,
        candidate_id: uuid::Uuid,
        grade: i32,
    ) -> Result<(), String> {
        if self.base_urls.is_empty() {
            return Ok(());
        }

        let payload = json!({
            "event_type": "grade_shared",
            "candidate_id": candidate_id,
            "grade": grade,
            "shared_at": chrono::Utc::now().to_rfc3339(),
        });

        let wrapper = json!({
            "requestBody": payload
        });

        info!(
            "Sharing grade to 1F: candidate {} grade {} → {} target(s)",
            candidate_id, grade, self.base_urls.len()
        );

        let urls: Vec<String> = self.base_urls.iter()
            .map(|base| format!("{}{}", base, PATH_CANDIDATE_RESPONSE))
            .collect();

        self.fan_out_post(&urls, &wrapper, "grade_shared").await;
        Ok(())
    }

    pub async fn notify_new_message(
        &self,
        candidate_id: uuid::Uuid,
        telegram_id: i64,
        text: &str,
    ) -> Result<(), String> {
        if self.base_urls.is_empty() {
            return Ok(());
        }

        let urls: Vec<String> = self.base_urls.iter()
            .map(|base| format!("{}{}", base, PATH_RECEIVE_MESSAGE))
            .collect();

        let payload = json!({
            "event_type": "new_message",
            "candidate_id": candidate_id,
            "telegram_id": telegram_id,
            "text": text,
            "received_at": chrono::Utc::now().to_rfc3339(),
        });

        let wrapper = json!({
            "requestBody": payload
        });

        info!(
            "Forwarding new message from candidate {} into 1F → {} target(s)",
            candidate_id, urls.len()
        );

        self.fan_out_post(&urls, &wrapper, "new_message").await;
        Ok(())
    }

    pub async fn notify_test_status(
        &self,
        payload: OneFTestStatusPayload,
    ) -> Result<(), String> {
        if self.base_urls.is_empty() {
            return Ok(());
        }

        let urls: Vec<String> = self.base_urls.iter()
            .map(|base| format!("{}{}", base, PATH_POST_TEST_STATUS))
            .collect();

        let wrapper = json!({
            "requestBody": payload
        });

        info!(
            "Pushing test status update to 1F: candidate {} status {} → {} target(s)",
            payload.candidate_id, payload.test_status, urls.len()
        );

        self.fan_out_post(&urls, &wrapper, "test_status").await;
        Ok(())
    }

    pub async fn notify_candidate_status(
        &self,
        candidate_id: uuid::Uuid,
        status: String,
    ) -> Result<(), String> {
        if self.base_urls.is_empty() {
            return Ok(());
        }

        let payload = OneFCandidateStatusPayload {
            event_type: "candidate_status_changed".to_string(),
            candidate_id,
            status,
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        let wrapper = json!({
            "requestBody": payload
        });

        info!(
            "Pushing candidate status update to 1F: candidate {} status {} → {} target(s)",
            candidate_id, payload.status, self.base_urls.len()
        );

        let urls: Vec<String> = self.base_urls.iter()
            .map(|base| format!("{}{}", base, PATH_CANDIDATE_RESPONSE))
            .collect();

        self.fan_out_post(&urls, &wrapper, "candidate_status_changed").await;
        Ok(())
    }

    async fn fan_out_post(
        &self,
        urls: &[String],
        body: &serde_json::Value,
        event_label: &str,
    ) {
        if urls.is_empty() {
            return;
        }

        if urls.len() == 1 {
            self.post_single(&urls[0], body, event_label).await;
            return;
        }

        let mut handles = Vec::with_capacity(urls.len());
        for url in urls {
            let client = self.client.clone();
            let url = url.clone();
            let body = body.clone();
            let label = event_label.to_string();
            handles.push(tokio::spawn(async move {
                Self::post_with_client(&client, &url, &body, &label).await;
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    async fn post_single(
        &self,
        url: &str,
        body: &serde_json::Value,
        event_label: &str,
    ) {
        Self::post_with_client(&self.client, url, body, event_label).await;
    }

    async fn post_with_client(
        client: &Client,
        url: &str,
        body: &serde_json::Value,
        event_label: &str,
    ) {
        match client.post(url).json(body).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    info!("1F {} → {} returned {}", event_label, url, status);
                } else {
                    let resp_body = resp.text().await.unwrap_or_default();
                    warn!(
                        "1F {} → {} returned {}: {}",
                        event_label, url, status, resp_body
                    );
                }
            }
            Err(e) => {
                error!("1F {} → {} failed: {}", event_label, url, e);
            }
        }
    }
}
