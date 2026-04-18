import net from "node:net";
import path from "node:path";
import os from "node:os";
import fs from "node:fs";

const SOCK_DIR = path.join(os.homedir(), ".psychological-operations");

function sockPath(pid: number): string {
  return path.join(SOCK_DIR, `agent-${pid}.sock`);
}

/**
 * Start an IPC server that listens for a single message on a Unix domain socket.
 * When a client connects and sends a message, resolves with the message.
 * If timeoutMs elapses with no connection, resolves with null.
 * The client's connection is kept alive and returned so output can be piped back.
 */
export function waitForMessage(
  pid: number,
  timeoutMs: number,
): Promise<{ message: string; client: net.Socket } | null> {
  const sock = sockPath(pid);
  fs.mkdirSync(SOCK_DIR, { recursive: true });

  // Clean up stale socket
  try { fs.unlinkSync(sock); } catch { /* doesn't exist */ }

  return new Promise((resolve) => {
    const server = net.createServer((client) => {
      const chunks: Buffer[] = [];
      client.on("data", (chunk: Buffer) => chunks.push(chunk));
      client.on("end", () => {
        // Client sent message and half-closed — but we keep the socket open for writing back
      });
      // Resolve once we get the first newline (message delimiter)
      client.once("data", () => {
        // Small delay to accumulate the full message
        setTimeout(() => {
          clearTimeout(timer);
          server.close();
          const message = Buffer.concat(chunks).toString("utf-8").trim();
          resolve({ message, client });
        }, 50);
      });
    });

    server.listen(sock);

    const timer = setTimeout(() => {
      server.close();
      try { fs.unlinkSync(sock); } catch { /* ignore */ }
      resolve(null);
    }, timeoutMs);

    server.on("close", () => {
      try { fs.unlinkSync(sock); } catch { /* ignore */ }
    });
  });
}

/**
 * Send a message to a running process's IPC socket and pipe its output
 * back to the current process's stdout/stderr until the socket closes.
 */
export async function sendMessage(pid: number, message: string): Promise<void> {
  const sock = sockPath(pid);

  if (!fs.existsSync(sock)) {
    throw new Error(`No agent waiting for input (socket not found: ${sock})`);
  }

  return new Promise((resolve, reject) => {
    const client = net.connect(sock);
    client.on("connect", () => {
      client.write(message + "\n");
      // Half-close write side — we're done sending
      client.end();
    });
    client.on("data", (chunk) => {
      process.stdout.write(chunk);
    });
    client.on("close", () => resolve());
    client.on("error", reject);
  });
}

/** Clean up socket file for a given PID. */
export function cleanupSocket(pid: number): void {
  try { fs.unlinkSync(sockPath(pid)); } catch { /* ignore */ }
}
