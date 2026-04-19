pub mod discord;
pub mod telegram;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Destination {
    #[serde(rename = "discord")]
    Discord { webhook_url: String },
    #[serde(rename = "telegram")]
    Telegram { bot_token: String, chat_id: String },
}

pub async fn notify(destinations: &[Destination], message: &str) {
    for dest in destinations {
        let result = match dest {
            Destination::Discord { webhook_url } => {
                discord::send(webhook_url, message).await
            }
            Destination::Telegram { bot_token, chat_id } => {
                telegram::send(bot_token, chat_id, message).await
            }
        };
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    }
}
