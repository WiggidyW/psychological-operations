import { Command } from "commander";
import { loadConfig, saveConfig } from "./config.js";
import { NotificationConfigSchema, type NotificationConfig } from "./notifications/index.js";

export function buildCli(): Command {
  const program = new Command()
    .name("psychological-operations")
    .description("Agentic X scraper with ObjectiveAI scoring pipeline");

  // ── run ──────────────────────────────────────────────────────────────────────

  program
    .command("run <psyop-name>")
    .description("Run a psyop by name")
    .action(async (name: string) => {
      // Dynamically import to avoid loading everything for config commands
      const { main } = await import("./index.js");
      await main(name);
    });

  // ── config ───────────────────────────────────────────────────────────────────

  const config = program
    .command("config")
    .description("Manage configuration");

  // agent-timeout
  const agentTimeout = config
    .command("agent-timeout")
    .description("Agent intervention timeout in seconds");

  agentTimeout
    .command("get")
    .description("Get current agent timeout")
    .action(() => {
      const cfg = loadConfig();
      console.log(cfg.agent_timeout);
    });

  agentTimeout
    .command("set <value>")
    .description("Set agent timeout")
    .action((value: string) => {
      const cfg = loadConfig();
      cfg.agent_timeout = parseInt(value, 10);
      saveConfig(cfg);
    });

  // agent-max-attempts
  const agentMaxAttempts = config
    .command("agent-max-attempts")
    .description("Agent intervention max retry attempts");

  agentMaxAttempts
    .command("get")
    .description("Get current max attempts")
    .action(() => {
      const cfg = loadConfig();
      console.log(cfg.agent_max_attempts);
    });

  agentMaxAttempts
    .command("set <value>")
    .description("Set max attempts")
    .action((value: string) => {
      const cfg = loadConfig();
      cfg.agent_max_attempts = parseInt(value, 10);
      saveConfig(cfg);
    });

  // notifications
  const notifications = config
    .command("notifications")
    .description("Manage notification targets");

  notifications
    .command("get [index]")
    .description("Get all notifications or one by index")
    .action((index?: string) => {
      const cfg = loadConfig();
      if (index !== undefined) {
        const i = parseInt(index, 10);
        const entry = cfg.notifications[i];
        if (entry === undefined) {
          console.error(`No notification at index ${i}`);
          process.exit(1);
        }
        console.log(JSON.stringify(entry, null, 2));
      } else {
        console.log(JSON.stringify(cfg.notifications, null, 2));
      }
    });

  notifications
    .command("add <json>")
    .description("Add a notification target (JSON string)")
    .action((json: string) => {
      const cfg = loadConfig();
      const parsed = NotificationConfigSchema.parse(JSON.parse(json));
      cfg.notifications.push(parsed);
      saveConfig(cfg);
      console.log(`Added notification at index ${cfg.notifications.length - 1}`);
    });

  notifications
    .command("del <index>")
    .description("Remove a notification target by index")
    .action((index: string) => {
      const cfg = loadConfig();
      const i = parseInt(index, 10);
      if (i < 0 || i >= cfg.notifications.length) {
        console.error(`No notification at index ${i}`);
        process.exit(1);
      }
      cfg.notifications.splice(i, 1);
      saveConfig(cfg);
      console.log(`Removed notification at index ${i}`);
    });

  return program;
}
