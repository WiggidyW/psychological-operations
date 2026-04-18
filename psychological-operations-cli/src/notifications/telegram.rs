const MAX_MESSAGE_LENGTH: usize = 4096;

pub async fn send(bot_token: &str, chat_id: &str, message: &str) -> Result<(), crate::error::Error> {
    let text = if message.len() > MAX_MESSAGE_LENGTH {
        format!("{}...", &message[..MAX_MESSAGE_LENGTH - 3])
    } else {
        message.to_string()
    };

    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown",
        }))
        .send()
        .await?;

    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(crate::error::Error::Other(format!("telegram api failed: {text}")));
    }
    Ok(())
}
