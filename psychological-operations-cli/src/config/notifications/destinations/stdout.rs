use serde::{Deserialize, Serialize};

use crate::psyop::PsyOp;
use crate::score::ScoredPost;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stdout {
    #[serde(default)]
    pub include_header: bool,
    #[serde(default = "default_include_score")]
    pub include_score: bool,
}

fn default_include_score() -> bool { true }

pub async fn send(
    cfg: &Stdout,
    psyop_name: &str,
    _psyop: &PsyOp,
    output: &[&ScoredPost],
) -> Result<(), crate::error::Error> {
    if cfg.include_header {
        println!("PsyOp \"{psyop_name}\"");
    }
    for s in output {
        let url = format!("https://x.com/{}/status/{}", s.post.handle, s.post.id);
        if cfg.include_score {
            println!("{:.4} — {url}", s.score);
        } else {
            println!("{url}");
        }
    }
    Ok(())
}
