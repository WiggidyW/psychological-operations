//! `scrapes run` — drive playwright concurrently for every enabled scrape.
//!
//! Each scrape gets a private snapshot of the base Chrome profile (via
//! `chrome_profile`) so concurrent `launchPersistentContext` calls don't
//! fight over the profile lock. When a filter URL comes back as
//! `unexpected` (login wall, captcha, etc.) the runner pauses and waits
//! for `agent reply --scrape <name>` via `agent::intervention::await_one`.

use crate::agent::intervention::{self, InterventionOutcome};
use crate::scrape::{valid_for_scrape, Filter, Scrape};
use crate::scrapes::chrome_profile;

/// Public entry point for `scrapes run`. Enumerates every scrape directory
/// on disk, filters out disabled ones for their current commit, and runs
/// the rest concurrently. One task's failure does not abort siblings —
/// per-task results are reported to stderr.
pub async fn run_all() -> Result<crate::Output, crate::error::Error> {
    let cfg = crate::config::load();
    let dir = crate::config::scrapes_dir();
    if !dir.exists() {
        eprintln!("no scrapes directory at {}", dir.display());
        return Ok(crate::Output::Empty);
    }

    let mut targets: Vec<String> = Vec::new();
    for ent in std::fs::read_dir(&dir)? {
        let ent = ent?;
        let path = ent.path();
        if !path.is_dir()
            || !path.join("scrape.json").exists()
            || !path.join(".git").exists()
        {
            continue;
        }
        let Some(name) = ent.file_name().to_str().map(|s| s.to_string()) else { continue };

        // Resolve disabled flag against (scrape, commit) at enumeration time
        // so sibling scrapes don't interfere via base-profile mutation.
        let commit_sha = (|| -> Result<String, git2::Error> {
            let repo = git2::Repository::open(&path)?;
            let head = repo.head()?.peel_to_commit()?;
            Ok(head.id().to_string())
        })().unwrap_or_default();
        if cfg.scrapes.get(&name).is_some_and(|o| o.disabled_for(&commit_sha)) {
            eprintln!("scrape \"{name}\" is disabled for commit {commit_sha}; skipping");
            continue;
        }

        targets.push(name);
    }

    if targets.is_empty() {
        eprintln!("no enabled scrapes to run");
        return Ok(crate::Output::Empty);
    }

    // Stagger spawns by `scraper_spawn_delay_secs` so a burst of scrapes
    // doesn't hammer X's IP-level rate limit all at once.
    let spawn_delay = std::time::Duration::from_secs(cfg.scraper_spawn_delay_secs);
    let mut handles: Vec<_> = Vec::with_capacity(targets.len());
    for (i, name) in targets.into_iter().enumerate() {
        if i > 0 && !spawn_delay.is_zero() {
            tokio::time::sleep(spawn_delay).await;
        }
        handles.push(tokio::spawn(async move {
            let result = run_scrape(&name).await;
            (name, result)
        }));
    }

    for join in futures::future::join_all(handles).await {
        match join {
            Ok((name, Ok(_))) => eprintln!("scrape \"{name}\" finished"),
            Ok((name, Err(e))) => eprintln!("scrape \"{name}\" failed: {e}"),
            Err(e) => eprintln!("scrape task panicked: {e}"),
        }
    }

    Ok(crate::Output::Empty)
}

/// Run a single scrape end-to-end: snapshot the Chrome profile, drive
/// playwright, deal with any required agent interventions, store the
/// resulting tagged posts, and copy the profile back via the `Drop` guard.
async fn run_scrape(name: &str) -> Result<(), crate::error::Error> {
    let cfg = crate::config::load();
    let scrape_dir = crate::config::scrapes_dir().join(name);
    let scrape_path = scrape_dir.join("scrape.json");
    if !scrape_path.exists() {
        return Err(crate::error::Error::PsyopNotFound(scrape_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&scrape_path)?;
    let scrape: Scrape = serde_json::from_str(&data)?;
    scrape.validate()?;

    // Scope the libgit2 handles tightly: they're !Send, so any await
    // afterwards (intervention loop, notifications) would poison the
    // tokio::spawn future if we held them open.
    let commit_sha = {
        let repo = git2::Repository::open(&scrape_dir)?;
        let head = repo.head()?.peel_to_commit()?;
        head.id().to_string()
    };

    let target = scrape.count.unwrap_or(0) as usize;
    let now = chrono::Utc::now();

    let db = crate::db::Db::open()?;
    let already = db.count_posts_for_scrape(name, &commit_sha)?;
    let shortfall = target.saturating_sub(already);
    if shortfall == 0 {
        eprintln!("scrape \"{name}\" already at target ({already}/{target}) for commit {commit_sha}");
        return Ok(());
    }

    // Snapshot the base Chrome profile so this scrape can run in parallel
    // with siblings without sharing a profile lock. The session guard runs
    // copy-back + cleanup on drop, even on panic.
    let (_session, profile_dir) = chrome_profile::ProfileSession::begin(name)?;

    // URL → originating filter map for per-tweet validation.
    let urls_by_filter: Vec<(String, &Filter)> = scrape.filters.iter()
        .map(|f| (f.url(), f))
        .collect();
    let urls: Vec<String> = urls_by_filter.iter().map(|(u, _)| u.clone()).collect();
    let filter_by_url: std::collections::HashMap<&str, &Filter> =
        urls_by_filter.iter().map(|(u, f)| (u.as_str(), *f)).collect();

    let mut pw = crate::playwright::Playwright::spawn_with_profile(&profile_dir)?;
    let states = pw.open_tabs(&urls)?;

    // Resolve any "unexpected" URLs via agent intervention, up to
    // max_attempts; whatever's still unexpected after that is dropped.
    let unexpected_urls: Vec<String> = states.iter()
        .filter(|(_, s)| s.as_str() == "unexpected")
        .map(|(u, _)| u.clone())
        .collect();
    if !unexpected_urls.is_empty() {
        resolve_interventions(name, &commit_sha, &scrape, &cfg, &mut pw, unexpected_urls).await?;
    }

    let mut collected = 0;
    while collected < shortfall {
        let Some((tweet, url)) = pw.next_tweet()? else { break };
        let Some(filter) = filter_by_url.get(url.as_str()).copied() else { continue };

        let validation = valid_for_scrape(
            &scrape, filter, &tweet.created,
            tweet.likes, tweet.retweets, tweet.replies, &now,
        );
        if !validation.valid {
            if validation.reason == Some("max_age") {
                pw.close_query(&url)?;
            }
            continue;
        }

        let post = crate::db::Post {
            id: tweet.id,
            handle: tweet.handle,
            text: tweet.text,
            images: tweet.images,
            videos: tweet.videos,
            created: tweet.created,
            likes: tweet.likes,
            retweets: tweet.retweets,
            replies: tweet.replies,
        };

        let inserted = db.insert_post(&post, name, &commit_sha, &url, &scrape.tags)?;
        if !inserted {
            // Already-seen post via this filter → loop has cycled; close
            // that query so we move on to the next filter.
            let prior = db.existing_post_query(&post.id, name, &commit_sha)?;
            if prior.as_deref() == Some(url.as_str()) {
                pw.close_query(&url)?;
            }
            continue;
        }

        collected += 1;
    }

    pw.close()?;

    eprintln!("scrape \"{name}\" collected {collected} new posts (commit {commit_sha})");

    let mut destinations = cfg.notifications.clone();
    if let Some(per_scrape) = cfg.scrapes.get(name) {
        destinations.extend(per_scrape.notifications_for(&commit_sha).iter().cloned());
    }
    crate::notifications::destinations::notify(
        &destinations,
        crate::notifications::destinations::Subject::Scrape {
            name,
            scrape: &scrape,
            collected,
        },
    ).await;

    Ok(())
}

/// Loop on intervention attempts until every URL has come unstuck or
/// `max_attempts` is reached. Each attempt fires a notification, blocks on
/// the TCP listener for the per-scrape `agent_timeout`, then re-validates.
async fn resolve_interventions(
    name: &str,
    commit_sha: &str,
    scrape: &Scrape,
    cfg: &crate::config::Config,
    pw: &mut crate::playwright::Playwright,
    initial_unexpected: Vec<String>,
) -> Result<(), crate::error::Error> {
    let (timeout_secs, max_attempts) = intervention::resolve_limits(cfg, name, commit_sha);
    let mut destinations = cfg.notifications.clone();
    if let Some(per_scrape) = cfg.scrapes.get(name) {
        destinations.extend(per_scrape.notifications_for(commit_sha).iter().cloned());
    }

    let mut unresolved = initial_unexpected;
    let mut attempt: u64 = 0;
    while !unresolved.is_empty() && attempt < max_attempts {
        attempt += 1;
        let prompt = build_prompt(name, commit_sha, attempt, max_attempts, &unresolved);
        let outcome = intervention::await_one(
            name, commit_sha, scrape, &destinations, &prompt, timeout_secs,
        ).await?;
        match outcome {
            InterventionOutcome::Timeout => {
                eprintln!(
                    "scrape \"{name}\" intervention attempt {attempt}/{max_attempts} timed out after {timeout_secs}s",
                );
            }
            InterventionOutcome::Reply(reply) => {
                if !reply.is_empty() {
                    eprintln!("scrape \"{name}\" intervention reply: {reply}");
                }
                let new_states = pw.retry_unexpected(&unresolved)?;
                unresolved.retain(|u| {
                    new_states.get(u).map(|s| s.as_str()) == Some("unexpected")
                });
            }
        }
    }

    if !unresolved.is_empty() {
        eprintln!(
            "scrape \"{name}\" giving up on {} unresolved url(s) after {max_attempts} attempt(s)",
            unresolved.len(),
        );
    }
    Ok(())
}

fn build_prompt(
    name: &str,
    commit_sha: &str,
    attempt: u64,
    max_attempts: u64,
    urls: &[String],
) -> String {
    let mut s = format!(
        "scrape \"{name}\" (commit {commit_sha}) attempt {attempt}/{max_attempts}: \
         resolve the following url(s) in the open chrome window, then reply to unblock:\n",
    );
    for url in urls {
        s.push_str("  - ");
        s.push_str(url);
        s.push('\n');
    }
    s
}
