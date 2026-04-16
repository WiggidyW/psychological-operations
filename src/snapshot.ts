#!/usr/bin/env node

import { chromium } from "playwright-core";
import path from "node:path";
import os from "node:os";
import fs from "node:fs";

const USER_DATA_DIR = path.join(os.homedir(), ".psychological-operations", "chrome-data");
const QUERY = process.argv[2] ?? "artificial intelligence";
const TARGET = 100;
const OUT_DIR = path.join(import.meta.dirname, "..", "test-fixtures");

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

  const url = `https://x.com/search?q=${encodeURIComponent(QUERY)}&src=typed_query&f=live`;
  console.log(`Navigating to: ${url}`);
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.waitForTimeout(3000);

  const collected = new Map<string, string>(); // tweet ID -> outerHTML
  let staleScrolls = 0;

  while (collected.size < TARGET && staleScrolls < 10) {
    const prevSize = collected.size;
    const articles = page.locator("article");
    const count = await articles.count();

    for (let i = 0; i < count; i++) {
      const article = articles.nth(i);

      // Extract tweet ID from permalink
      const id = await article.evaluate((el) => {
        const link = el.querySelector('a[href*="/status/"]');
        if (!link) return null;
        const match = /\/status\/(\d+)/.exec(link.getAttribute("href") ?? "");
        return match?.[1] ?? null;
      });

      if (!id || collected.has(id)) continue;

      const html = await article.evaluate((el) => el.outerHTML);
      collected.set(id, html);
      console.log(`[${collected.size}/${TARGET}] Captured tweet ${id}`);
    }

    if (collected.size === prevSize) {
      staleScrolls++;
    } else {
      staleScrolls = 0;
    }

    await page.evaluate(() => window.scrollBy(0, window.innerHeight * 2));
    await page.waitForTimeout(2000);
  }

  fs.mkdirSync(OUT_DIR, { recursive: true });
  const outPath = path.join(OUT_DIR, "articles.json");
  fs.writeFileSync(outPath, JSON.stringify([...collected.values()], null, 2), "utf-8");
  console.log(`\nSaved ${collected.size} articles to ${outPath}`);

  await context.close();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
