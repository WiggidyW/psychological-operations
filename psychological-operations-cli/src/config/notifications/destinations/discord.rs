use crate::psyop::PsyOp;
use crate::score::ScoredPost;

const MAX_CONTENT_LENGTH: usize = 2000;

pub async fn send(
    webhook_url: &str,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    let mut content = format!("**PsyOp \"{psyop_name}\"**");
    for s in output {
        content.push_str(&format!(
            "\n{:.4} — https://x.com/{}/status/{}",
            s.score, s.post.handle, s.post.id,
        ));
    }
    if content.len() > MAX_CONTENT_LENGTH {
        content.truncate(MAX_CONTENT_LENGTH - 3);
        content.push_str("...");
    }

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
