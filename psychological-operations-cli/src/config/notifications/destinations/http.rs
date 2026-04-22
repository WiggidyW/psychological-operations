use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::db::MediaUrl;
use crate::psyop::PsyOp;
use crate::score::ScoredPost;

/// Configuration for an HTTP notification destination. Sends a JSON body
/// describing the psyop and its scored output to an arbitrary endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http {
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
}

fn default_method() -> String { "POST".to_string() }

/// Top-level body sent to the configured endpoint.
#[derive(Debug, Serialize)]
pub struct Body<'a> {
    pub psyop: &'a str,
    pub results: Vec<Result_<'a>>,
}

/// One scored tweet in the body.
#[derive(Debug, Serialize)]
#[serde(rename = "Result")]
pub struct Result_<'a> {
    pub score: f64,
    pub id: &'a str,
    pub handle: &'a str,
    pub text: &'a str,
    pub images: &'a [MediaUrl],
    pub videos: &'a [MediaUrl],
    pub created: &'a str,
    pub community: Option<&'a str>,
    pub query: &'a str,
    pub url: String,
}

pub async fn send(
    cfg: &Http,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> std::result::Result<(), crate::error::Error> {
    let body = Body {
        psyop: psyop_name,
        results: output.iter().map(|s| Result_ {
            score: s.score,
            id: &s.post.id,
            handle: &s.post.handle,
            text: &s.post.text,
            images: &s.post.images,
            videos: &s.post.videos,
            created: &s.post.created,
            community: s.post.community.as_deref(),
            query: &s.post.query,
            url: format!("https://x.com/{}/status/{}", s.post.handle, s.post.id),
        }).collect(),
    };

    let method = reqwest::Method::from_bytes(cfg.method.as_bytes())
        .map_err(|e| crate::error::Error::Other(format!("invalid http method \"{}\": {e}", cfg.method)))?;

    let client = reqwest::Client::new();
    let mut req = client.request(method, &cfg.url).json(&body);
    for (k, v) in &cfg.headers {
        req = req.header(k, v);
    }

    let res = req.send().await?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(crate::error::Error::Other(format!(
            "http notification failed: {status}: {body}",
        )));
    }
    Ok(())
}
