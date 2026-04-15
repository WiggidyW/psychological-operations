import Database from "better-sqlite3";
import path from "node:path";
import os from "node:os";
import type { AgentCompletionsMessageRichContent } from "objectiveai";

const DB_PATH = path.join(os.homedir(), ".psychological-operations", "data.db");

const SCHEMA = `
  CREATE TABLE IF NOT EXISTS posts (
    id TEXT PRIMARY KEY,
    scrape_id TEXT NOT NULL,
    handle TEXT NOT NULL,
    content TEXT NOT NULL,
    created TEXT NOT NULL,
    community TEXT,
    scraped_at TEXT NOT NULL DEFAULT (datetime('now'))
  )
`;

export interface Post {
  id: string;
  scrape_id: string;
  handle: string;
  content: AgentCompletionsMessageRichContent;
  created: string;
  community: string | null;
  scraped_at: string;
}

export class Db {
  private db: Database.Database;

  constructor() {
    this.db = new Database(DB_PATH);
    this.db.pragma("journal_mode = WAL");
    this.db.exec(SCHEMA);
  }

  insertPost(post: Omit<Post, "scraped_at">): void {
    this.db.prepare(`
      INSERT OR IGNORE INTO posts (id, scrape_id, handle, content, created, community)
      VALUES (@id, @scrape_id, @handle, @content, @created, @community)
    `).run({
      ...post,
      content: JSON.stringify(post.content),
    });
  }

  getPosts(scrapeId: string): Post[] {
    const rows = this.db.prepare("SELECT * FROM posts WHERE scrape_id = ? ORDER BY scraped_at DESC").all(scrapeId);
    return (rows as Array<Record<string, unknown>>).map((row) => ({
      ...row,
      content: JSON.parse(row["content"] as string) as AgentCompletionsMessageRichContent,
    })) as Post[];
  }

  close(): void {
    this.db.close();
  }
}
