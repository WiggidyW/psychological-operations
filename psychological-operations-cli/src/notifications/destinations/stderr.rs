use serde::{Deserialize, Serialize};

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
pub struct Stderr {
    pub mode: Mode,
}

pub async fn send(
    cfg: &Stderr,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    match cfg.mode {
        Mode::Urls => {
            for s in output {
                eprintln!("https://x.com/{}/status/{}", s.post.handle, s.post.id);
            }
        }
        Mode::UrlsWithScores => {
            for s in output {
                eprintln!(
                    "{:.4} — https://x.com/{}/status/{}",
                    s.score, s.post.handle, s.post.id,
                );
            }
        }
        Mode::Json => {
            let body = json_body::build(psyop_name, output);
            eprintln!("{}", serde_json::to_string(&body)?);
        }
    }
    Ok(())
}
