#!/usr/bin/env node

import { chromium } from "playwright-core";
import path from "node:path";
import os from "node:os";

const USER_DATA_DIR = path.join(os.homedir(), ".psychological-operations", "chrome-data");
const SEARCH_QUERY = process.argv[2];

if (!SEARCH_QUERY) {
  console.error("Usage: npx psychological-operations <search query>");
  process.exit(1);
}

async function main() {
  const context = await chromium.launchPersistentContext(USER_DATA_DIR, {
    headless: false,
    channel: "chrome",
    args: [
      "--remote-debugging-port=9222",
      "--disable-blink-features=AutomationControlled",
    ],
  });

  const page = context.pages()[0] ?? await context.newPage();

  // Navigate to X search
  const url = `https://x.com/search?q=${encodeURIComponent(SEARCH_QUERY)}&src=typed_query&f=live`;
  console.log(`Searching X for: ${SEARCH_QUERY}`);
  await page.goto(url, { waitUntil: "domcontentloaded" });

  console.log(`Current URL: ${page.url()}`);
  console.log("Sign in if needed, then close the browser.");

  // Wait for browser to be closed manually
  await new Promise<void>((resolve) => {
    context.on("close", () => resolve());
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
