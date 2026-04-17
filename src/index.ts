#!/usr/bin/env node

import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import git from "isomorphic-git";
import { ObjectiveAI } from "objectiveai";
import { PsyOpSchema } from "./psyop.js";
import { Db } from "./db.js";
import { scrape } from "./scrape.js";

const PSYOPS_DIR = path.join(os.homedir(), ".psychological-operations", "psyops");
const PSYOP_NAME = process.argv[2];

if (!PSYOP_NAME) {
  console.error("Usage: npx psychological-operations <psyop-name>");
  process.exit(1);
}

async function main() {
  const configPath = path.join(PSYOPS_DIR, PSYOP_NAME, "psyop.json");
  if (!fs.existsSync(configPath)) {
    console.error(`PsyOp not found: ${configPath}`);
    process.exit(1);
  }

  const raw = JSON.parse(fs.readFileSync(configPath, "utf-8")) as unknown;
  const psyop = PsyOpSchema.parse(raw);

  const psyopDir = path.join(PSYOPS_DIR, PSYOP_NAME);
  const commitSha = await git.resolveRef({ fs, dir: psyopDir, ref: "HEAD" });

  const client = new ObjectiveAI();
  const db = new Db();
  try {
    await scrape(client, psyop, PSYOP_NAME, commitSha, db);
  } finally {
    db.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
