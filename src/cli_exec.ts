import { spawn } from "node:child_process";

const LOG_ID_PREFIX = "Logs ID: ";

/**
 * Run an objectiveai CLI command.
 * stdout and stderr are passed through directly to the terminal.
 * stdout is also captured and returned for parsing.
 */
export async function objectiveaiExec(args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    const proc = spawn("objectiveai", args, { stdio: ["inherit", "pipe", "inherit"] });

    const chunks: Buffer[] = [];
    proc.stdout.on("data", (chunk: Buffer) => {
      process.stdout.write(chunk); // passthrough
      chunks.push(chunk);
    });

    proc.on("close", (code) => {
      const stdout = Buffer.concat(chunks).toString("utf-8");
      if (code !== 0) {
        reject(new Error(`objectiveai exited with code ${code}`));
      } else {
        resolve(stdout);
      }
    });

    proc.on("error", reject);
  });
}

/** Parse the log ID from CLI output. */
function parseLogId(stdout: string): string | undefined {
  for (const line of stdout.split("\n")) {
    if (line.startsWith(LOG_ID_PREFIX)) {
      return line.slice(LOG_ID_PREFIX.length).trim();
    }
  }
  return undefined;
}

/** Extract text content after the log ID line. */
function parseTextAfterLogId(stdout: string): string {
  const lines = stdout.split("\n");
  let pastLogId = false;
  const result: string[] = [];
  for (const line of lines) {
    if (!pastLogId && line.startsWith(LOG_ID_PREFIX)) {
      pastLogId = true;
      continue;
    }
    if (pastLogId) {
      result.push(line);
    }
  }
  return result.join("\n").trim();
}

export interface ExecutionResult {
  output: number | number[] | number[][] | unknown;
  errors?: Array<{ path: string | number[]; error: unknown }>;
}

/**
 * Run a function execution via the CLI.
 */
export async function runFunctionExecution(
  functionJson: string,
  profileJson: string,
  inputJson: string,
  strategy: "standard" | "swiss-system" = "standard",
): Promise<ExecutionResult> {
  const args = [
    "functions", "executions", "create", strategy,
    "--function-inline", functionJson,
    "--profile-inline", profileJson,
    "--input-inline", inputJson,
  ];

  const stdout = await objectiveaiExec(args);

  // The last line is the JSON result
  const lines = stdout.trim().split("\n");
  const jsonLine = lines[lines.length - 1];
  if (jsonLine === undefined) {
    throw new Error("objectiveai function execution returned no output");
  }
  return JSON.parse(jsonLine) as ExecutionResult;
}

/**
 * Fetch an agent definition by remote reference.
 * Returns the inline agent JSON.
 */
export async function fetchAgent(ref: string): Promise<string> {
  const stdout = await objectiveaiExec(["agents", "get", "--path", ref]);
  return stdout.trim();
}

/**
 * Run an agent completion via the CLI.
 * Returns the assistant text and log ID.
 */
export async function runAgentCompletion(
  agentJson: string,
  messagesJson: string,
  continuation?: string,
): Promise<{ text: string; logId?: string }> {
  const args = [
    "agents", "completions", "create", "standard",
    "--agent-inline", agentJson,
    "--messages-inline", messagesJson,
  ];

  if (continuation !== undefined) {
    args.push("--openrouter-continuation-from-response", continuation);
  }

  const stdout = await objectiveaiExec(args);
  const logId = parseLogId(stdout);
  const text = parseTextAfterLogId(stdout);

  return { text, logId };
}

/**
 * Get the continuation token from an agent completion log.
 */
export async function getAgentContinuation(logId: string): Promise<string | undefined> {
  try {
    const stdout = await objectiveaiExec([
      "agents", "completions", "continuations", "logs", "get", logId,
    ]);
    const trimmed = stdout.trim();
    return trimmed || undefined;
  } catch {
    return undefined;
  }
}
