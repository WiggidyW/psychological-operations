//! `scrapes run <name>` — drive playwright to collect tweets matching the
//! scrape's filters, store them with the scrape's tags, and stop once the
//! target count is reached.

use crate::scrape::{valid_for_scrape, Filter, Scrape};

pub async fn run_scrape(name: &str) -> Result<crate::Output, crate::error::Error> {
    let cfg = crate::config::load();
    let scrape_dir = crate::config::scrapes_dir().join(name);
    let scrape_path = scrape_dir.join("scrape.json");
    if !scrape_path.exists() {
        return Err(crate::error::Error::PsyopNotFound(scrape_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&scrape_path)?;
    let scrape: Scrape = serde_json::from_str(&data)?;
    scrape.validate()?;

    let repo = git2::Repository::open(&scrape_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    // Resolve disabled flag against (scrape, commit).
    if cfg.scrapes.get(name).is_some_and(|o| o.disabled_for(&commit_sha)) {
        eprintln!("scrape \"{name}\" is disabled for commit {commit_sha}; skipping");
        return Ok(crate::Output::Empty);
    }

    let target = scrape.count.unwrap_or(0) as usize;
    let now = chrono::Utc::now();

    let db = crate::db::Db::open()?;
    let already = db.count_posts_for_scrape(name, &commit_sha)?;
    let shortfall = target.saturating_sub(already);
    if shortfall == 0 {
        eprintln!("scrape \"{name}\" already at target ({already}/{target}) for commit {commit_sha}");
        return Ok(crate::Output::Empty);
    }

    // URL → originating filter map for per-tweet validation.
    let urls_by_filter: Vec<(String, &Filter)> = scrape.filters.iter()
        .map(|f| (f.url(), f))
        .collect();
    let urls: Vec<String> = urls_by_filter.iter().map(|(u, _)| u.clone()).collect();
    let filter_by_url: std::collections::HashMap<&str, &Filter> =
        urls_by_filter.iter().map(|(u, f)| (u.as_str(), *f)).collect();

    let mut pw = crate::playwright::Playwright::spawn()?;
    let states = pw.open_tabs(&urls)?;
    for (url, state) in &states {
        if state == "unexpected" {
            return Err(crate::error::Error::Playwright(
                format!("unexpected page state for filter url \"{url}\""),
            ));
        }
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

    Ok(crate::Output::Empty)
}
