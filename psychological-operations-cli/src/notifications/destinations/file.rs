use std::io::Write;
use std::path::PathBuf;

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
pub struct File {
    pub mode: Mode,
    pub path: PathBuf,
}

pub async fn send(
    cfg: &File,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    if let Some(parent) = cfg.path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.path)?;

    match cfg.mode {
        Mode::Urls => {
            for s in output {
                writeln!(f, "https://x.com/{}/status/{}", s.post.handle, s.post.id)?;
            }
        }
        Mode::UrlsWithScores => {
            for s in output {
                writeln!(
                    f,
                    "{:.4} — https://x.com/{}/status/{}",
                    s.score, s.post.handle, s.post.id,
                )?;
            }
        }
        Mode::Json => {
            let body = json_body::build(psyop_name, output);
            writeln!(f, "{}", serde_json::to_string(&body)?)?;
        }
    }
    Ok(())
}
