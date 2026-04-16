import Database from "better-sqlite3";
import path from "node:path";
import os from "node:os";
import type { AgentCompletionsMessageRichContent } from "objectiveai";

const DB_PATH = path.join(os.homedir(), ".psychological-operations", "data.db");

const SCHEMA = `
  CREATE TABLE IF NOT EXISTS posts_queue (
    id TEXT PRIMARY KEY,
    scrape_id TEXT NOT NULL,
    handle TEXT NOT NULL,
    content TEXT NOT NULL,
    created TEXT NOT NULL,
    community TEXT,
    psyop TEXT NOT NULL,
    psyop_commit_sha TEXT NOT NULL,
    scraped_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
  CREATE TABLE IF NOT EXISTS posts_completed (
    id TEXT PRIMARY KEY,
    scrape_id TEXT NOT NULL,
    content TEXT NOT NULL,
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
  handle: string;
  content: AgentCompletionsMessageRichContent;
  created: string;
  community: string | null;
  psyop: string;
  psyop_commit_sha: string;
  scraped_at: string;
}

export interface CompletedPost {
  id: string;
  scrape_id: string;
  content: AgentCompletionsMessageRichContent;
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

  insertPost(post: Omit<QueuedPost, "scraped_at">): void {
    this.db.prepare(`
      INSERT OR IGNORE INTO posts_queue (id, scrape_id, handle, content, created, community, psyop, psyop_commit_sha)
      VALUES (@id, @scrape_id, @handle, @content, @created, @community, @psyop, @psyop_commit_sha)
    `).run({
      ...post,
      content: JSON.stringify(post.content),
    });
  }

  getPosts(scrapeId: string): QueuedPost[] {
    const rows = this.db.prepare("SELECT * FROM posts_queue WHERE scrape_id = ? ORDER BY scraped_at DESC").all(scrapeId);
    return (rows as Array<Record<string, unknown>>).map((row) => ({
      ...row,
      content: JSON.parse(row["content"] as string) as AgentCompletionsMessageRichContent,
    })) as QueuedPost[];
  }

  finishPosts(ids: string[], scores: number[], scrapeId: string): void {
    const move = this.db.transaction(() => {
      for (let i = 0; i < ids.length; i++) {
        const row = this.db.prepare("SELECT * FROM posts_queue WHERE id = ?").get(ids[i]!) as Record<string, unknown> | undefined;
        if (!row) continue;

        this.db.prepare(`
          INSERT OR IGNORE INTO posts_completed (id, scrape_id, content, created, community, score, psyop, psyop_commit_sha, scraped_at)
          VALUES (@id, @scrape_id, @content, @created, @community, @score, @psyop, @psyop_commit_sha, @scraped_at)
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
      content: JSON.parse(row["content"] as string) as AgentCompletionsMessageRichContent,
    })) as CompletedPost[];
  }

  close(): void {
    this.db.close();
  }
}
