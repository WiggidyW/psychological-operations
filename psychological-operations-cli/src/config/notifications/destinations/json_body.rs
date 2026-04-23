use serde::Serialize;

use crate::db::MediaUrl;
use crate::score::ScoredPost;

/// Top-level JSON body shared by destinations that emit a structured payload
/// (currently `http` and the `json` mode of `stdout`/`stderr`).
#[derive(Debug, Serialize)]
pub struct Body<'a> {
    pub psyop: &'a str,
    pub results: Vec<Result_<'a>>,
}

/// One scored tweet in the body.
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

pub fn build<'a>(psyop_name: &'a str, output: &'a [&'a ScoredPost]) -> Body<'a> {
    Body {
        psyop: psyop_name,
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
    }
}
