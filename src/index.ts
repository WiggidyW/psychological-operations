#!/usr/bin/env node

import { chromium } from "playwright-core";
import path from "node:path";
import os from "node:os";
import fs from "node:fs";

const STATE_PATH = path.join(os.homedir(), ".psychological-operations", "state.json");
const SEARCH_QUERY = process.argv[2];

if (!SEARCH_QUERY) {
  console.error("Usage: npx psychological-operations <search query>");
  process.exit(1);
}

async function main() {
  const browser = await chromium.launch({
    headless: false,
    channel: "chrome",
    args: ["--remote-debugging-port=9222"],
  });

  // Load stored state if it exists
  const hasState = fs.existsSync(STATE_PATH);
  const context = await browser.newContext(
    hasState ? { storageState: STATE_PATH } : undefined,
  );
  const page = await context.newPage();

  // Navigate to X search
  const url = `https://x.com/search?q=${encodeURIComponent(SEARCH_QUERY)}&src=typed_query&f=live`;
  console.log(`Searching X for: ${SEARCH_QUERY}`);
  await page.goto(url, { waitUntil: "domcontentloaded" });

  console.log(`Current URL: ${page.url()}`);

  // Save state for next run
  fs.mkdirSync(path.dirname(STATE_PATH), { recursive: true });
  await context.storageState({ path: STATE_PATH });

  await browser.close();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
