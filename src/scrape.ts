import { chromium } from "playwright-core";
import path from "node:path";
import os from "node:os";
import type {
  AgentCompletionsMessageRichContent,
  AgentCompletionsMessageRichContentPart,
} from "objectiveai";
import type { Db } from "./db.js";
import type { PsyOp } from "./psyop.js";

const USER_DATA_DIR = path.join(os.homedir(), ".psychological-operations", "chrome-data");

interface TweetData {
  id: string;
  handle: string;
  content: AgentCompletionsMessageRichContent;
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

/** Extract image URLs from a tweet. */
async function getImages(article: import("playwright-core").Locator): Promise<string[]> {
  const imgs = article.locator('[data-testid="tweetPhoto"] img');
  const count = await imgs.count();
  const urls: string[] = [];
  for (let i = 0; i < count; i++) {
    const src = await imgs.nth(i).getAttribute("src").catch(() => null);
    if (src) urls.push(src);
  }
  return urls;
}

/** Extract video URLs from a tweet. */
async function getVideos(article: import("playwright-core").Locator): Promise<string[]> {
  const videos = article.locator("video");
  const count = await videos.count();
  const urls: string[] = [];
  for (let i = 0; i < count; i++) {
    const src = await videos.nth(i).getAttribute("src").catch(() => null);
    if (src) urls.push(src);
  }
  return urls;
}

/** Build a RichContent value from text, images, and videos. */
function buildContent(
  text: string,
  imageUrls: string[],
  videoUrls: string[],
): AgentCompletionsMessageRichContent {
  if (imageUrls.length === 0 && videoUrls.length === 0) {
    return text;
  }

  const parts: AgentCompletionsMessageRichContentPart[] = [];
  if (text) {
    parts.push({ type: "text", text });
  }
  for (const url of imageUrls) {
    parts.push({ type: "image_url", image_url: { url } });
  }
  for (const url of videoUrls) {
    parts.push({ type: "video_url", video_url: { url } });
  }
  return parts;
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

  const [created, text, imageUrls, videoUrls, community] = await Promise.all([
    getCreated(article),
    getText(article),
    getImages(article),
    getVideos(article),
    getCommunity(article),
  ]);

  return {
    id,
    handle,
    content: buildContent(text, imageUrls, videoUrls),
    created,
    community,
  };
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
  const maxPosts = psyop.stages[0]!.count;

  const context = await chromium.launchPersistentContext(USER_DATA_DIR, {
    headless: false,
    channel: "chrome",
    args: [
      "--remote-debugging-port=9222",
      "--disable-blink-features=AutomationControlled",
    ],
  });

  const page = context.pages()[0] ?? await context.newPage();

  const url = `https://x.com/search?q=${encodeURIComponent(psyop.query)}&src=typed_query&f=live`;
  console.log(`Searching X for: ${psyop.query}`);
  await page.goto(url, { waitUntil: "domcontentloaded" });

  const seen = new Set<string>();
  let staleScrolls = 0;

  while (seen.size < maxPosts && staleScrolls < 5) {
    const articles = page.locator("article");
    const count = await articles.count();
    const prevSize = seen.size;

    for (let i = 0; i < count; i++) {
      if (seen.size >= maxPosts) break;

      const article = articles.nth(i);
      const tweet = await parseTweet(article);
      if (!tweet || seen.has(tweet.id)) continue;

      seen.add(tweet.id);
      db.insertPost({ ...tweet, scrape_id: name, psyop: name, psyop_commit_sha: psyopCommitSha });
      console.log(`[${seen.size}] @${tweet.handle}: ${typeof tweet.content === "string" ? tweet.content.slice(0, 80) : "(media)"}`);
    }

    if (seen.size === prevSize) {
      staleScrolls++;
    } else {
      staleScrolls = 0;
    }

    await page.evaluate(() => window.scrollBy(0, window.innerHeight * 2));
    await page.waitForTimeout(2000);
  }

  await context.close();
  console.log(`Scraped ${seen.size} tweets.`);
  return seen.size;
}
