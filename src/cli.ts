import { Command } from "commander";
import { loadConfig, saveConfig } from "./config.js";
import { NotificationConfigSchema } from "./notifications/index.js";

/** Print a config value as compact JSON (matches objectiveai-cli format). */
function configGet(value: unknown): void {
  console.log(JSON.stringify(value));
}

/** Print config set confirmation (matches objectiveai-cli format). */
function configSet(): void {
  console.log("ok");
}

export function buildCli(): Command {
  const program = new Command()
    .name("psychological-operations")
    .description("Agentic X scraper with ObjectiveAI scoring pipeline");

  // ── run ──────────────────────────────────────────────────────────────────────

  program
    .command("run <psyop-name>")
    .description("Run a psyop by name")
    .action(async (name: string) => {
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
      configGet(loadConfig().agent_timeout);
    });

  agentTimeout
    .command("set <value>")
    .description("Set agent timeout")
    .action((value: string) => {
      const cfg = loadConfig();
      cfg.agent_timeout = parseInt(value, 10);
      saveConfig(cfg);
      configSet();
    });

  // agent-max-attempts
  const agentMaxAttempts = config
    .command("agent-max-attempts")
    .description("Agent intervention max retry attempts");

  agentMaxAttempts
    .command("get")
    .description("Get current max attempts")
    .action(() => {
      configGet(loadConfig().agent_max_attempts);
    });

  agentMaxAttempts
    .command("set <value>")
    .description("Set max attempts")
    .action((value: string) => {
      const cfg = loadConfig();
      cfg.agent_max_attempts = parseInt(value, 10);
      saveConfig(cfg);
      configSet();
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
          console.error(`error: no notification at index ${i}`);
          process.exit(1);
        }
        configGet(entry);
      } else {
        configGet(cfg.notifications);
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
      configSet();
    });

  notifications
    .command("del <index>")
    .description("Remove a notification target by index")
    .action((index: string) => {
      const cfg = loadConfig();
      const i = parseInt(index, 10);
      if (i < 0 || i >= cfg.notifications.length) {
        console.error(`error: no notification at index ${i}`);
        process.exit(1);
      }
      cfg.notifications.splice(i, 1);
      saveConfig(cfg);
      configSet();
    });

  return program;
}
