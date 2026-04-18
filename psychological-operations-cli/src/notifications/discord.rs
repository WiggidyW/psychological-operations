const MAX_CONTENT_LENGTH: usize = 2000;

pub async fn send(webhook_url: &str, message: &str) -> Result<(), crate::error::Error> {
    let content = if message.len() > MAX_CONTENT_LENGTH {
        format!("{}...", &message[..MAX_CONTENT_LENGTH - 3])
    } else {
        message.to_string()
    };

    let client = reqwest::Client::new();
    let res = client
        .post(webhook_url)
        .json(&serde_json::json!({ "content": content }))
        .send()
        .await?;

    if !res.status().is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(crate::error::Error::Other(format!("discord webhook failed: {text}")));
    }
    Ok(())
}
