import { spawn, type ChildProcess } from "node:child_process";
import net from "node:net";
import readline from "node:readline";
import {
  ObjectiveAI,
  agentCompletionsCreateAgentCompletion,
  type AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional,
} from "objectiveai";

import type { Config } from "./config.js";

/** Find an available port. */
async function findPort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, () => {
      const addr = server.address();
      if (addr === null || typeof addr === "string") {
        server.close(() => reject(new Error("Could not determine port")));
        return;
      }
      const port = addr.port;
      server.close(() => resolve(port));
    });
    server.on("error", reject);
  });
}

/** Start the Playwright MCP server on a dynamic port, attached to the existing browser. */
async function startMcpServer(port: number): Promise<ChildProcess> {
  const proc = spawn("npx", ["@playwright/mcp@latest", "--cdp-endpoint", "http://localhost:9222", "--port", String(port)], {
    stdio: "ignore",
    shell: true,
  });

  // Wait for the server to be ready by polling the port
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("MCP server failed to start")), 15_000);
    const check = () => {
      const sock = net.connect(port, "127.0.0.1");
      sock.on("connect", () => { sock.destroy(); clearTimeout(timeout); resolve(); });
      sock.on("error", () => setTimeout(check, 200));
    };
    check();
  });

  return proc;
}

/** Prompt the user for input with a timeout. Returns null on timeout. */
async function promptUser(timeoutMs: number): Promise<string | null> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((resolve) => {
    const timer = setTimeout(() => { rl.close(); resolve(null); }, timeoutMs);
    rl.question("Provide guidance to the agent (or wait for auto-retry): ", (answer) => {
      clearTimeout(timer);
      rl.close();
      resolve(answer.trim() || null);
    });
  });
}

/** Inject the Playwright MCP server URL into the agent's mcp_servers list. */
function withMcpServer(
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional,
  mcpUrl: string,
): AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional {
  // The agent could be inline or remote — we need to inject mcp_servers on the inline base
  const agentBase = (agent as Record<string, unknown>);
  const existing = (agentBase["mcp_servers"] as Array<{ url: string; authorization: boolean }>) ?? [];
  return {
    ...agent,
    mcp_servers: [...existing, { url: mcpUrl, authorization: false }],
  } as AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional;
}

/**
 * Spawn an agent to handle an unexpected page state.
 * The agent uses the Playwright MCP server to observe and interact with the browser.
 * Returns when the agent finishes.
 */
async function runAgent(
  client: ObjectiveAI,
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional,
  mcpUrl: string,
  systemPrompt: string,
  userMessage: string,
): Promise<void> {
  const agentWithMcp = withMcpServer(agent, mcpUrl);

  let continuation: string | undefined;
  let message = userMessage;

  while (true) {
    const result = await agentCompletionsCreateAgentCompletion(client, {
      agent: agentWithMcp,
      messages: [
        { role: "system", content: systemPrompt },
        { role: "user", content: message },
      ],
      ...(continuation !== undefined ? { continuation } : {}),
    });

    continuation = result.continuation ?? undefined;

    // Find the last assistant response
    const lastAssistant = [...result.messages].reverse().find(
      (m) => "finish_reason" in m,
    );

    if (!lastAssistant || !("finish_reason" in lastAssistant)) break;

    // If the agent stopped (not calling tools), it's done
    if (lastAssistant.finish_reason !== "tool_calls") break;

    // Tool calls are handled server-side via continuation — just loop
  }
}

/**
 * Handle an unexpected page state by spawning an agent intervention.
 * Retries up to MAX_RETRIES times. Resets retry count when user provides input.
 *
 * @param client - ObjectiveAI client
 * @param agent - The psyop's agent config
 * @param query - The query that produced the unexpected page
 * @param pageUrl - Current URL of the unexpected page
 */
export async function intervene(
  client: ObjectiveAI,
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional,
  query: string,
  pageUrl: string,
  config: Config,
): Promise<void> {
  const port = await findPort();
  const mcpProc = await startMcpServer(port);
  const mcpUrl = `http://127.0.0.1:${port}/mcp`;

  const systemPrompt =
    `You are a browser automation agent. The browser navigated to X (twitter) ` +
    `to search for "${query}" but encountered an unexpected page state. ` +
    `The current URL is: ${pageUrl}. ` +
    `Use the Playwright MCP tools to observe the page and resolve the issue. ` +
    `Common issues include: login walls, captchas, cookie consent dialogs, ` +
    `age gates, or rate limiting pages. ` +
    `Get the browser to a state where X search results are visible.`;

  let retries = 0;
  let userMessage = "Please observe the current page state and try to resolve the issue.";

  try {
    const maxAttempts = config.agent_max_attempts;
    const timeoutMs = config.agent_timeout * 1000;

    while (retries < maxAttempts) {
      console.log(`Agent intervention attempt ${retries + 1}/${maxAttempts}...`);
      await runAgent(client, agent, mcpUrl, systemPrompt, userMessage);

      // Check if we can return (caller will re-validate)
      retries++;
      if (retries >= maxAttempts) break;

      // Wait for user input
      console.log(`Agent finished. Waiting ${config.agent_timeout} seconds for user guidance...`);
      const input = await promptUser(timeoutMs);
      if (input !== null) {
        retries = 0;
        userMessage = input;
      } else {
        userMessage = "The previous attempt did not resolve the issue. Please try again.";
      }
    }
  } finally {
    mcpProc.kill();
  }
}
