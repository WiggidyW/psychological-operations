import { spawn, type ChildProcess } from "node:child_process";
import net from "node:net";
import { findPort } from "./port.js";

let mcpProc: ChildProcess | null = null;

/** Start the Playwright MCP server on a dynamic port, connecting to Chrome via the given CDP port. */
export async function startMcpServer(cdpPort: number): Promise<number> {
  if (mcpProc !== null) {
    throw new Error("MCP server already running");
  }

  const mcpPort = await findPort();

  mcpProc = spawn("npx", [
    "@playwright/mcp@latest",
    "--cdp-endpoint", `http://localhost:${cdpPort}`,
    "--port", String(mcpPort),
  ], {
    stdio: "ignore",
    shell: true,
  });

  // Wait for the server to be ready
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("MCP server failed to start")), 15_000);
    const check = () => {
      const sock = net.connect(mcpPort, "127.0.0.1");
      sock.on("connect", () => { sock.destroy(); clearTimeout(timeout); resolve(); });
      sock.on("error", () => setTimeout(check, 200));
    };
    check();
  });

  return mcpPort;
}

/** Stop the running MCP server. */
export function stopMcpServer(): void {
  if (mcpProc !== null) {
    mcpProc.kill();
    mcpProc = null;
  }
}
