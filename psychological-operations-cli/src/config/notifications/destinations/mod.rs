pub mod discord;
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
}

pub async fn notify(
    destinations: &[Destination],
    psyop_name: &str,
    psyop: &PsyOp,
    output: &[&ScoredPost],
) {
    for dest in destinations {
        let result = match dest {
            Destination::Discord { webhook_url } => {
                discord::send(webhook_url, psyop_name, psyop, output).await
            }
            Destination::Telegram { bot_token, chat_id } => {
                telegram::send(bot_token, chat_id, psyop_name, psyop, output).await
            }
        };
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    }
}
