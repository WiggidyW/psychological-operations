import { z } from "zod";
import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import { NotificationConfigSchema } from "./notifications/index.js";

const CONFIG_PATH = path.join(os.homedir(), ".psychological-operations", "config.json");

export const ConfigSchema = z.object({
  agent_timeout: z.number().int().positive().default(180),
  agent_max_attempts: z.number().int().positive().default(3),
  notifications: z.array(NotificationConfigSchema).default([]),
});

export type Config = z.infer<typeof ConfigSchema>;

export function loadConfig(): Config {
  if (!fs.existsSync(CONFIG_PATH)) {
    return ConfigSchema.parse({});
  }
  const raw = JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8")) as unknown;
  return ConfigSchema.parse(raw);
}

export function saveConfig(config: Config): void {
  fs.mkdirSync(path.dirname(CONFIG_PATH), { recursive: true });
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2) + "\n", "utf-8");
}
