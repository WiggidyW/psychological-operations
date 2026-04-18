#!/usr/bin/env node

import readline from "node:readline";
import { handleCommand } from "./protocol.js";

const rl = readline.createInterface({ input: process.stdin });

rl.on("line", async (line) => {
  try {
    const cmd = JSON.parse(line) as Record<string, unknown>;
    const result = await handleCommand(cmd);
    process.stdout.write(JSON.stringify(result) + "\n");
  } catch (err) {
    process.stdout.write(JSON.stringify({ error: String(err) }) + "\n");
  }
});
