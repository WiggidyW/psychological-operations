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
        scrape TEXT NOT NULL,
        scrape_commit_sha TEXT NOT NULL,
        query TEXT NOT NULL,
        scraped_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (id, scrape, scrape_commit_sha)
    );
    CREATE TABLE IF NOT EXISTS post_contents (
        post_id TEXT PRIMARY KEY,
        text TEXT NOT NULL,
        images TEXT NOT NULL DEFAULT '[]',
        videos TEXT NOT NULL DEFAULT '[]'
    );
    CREATE TABLE IF NOT EXISTS post_tags (
        post_id TEXT NOT NULL,
        scrape TEXT NOT NULL,
        scrape_commit_sha TEXT NOT NULL,
        tag TEXT NOT NULL,
        PRIMARY KEY (post_id, scrape, scrape_commit_sha, tag)
    );
    CREATE INDEX IF NOT EXISTS post_tags_by_tag ON post_tags(tag);
    CREATE TABLE IF NOT EXISTS scores (
        post_id TEXT NOT NULL,
        psyop TEXT NOT NULL,
        psyop_commit_sha TEXT NOT NULL,
        score REAL NOT NULL,
        scored_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (psyop, psyop_commit_sha, post_id)
    );
    CREATE TABLE IF NOT EXISTS score_tags (
        post_id TEXT NOT NULL,
        psyop TEXT NOT NULL,
        psyop_commit_sha TEXT NOT NULL,
        tag TEXT NOT NULL,
        PRIMARY KEY (post_id, psyop, psyop_commit_sha, tag)
    );
    CREATE INDEX IF NOT EXISTS score_tags_by_tag ON score_tags(tag);
";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaUrl {
    pub url: String,
}

/// Canonical tweet (engagement metadata + content).
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

/// A `(post, query)` pair returned by score-time selection. `query` is
/// the originating scrape filter URL.
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

    /// Insert a new (post, scrape, commit) scrape row plus its content and
    /// tags. Returns whether a new posts row was created (false if the
    /// (id, scrape, commit) triple already existed). Content is upserted
    /// regardless; tags are upserted per-row (`INSERT OR IGNORE`).
    pub fn insert_post(
        &self,
        post: &Post,
        scrape: &str,
        scrape_commit_sha: &str,
        query: &str,
        tags: &[String],
    ) -> Result<bool, crate::error::Error> {
        let tx = self.conn.unchecked_transaction()?;
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO posts (id, handle, created, likes, retweets, replies, scrape, scrape_commit_sha, query)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                post.id, post.handle, post.created,
                post.likes as i64, post.retweets as i64, post.replies as i64,
                scrape, scrape_commit_sha, query,
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
        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO post_tags (post_id, scrape, scrape_commit_sha, tag)
                 VALUES (?1, ?2, ?3, ?4)",
                params![post.id, scrape, scrape_commit_sha, tag],
            )?;
        }
        tx.commit()?;
        Ok(inserted)
    }

    /// How many distinct posts have been stored for this `(scrape, commit)`.
    pub fn count_posts_for_scrape(
        &self,
        scrape: &str,
        scrape_commit_sha: &str,
    ) -> Result<usize, crate::error::Error> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM posts WHERE scrape = ?1 AND scrape_commit_sha = ?2",
            params![scrape, scrape_commit_sha],
            |row| row.get(0),
        )?;
        Ok(n as usize)
    }

    /// The `query` recorded on the existing `posts` row for this
    /// `(id, scrape, commit)`, or `None` if no such row exists. Used to
    /// detect when a search query has cycled back to a tweet it already
    /// produced.
    pub fn existing_post_query(
        &self,
        post_id: &str,
        scrape: &str,
        scrape_commit_sha: &str,
    ) -> Result<Option<String>, crate::error::Error> {
        let result = self.conn.query_row(
            "SELECT query FROM posts WHERE id = ?1 AND scrape = ?2 AND scrape_commit_sha = ?3",
            params![post_id, scrape, scrape_commit_sha],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(q) => Ok(Some(q)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Count distinct posts that carry any of the given tags AND have no
    /// matching scores row for `(psyop, psyop_commit_sha)`. When
    /// `min_score` is `Some(t)`, also requires that the post has at least
    /// one score row (under any psyop) at or above `t`.
    pub fn count_unscored_for_tags(
        &self,
        psyop: &str,
        psyop_commit_sha: &str,
        tags: &[String],
        min_score: Option<f64>,
    ) -> Result<usize, crate::error::Error> {
        if tags.is_empty() { return Ok(0); }
        let placeholders = vec!["?"; tags.len()].join(",");
        let min_score_clause = if min_score.is_some() {
            "AND EXISTS (
               SELECT 1 FROM scores prev
               WHERE prev.post_id = t.post_id AND prev.score >= ?
             )"
        } else { "" };
        let sql = format!(
            "SELECT COUNT(DISTINCT t.post_id)
             FROM post_tags t
             WHERE t.tag IN ({placeholders})
               AND NOT EXISTS (
                 SELECT 1 FROM scores s
                 WHERE s.post_id = t.post_id
                   AND s.psyop = ?
                   AND s.psyop_commit_sha = ?
               )
               {min_score_clause}",
        );
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
        params_vec.push(&psyop);
        params_vec.push(&psyop_commit_sha);
        if let Some(t) = &min_score {
            params_vec.push(t);
        }
        let n: i64 = self.conn.query_row(&sql, params_vec.as_slice(), |row| row.get(0))?;
        Ok(n as usize)
    }

    /// Take the `limit` oldest unscored entries for this psyop, joined with
    /// their content. A post is "unscored for psyop" if no scores row exists
    /// for `(psyop, psyop_commit_sha, post_id)`. Selection is by tag —
    /// posts whose `post_tags.tag` matches any of the given tags. Ordered
    /// by the earliest matching scrape's `scraped_at`.
    pub fn get_oldest_unscored_for_tags(
        &self,
        psyop: &str,
        psyop_commit_sha: &str,
        tags: &[String],
        min_score: Option<f64>,
        limit: usize,
    ) -> Result<Vec<UnscoredEntry>, crate::error::Error> {
        if tags.is_empty() { return Ok(Vec::new()); }
        let placeholders = vec!["?"; tags.len()].join(",");
        let min_score_clause = if min_score.is_some() {
            "AND EXISTS (
               SELECT 1 FROM scores prev
               WHERE prev.post_id = p.id AND prev.score >= ?
             )"
        } else { "" };
        let sql = format!(
            "SELECT p.id, p.handle, p.created, p.likes, p.retweets, p.replies,
                    c.text, c.images, c.videos, p.query
             FROM posts p
             JOIN post_contents c ON c.post_id = p.id
             WHERE EXISTS (
               SELECT 1 FROM post_tags t
               WHERE t.post_id = p.id
                 AND t.scrape = p.scrape
                 AND t.scrape_commit_sha = p.scrape_commit_sha
                 AND t.tag IN ({placeholders})
             )
               AND NOT EXISTS (
                 SELECT 1 FROM scores s
                 WHERE s.post_id = p.id
                   AND s.psyop = ?
                   AND s.psyop_commit_sha = ?
               )
               {min_score_clause}
             GROUP BY p.id
             ORDER BY MIN(p.scraped_at) ASC
             LIMIT ?",
        );
        let limit_i64 = limit as i64;
        let mut params_vec: Vec<&dyn rusqlite::ToSql> = tags.iter().map(|t| t as &dyn rusqlite::ToSql).collect();
        params_vec.push(&psyop);
        params_vec.push(&psyop_commit_sha);
        if let Some(t) = &min_score {
            params_vec.push(t);
        }
        params_vec.push(&limit_i64);
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_vec.as_slice(), |row| {
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

    /// Insert score rows + per-score tags. `scored_at` defaults to now via
    /// the schema. Tag rows are upserted per `(post_id, psyop, commit, tag)`.
    pub fn set_scores(
        &self,
        psyop: &str,
        psyop_commit_sha: &str,
        ids: &[String],
        scores: &[f64],
        tags: &[String],
    ) -> Result<(), crate::error::Error> {
        let tx = self.conn.unchecked_transaction()?;
        for (id, score) in ids.iter().zip(scores.iter()) {
            tx.execute(
                "INSERT OR REPLACE INTO scores (post_id, psyop, psyop_commit_sha, score)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, psyop, psyop_commit_sha, score],
            )?;
            for tag in tags {
                tx.execute(
                    "INSERT OR IGNORE INTO score_tags (post_id, psyop, psyop_commit_sha, tag)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![id, psyop, psyop_commit_sha, tag],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
