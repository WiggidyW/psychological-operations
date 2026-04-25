use serde::Serialize;

use crate::db::MediaUrl;
use crate::score::ScoredPost;

use super::Subject;

/// Top-level JSON body shared by destinations that emit a structured payload
/// (currently `http`, the `json` mode of `stdout`/`stderr`/`file`, and `exec`).
/// Tagged on `kind` so consumers can branch on psyop vs scrape.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body<'a> {
    Psyop {
        name: &'a str,
        tags: &'a [String],
        results: Vec<Result_<'a>>,
    },
    Scrape {
        name: &'a str,
        tags: &'a [String],
        collected: usize,
    },
}

/// One scored tweet in the psyop body.
#[derive(Debug, Serialize)]
#[serde(rename = "Result")]
pub struct Result_<'a> {
    pub score: f64,
    pub id: &'a str,
    pub handle: &'a str,
    pub text: &'a str,
    pub images: &'a [MediaUrl],
    pub videos: &'a [MediaUrl],
    pub created: &'a str,
    pub query: &'a str,
    pub url: String,
}

pub fn build<'a>(subject: &'a Subject<'a>) -> Body<'a> {
    match subject {
        Subject::Psyop { name, psyop, output } => Body::Psyop {
            name,
            tags: &psyop.tags,
            results: output.iter().map(|s| Result_ {
                score: s.score,
                id: &s.post.id,
                handle: &s.post.handle,
                text: &s.post.text,
                images: &s.post.images,
                videos: &s.post.videos,
                created: &s.post.created,
                query: &s.query,
                url: format!("https://x.com/{}/status/{}", s.post.handle, s.post.id),
            }).collect(),
        },
        Subject::Scrape { name, scrape, collected } => Body::Scrape {
            name,
            tags: &scrape.tags,
            collected: *collected,
        },
    }
}

/// Helper for text-mode destinations: flatten the subject into a header line
/// plus a list of (label, url) pairs. Empty list when subject has no items
/// to enumerate (e.g. scrape).
pub fn lines(subject: &Subject) -> (String, Vec<(String, String)>) {
    match subject {
        Subject::Psyop { name, output, .. } => {
            let header = format!("PsyOp \"{name}\"");
            let lines = output.iter().map(|s| {
                let url = format!("https://x.com/{}/status/{}", s.post.handle, s.post.id);
                (format!("{:.4}", s.score), url)
            }).collect();
            (header, lines)
        }
        Subject::Scrape { name, collected, .. } => {
            let header = format!("Scrape \"{name}\" — collected {collected} new posts");
            (header, Vec::new())
        }
    }
}

// Lifetimes needed by ScoredPost reference imports above.
#[allow(dead_code)]
fn _phantom(_: &ScoredPost) {}
