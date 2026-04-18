#!/usr/bin/env node

import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import git from "isomorphic-git";
import { PsyOpSchema } from "./psyop.js";
import { Db } from "./db.js";
import { scrape } from "./scrape.js";
import { loadConfig } from "./config.js";
import { notify } from "./notifications/index.js";
import { buildCli } from "./cli.js";

const PSYOPS_DIR = path.join(os.homedir(), ".psychological-operations", "psyops");

export async function main(name: string, detachStdin: boolean = false): Promise<void> {
  const config = loadConfig();

  const configPath = path.join(PSYOPS_DIR, name, "psyop.json");
  if (!fs.existsSync(configPath)) {
    console.error(`PsyOp not found: ${configPath}`);
    process.exit(1);
  }

  const raw = JSON.parse(fs.readFileSync(configPath, "utf-8")) as unknown;
  const psyop = PsyOpSchema.parse(raw);

  const psyopDir = path.join(PSYOPS_DIR, name);
  const commitSha = await git.resolveRef({ fs, dir: psyopDir, ref: "HEAD" });

  const db = new Db();
  try {
    const count = await scrape(psyop, name, commitSha, db, config, detachStdin);
    await notify(config.notifications, `PsyOp "${name}": scraped ${count} posts.`);
  } finally {
    db.close();
  }
}

buildCli().parseAsync(process.argv).catch((err) => {
  console.error(err);
  process.exit(1);
});
