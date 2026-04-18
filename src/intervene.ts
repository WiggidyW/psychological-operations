import { spawn, type ChildProcess } from "node:child_process";
import net from "node:net";
import readline from "node:readline";
import type { AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional } from "objectiveai";
import { runAgentCompletion, getAgentContinuation, fetchAgent } from "./cli_exec.js";
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

/**
 * Resolve the agent to an inline JSON string, fetching it if it's a remote ref.
 * Then inject the Playwright MCP server URL.
 */
async function resolveAgentWithMcp(
  agent: AgentInlineAgentBaseWithFallbacksOrRemoteCommitOptional,
  mcpUrl: string,
): Promise<string> {
  let agentObj: Record<string, unknown>;

  // Check if this is a remote agent (has remote/mock/github fields at top level)
  const raw = agent as Record<string, unknown>;
  if ("remote" in raw && typeof raw["remote"] === "string") {
    // Remote agent — fetch the full definition via CLI
    const ref = formatRemoteRef(raw);
    const fetched = await fetchAgent(ref);
    agentObj = JSON.parse(fetched) as Record<string, unknown>;
  } else {
    agentObj = { ...raw };
  }

  // Inject MCP server
  const existing = (agentObj["mcp_servers"] as Array<{ url: string; authorization: boolean }>) ?? [];
  agentObj["mcp_servers"] = [...existing, { url: mcpUrl, authorization: false }];

  return JSON.stringify(agentObj);
}

/** Format a remote agent ref for the CLI --path argument. */
function formatRemoteRef(raw: Record<string, unknown>): string {
  const remote = raw["remote"] as string;
  if (remote === "mock") {
    return `remote=mock,name=${raw["name"] as string}`;
  }
  // GitHub remote
  const parts = [`remote=${remote}`];
  if (raw["owner"]) parts.push(`owner=${raw["owner"] as string}`);
  if (raw["repository"]) parts.push(`repository=${raw["repository"] as string}`);
  if (raw["commit"]) parts.push(`commit=${raw["commit"] as string}`);
  return parts.join(",");
}

/**
 * Handle an unexpected page state by spawning an agent intervention.
 * Maintains continuation between attempts so the agent keeps context.
 * User input resets retry count and adds their message to the conversation.
 */
export async function intervene(
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

  const agentJson = await resolveAgentWithMcp(agent, mcpUrl);

  let retries = 0;
  let continuation: string | undefined;
  let userMessage = "Please observe the current page state and try to resolve the issue.";

  try {
    const maxAttempts = config.agent_max_attempts;
    const timeoutMs = config.agent_timeout * 1000;

    while (retries < maxAttempts) {
      console.log(`Agent intervention attempt ${retries + 1}/${maxAttempts}...`);

      const messages = [
        { role: "system", content: systemPrompt },
        { role: "user", content: userMessage },
      ];

      const result = await runAgentCompletion(
        agentJson,
        JSON.stringify(messages),
        continuation,
      );

      if (result.text) {
        console.log(`Agent: ${result.text}`);
      }

      // Get continuation for next attempt
      if (result.logId !== undefined) {
        continuation = await getAgentContinuation(result.logId);
      }

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
