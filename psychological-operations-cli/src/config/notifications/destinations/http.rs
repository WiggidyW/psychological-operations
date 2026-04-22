use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::psyop::PsyOp;
use crate::score::ScoredPost;

use super::json_body;

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

pub async fn send(
    cfg: &Http,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    let body = json_body::build(psyop_name, output);

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
