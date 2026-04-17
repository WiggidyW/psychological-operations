import { sendDiscord } from "./discord.js";
import { sendTelegram } from "./telegram.js";

export interface NotificationConfig {
  discord?: { webhook_url: string };
  telegram?: { bot_token: string; chat_id: string };
}

export async function notify(config: NotificationConfig, message: string): Promise<void> {
  const tasks: Promise<void>[] = [];

  if (config.discord) {
    tasks.push(sendDiscord(config.discord.webhook_url, message));
  }
  if (config.telegram) {
    tasks.push(sendTelegram(config.telegram.bot_token, config.telegram.chat_id, message));
  }

  const results = await Promise.allSettled(tasks);
  for (const result of results) {
    if (result.status === "rejected") {
      console.error("Notification failed:", result.reason);
    }
  }
}
