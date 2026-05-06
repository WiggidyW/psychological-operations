//! The view of a tweet that filtering and sorting operate on.
//!
//! `db::Post` is the persistence shape (engagement metadata + body
//! text + media URLs); `Tweet` is the runtime shape (engagement
//! metadata + precomputed `age`, no content) used by
//! `psyops::Filter::evaluate` and `psyops::SortBy::evaluate`.
//! Content lives in `db::contents` and is not loaded for filtering /
//! sorting paths.

use serde::{Deserialize, Serialize};

use starlark::values::Heap;
use starlark::values::Value;
use starlark::values::dict::AllocDict;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tweet {
    pub id: String,
    pub handle: String,
    /// RFC 3339. Kept on the struct so `SortBy::Newest` / `Oldest`
    /// can sort lexically and the Custom-sort starlark expression
    /// can reach it.
    pub created: String,
    /// Seconds since `created`. Computed once at hydration time so
    /// `Filter::evaluate` doesn't need an ambient `now`.
    pub age: u64,
    pub likes: u64,
    pub retweets: u64,
    pub replies: u64,
    pub impressions: u64,
}

/// Convert a `&Tweet` into a Starlark dict with stable string keys.
/// Used by both `Filter` (via the `_per_tweet` Custom path, if it
/// ever needs the whole tweet) and `SortBy::Custom` (which sees a
/// list of these dicts).
pub fn alloc_dict<'v>(t: &Tweet, heap: &'v Heap) -> Value<'v> {
    heap.alloc(AllocDict([
        ("id",          heap.alloc(t.id.clone())),
        ("handle",      heap.alloc(t.handle.clone())),
        ("created",     heap.alloc(t.created.clone())),
        ("age",         heap.alloc(t.age as i64)),
        ("likes",       heap.alloc(t.likes as i64)),
        ("retweets",    heap.alloc(t.retweets as i64)),
        ("replies",     heap.alloc(t.replies as i64)),
        ("impressions", heap.alloc(t.impressions as i64)),
    ]))
}

#[cfg(test)]
pub(crate) fn tw_default(id: &str) -> Tweet {
    Tweet {
        id: id.into(),
        handle: "anon".into(),
        created: "2026-01-01T00:00:00Z".into(),
        age: 0,
        likes: 0,
        retweets: 0,
        replies: 0,
        impressions: 0,
    }
}
