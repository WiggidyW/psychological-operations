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
pub struct Stdout {
    pub mode: Mode,
}

pub async fn send(
    cfg: &Stdout,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    match cfg.mode {
        Mode::Urls => {
            for s in output {
                println!("https://x.com/{}/status/{}", s.post.handle, s.post.id);
            }
        }
        Mode::UrlsWithScores => {
            for s in output {
                println!(
                    "{:.4} — https://x.com/{}/status/{}",
                    s.score, s.post.handle, s.post.id,
                );
            }
        }
        Mode::Json => {
            let body = json_body::build(psyop_name, output);
            println!("{}", serde_json::to_string(&body)?);
        }
    }
    Ok(())
}
