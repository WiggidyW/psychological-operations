pub mod discord;
pub mod http;
pub mod stderr;
pub mod stdout;
pub mod telegram;

use serde::{Deserialize, Serialize};

use crate::psyop::PsyOp;
use crate::score::ScoredPost;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Destination {
    #[serde(rename = "discord")]
    Discord { webhook_url: String },
    #[serde(rename = "telegram")]
    Telegram { bot_token: String, chat_id: String },
    #[serde(rename = "http")]
    Http(http::Http),
    #[serde(rename = "stdout")]
    Stdout(stdout::Stdout),
    #[serde(rename = "stderr")]
    Stderr(stderr::Stderr),
}

pub async fn notify(
    destinations: &[Destination],
    psyop_name: &str,
    psyop: &PsyOp,
    output: &[&ScoredPost],
) {
    let futs = destinations.iter().map(|dest| async move {
        match dest {
            Destination::Discord { webhook_url } => {
                discord::send(webhook_url, psyop_name, psyop, output).await
            }
            Destination::Telegram { bot_token, chat_id } => {
                telegram::send(bot_token, chat_id, psyop_name, psyop, output).await
            }
            Destination::Http(cfg) => {
                http::send(cfg, psyop_name, psyop, output).await
            }
            Destination::Stdout(cfg) => {
                stdout::send(cfg, psyop_name, psyop, output).await
            }
            Destination::Stderr(cfg) => {
                stderr::send(cfg, psyop_name, psyop, output).await
            }
        }
    });
    for result in futures::future::join_all(futs).await {
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    }
}
