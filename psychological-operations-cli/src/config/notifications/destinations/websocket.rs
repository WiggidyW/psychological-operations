use std::collections::BTreeMap;

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, http::HeaderValue, Message};

use crate::psyop::PsyOp;
use crate::score::ScoredPost;

use super::json_body;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Urls,
    UrlsWithScores,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocket {
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    pub mode: Mode,
}

pub async fn send(
    cfg: &WebSocket,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    let payload = render(&cfg.mode, psyop_name, output)?;

    let mut request = cfg.url.as_str().into_client_request()
        .map_err(|e| crate::error::Error::Other(format!("websocket invalid url: {e}")))?;
    let req_headers = request.headers_mut();
    for (k, v) in &cfg.headers {
        let value = HeaderValue::from_str(v)
            .map_err(|e| crate::error::Error::Other(format!("websocket invalid header value for \"{k}\": {e}")))?;
        req_headers.insert(
            tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| crate::error::Error::Other(format!("websocket invalid header name \"{k}\": {e}")))?,
            value,
        );
    }

    let (mut stream, _resp) = tokio_tungstenite::connect_async(request).await
        .map_err(|e| crate::error::Error::Other(format!("websocket connect failed: {e}")))?;

    stream.send(Message::Text(payload.into())).await
        .map_err(|e| crate::error::Error::Other(format!("websocket send failed: {e}")))?;
    stream.close(None).await
        .map_err(|e| crate::error::Error::Other(format!("websocket close failed: {e}")))?;

    // Drain remaining frames so the close handshake completes cleanly.
    while let Some(msg) = stream.next().await {
        if msg.is_err() { break; }
    }
    Ok(())
}

fn render(mode: &Mode, psyop_name: &str, output: &[&ScoredPost]) -> Result<String, crate::error::Error> {
    let mut s = String::new();
    match mode {
        Mode::Urls => {
            for tw in output {
                s.push_str(&format!("https://x.com/{}/status/{}\n", tw.post.handle, tw.post.id));
            }
        }
        Mode::UrlsWithScores => {
            for tw in output {
                s.push_str(&format!(
                    "{:.4} — https://x.com/{}/status/{}\n",
                    tw.score, tw.post.handle, tw.post.id,
                ));
            }
        }
        Mode::Json => {
            let body = json_body::build(psyop_name, output);
            s.push_str(&serde_json::to_string(&body)?);
        }
    }
    Ok(s)
}
