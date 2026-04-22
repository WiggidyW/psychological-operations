use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::config;

const SCHEMA: &str = "
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
";

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUrl {
    pub url: String,
}

#[derive(Debug)]
pub struct QueuedPost {
    pub id: String,
    pub scrape_id: String,
    pub query: String,
    pub handle: String,
    pub text: String,
    pub images: Vec<MediaUrl>,
    pub videos: Vec<MediaUrl>,
    pub created: String,
    pub community: Option<String>,
    pub psyop: String,
    pub psyop_commit_sha: String,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open() -> Result<Self, crate::error::Error> {
        let path = config::db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn insert_post(&self, post: &QueuedPost) -> Result<bool, crate::error::Error> {
        // Check completed
        let in_completed: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM posts_completed WHERE id = ?1 AND psyop = ?2 AND psyop_commit_sha = ?3)",
            params![post.id, post.psyop, post.psyop_commit_sha],
            |row| row.get(0),
        )?;
        if in_completed { return Ok(false); }

        // Check queue
        let in_queue: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM posts_queue WHERE id = ?1 AND psyop = ?2 AND psyop_commit_sha = ?3)",
            params![post.id, post.psyop, post.psyop_commit_sha],
            |row| row.get(0),
        )?;
        if in_queue { return Ok(false); }

        let images_json = serde_json::to_string(&post.images)?;
        let videos_json = serde_json::to_string(&post.videos)?;

        self.conn.execute(
            "INSERT OR IGNORE INTO posts_queue (id, scrape_id, query, handle, text, images, videos, created, community, psyop, psyop_commit_sha)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![post.id, post.scrape_id, post.query, post.handle, post.text,
                    images_json, videos_json, post.created, post.community,
                    post.psyop, post.psyop_commit_sha],
        )?;
        Ok(true)
    }

    pub fn has_existing_post(&self, id: &str, query: &str, psyop: &str, psyop_commit_sha: &str) -> Result<bool, crate::error::Error> {
        let in_completed: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM posts_completed WHERE id = ?1 AND query = ?2 AND psyop = ?3 AND psyop_commit_sha = ?4)",
            params![id, query, psyop, psyop_commit_sha],
            |row| row.get(0),
        )?;
        if in_completed { return Ok(true); }

        let in_queue: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM posts_queue WHERE id = ?1 AND query = ?2 AND psyop = ?3 AND psyop_commit_sha = ?4)",
            params![id, query, psyop, psyop_commit_sha],
            |row| row.get(0),
        )?;
        Ok(in_queue)
    }

    pub fn get_posts(&self, scrape_id: &str) -> Result<Vec<QueuedPost>, crate::error::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scrape_id, query, handle, text, images, videos, created, community, psyop, psyop_commit_sha FROM posts_queue WHERE scrape_id = ?1 ORDER BY scraped_at DESC"
        )?;
        let rows = stmt.query_map(params![scrape_id], |row| {
            let images_str: String = row.get(5)?;
            let videos_str: String = row.get(6)?;
            Ok(QueuedPost {
                id: row.get(0)?,
                scrape_id: row.get(1)?,
                query: row.get(2)?,
                handle: row.get(3)?,
                text: row.get(4)?,
                images: serde_json::from_str(&images_str).unwrap_or_default(),
                videos: serde_json::from_str(&videos_str).unwrap_or_default(),
                created: row.get(7)?,
                community: row.get(8)?,
                psyop: row.get(9)?,
                psyop_commit_sha: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Count queued posts for this psyop.
    pub fn count_queued(&self, psyop: &str) -> Result<usize, crate::error::Error> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM posts_queue WHERE psyop = ?1",
            params![psyop],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    /// Take the `limit` oldest queued posts for this psyop, oldest `scraped_at` first.
    pub fn get_oldest_queued(&self, psyop: &str, limit: usize) -> Result<Vec<QueuedPost>, crate::error::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scrape_id, query, handle, text, images, videos, created, community, psyop, psyop_commit_sha FROM posts_queue WHERE psyop = ?1 ORDER BY scraped_at ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![psyop, limit as i64], |row| {
            let images_str: String = row.get(5)?;
            let videos_str: String = row.get(6)?;
            Ok(QueuedPost {
                id: row.get(0)?,
                scrape_id: row.get(1)?,
                query: row.get(2)?,
                handle: row.get(3)?,
                text: row.get(4)?,
                images: serde_json::from_str(&images_str).unwrap_or_default(),
                videos: serde_json::from_str(&videos_str).unwrap_or_default(),
                created: row.get(7)?,
                community: row.get(8)?,
                psyop: row.get(9)?,
                psyop_commit_sha: row.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn finish_posts(&self, ids: &[String], scores: &[f64]) -> Result<(), crate::error::Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (id, score) in ids.iter().zip(scores.iter()) {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM posts_queue WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )?;
            if !exists { continue; }

            tx.execute(
                "INSERT OR IGNORE INTO posts_completed (id, scrape_id, query, text, images, videos, created, community, score, psyop, psyop_commit_sha, scraped_at)
                 SELECT id, scrape_id, query, text, images, videos, created, community, ?2, psyop, psyop_commit_sha, scraped_at FROM posts_queue WHERE id = ?1",
                params![id, score],
            )?;
            tx.execute("DELETE FROM posts_queue WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }
}
