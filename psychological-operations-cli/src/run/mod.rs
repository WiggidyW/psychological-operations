use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Run a psyop by name
    Standard {
        name: String,
        /// Detach when agent needs input, printing PID
        #[arg(long)]
        detach_stdin: bool,
    },
}

impl Commands {
    pub async fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Standard { name, detach_stdin: _ } => {
                run_psyop(&name).await?;
                Ok(crate::Output::Empty)
            }
        }
    }
}

async fn run_psyop(name: &str) -> Result<(), crate::error::Error> {
    let cfg = crate::config::load();
    let psyop_dir = crate::config::psyops_dir().join(name);
    let config_path = psyop_dir.join("psyop.json");

    if !config_path.exists() {
        return Err(crate::error::Error::PsyopNotFound(config_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&config_path)?;
    let psyop: crate::psyop::PsyOp = serde_json::from_str(&data)?;

    // Get commit SHA
    let repo = git2::Repository::open(&psyop_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    // Scrape
    let mut pw = crate::playwright::Playwright::spawn()?;

    let target_count = psyop.stages.first()
        .and_then(|s| s.count)
        .unwrap_or(100) as usize;
    let now = chrono::Utc::now();

    let db = crate::db::Db::open()?;

    // Open tabs
    let states = pw.open_tabs(&psyop.queries)?;
    for (query, state) in &states {
        if state == "unexpected" {
            // TODO: agent intervention
            return Err(crate::error::Error::Playwright(format!("unexpected page state for query \"{query}\"")));
        }
    }

    // Scrape tweets
    let mut collected = 0;
    while collected < target_count {
        let Some((tweet, query)) = pw.next_tweet()? else { break };

        let validation = crate::psyop::valid_for_psyop(&psyop, &tweet.created, tweet.likes, &now);
        if !validation.valid {
            if validation.reason == Some("max_age") {
                pw.close_query(&query)?;
            }
            continue;
        }

        let post = crate::db::QueuedPost {
            id: tweet.id,
            scrape_id: name.to_string(),
            query: query.clone(),
            handle: tweet.handle,
            text: tweet.text,
            images: tweet.images,
            videos: tweet.videos,
            created: tweet.created,
            community: tweet.community,
            psyop: name.to_string(),
            psyop_commit_sha: commit_sha.clone(),
        };

        let inserted = db.insert_post(&post)?;
        if !inserted {
            if db.has_existing_post(&post.id, &query, name, &commit_sha)? {
                pw.close_query(&query)?;
            }
            continue;
        }

        collected += 1;
    }

    pw.close()?;

    // Score
    let posts = db.get_posts(name)?;
    if !posts.is_empty() {
        let scored = crate::score::score(&psyop, posts)?;
        let ids: Vec<String> = scored.iter().map(|s| s.post.id.clone()).collect();
        let scores: Vec<f64> = scored.iter().map(|s| s.score).collect();
        db.finish_posts(&ids, &scores)?;
    }

    // Notify
    crate::notifications::notify(&cfg.notifications, &format!("PsyOp \"{name}\": scraped {collected} posts.")).await;

    Ok(())
}
