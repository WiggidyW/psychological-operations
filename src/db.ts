import Database from "better-sqlite3";
import path from "node:path";
import os from "node:os";
import type {
  AgentCompletionsMessageImageUrl,
  AgentCompletionsMessageVideoUrl,
} from "objectiveai";

const DB_PATH = path.join(os.homedir(), ".psychological-operations", "data.db");

const SCHEMA = `
  CREATE TABLE IF NOT EXISTS posts_queue (
    id TEXT PRIMARY KEY,
    scrape_id TEXT NOT NULL,
    query TEXT NOT NULL,
    handle TEXT NOT NULL,
    text TEXT NOT NULL,
    images TEXT NOT NULL DEFAULT '[]',
    videos TEXT NOT NULL DEFAULT '[]',
    created TEXT NOT NULL,
    community TEXT,
    psyop TEXT NOT NULL,
    psyop_commit_sha TEXT NOT NULL,
    scraped_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
  CREATE TABLE IF NOT EXISTS posts_completed (
    id TEXT PRIMARY KEY,
    scrape_id TEXT NOT NULL,
    query TEXT NOT NULL,
    text TEXT NOT NULL,
    images TEXT NOT NULL DEFAULT '[]',
    videos TEXT NOT NULL DEFAULT '[]',
    created TEXT NOT NULL,
    community TEXT,
    score REAL NOT NULL,
    psyop TEXT NOT NULL,
    psyop_commit_sha TEXT NOT NULL,
    scraped_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
`;

export interface QueuedPost {
  id: string;
  scrape_id: string;
  query: string;
  handle: string;
  text: string;
  images: AgentCompletionsMessageImageUrl[];
  videos: AgentCompletionsMessageVideoUrl[];
  created: string;
  community: string | null;
  psyop: string;
  psyop_commit_sha: string;
  scraped_at: string;
}

export interface CompletedPost {
  id: string;
  scrape_id: string;
  query: string;
  text: string;
  images: AgentCompletionsMessageImageUrl[];
  videos: AgentCompletionsMessageVideoUrl[];
  created: string;
  community: string | null;
  score: number;
  psyop: string;
  psyop_commit_sha: string;
  scraped_at: string;
}

export class Db {
  private db: Database.Database;

  constructor() {
    this.db = new Database(DB_PATH);
    this.db.pragma("journal_mode = WAL");
    this.db.exec(SCHEMA);
  }

  insertPost(post: Omit<QueuedPost, "scraped_at">): boolean {
    const inCompleted = this.db.prepare(
      "SELECT 1 FROM posts_completed WHERE id = ? AND psyop = ? AND psyop_commit_sha = ?",
    ).get(post.id, post.psyop, post.psyop_commit_sha);
    if (inCompleted) return false;

    const inQueue = this.db.prepare(
      "SELECT 1 FROM posts_queue WHERE id = ? AND psyop = ? AND psyop_commit_sha = ?",
    ).get(post.id, post.psyop, post.psyop_commit_sha);
    if (inQueue) return false;

    this.db.prepare(`
      INSERT OR IGNORE INTO posts_queue (id, scrape_id, query, handle, text, images, videos, created, community, psyop, psyop_commit_sha)
      VALUES (@id, @scrape_id, @query, @handle, @text, @images, @videos, @created, @community, @psyop, @psyop_commit_sha)
    `).run({
      ...post,
      images: JSON.stringify(post.images),
      videos: JSON.stringify(post.videos),
    });
    return true;
  }

  getPosts(scrapeId: string): QueuedPost[] {
    const rows = this.db.prepare("SELECT * FROM posts_queue WHERE scrape_id = ? ORDER BY scraped_at DESC").all(scrapeId);
    return (rows as Array<Record<string, unknown>>).map((row) => ({
      ...row,
      images: JSON.parse(row["images"] as string) as AgentCompletionsMessageImageUrl[],
      videos: JSON.parse(row["videos"] as string) as AgentCompletionsMessageVideoUrl[],
    })) as QueuedPost[];
  }

  finishPosts(ids: string[], scores: number[]): void {
    const move = this.db.transaction(() => {
      for (let i = 0; i < ids.length; i++) {
        const row = this.db.prepare("SELECT * FROM posts_queue WHERE id = ?").get(ids[i]!) as Record<string, unknown> | undefined;
        if (!row) continue;

        this.db.prepare(`
          INSERT OR IGNORE INTO posts_completed (id, scrape_id, query, text, images, videos, created, community, score, psyop, psyop_commit_sha, scraped_at)
          VALUES (@id, @scrape_id, @query, @text, @images, @videos, @created, @community, @score, @psyop, @psyop_commit_sha, @scraped_at)
        `).run({
          ...row,
          score: scores[i]!,
        });

        this.db.prepare("DELETE FROM posts_queue WHERE id = ?").run(ids[i]!);
      }
    });
    move();
  }

  getCompletedPosts(scrapeId: string): CompletedPost[] {
    const rows = this.db.prepare("SELECT * FROM posts_completed WHERE scrape_id = ? ORDER BY score DESC").all(scrapeId);
    return (rows as Array<Record<string, unknown>>).map((row) => ({
      ...row,
      images: JSON.parse(row["images"] as string) as AgentCompletionsMessageImageUrl[],
      videos: JSON.parse(row["videos"] as string) as AgentCompletionsMessageVideoUrl[],
    })) as CompletedPost[];
  }

  hasExistingPost(id: string, query: string, psyop: string, psyopCommitSha: string): boolean {
    const inCompleted = this.db.prepare(
      "SELECT 1 FROM posts_completed WHERE id = ? AND query = ? AND psyop = ? AND psyop_commit_sha = ?",
    ).get(id, query, psyop, psyopCommitSha);
    if (inCompleted) return true;

    const inQueue = this.db.prepare(
      "SELECT 1 FROM posts_queue WHERE id = ? AND query = ? AND psyop = ? AND psyop_commit_sha = ?",
    ).get(id, query, psyop, psyopCommitSha);
    return inQueue !== undefined;
  }

  close(): void {
    this.db.close();
  }
}
