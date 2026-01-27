use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFRequestWrapper {
    #[serde(rename = "requestBody")]
    pub request_body: OneFApplicationPayload,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFApplicationPayload {
    pub vacancy_id: i64,
    pub vacancy_name: String,
    pub candidate: OneFCandidateInfo,
    pub applied_at: String,
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
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFWebhookResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct OneFService {
    client: Client,
    webhook_url: Option<String>,
}

impl OneFService {
    pub fn new(webhook_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client for 1F service");
        
        let webhook_url = webhook_url.filter(|url| !url.trim().is_empty());

        if let Some(ref url) = webhook_url {
            info!("1F integration enabled, webhook URL: {}", url);
        } else {
            info!("1F integration disabled (ONEF_WEBHOOK_URL not set or empty)");
        }
        
        Self { client, webhook_url }
    }

    pub fn is_enabled(&self) -> bool {
        self.webhook_url.is_some()
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
    ) -> Result<(), String> {
        let webhook_url = match &self.webhook_url {
            Some(url) => url,
            None => {
                return Ok(());
            }
        };

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

        let payload = OneFApplicationPayload {
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
                cv_url: cv_url.map(|path| {
                    let config = crate::config::get_config();
                    let clean_path = path.trim_start_matches("./");
                    format!("{}/{}", config.webapp_url.trim_end_matches('/'), clean_path)
                }),
            },
            applied_at: chrono::Utc::now().to_rfc3339(),
        };

        let wrapper = OneFRequestWrapper {
            request_body: payload,
        };

        info!(
            "Sending application to 1F: candidate {} applied for vacancy {}",
            candidate_id, vacancy_id
        );

        match self.send_webhook(&webhook_url, &wrapper).await {
            Ok(response) => {
                if response.success {
                    info!(
                        "1F webhook successful for candidate {} vacancy {}",
                        candidate_id, vacancy_id
                    );
                } else {
                    warn!(
                        "1F webhook returned failure for candidate {} vacancy {}: {:?}",
                        candidate_id, vacancy_id, response.message
                    );
                }
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to send 1F webhook for candidate {} vacancy {}: {}",
                    candidate_id, vacancy_id, e
                );
                Err(e)
            }
        }
    }

    async fn send_webhook(
        &self,
        url: &str,
        payload: &OneFRequestWrapper,
    ) -> Result<OneFWebhookResponse, String> {
        let response = self
            .client
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("HTTP error {}: {}", status, body));
        }
        match response.json::<OneFWebhookResponse>().await {
            Ok(resp) => Ok(resp),
            Err(_) => Ok(OneFWebhookResponse {
                success: true,
                message: Some("Response received but not JSON".to_string()),
            }),
        }
    }
}
