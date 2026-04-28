//! `scrapes run` — drive a single shared Chrome session through every
//! enabled scrape's filters in sequence. The browser is opened once at
//! `https://x.com/`; for each filter we type the query into the in-page
//! search bar with human-paced jitter, click the Latest tab, and run the
//! existing scroll-and-parse loop until the scrape's `count` is satisfied.
//! When a typed search lands on an unexpected page state (login wall,
//! captcha, etc.), the runner pauses for `agent reply --scrape <name>`
//! via `agent::intervention::await_one`.

use crate::agent::intervention::{self, InterventionOutcome};
use crate::scrape::{valid_for_scrape, Scrape};

/// Public entry point for `scrapes run`. With no flags, runs every
/// enabled scrape. `--name X` narrows to one; `--commit Y` additionally
/// requires HEAD to match.
pub async fn run_all(name_filter: Option<&str>, commit_filter: Option<&str>) -> Result<crate::Output, crate::error::Error> {
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

        if let Some(want) = name_filter {
            if name != want { continue; }
        }
        if let Some(want_commit) = commit_filter {
            let head = (|| -> Result<String, git2::Error> {
                let repo = git2::Repository::open(&path)?;
                let head = repo.head()?.peel_to_commit()?;
                Ok(head.id().to_string())
            })().unwrap_or_default();
            if head != want_commit {
                eprintln!(
                    "scrape \"{name}\" HEAD is {head}, not requested commit {want_commit}; skipping",
                );
                continue;
            }
        }

        match should_run(&name, &cfg, name_filter.is_some()) {
            Ok(true) => targets.push(name),
            Ok(false) => {}
            Err(e) => eprintln!("scrape \"{name}\" eligibility check failed: {e}"),
        }
    }

    if targets.is_empty() {
        eprintln!("no enabled scrapes to run");
        return Ok(crate::Output::Empty);
    }

    // ONE browser session for the whole run. Opens https://x.com/ and parks
    // there; each scrape's filters are issued as typed searches.
    let mut pw = crate::playwright::Playwright::spawn()?;
    pw.start_session().await?;

    for name in &targets {
        match run_scrape(&mut pw, name, &cfg).await {
            Ok(_)  => eprintln!("scrape \"{name}\" finished"),
            Err(e) => eprintln!("scrape \"{name}\" failed: {e}"),
        }
    }

    pw.close().await?;

    Ok(crate::Output::Empty)
}

/// Should this scrape be in the run set?
///   - Disabled (resolved per-commit) → no, unless `override_disabled`.
///   - `count: None` (unlimited) → always yes.
///   - `count: Some(n)` and at least `n` posts already stored for
///     `(scrape, commit)` → no, queue is already full.
///   - Otherwise → yes.
fn should_run(name: &str, cfg: &crate::config::Config, override_disabled: bool) -> Result<bool, crate::error::Error> {
    let scrape_dir = crate::config::scrapes_dir().join(name);
    let scrape_path = scrape_dir.join("scrape.json");
    if !scrape_path.exists() {
        return Ok(false);
    }

    let data = std::fs::read_to_string(&scrape_path)?;
    let scrape: Scrape = serde_json::from_str(&data)?;

    let commit_sha = {
        let repo = git2::Repository::open(&scrape_dir)?;
        let head = repo.head()?.peel_to_commit()?;
        head.id().to_string()
    };

    if !override_disabled
        && cfg.scrapes.get(name).is_some_and(|o| o.disabled_for(&commit_sha))
    {
        eprintln!("scrape \"{name}\" is disabled for commit {commit_sha}; skipping");
        return Ok(false);
    }

    let Some(target) = scrape.count else {
        return Ok(true);
    };
    let db = crate::db::Db::open()?;
    let already = db.count_posts_for_scrape(name, &commit_sha)?;
    if already >= target as usize {
        eprintln!(
            "scrape \"{name}\" already at target ({already}/{target}) for commit {commit_sha}; skipping",
        );
        return Ok(false);
    }
    Ok(true)
}

/// Run a single scrape against the shared browser session. Iterate the
/// scrape's filters serially, typing each as a search, and store eligible
/// tweets until the per-scrape `count` is met (or all filters exhausted).
async fn run_scrape(
    pw: &mut crate::playwright::Playwright,
    name: &str,
    cfg: &crate::config::Config,
) -> Result<(), crate::error::Error> {
    let scrape_dir = crate::config::scrapes_dir().join(name);
    let scrape_path = scrape_dir.join("scrape.json");
    if !scrape_path.exists() {
        return Err(crate::error::Error::PsyopNotFound(scrape_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&scrape_path)?;
    let scrape: Scrape = serde_json::from_str(&data)?;
    scrape.validate()?;

    // Scope libgit2 handles tightly: they're !Send. The single-session
    // model has no tokio::spawn anymore, but keeping the scope clean
    // makes the code robust to future reintroduction.
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

    let mut collected: usize = 0;
    'filters: for filter in &scrape.filters {
        if collected >= shortfall { break 'filters; }

        // Type the filter's raw query (`from:user`, `keyword`, etc.) into
        // the X search bar. On unexpected page state, pause for agent
        // intervention; on still-unexpected after max attempts, skip this
        // filter and move on.
        let mut state = pw.run_query(&filter.query).await?;
        if state == "unexpected" {
            state = resolve_intervention(name, &commit_sha, &scrape, cfg, pw, &filter.query).await?;
        }
        if state != "results" {
            // empty or persistently unexpected → nothing to scrape from
            // this filter; move on.
            pw.close_query().await?;
            continue 'filters;
        }

        loop {
            if collected >= shortfall { break; }
            let Some(tweet) = pw.next_tweet().await? else { break };

            let validation = valid_for_scrape(
                &scrape, filter, &tweet.created,
                tweet.likes, tweet.retweets, tweet.replies, &now,
            );
            if !validation.valid {
                if validation.reason == Some("max_age") {
                    // Once we hit a too-old tweet on the chronological
                    // (Latest) feed, every later tweet will also be too
                    // old. Stop scrolling this filter.
                    break;
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

            let inserted = db.insert_post(&post, name, &commit_sha, &filter.query, &scrape.tags)?;
            if !inserted {
                // Already-seen post via this filter → loop has cycled;
                // this filter is exhausted.
                let prior = db.existing_post_query(&post.id, name, &commit_sha)?;
                if prior.as_deref() == Some(filter.query.as_str()) {
                    break;
                }
                continue;
            }

            collected += 1;
        }

        pw.close_query().await?;
    }

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

/// Loop on intervention attempts until the typed query yields a usable
/// page state or `max_attempts` is reached. Each attempt prints the
/// prompt locally, blocks on the TCP listener for the per-scrape
/// `agent_timeout`, and on reply re-types the query into the search bar.
async fn resolve_intervention(
    name: &str,
    commit_sha: &str,
    scrape: &Scrape,
    cfg: &crate::config::Config,
    pw: &mut crate::playwright::Playwright,
    filter_query: &str,
) -> Result<String, crate::error::Error> {
    let (timeout_secs, max_attempts) = intervention::resolve_limits(cfg, name, commit_sha);

    let mut state = "unexpected".to_string();
    let mut attempt: u64 = 0;
    while state == "unexpected" && attempt < max_attempts {
        attempt += 1;
        let prompt = build_prompt(name, commit_sha, attempt, max_attempts, filter_query);
        let outcome = intervention::await_one(
            name, commit_sha, scrape, &prompt, timeout_secs,
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
                state = pw.run_query(filter_query).await?;
            }
        }
    }

    if state == "unexpected" {
        eprintln!(
            "scrape \"{name}\" giving up on filter \"{filter_query}\" after {max_attempts} attempt(s)",
        );
    }
    Ok(state)
}

fn build_prompt(
    name: &str,
    commit_sha: &str,
    attempt: u64,
    max_attempts: u64,
    filter_query: &str,
) -> String {
    format!(
        "scrape \"{name}\" (commit {commit_sha}) attempt {attempt}/{max_attempts}: \
         the typed search for \"{filter_query}\" landed on an unexpected page state. \
         resolve it in the open chrome window, then reply to retry the search.",
    )
}
