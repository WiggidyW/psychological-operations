import { spawn, type ChildProcess } from "node:child_process";
import net from "node:net";

let mcpProc: ChildProcess | null = null;
let mcpPort: number | null = null;

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

/** Start the Playwright MCP server on a dynamic port. Returns the port number. */
export async function startMcpServer(): Promise<number> {
  if (mcpProc !== null) {
    throw new Error("MCP server already running");
  }

  const port = await findPort();

  mcpProc = spawn("npx", ["@playwright/mcp@latest", "--cdp-endpoint", "http://localhost:9222", "--port", String(port)], {
    stdio: "ignore",
    shell: true,
  });

  // Wait for the server to be ready
  await new Promise<void>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("MCP server failed to start")), 15_000);
    const check = () => {
      const sock = net.connect(port, "127.0.0.1");
      sock.on("connect", () => { sock.destroy(); clearTimeout(timeout); resolve(); });
      sock.on("error", () => setTimeout(check, 200));
    };
    check();
  });

  mcpPort = port;
  return port;
}

/** Stop the running MCP server. */
export function stopMcpServer(): void {
  if (mcpProc !== null) {
    mcpProc.kill();
    mcpProc = null;
    mcpPort = null;
  }
}
