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
}

async function getTweetId(article: Locator): Promise<string | null> {
  const link = article.locator('a[href*="/status/"]').first();
  const href = await link.getAttribute("href").catch(() => null);
  if (!href) return null;
  const match = /\/status\/(\d+)/.exec(href);
  return match?.[1] ?? null;
}

async function getHandle(article: Locator): Promise<string | null> {
  const link = article.locator('a[href^="/"][role="link"] span').filter({ hasText: /^@/ }).first();
  const text = await link.textContent().catch(() => null);
  return text?.replace(/^@/, "") ?? null;
}

async function getCreated(article: Locator): Promise<string> {
  const time = article.locator("time").first();
  return await time.getAttribute("datetime") ?? new Date().toISOString();
}

async function getText(article: Locator): Promise<string> {
  const tweetText = article.locator('[data-testid="tweetText"]').first();

  // Expand truncated tweets
  const showMore = article.getByRole("button", { name: /Show more/i }).first();
  if (await showMore.isVisible().catch(() => false)) {
    const before = await tweetText.textContent().catch(() => "") ?? "";
    await showMore.click();
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

async function getImages(article: Locator): Promise<Array<{ url: string }>> {
  const imgs = article.locator('[data-testid="tweetPhoto"] img');
  const count = await imgs.count();
  const results: Array<{ url: string }> = [];
  for (let i = 0; i < count; i++) {
    const src = await imgs.nth(i).getAttribute("src").catch(() => null);
    if (src) results.push({ url: src });
  }
  return results;
}

async function getVideos(article: Locator): Promise<Array<{ url: string }>> {
  const vids = article.locator("video");
  const count = await vids.count();
  const results: Array<{ url: string }> = [];
  for (let i = 0; i < count; i++) {
    const src = await vids.nth(i).getAttribute("src").catch(() => null);
    if (src) results.push({ url: src });
  }
  return results;
}

async function getLikes(article: Locator): Promise<number> {
  const btn = article.locator('[data-testid="like"], [data-testid="unlike"]').first();
  const label = await btn.getAttribute("aria-label").catch(() => null);
  if (!label) return 0;
  const match = /(\d+)/.exec(label);
  return match ? parseInt(match[1]!, 10) : 0;
}

async function getCommunity(article: Locator): Promise<string | null> {
  const note = article.locator('[data-testid="birdwatch-pivot"]').first();
  return await note.textContent().catch(() => null);
}

export async function parseTweet(article: Locator): Promise<TweetData | null> {
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
