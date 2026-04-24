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
    let header = format!("*PsyOp \"{psyop_name}\"*");
    let lines: Vec<String> = output.iter().map(|s| format!(
        "`{:.4}` — [@{}](https://x.com/{}/status/{})",
        s.score, s.post.handle, s.post.handle, s.post.id,
    )).collect();

    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let client = reqwest::Client::new();
    for chunk in split_lines(&header, &lines, MAX_MESSAGE_LENGTH) {
        let res = client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "Markdown",
            }))
            .send()
            .await?;
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(crate::error::Error::Other(format!("telegram api failed: {body}")));
        }
    }
    Ok(())
}

/// Pack `header` plus per-post `lines` into messages of at most `max_len` chars,
/// splitting on line boundaries. Header is sent once, in the first message only.
/// Lines longer than `max_len` on their own are hard-truncated with "...".
fn split_lines(header: &str, lines: &[String], max_len: usize) -> Vec<String> {
    let mut messages = Vec::new();
    let mut current = String::from(header);
    for line in lines {
        let candidate_len = current.len() + 1 + line.len();
        if candidate_len <= max_len {
            current.push('\n');
            current.push_str(line);
            continue;
        }
        if !current.is_empty() {
            messages.push(std::mem::take(&mut current));
        }
        if line.len() <= max_len {
            current.push_str(line);
        } else {
            let mut truncated = line.clone();
            truncated.truncate(max_len - 3);
            truncated.push_str("...");
            messages.push(truncated);
        }
    }
    if !current.is_empty() {
        messages.push(current);
    }
    messages
}
