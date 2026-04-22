use crate::psyop::PsyOp;
use crate::score::ScoredPost;

const MAX_MESSAGE_LENGTH: usize = 4096;

pub async fn send(
    bot_token: &str,
    chat_id: &str,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    let mut text = format!("*PsyOp \"{psyop_name}\"*");
    for s in output {
        text.push_str(&format!(
            "\n`{:.4}` — [@{}](https://x.com/{}/status/{})",
            s.score, s.post.handle, s.post.handle, s.post.id,
        ));
    }
    if text.len() > MAX_MESSAGE_LENGTH {
        text.truncate(MAX_MESSAGE_LENGTH - 3);
        text.push_str("...");
    }

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
        let body = res.text().await.unwrap_or_default();
        return Err(crate::error::Error::Other(format!("telegram api failed: {body}")));
    }
    Ok(())
}
