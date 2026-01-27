use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

pub fn verify_telegram_data(init_data: &str, bot_token: &str) -> Option<i64> {
    let mut params: HashMap<String, String> = HashMap::new();
    for pair in init_data.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
            let decoded_value = url::form_urlencoded::parse(value.as_bytes())
                .next()
                .map(|(_, v)| v.to_string())
                .unwrap_or(value.to_string());
            params.insert(key.to_string(), decoded_value);
        }
    }

    let hash = params.get("hash")?;
    
    let mut pairs: Vec<(String, String)> = init_data.split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?;
            let value = parts.next()?;
            if key == "hash" { return None; }
            Some((key.to_string(), value.to_string()))
        })
        .collect();
        
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    
    let data_check_string = pairs.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("\n");

    let secret_key = HmacSha256::new_from_slice("WebAppData".as_bytes())
        .and_then(|mut mac| {
            mac.update(bot_token.as_bytes());
            Ok(mac.finalize().into_bytes())
        })
        .ok()?;

    let mut mac = HmacSha256::new_from_slice(&secret_key).ok()?;
    mac.update(data_check_string.as_bytes());
    let calculated_hash = hex::encode(mac.finalize().into_bytes());

    if calculated_hash.eq_ignore_ascii_case(hash) {
        let user_encoded = params.get("user")?;
        let user: serde_json::Value = serde_json::from_str(user_encoded).ok()?;
        user.get("id")?.as_i64()
    } else {
        None
    }
}
