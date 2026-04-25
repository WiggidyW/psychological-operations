//! `psyops run <name>` — for each Source on the psyop, pull oldest unscored
//! posts whose tags match `source.tag`, apply the per-source eligibility
//! checks, score the union via the psyop's function, persist the scores
//! tagged with `psyop.tags`, and notify.

use std::collections::HashSet;

use crate::db::UnscoredEntry;
use crate::psyop::{valid_for_source, PsyOp, Source};

pub async fn run_psyop(name: &str) -> Result<crate::Output, crate::error::Error> {
    let cfg = crate::config::load();
    let psyop_dir = crate::config::psyops_dir().join(name);
    let psyop_path = psyop_dir.join("psyop.json");
    if !psyop_path.exists() {
        return Err(crate::error::Error::PsyopNotFound(psyop_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&psyop_path)?;
    let psyop: PsyOp = serde_json::from_str(&data)?;
    psyop.validate()?;

    let repo = git2::Repository::open(&psyop_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    if cfg.psyops.get(name).is_some_and(|o| o.disabled_for(&commit_sha)) {
        eprintln!("psyop \"{name}\" is disabled for commit {commit_sha}; skipping");
        return Ok(crate::Output::Empty);
    }

    let now = chrono::Utc::now();
    let db = crate::db::Db::open()?;

    // Collect candidates per source. Each Source pulls its own quota of
    // oldest-unscored posts that carry its tag, then per-tweet validation
    // is applied. Posts are deduped across sources by id.
    let mut entries: Vec<UnscoredEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for source in &psyop.sources {
        let pool = collect_for_source(&db, name, &commit_sha, source, &now)?;
        for entry in pool {
            if seen.insert(entry.post.id.clone()) {
                entries.push(entry);
            }
        }
    }

    let mut scored_posts: Vec<crate::score::ScoredPost> = Vec::new();
    if !entries.is_empty() {
        scored_posts = crate::score::score(&psyop, entries)?;
        let ids: Vec<String> = scored_posts.iter().map(|s| s.post.id.clone()).collect();
        let scores: Vec<f64> = scored_posts.iter().map(|s| s.score).collect();
        db.set_scores(name, &commit_sha, &ids, &scores, &psyop.tags)?;
    }

    let output: Vec<&crate::score::ScoredPost> = scored_posts.iter().collect();

    let mut destinations = cfg.notifications.clone();
    if let Some(per_psyop) = cfg.psyops.get(name) {
        destinations.extend(per_psyop.notifications_for(&commit_sha).iter().cloned());
    }
    crate::notifications::destinations::notify(
        &destinations,
        crate::notifications::destinations::Subject::Psyop {
            name,
            psyop: &psyop,
            output: &output,
        },
    ).await;

    Ok(crate::Output::Empty)
}

/// Pull `source.count` oldest unscored posts carrying `source.tag` for
/// `(psyop, commit)`, then apply per-tweet eligibility (`valid_for_source`)
/// and an over-fetch retry loop until the source's count is satisfied or
/// the pool runs dry.
fn collect_for_source(
    db: &crate::db::Db,
    psyop_name: &str,
    commit_sha: &str,
    source: &Source,
    now: &chrono::DateTime<chrono::Utc>,
) -> Result<Vec<UnscoredEntry>, crate::error::Error> {
    let target = source.count.map(|n| n as usize);
    let tags = vec![source.tag.clone()];
    // Pull a generous over-fetch so per-tweet validation has room to
    // discard ineligible candidates without going back to the DB.
    let pull_limit = target.map(|t| t.saturating_mul(4).max(t + 16)).unwrap_or(usize::MAX / 4);
    let raw = db.get_oldest_unscored_for_tags(psyop_name, commit_sha, &tags, source.min_score, pull_limit)?;

    let mut accepted: Vec<UnscoredEntry> = Vec::new();
    for entry in raw {
        let v = valid_for_source(
            source,
            &entry.post.created,
            entry.post.likes,
            entry.post.retweets,
            entry.post.replies,
            now,
        );
        if v.valid {
            accepted.push(entry);
            if let Some(t) = target {
                if accepted.len() >= t { break; }
            }
        }
    }
    Ok(accepted)
}
