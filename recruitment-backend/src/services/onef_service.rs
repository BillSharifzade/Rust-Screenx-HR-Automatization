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
    pub cv_file_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cv_filename: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFTestStatusPayload {
    pub event_type: String,
    pub attempt_id: uuid::Uuid,
    pub candidate_id: uuid::Uuid,
    pub test_id: uuid::Uuid,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passed: Option<bool>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneFCandidateStatusPayload {
    pub event_type: String,
    pub candidate_id: uuid::Uuid,
    pub status: String,
    pub updated_at: String,
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
        ai_rating: Option<i32>,
        ai_comment: Option<String>,
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

        let (cv_file_base64, cv_filename) = if let Some(path) = &cv_url {
            let upload_root = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "/app/uploads".to_string());
            let clean_path = path.trim_start_matches("./").trim_start_matches("uploads/");
            let clean_path = clean_path.trim_start_matches('/');
            let abs_path = format!("{}/{}", upload_root, clean_path);
            
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin");
            let filename = format!("cv.{}", ext);
            
            if let Ok(data) = tokio::fs::read(&abs_path).await {
                use base64::prelude::*;
                (Some(BASE64_STANDARD.encode(&data)), Some(filename))
            } else {
                warn!("Failed to read cv file for base64 encoding from path: {}", abs_path);
                (None, None)
            }
        } else {
            (None, None)
        };

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
                cv_file_base64,
                cv_filename,
                ai_rating,
                ai_comment,
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

    pub async fn notify_grade(
        &self,
        candidate_id: uuid::Uuid,
        grade: i32,
    ) -> Result<(), String> {
        let webhook_url = match &self.webhook_url {
            Some(url) => url,
            None => {
                return Ok(());
            }
        };

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
            "Sharing grade to 1F: candidate {} grade {}",
            candidate_id, grade
        );

        let response = self
            .client
            .post(webhook_url)
            .json(&wrapper)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("1F grade sharing failed with status {}: {}", status, body);
            return Err(format!("HTTP error {}: {}", status, body));
        }

        info!("Successfully shared grade to 1F for candidate {}", candidate_id);
        info!("Successfully shared grade to 1F for candidate {}", candidate_id);
        Ok(())
    }

    pub async fn notify_new_message(
        &self,
        candidate_id: uuid::Uuid,
        telegram_id: i64,
        text: &str,
    ) -> Result<(), String> {
        let webhook_url = match &self.webhook_url {
            Some(url) => url,
            None => return Ok(()),
        };

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

        info!("Forwarding new message from candidate {} into 1F webhook", candidate_id);

        let response = self.client.post(webhook_url)
            .json(&wrapper)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("1F message forwarding failed: {}", body);
        }
        Ok(())
    }

    pub async fn notify_test_status(
        &self,
        payload: OneFTestStatusPayload,
    ) -> Result<(), String> {
        let webhook_url = match &self.webhook_url {
            Some(url) => url,
            None => return Ok(()),
        };

        let wrapper = json!({
            "requestBody": payload
        });

        info!(
            "Pushing test status update to 1F: attempt {} status {}",
            payload.attempt_id, payload.status
        );

        let response = self.client.post(webhook_url)
            .json(&wrapper)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("1F test status update failed: {}", body);
        }

        Ok(())
    }

    pub async fn notify_candidate_status(
        &self,
        candidate_id: uuid::Uuid,
        status: String,
    ) -> Result<(), String> {
        let webhook_url = match &self.webhook_url {
            Some(url) => url,
            None => return Ok(()),
        };

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
            "Pushing candidate status update to 1F: candidate {} status {}",
            candidate_id, payload.status
        );

        let response = self.client.post(webhook_url)
            .json(&wrapper)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            error!("1F candidate status update failed: {}", body);
        }

        Ok(())
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
