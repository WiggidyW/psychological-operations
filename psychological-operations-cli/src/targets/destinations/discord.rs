use super::{json_body, Subject};

const MAX_CONTENT_LENGTH: usize = 2000;

pub async fn send(webhook_url: &str, subject: &Subject<'_>) -> Result<(), crate::error::Error> {
    let (header_plain, items) = json_body::lines(subject);
    let header = format!("**{header_plain}**");
    let lines: Vec<String> = items.into_iter()
        .map(|(label, url)| format!("{label} — {url}"))
        .collect();

    let client = reqwest::Client::new();
    for chunk in split_lines(&header, &lines, MAX_CONTENT_LENGTH) {
        let res = client
            .post(webhook_url)
            .json(&serde_json::json!({ "content": chunk }))
            .send()
            .await?;
        if !res.status().is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(crate::error::Error::Other(format!("discord webhook failed: {text}")));
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
