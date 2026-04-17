#!/usr/bin/env node

import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import git from "isomorphic-git";
import { ObjectiveAI } from "objectiveai";
import { PsyOpSchema } from "./psyop.js";
import { Db } from "./db.js";
import { scrape } from "./scrape.js";
import { buildCli } from "./cli.js";

const PSYOPS_DIR = path.join(os.homedir(), ".psychological-operations", "psyops");

export async function main(name: string): Promise<void> {
  const configPath = path.join(PSYOPS_DIR, name, "psyop.json");
  if (!fs.existsSync(configPath)) {
    console.error(`PsyOp not found: ${configPath}`);
    process.exit(1);
  }

  const raw = JSON.parse(fs.readFileSync(configPath, "utf-8")) as unknown;
  const psyop = PsyOpSchema.parse(raw);

  const psyopDir = path.join(PSYOPS_DIR, name);
  const commitSha = await git.resolveRef({ fs, dir: psyopDir, ref: "HEAD" });

  const client = new ObjectiveAI();
  const db = new Db();
  try {
    await scrape(client, psyop, name, commitSha, db);
  } finally {
    db.close();
  }
}

buildCli().parseAsync(process.argv).catch((err) => {
  console.error(err);
  process.exit(1);
});
