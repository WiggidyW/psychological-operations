use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::config;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS posts (
        id TEXT NOT NULL,
        handle TEXT NOT NULL,
        created TEXT NOT NULL,
        likes INTEGER NOT NULL DEFAULT 0,
        retweets INTEGER NOT NULL DEFAULT 0,
        replies INTEGER NOT NULL DEFAULT 0,
        psyop TEXT NOT NULL,
        psyop_commit_sha TEXT NOT NULL,
        query TEXT NOT NULL,
        scraped_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (id, psyop, psyop_commit_sha)
    );
    CREATE TABLE IF NOT EXISTS post_contents (
        post_id TEXT PRIMARY KEY,
        text TEXT NOT NULL,
        images TEXT NOT NULL DEFAULT '[]',
        videos TEXT NOT NULL DEFAULT '[]'
    );
    CREATE TABLE IF NOT EXISTS scores (
        post_id TEXT NOT NULL,
        psyop TEXT NOT NULL,
        psyop_commit_sha TEXT NOT NULL,
        score REAL NOT NULL,
        scored_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (psyop, psyop_commit_sha, post_id)
    );
";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaUrl {
    pub url: String,
}

/// Canonical tweet (engagement metadata + content + scrape provenance).
/// Engagement+provenance fields land in `posts`; content fields land in
/// `post_contents`.
#[derive(Debug, Clone)]
pub struct Post {
    pub id: String,
    pub handle: String,
    pub text: String,
    pub images: Vec<MediaUrl>,
    pub videos: Vec<MediaUrl>,
    pub created: String,
    pub likes: u64,
    pub retweets: u64,
    pub replies: u64,
}

/// A `(post, query)` pair from `posts` for which no row exists in `scores`
/// for the matching `(psyop, psyop_commit_sha)`.
#[derive(Debug)]
pub struct UnscoredEntry {
    pub post: Post,
    pub query: String,
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

    /// Insert a new (post, psyop, commit) scrape row plus its content. Returns
    /// whether a new posts row was created (false if the (id, psyop, commit)
    /// triple already existed). Content is upserted regardless.
    pub fn insert_post(
        &self,
        post: &Post,
        psyop: &str,
        psyop_commit_sha: &str,
        query: &str,
    ) -> Result<bool, crate::error::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO posts (id, handle, created, likes, retweets, replies, psyop, psyop_commit_sha, query)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                post.id, post.handle, post.created,
                post.likes as i64, post.retweets as i64, post.replies as i64,
                psyop, psyop_commit_sha, query,
            ],
        )? > 0;
        let images_json = serde_json::to_string(&post.images)?;
        let videos_json = serde_json::to_string(&post.videos)?;
        tx.execute(
            "INSERT INTO post_contents (post_id, text, images, videos)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(post_id) DO UPDATE SET
                 text = excluded.text,
                 images = excluded.images,
                 videos = excluded.videos",
            params![post.id, post.text, images_json, videos_json],
        )?;
        tx.commit()?;
        Ok(inserted)
    }

    /// Returns the `query` recorded on the existing `posts` row for this
    /// `(id, psyop, commit)`, or `None` if no such row exists. Used to detect
    /// when a search query has cycled back to a tweet it already produced.
    pub fn existing_post_query(
        &self,
        post_id: &str,
        psyop: &str,
        psyop_commit_sha: &str,
    ) -> Result<Option<String>, crate::error::Error> {
        let result = self.conn.query_row(
            "SELECT query FROM posts WHERE id = ?1 AND psyop = ?2 AND psyop_commit_sha = ?3",
            params![post_id, psyop, psyop_commit_sha],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(q) => Ok(Some(q)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// How many posts scraped under this psyop have no matching scores row
    /// (for the same psyop + the post's own commit).
    pub fn count_unscored(&self, psyop: &str) -> Result<usize, crate::error::Error> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM posts p
             WHERE p.psyop = ?1
               AND NOT EXISTS (
                 SELECT 1 FROM scores s
                 WHERE s.post_id = p.id
                   AND s.psyop = p.psyop
                   AND s.psyop_commit_sha = p.psyop_commit_sha
               )",
            params![psyop],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    /// Take the `limit` oldest unscored entries for this psyop, joined with
    /// their content. Ordered by `posts.scraped_at ASC`.
    pub fn get_oldest_unscored(
        &self,
        psyop: &str,
        limit: usize,
    ) -> Result<Vec<UnscoredEntry>, crate::error::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.handle, p.created, p.likes, p.retweets, p.replies,
                    c.text, c.images, c.videos,
                    p.query
             FROM posts p
             JOIN post_contents c ON c.post_id = p.id
             WHERE p.psyop = ?1
               AND NOT EXISTS (
                 SELECT 1 FROM scores s
                 WHERE s.post_id = p.id
                   AND s.psyop = p.psyop
                   AND s.psyop_commit_sha = p.psyop_commit_sha
               )
             ORDER BY p.scraped_at ASC
             LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![psyop, limit as i64], |row| {
            let images_str: String = row.get(7)?;
            let videos_str: String = row.get(8)?;
            Ok(UnscoredEntry {
                post: Post {
                    id: row.get(0)?,
                    handle: row.get(1)?,
                    created: row.get(2)?,
                    likes: row.get::<_, i64>(3)? as u64,
                    retweets: row.get::<_, i64>(4)? as u64,
                    replies: row.get::<_, i64>(5)? as u64,
                    text: row.get(6)?,
                    images: serde_json::from_str(&images_str).unwrap_or_default(),
                    videos: serde_json::from_str(&videos_str).unwrap_or_default(),
                },
                query: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Insert score rows. `scored_at` defaults to now via the schema.
    pub fn set_scores(
        &self,
        psyop: &str,
        psyop_commit_sha: &str,
        ids: &[String],
        scores: &[f64],
    ) -> Result<(), crate::error::Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (id, score) in ids.iter().zip(scores.iter()) {
            tx.execute(
                "INSERT OR REPLACE INTO scores (post_id, psyop, psyop_commit_sha, score)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, psyop, psyop_commit_sha, score],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
