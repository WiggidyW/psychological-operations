pub mod discord;
pub mod exec;
pub mod file;
pub mod http;
pub mod json_body;
pub mod stderr;
pub mod stdout;
pub mod telegram;
pub mod websocket;

use serde::{Deserialize, Serialize};

use crate::psyop::PsyOp;
use crate::scrape::Scrape;
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
    #[serde(rename = "file")]
    File(file::File),
    #[serde(rename = "exec")]
    Exec(exec::Exec),
    #[serde(rename = "websocket")]
    WebSocket(websocket::WebSocket),
}

/// What's being notified about. Destinations format Psyop and Scrape
/// subjects independently — text-mode renderers print a per-tweet line list
/// for psyops and a single summary line for scrapes; JSON-mode renderers
/// emit a tagged Body via `json_body::build`. Agent-intervention prompts
/// are deliberately not modelled here — they're local stderr output, not
/// remote notifications.
pub enum Subject<'a> {
    Psyop {
        name: &'a str,
        psyop: &'a PsyOp,
        output: &'a [&'a ScoredPost],
    },
    Scrape {
        name: &'a str,
        scrape: &'a Scrape,
        collected: usize,
    },
}

pub async fn notify(destinations: &[Destination], subject: Subject<'_>) {
    let subject_ref = &subject;
    let futs = destinations.iter().map(|dest| async move {
        match dest {
            Destination::Discord { webhook_url } => discord::send(webhook_url, subject_ref).await,
            Destination::Telegram { bot_token, chat_id } => telegram::send(bot_token, chat_id, subject_ref).await,
            Destination::Http(cfg) => http::send(cfg, subject_ref).await,
            Destination::Stdout(cfg) => stdout::send(cfg, subject_ref).await,
            Destination::Stderr(cfg) => stderr::send(cfg, subject_ref).await,
            Destination::File(cfg) => file::send(cfg, subject_ref).await,
            Destination::Exec(cfg) => exec::send(cfg, subject_ref).await,
            Destination::WebSocket(cfg) => websocket::send(cfg, subject_ref).await,
        }
    });
    for result in futures::future::join_all(futs).await {
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    }
}
