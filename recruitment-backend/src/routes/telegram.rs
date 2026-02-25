use axum::{extract::State, Json};
use serde::Deserialize;
use crate::{AppState, error::Result};

#[derive(Debug, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub from: TelegramUser,
    pub chat: TelegramChat,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    pub r#type: String,
}

pub async fn handle_webhook(
    State(state): State<AppState>,
    Json(update): Json<TelegramUpdate>,
) -> Result<impl axum::response::IntoResponse> {
    tracing::info!("Received Telegram webhook update ID: {}", update.update_id);
    if let Some(message) = update.message {
        if let Some(text) = &message.text {
            let user_id = message.from.id;
            let chat_id = message.chat.id;
            
            if let Ok(Some(candidate)) = state.candidate_service.get_by_telegram_id(user_id).await {
                let create_msg = crate::models::message::CreateMessage {
                    candidate_id: candidate.id,
                    telegram_id: user_id,
                    direction: "inbound".to_string(),
                    text: text.clone(),
                };
                if let Err(e) = state.message_service.create(create_msg).await {
                    tracing::warn!("Failed to store incoming message: {:?}", e);
                }
                
                let onef = state.onef_service.clone();
                let cid = candidate.id;
                let t_text = text.clone();
                tokio::spawn(async move {
                    if onef.is_enabled() {
                         let _ = onef.notify_new_message(cid, user_id, &t_text).await;
                    }
                });
            }
            
            if text.starts_with("/start") {
                tracing::info!("Handling /start from user: {} (id: {})", message.from.first_name, user_id);
                
                let candidate = state.candidate_service.get_by_telegram_id(user_id).await?;
                
                if candidate.is_some() {
                    tracing::info!("Found existing candidate for telegram_id: {}", user_id);
                } else {
                    tracing::info!("No candidate found for telegram_id: {}. Offering registration.", user_id);
                }
                
                let config = crate::config::get_config();
                let webapp_url = &config.webapp_url;
                
                let (msg_text, button_text, web_app_url) = if let Some(c) = candidate {
                    (
                        "С возвращением! Вы можете просмотреть свой профиль здесь:", 
                        "Просмотреть профиль", 
                        format!("{}/candidate/{}", webapp_url, c.id)
                    )
                } else {
                    let register_url = format!("{}/candidate/register?", webapp_url);
                    let mut params = Vec::new();
                    
                    if let Some(ln) = &message.from.last_name {
                        params.push(format!("name={} {}", message.from.first_name, ln));
                    } else {
                        params.push(format!("name={}", message.from.first_name));
                    }
                    
                    params.push(format!("telegram_id={}", user_id));

                    if let Some(dob) = fetch_telegram_birthdate(user_id).await {
                        params.push(format!("dob={}", dob));
                        tracing::info!("Fetched birthday for user {}: {}", user_id, dob);
                    }
                    
                    (
                        "Здравствуйте! Чтобы присоединиться к нашему процессу найма, пожалуйста, зарегистрируйте свой профиль:", 
                        "Зарегистрировать профиль", 
                        format!("{}{}", register_url, params.join("&"))
                    )
                };

                let reply_markup = serde_json::json!({
                    "inline_keyboard": [[
                        {
                            "text": button_text,
                            "web_app": { "url": web_app_url }
                        }
                    ]]
                });

                send_telegram_message(chat_id, msg_text, Some(reply_markup)).await?;
            } else {
                 let help_text = "Чтобы начать работу или открыть свой профиль, пожалуйста, используйте команду /start";
                 let _ = send_telegram_message(chat_id, help_text, None).await;
            }
        }
    }

    Ok(axum::http::StatusCode::OK)
}

async fn send_telegram_message(
    chat_id: i64,
    text: &str,
    reply_markup: Option<serde_json::Value>,
) -> Result<()> {
    let config = crate::config::get_config();
    let url = format!("https://api.telegram.org/bot{}/sendMessage", config.telegram_bot_token);
    
    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
    });
    
    if let Some(markup) = reply_markup {
        body["reply_markup"] = markup;
    }

    println!("Sending Telegram message to chat_id: {}", chat_id);

    let client = reqwest::Client::new();
    let response = client.post(&url).json(&body).send().await.map_err(|e| crate::error::Error::Internal(e.to_string()))?;
    
    let status = response.status();
    let response_text = response.text().await.unwrap_or_default();
    println!("Telegram API response: {} - {}", status, response_text);
    
    Ok(())
}

/// Calls Telegram Bot API `getChat` to fetch the user's birthday.
/// Returns `Some("YYYY-MM-DD")` if the user has a birthday set, `None` otherwise.
/// The `birthdate` field (Bot API 7.2+) contains `day`, `month`, and optionally `year`.
async fn fetch_telegram_birthdate(user_id: i64) -> Option<String> {
    let config = crate::config::get_config();
    let url = format!(
        "https://api.telegram.org/bot{}/getChat?chat_id={}",
        config.telegram_bot_token, user_id
    );

    let resp = reqwest::get(&url).await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;

    let birthdate = json.get("result")?.get("birthdate")?;
    let day = birthdate.get("day")?.as_u64()?;
    let month = birthdate.get("month")?.as_u64()?;

    // `year` is optional in Telegram's API — some users only set day+month
    let year = birthdate.get("year").and_then(|y| y.as_u64());

    if let Some(y) = year {
        Some(format!("{:04}-{:02}-{:02}", y, month, day))
    } else {
        // Without a year we can't use it as a full DOB — skip
        tracing::info!("User {} has birthday day/month but no year, skipping DOB auto-fill", user_id);
        None
    }
}
