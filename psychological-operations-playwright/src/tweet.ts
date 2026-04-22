import type { Locator } from "playwright-core";

export interface TweetData {
  id: string;
  handle: string;
  text: string;
  images: Array<{ url: string }>;
  videos: Array<{ url: string }>;
  created: string;
  community: string | null;
  likes: number;
  retweets: number;
  replies: number;
}

interface RawTweet {
  id: string | null;
  handle: string | null;
  text: string;
  created: string | null;
  images: string[];
  videos: string[];
  community: string | null;
  likes: number;
  retweets: number;
  replies: number;
  hasShowMore: boolean;
}

async function extractRaw(article: Locator): Promise<RawTweet | null> {
  return await article.evaluate((el): RawTweet => {
    const q = (sel: string) => el.querySelector(sel);
    const qa = (sel: string) => Array.from(el.querySelectorAll(sel));

    const statusLink = q('a[href*="/status/"]');
    const href = statusLink?.getAttribute("href") ?? "";
    const idMatch = /\/status\/(\d+)/.exec(href);
    const id = idMatch?.[1] ?? null;

    let handle: string | null = null;
    const handleSpans = el.querySelectorAll('a[role="link"] span');
    for (const span of Array.from(handleSpans)) {
      const txt = span.textContent ?? "";
      if (txt.startsWith("@")) { handle = txt.slice(1); break; }
    }

    const created = q("time")?.getAttribute("datetime") ?? null;

    const text = q('[data-testid="tweetText"]')?.textContent ?? "";

    const images = qa('[data-testid="tweetPhoto"] img')
      .map((i) => i.getAttribute("src") ?? "")
      .filter((s) => s !== "");

    const videos = qa("video")
      .map((v) => v.getAttribute("src") ?? "")
      .filter((s) => s !== "");

    const community = q('[data-testid="birdwatch-pivot"]')?.textContent ?? null;

    const countFromAria = (selectors: string[]): number => {
      for (const sel of selectors) {
        const btn = q(sel);
        if (!btn) continue;
        const label = btn.getAttribute("aria-label") ?? "";
        const m = /(\d+)/.exec(label);
        if (m) return parseInt(m[1]!, 10);
      }
      return 0;
    };

    const likes = countFromAria(['[data-testid="like"]', '[data-testid="unlike"]']);
    const retweets = countFromAria(['[data-testid="retweet"]', '[data-testid="unretweet"]']);
    const replies = countFromAria(['[data-testid="reply"]']);

    const buttons = qa('[role="button"]');
    const hasShowMore = buttons.some((b) => /show more/i.test(b.textContent ?? ""));

    return { id, handle, text, created, images, videos, community, likes, retweets, replies, hasShowMore };
  }, undefined, { timeout: 3000 });
}

async function expandShowMore(article: Locator): Promise<void> {
  const showMore = article.getByRole("button", { name: /Show more/i }).first();
  const tweetText = article.locator('[data-testid="tweetText"]').first();
  const before = await tweetText.textContent({ timeout: 1000 }).catch(() => "") ?? "";
  await showMore.click({ timeout: 3000 }).catch(() => {});
  await tweetText.evaluate(
    (el, prev) => new Promise<void>((resolve) => {
      if (el.textContent !== prev) { resolve(); return; }
      const timer = setTimeout(() => { obs.disconnect(); resolve(); }, 2000);
      const obs = new MutationObserver(() => {
        if (el.textContent !== prev) {
          clearTimeout(timer);
          obs.disconnect();
          resolve();
        }
      });
      obs.observe(el, { childList: true, subtree: true, characterData: true });
    }),
    before,
    { timeout: 3000 },
  ).catch(() => {});
}

export async function parseTweet(article: Locator): Promise<TweetData | null> {
  const raw = await extractRaw(article).catch(() => null);
  if (!raw || !raw.id || !raw.handle) return null;

  let text = raw.text;
  if (raw.hasShowMore) {
    await expandShowMore(article);
    const updated = await article.evaluate(
      (el) => el.querySelector('[data-testid="tweetText"]')?.textContent ?? "",
      undefined,
      { timeout: 3000 },
    ).catch(() => text);
    text = updated;
  }

  return {
    id: raw.id,
    handle: raw.handle,
    text,
    images: raw.images.map((url) => ({ url })),
    videos: raw.videos.map((url) => ({ url })),
    created: raw.created ?? new Date().toISOString(),
    community: raw.community,
    likes: raw.likes,
    retweets: raw.retweets,
    replies: raw.replies,
  };
}
