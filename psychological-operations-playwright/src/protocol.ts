import { chromium, type BrowserContext, type Page } from "playwright-core";
import path from "node:path";
import os from "node:os";
import { startMcpServer, stopMcpServer } from "./mcp.js";
import { parseTweet, type TweetData } from "./tweet.js";
import { findPort } from "./port.js";

function userDataDir(): string {
  return path.join(os.homedir(), ".psychological-operations", "chrome-data");
}

let context: BrowserContext | null = null;
let cdpPort: number | null = null;
// Single shared page for the entire scrapes-run process. Opened once at
// `start_session`, reused for every typed search.
let page: Page | null = null;

interface QueryState {
  query: string;
  buffer: TweetData[];
  seen: Set<string>;
  open: boolean;
  staleScrolls: number;
}

// At most one active query at a time (sequential model).
let current: QueryState | null = null;

// AV / cold-cache slowness: first-launch chrome scanned by Defender can blow
// past Playwright's default 30s launch timeout. We launch exactly once per
// `scrapes run` process now, but keep the retry / extended timeout for
// robustness.
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
      const base = Math.min(2000 * attempt, 15_000);
      const jitter = Math.floor(Math.random() * 2000);
      await new Promise((r) => setTimeout(r, base + jitter));
    }
  }
  throw lastErr instanceof Error
    ? lastErr
    : new Error(`launch failed after ${LAUNCH_MAX_ATTEMPTS} attempts: ${String(lastErr)}`);
}

async function validatePage(p: Page): Promise<"results" | "empty" | "unexpected"> {
  // Wait up to 15s for an article to appear. X has persistent connections, so
  // networkidle never fires — we can't rely on it as a fallback.
  try {
    await p.locator("article").first().waitFor({ timeout: 15_000 });
    return "results";
  } catch {
    const noResults = await p.getByText(/No results for/).first().isVisible().catch(() => false);
    if (noResults) return "empty";
    return "unexpected";
  }
}

async function refillBuffer(state: QueryState): Promise<void> {
  if (page === null) return;
  const articles = page.locator("article");
  const count = await articles.count();
  for (let i = 0; i < count; i++) {
    const tweet = await parseTweet(articles.nth(i));
    if (!tweet || state.seen.has(tweet.id)) continue;
    state.seen.add(tweet.id);
    state.buffer.push(tweet);
  }
}

async function scrollPage(): Promise<void> {
  if (page === null) return;
  await page.evaluate(() => window.scrollBy(0, window.innerHeight * 2));
  await page.waitForTimeout(2000);
}

// ── Typed search helpers ────────────────────────────────────────────────────

const SEARCH_INPUT_SELECTORS = [
  '[data-testid="SearchBox_Search_Input"]',
  'input[role="combobox"][placeholder*="Search"]',
];

async function focusSearchInput(): Promise<void> {
  if (page === null) throw new Error("session not started");
  for (const sel of SEARCH_INPUT_SELECTORS) {
    const loc = page.locator(sel).first();
    if (await loc.isVisible().catch(() => false)) {
      await loc.click({ clickCount: 3 }); // triple-click selects existing text
      await page.keyboard.press("Backspace");
      await loc.focus();
      return;
    }
  }
  throw new Error(`search input not found (tried: ${SEARCH_INPUT_SELECTORS.join(", ")})`);
}

async function typeWithJitter(text: string): Promise<void> {
  if (page === null) throw new Error("session not started");
  for (const ch of text) {
    const delay = 50 + Math.floor(Math.random() * 100); // 50–150ms per char
    await page.keyboard.type(ch, { delay });
  }
}

async function clickLatestTab(): Promise<void> {
  if (page === null) return;
  // Best-effort: try a few selectors; if none match (already on Latest, or
  // X UI churn), validatePage will report results regardless.
  const candidates = [
    'a[role="tab"]:has-text("Latest")',
    '[href*="f=live"][role="tab"]',
  ];
  for (const sel of candidates) {
    const loc = page.locator(sel).first();
    if (await loc.isVisible().catch(() => false)) {
      await loc.click().catch(() => undefined);
      // Allow the timeline to swap.
      await page.waitForTimeout(1500);
      return;
    }
  }
}

// ── Command handlers ────────────────────────────────────────────────────────

async function startSession(): Promise<void> {
  const ctx = await ensureContext();
  if (page !== null) return;
  page = await ctx.newPage();
  await page.goto("https://x.com/home", { waitUntil: "domcontentloaded" });
  // Close any leftover blank tab.
  const defaultPage = ctx.pages().find((p) => p !== page);
  if (defaultPage) await defaultPage.close().catch(() => undefined);
  // Wait for the search input to be present so subsequent run_query calls
  // don't race the UI.
  await page.locator(SEARCH_INPUT_SELECTORS[0]!).first().waitFor({ timeout: 30_000 }).catch(() => undefined);
}

async function runQuery(query: string): Promise<"results" | "empty" | "unexpected"> {
  if (page === null) throw new Error("session not started — call start_session first");
  // Always reset visible scroll before re-typing so search bar is in view.
  await page.evaluate(() => window.scrollTo(0, 0)).catch(() => undefined);
  try {
    await focusSearchInput();
  } catch {
    return "unexpected";
  }
  await typeWithJitter(query);
  await page.keyboard.press("Enter");
  // Wait for navigation/results to settle. X uses client-side routing so
  // load events may not fire; just give the URL/timeline time to swap.
  await page.waitForTimeout(2000);
  await clickLatestTab();
  const state = await validatePage(page);
  if (state === "results") {
    current = { query, buffer: [], seen: new Set(), open: true, staleScrolls: 0 };
  } else {
    current = null;
  }
  return state;
}

async function nextTweet(): Promise<{ tweet: TweetData; query: string } | null> {
  if (current === null || !current.open) return null;
  if (current.buffer.length === 0) {
    await refillBuffer(current);
    if (current.buffer.length === 0) {
      await scrollPage();
      await refillBuffer(current);
      if (current.buffer.length === 0) {
        current.staleScrolls++;
        if (current.staleScrolls >= 5) {
          current.open = false;
          return null;
        }
      } else {
        current.staleScrolls = 0;
      }
    } else {
      current.staleScrolls = 0;
    }
  }
  const tweet = current.buffer.shift();
  if (tweet === undefined) return null;
  return { tweet, query: current.query };
}

function closeCurrent(): void {
  current = null;
}

async function close(): Promise<void> {
  if (context !== null) {
    await context.close();
    context = null;
    cdpPort = null;
  }
  page = null;
  current = null;
}

// ── Protocol dispatch ───────────────────────────────────────────────────────

export async function handleCommand(cmd: Record<string, unknown>): Promise<unknown> {
  switch (cmd["cmd"]) {
    case "start_session":
      await startSession();
      return { ok: true };

    case "run_query": {
      const state = await runQuery(cmd["query"] as string);
      return { state };
    }

    case "next_tweet": {
      const result = await nextTweet();
      if (result === null) return { done: true };
      return { done: false, tweet: result.tweet, query: result.query };
    }

    case "close_query":
      closeCurrent();
      return { ok: true };

    case "start_mcp": {
      if (cdpPort === null) {
        return { error: "browser not started — call start_session first" };
      }
      const port = await startMcpServer(cdpPort);
      return { mcp_port: port };
    }

    case "stop_mcp":
      stopMcpServer();
      return { ok: true };

    case "get_page_url":
      return { url: page?.url() ?? null };

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
