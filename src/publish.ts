import path from "node:path";
import os from "node:os";
import fs from "node:fs";
import git from "isomorphic-git";
import type { PsyOp } from "./psyop.js";

const PSYOPS_DIR = path.join(os.homedir(), ".psychological-operations", "psyops");

/**
 * Publish a psyop config to its git repo folder.
 * Creates the repo if it doesn't exist, otherwise commits as a new version.
 */
export async function publish(name: string, psyop: PsyOp, message: string): Promise<string> {
  const dir = path.join(PSYOPS_DIR, name);
  const configPath = path.join(dir, "psyop.json");

  // Initialize repo if it doesn't exist
  if (!fs.existsSync(path.join(dir, ".git"))) {
    fs.mkdirSync(dir, { recursive: true });
    await git.init({ fs, dir });
  }

  // Write the psyop config
  fs.writeFileSync(configPath, JSON.stringify(psyop, null, 2) + "\n", "utf-8");

  // Stage and commit
  await git.add({ fs, dir, filepath: "psyop.json" });
  const sha = await git.commit({
    fs,
    dir,
    message,
    author: { name: "psychological-operations", email: "psyops@localhost" },
  });

  console.log(`Published psyop "${name}" at ${sha}`);
  return sha;
}
