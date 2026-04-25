import { chromium, type BrowserContext, type Page } from "playwright-core";
import path from "node:path";
import os from "node:os";
import { startMcpServer, stopMcpServer } from "./mcp.js";
import { parseTweet, type TweetData } from "./tweet.js";
import { findPort } from "./port.js";

function userDataDir(): string {
  // POPS_CHROME_DATA_DIR is set by `Playwright::spawn_with_profile` so each
  // concurrent scrape can run against an isolated snapshot of the shared
  // base profile (concurrent launchPersistentContext on the same dir would
  // fight over Chrome's profile lock).
  const override_ = process.env["POPS_CHROME_DATA_DIR"];
  if (override_ && override_.length > 0) return override_;
  return path.join(os.homedir(), ".psychological-operations", "chrome-data");
}

let context: BrowserContext | null = null;
let cdpPort: number | null = null;

interface QueryTab {
  query: string;
  page: Page;
  buffer: TweetData[];
  seen: Set<string>;
  open: boolean;
  staleScrolls: number;
}

const tabs: QueryTab[] = [];
// Pages that came back "unexpected" from open_tabs. Kept around so the user
// can resolve the issue (login, captcha, etc.) in the visible Chrome window;
// `retry_unexpected` then re-runs validatePage on each.
const unexpectedPages: Map<string, Page> = new Map();

// Concurrent scrape runs spawn many Chrome instances at once. Two failure
// modes emerge under load on Windows:
//   1. CDP port collision — `findPort` binds→closes, leaving a TOCTOU
//      window where another concurrent launch can grab the same port.
//      Chrome then can't bind `--remote-debugging-port` and exits.
//   2. AV / cold-cache slowness — first-launch chrome scanned by Defender
//      can blow past Playwright's default 30s launch timeout.
// Fix: retry with a fresh port and an extended timeout.
const LAUNCH_TIMEOUT_MS = 120_000;
const LAUNCH_MAX_ATTEMPTS = 8;

async function launchOnce(): Promise<BrowserContext> {
  cdpPort = await findPort();
  return chromium.launchPersistentContext(userDataDir(), {
    headless: false,
    channel: "chrome",
    timeout: LAUNCH_TIMEOUT_MS,
    args: [
      `--remote-debugging-port=${cdpPort}`,
      "--disable-blink-features=AutomationControlled",
    ],
  });
}

async function ensureContext(): Promise<BrowserContext> {
  if (context !== null) return context;
  let lastErr: unknown = null;
  for (let attempt = 1; attempt <= LAUNCH_MAX_ATTEMPTS; attempt++) {
    try {
      context = await launchOnce();
      // pkg's esbuild wraps our functions with __name(fn, "name") helpers.
      // When Playwright serializes a function via .toString() to run it in
      // the page, that wrapping comes along, and __name isn't defined on
      // the page. Shim it as an identity function so the wrapped code
      // executes normally.
      await context.addInitScript("globalThis.__name = globalThis.__name || function (fn) { return fn; };");
      return context;
    } catch (err) {
      lastErr = err;
      // Exponential backoff with jitter spreads retries so concurrent
      // siblings don't all retry on the same beat.
      const base = Math.min(2000 * attempt, 15_000);
      const jitter = Math.floor(Math.random() * 2000);
      await new Promise((r) => setTimeout(r, base + jitter));
    }
  }
  throw lastErr instanceof Error
    ? lastErr
    : new Error(`launch failed after ${LAUNCH_MAX_ATTEMPTS} attempts: ${String(lastErr)}`);
}

async function validatePage(page: Page): Promise<"results" | "empty" | "unexpected"> {
  // Wait up to 15s for an article to appear. X has persistent connections, so
  // networkidle never fires — we can't rely on it as a fallback.
  try {
    await page.locator("article").first().waitFor({ timeout: 15_000 });
    return "results";
  } catch {
    const noResults = await page.getByText(/No results for/).first().isVisible().catch(() => false);
    if (noResults) return "empty";
    return "unexpected";
  }
}

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

// ── Command handlers ────────────────────────────────────────────────────────

async function openTabs(urls: string[]): Promise<Record<string, string>> {
  const ctx = await ensureContext();
  const results: Record<string, string> = {};

  for (const url of urls) {
    const page = await ctx.newPage();
    await page.goto(url, { waitUntil: "domcontentloaded" });
    const state = await validatePage(page);
    results[url] = state;

    if (state === "empty") {
      await page.close();
      continue;
    }
    if (state === "unexpected") {
      // Keep the page open so the user can resolve the issue manually; the
      // Rust side will call retry_unexpected after agent intervention.
      unexpectedPages.set(url, page);
      continue;
    }

    // Use the URL as the tab's stable identifier; the Rust caller maps it
    // back to the originating filter for validation.
    tabs.push({ query: url, page, buffer: [], seen: new Set(), open: true, staleScrolls: 0 });
  }

  // Close default blank tab
  const defaultPage = ctx.pages()[0];
  if (defaultPage && !tabs.some((t) => t.page === defaultPage)) {
    await defaultPage.close();
  }

  return results;
}

function pickNewest(): { tab: QueryTab; tweet: TweetData } | null {
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

async function nextTweet(): Promise<{ tweet: TweetData; query: string } | null> {
  // Refill empty buffers
  for (const tab of tabs) {
    if (!tab.open || tab.buffer.length > 0) continue;
    await refillBuffer(tab);
    if (tab.buffer.length === 0) {
      await scrollTab(tab);
      await refillBuffer(tab);
      if (tab.buffer.length === 0) {
        tab.staleScrolls++;
        if (tab.staleScrolls >= 5) {
          tab.open = false;
        }
      } else {
        tab.staleScrolls = 0;
      }
    } else {
      tab.staleScrolls = 0;
    }
  }

  const pick = pickNewest();
  if (!pick) return null;

  pick.tab.buffer.shift();
  return { tweet: pick.tweet, query: pick.tab.query };
}

function closeQuery(query: string): void {
  const tab = tabs.find((t) => t.query === query);
  if (tab) tab.open = false;
}

async function retryUnexpected(urls: string[]): Promise<Record<string, string>> {
  const results: Record<string, string> = {};
  for (const url of urls) {
    const page = unexpectedPages.get(url);
    if (!page) {
      results[url] = "unexpected";
      continue;
    }
    // Re-navigate to the original URL in case the user wandered off (e.g.,
    // logged in via a redirect chain that landed on home). Then revalidate.
    try {
      await page.goto(url, { waitUntil: "domcontentloaded" });
    } catch {
      // Navigation failure → still treat as unexpected so the runner can
      // retry intervention.
      results[url] = "unexpected";
      continue;
    }
    const state = await validatePage(page);
    results[url] = state;
    if (state === "results") {
      unexpectedPages.delete(url);
      tabs.push({ query: url, page, buffer: [], seen: new Set(), open: true, staleScrolls: 0 });
    } else if (state === "empty") {
      unexpectedPages.delete(url);
      await page.close();
    }
  }
  return results;
}

function hasOpenTabs(): boolean {
  return tabs.some((t) => t.open);
}

async function close(): Promise<void> {
  if (context !== null) {
    await context.close();
    context = null;
    cdpPort = null;
  }
  tabs.length = 0;
  unexpectedPages.clear();
}

// ── Protocol dispatch ───────────────────────────────────────────────────────

export async function handleCommand(cmd: Record<string, unknown>): Promise<unknown> {
  switch (cmd["cmd"]) {
    case "open_tabs":
      return { states: await openTabs(cmd["urls"] as string[]) };

    case "next_tweet": {
      const result = await nextTweet();
      if (result === null) return { done: true };
      return { done: false, tweet: result.tweet, query: result.query };
    }

    case "close_query":
      closeQuery(cmd["query"] as string);
      return { ok: true };

    case "retry_unexpected":
      return { states: await retryUnexpected(cmd["urls"] as string[]) };

    case "has_open_tabs":
      return { open: hasOpenTabs() };

    case "start_mcp": {
      if (cdpPort === null) {
        return { error: "browser not started — call open_tabs first" };
      }
      const port = await startMcpServer(cdpPort);
      return { mcp_port: port };
    }

    case "stop_mcp":
      stopMcpServer();
      return { ok: true };

    case "get_page_url": {
      const query = cmd["query"] as string;
      const tab = tabs.find((t) => t.query === query);
      return { url: tab?.page.url() ?? null };
    }

    case "install_browser": {
      try {
        // @ts-expect-error playwright-core internal API
        const { installBrowsersForNpmInstall } = await import("playwright-core/lib/server");
        await (installBrowsersForNpmInstall as (browsers: string[]) => Promise<void>)(["chromium"]);
        return { ok: true };
      } catch {
        // Fallback: try CLI approach
        const { execFileSync } = await import("node:child_process");
        try {
          execFileSync(process.execPath, ["-e", "require('playwright-core/cli').program.parse(['node', 'playwright', 'install', 'chromium'])"], { stdio: "inherit" });
          return { ok: true };
        } catch (err) {
          return { error: `browser install failed: ${err}` };
        }
      }
    }

    case "close":
      await close();
      return { ok: true };

    default:
      return { error: `unknown command: ${cmd["cmd"]}` };
  }
}
