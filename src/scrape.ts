import { chromium } from "playwright-core";
import path from "node:path";
import os from "node:os";
import type {
  AgentCompletionsMessageImageUrl,
  AgentCompletionsMessageVideoUrl,
} from "objectiveai";
import type { Db } from "./db.js";
import type { PsyOp } from "./psyop.js";

const USER_DATA_DIR = path.join(os.homedir(), ".psychological-operations", "chrome-data");

interface TweetData {
  id: string;
  handle: string;
  text: string;
  images: AgentCompletionsMessageImageUrl[];
  videos: AgentCompletionsMessageVideoUrl[];
  created: string;
  community: string | null;
}

/** Extract the tweet ID from an article's permalink. */
async function getTweetId(article: import("playwright-core").Locator): Promise<string | null> {
  const link = article.locator('a[href*="/status/"]').first();
  const href = await link.getAttribute("href").catch(() => null);
  if (!href) return null;
  const match = /\/status\/(\d+)/.exec(href);
  return match?.[1] ?? null;
}

/** Extract the handle from an article. */
async function getHandle(article: import("playwright-core").Locator): Promise<string | null> {
  const link = article.locator('a[href^="/"][role="link"] span').filter({ hasText: /^@/ }).first();
  const text = await link.textContent().catch(() => null);
  return text?.replace(/^@/, "") ?? null;
}

/** Extract the timestamp from an article. */
async function getCreated(article: import("playwright-core").Locator): Promise<string> {
  const time = article.locator("time").first();
  return await time.getAttribute("datetime") ?? new Date().toISOString();
}

/** Extract text content from a tweet. */
async function getText(article: import("playwright-core").Locator): Promise<string> {
  const tweetText = article.locator('[data-testid="tweetText"]').first();
  return await tweetText.textContent().catch(() => "") ?? "";
}

/** Extract images from a tweet. */
async function getImages(article: import("playwright-core").Locator): Promise<AgentCompletionsMessageImageUrl[]> {
  const imgs = article.locator('[data-testid="tweetPhoto"] img');
  const count = await imgs.count();
  const results: AgentCompletionsMessageImageUrl[] = [];
  for (let i = 0; i < count; i++) {
    const src = await imgs.nth(i).getAttribute("src").catch(() => null);
    if (src) results.push({ url: src });
  }
  return results;
}

/** Extract videos from a tweet. */
async function getVideos(article: import("playwright-core").Locator): Promise<AgentCompletionsMessageVideoUrl[]> {
  const vids = article.locator("video");
  const count = await vids.count();
  const results: AgentCompletionsMessageVideoUrl[] = [];
  for (let i = 0; i < count; i++) {
    const src = await vids.nth(i).getAttribute("src").catch(() => null);
    if (src) results.push({ url: src });
  }
  return results;
}

/** Extract community note text if present. */
async function getCommunity(article: import("playwright-core").Locator): Promise<string | null> {
  const note = article.locator('[data-testid="birdwatch-pivot"]').first();
  return await note.textContent().catch(() => null);
}

/** Parse a single article element into TweetData. */
async function parseTweet(article: import("playwright-core").Locator): Promise<TweetData | null> {
  const id = await getTweetId(article);
  if (!id) return null;

  const handle = await getHandle(article);
  if (!handle) return null;

  const [created, text, images, videos, community] = await Promise.all([
    getCreated(article),
    getText(article),
    getImages(article),
    getVideos(article),
    getCommunity(article),
  ]);

  return { id, handle, text, images, videos, created, community };
}

/**
 * Open a browser, navigate to X search, and scrape tweets.
 * Uses the first stage's count as the target number of posts.
 */
export async function scrape(
  psyop: PsyOp,
  name: string,
  psyopCommitSha: string,
  db: Db,
): Promise<number> {
  const maxPerQuery = Math.ceil((psyop.stages[0]!.count ?? 100) / psyop.queries.length);

  const context = await chromium.launchPersistentContext(USER_DATA_DIR, {
    headless: false,
    channel: "chrome",
    args: [
      "--remote-debugging-port=9222",
      "--disable-blink-features=AutomationControlled",
    ],
  });

  const page = context.pages()[0] ?? await context.newPage();
  const seen = new Set<string>();

  for (const query of psyop.queries) {
    const url = `https://x.com/search?q=${encodeURIComponent(query)}&src=typed_query&f=live`;
    console.log(`Searching X for: ${query}`);
    await page.goto(url, { waitUntil: "domcontentloaded" });

    let staleScrolls = 0;
    const queryStart = seen.size;

    while (seen.size - queryStart < maxPerQuery && staleScrolls < 5) {
      const articles = page.locator("article");
      const count = await articles.count();
      const prevSize = seen.size;

      for (let i = 0; i < count; i++) {
        if (seen.size - queryStart >= maxPerQuery) break;

        const article = articles.nth(i);
        const tweet = await parseTweet(article);
        if (!tweet || seen.has(tweet.id)) continue;

        seen.add(tweet.id);
        db.insertPost({ ...tweet, scrape_id: name, query, psyop: name, psyop_commit_sha: psyopCommitSha });
        console.log(`[${seen.size}] @${tweet.handle}: ${tweet.text.slice(0, 80)}`);
      }

      if (seen.size === prevSize) {
        staleScrolls++;
      } else {
        staleScrolls = 0;
      }

      await page.evaluate(() => window.scrollBy(0, window.innerHeight * 2));
      await page.waitForTimeout(2000);
    }
  }

  await context.close();
  console.log(`Scraped ${seen.size} tweets.`);
  return seen.size;
}
