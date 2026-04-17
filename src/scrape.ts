import { chromium } from "playwright-core";
import type { Page } from "playwright-core";
import path from "node:path";
import os from "node:os";
import type {
  AgentCompletionsMessageImageUrl,
  AgentCompletionsMessageVideoUrl,
} from "objectiveai";
import type { Db } from "./db.js";
import { validForPsyop, type PsyOp } from "./psyop.js";

const USER_DATA_DIR = path.join(os.homedir(), ".psychological-operations", "chrome-data");

interface TweetData {
  id: string;
  handle: string;
  text: string;
  images: AgentCompletionsMessageImageUrl[];
  videos: AgentCompletionsMessageVideoUrl[];
  created: string;
  community: string | null;
  likes: number;
}

interface QueryTab {
  query: string;
  page: Page;
  buffer: TweetData[];
  seen: Set<string>;
  open: boolean;
  staleScrolls: number;
}

// ---------------------------------------------------------------------------
// Tweet extraction helpers
// ---------------------------------------------------------------------------

async function getTweetId(article: import("playwright-core").Locator): Promise<string | null> {
  const link = article.locator('a[href*="/status/"]').first();
  const href = await link.getAttribute("href").catch(() => null);
  if (!href) return null;
  const match = /\/status\/(\d+)/.exec(href);
  return match?.[1] ?? null;
}

async function getHandle(article: import("playwright-core").Locator): Promise<string | null> {
  const link = article.locator('a[href^="/"][role="link"] span').filter({ hasText: /^@/ }).first();
  const text = await link.textContent().catch(() => null);
  return text?.replace(/^@/, "") ?? null;
}

async function getCreated(article: import("playwright-core").Locator): Promise<string> {
  const time = article.locator("time").first();
  return await time.getAttribute("datetime") ?? new Date().toISOString();
}

async function getText(article: import("playwright-core").Locator): Promise<string> {
  const tweetText = article.locator('[data-testid="tweetText"]').first();

  // Expand truncated tweets
  const showMore = article.getByRole("button", { name: /Show more/i }).first();
  if (await showMore.isVisible().catch(() => false)) {
    const before = await tweetText.textContent().catch(() => "") ?? "";
    await showMore.click();
    // Wait for the text to actually change
    await tweetText.evaluate(
      (el, prev) => new Promise<void>((resolve) => {
        if (el.textContent !== prev) { resolve(); return; }
        const obs = new MutationObserver(() => {
          if (el.textContent !== prev) { obs.disconnect(); resolve(); }
        });
        obs.observe(el, { childList: true, subtree: true, characterData: true });
      }),
      before,
    ).catch(() => {});
  }

  return await tweetText.textContent().catch(() => "") ?? "";
}

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

async function getLikes(article: import("playwright-core").Locator): Promise<number> {
  const btn = article.locator('[data-testid="like"], [data-testid="unlike"]').first();
  const label = await btn.getAttribute("aria-label").catch(() => null);
  if (!label) return 0;
  const match = /(\d+)/.exec(label);
  return match ? parseInt(match[1]!, 10) : 0;
}

async function getCommunity(article: import("playwright-core").Locator): Promise<string | null> {
  const note = article.locator('[data-testid="birdwatch-pivot"]').first();
  return await note.textContent().catch(() => null);
}

async function parseTweet(article: import("playwright-core").Locator): Promise<TweetData | null> {
  const id = await getTweetId(article);
  if (!id) return null;

  const handle = await getHandle(article);
  if (!handle) return null;

  const [created, text, images, videos, community, likes] = await Promise.all([
    getCreated(article),
    getText(article),
    getImages(article),
    getVideos(article),
    getCommunity(article),
    getLikes(article),
  ]);

  return { id, handle, text, images, videos, created, community, likes };
}

// ---------------------------------------------------------------------------
// Buffer management
// ---------------------------------------------------------------------------

async function refillBuffer(tab: QueryTab): Promise<void> {
  const articles = tab.page.locator("article");
  const count = await articles.count();
  for (let i = 0; i < count; i++) {
    const tweet = await parseTweet(articles.nth(i));
    if (!tweet || tab.seen.has(tweet.id)) continue;
    tab.seen.add(tweet.id);
    tab.buffer.push(tweet);
  }
}

async function scrollTab(tab: QueryTab): Promise<void> {
  await tab.page.evaluate(() => window.scrollBy(0, window.innerHeight * 2));
  await tab.page.waitForTimeout(2000);
}

/** Pick the tab whose first buffered tweet has the most recent timestamp. */
function pickNewest(tabs: QueryTab[]): { tab: QueryTab; tweet: TweetData } | null {
  let best: { tab: QueryTab; tweet: TweetData } | null = null;
  for (const tab of tabs) {
    if (!tab.open || tab.buffer.length === 0) continue;
    const tweet = tab.buffer[0]!;
    if (!best || tweet.created > best.tweet.created) {
      best = { tab, tweet };
    }
  }
  return best;
}

// ---------------------------------------------------------------------------
// Main scrape function
// ---------------------------------------------------------------------------

/**
 * Open a tab per query, scrape tweets by always picking the most recent
 * across all tabs. Stops at first stage's count or when all tabs are closed.
 *
 * NOTE: If all queries close before reaching the target count, we exit early.
 * This case needs handling once we decide what to do (retry, warn, etc.).
 */
export async function scrape(
  psyop: PsyOp,
  name: string,
  psyopCommitSha: string,
  db: Db,
): Promise<number> {
  const targetCount = psyop.stages[0]!.count ?? 100;
  const now = new Date();

  const context = await chromium.launchPersistentContext(USER_DATA_DIR, {
    headless: false,
    channel: "chrome",
    args: [
      "--remote-debugging-port=9222",
      "--disable-blink-features=AutomationControlled",
    ],
  });

  // Open a tab for each query
  const tabs: QueryTab[] = [];
  for (const query of psyop.queries) {
    const page = await context.newPage();
    const url = `https://x.com/search?q=${encodeURIComponent(query)}&src=typed_query&f=live`;
    console.log(`Opening tab for: ${query}`);
    await page.goto(url, { waitUntil: "domcontentloaded" });
    tabs.push({ query, page, buffer: [], seen: new Set(), open: true, staleScrolls: 0 });
  }

  // Close the default blank tab if one was created
  const defaultPage = context.pages()[0];
  if (defaultPage && !tabs.some((t) => t.page === defaultPage)) {
    await defaultPage.close();
  }

  let collected = 0;

  while (collected < targetCount) {
    // Refill empty buffers for open tabs
    for (const tab of tabs) {
      if (!tab.open) continue;
      if (tab.buffer.length === 0) {
        await refillBuffer(tab);
        if (tab.buffer.length === 0) {
          await scrollTab(tab);
          await refillBuffer(tab);
          if (tab.buffer.length === 0) {
            tab.staleScrolls++;
            if (tab.staleScrolls >= 5) {
              console.log(`Closing query "${tab.query}" — no more tweets.`);
              tab.open = false;
            }
          } else {
            tab.staleScrolls = 0;
          }
        } else {
          tab.staleScrolls = 0;
        }
      }
    }

    const pick = pickNewest(tabs);
    if (!pick) break; // NOTE: all queries closed before reaching target count

    const { tab, tweet } = pick;
    tab.buffer.shift();

    // Validate against psyop rules
    const validation = validForPsyop(psyop, tweet, now);
    if (!validation.valid) {
      if (validation.reason === "max_age") {
        console.log(`Closing query "${tab.query}" — post too old.`);
        tab.open = false;
      }
      continue;
    }

    // Try to insert into queue
    const inserted = db.insertPost({
      ...tweet,
      scrape_id: name,
      query: tab.query,
      psyop: name,
      psyop_commit_sha: psyopCommitSha,
    });

    if (!inserted) {
      // Already completed for this query — check if we should close the tab
      if (db.hasExistingPost(tweet.id, tab.query, name, psyopCommitSha)) {
        console.log(`Closing query "${tab.query}" — reached previously completed posts.`);
        tab.open = false;
      }
      continue;
    }

    collected++;
    console.log(`[${collected}/${targetCount}] @${tweet.handle}: ${tweet.text.slice(0, 80)}`);
  }

  await context.close();
  console.log(`Scraped ${collected} tweets.`);
  return collected;
}
