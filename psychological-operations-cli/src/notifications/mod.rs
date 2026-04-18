pub mod discord;
pub mod telegram;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationConfig {
    #[serde(rename = "discord")]
    Discord { webhook_url: String },
    #[serde(rename = "telegram")]
    Telegram { bot_token: String, chat_id: String },
}

pub async fn notify(configs: &[NotificationConfig], message: &str) {
    for config in configs {
        let result = match config {
            NotificationConfig::Discord { webhook_url } => {
                discord::send(webhook_url, message).await
            }
            NotificationConfig::Telegram { bot_token, chat_id } => {
                telegram::send(bot_token, chat_id, message).await
            }
        };
        if let Err(e) = result {
            eprintln!("notification failed: {e}");
        }
    }
}
