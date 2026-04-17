import { z } from "zod";
import { sendDiscord } from "./discord.js";
import { sendTelegram } from "./telegram.js";

const DiscordNotificationSchema = z.object({
  type: z.literal("discord"),
  webhook_url: z.string(),
});

const TelegramNotificationSchema = z.object({
  type: z.literal("telegram"),
  bot_token: z.string(),
  chat_id: z.string(),
});

export const NotificationConfigSchema = z.discriminatedUnion("type", [
  DiscordNotificationSchema,
  TelegramNotificationSchema,
]);

export type NotificationConfig = z.infer<typeof NotificationConfigSchema>;

export async function notify(configs: NotificationConfig[], message: string): Promise<void> {
  const tasks: Promise<void>[] = [];

  for (const config of configs) {
    switch (config.type) {
      case "discord":
        tasks.push(sendDiscord(config.webhook_url, message));
        break;
      case "telegram":
        tasks.push(sendTelegram(config.bot_token, config.chat_id, message));
        break;
    }
  }

  const results = await Promise.allSettled(tasks);
  for (const result of results) {
    if (result.status === "rejected") {
      console.error("Notification failed:", result.reason);
    }
  }
}
