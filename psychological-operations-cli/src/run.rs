use clap::{Args, Parser, Subcommand};

use crate::agent;
use crate::config;
use crate::publish;
use crate::invent;
use crate::error;

#[derive(Parser)]
#[command(name = "psychological-operations")]
#[command(about = "Agentic X scraper with ObjectiveAI scoring pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a psyop
    Run {
        #[command(flatten)]
        args: RunArgs,
    },
    /// Publish a psyop definition
    Publish {
        #[command(flatten)]
        args: publish::PublishArgs,
    },
    /// Invent a function for scoring posts
    Invent {
        #[command(subcommand)]
        command: invent::Commands,
    },
    /// List all psyops
    List,
    /// Interact with a running agent intervention
    Agent {
        #[command(subcommand)]
        command: agent::Commands,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: config::Commands,
    },
}

pub enum Output {
    ConfigGet(String),
    ConfigSet,
    Api(String),
    Empty,
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::ConfigGet(s) => write!(f, "{s}"),
            Output::ConfigSet => write!(f, "ok"),
            Output::Api(s) => write!(f, "{s}"),
            Output::Empty => Ok(()),
        }
    }
}

pub async fn run() -> Result<Output, error::Error> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { args } => args.handle().await,
        Commands::Publish { args } => args.handle(),
        Commands::Invent { command } => command.handle(),
        Commands::List => list_psyops(),
        Commands::Agent { command } => command.handle().await,
        Commands::Config { command } => command.handle(),
    }
}

fn list_psyops() -> Result<Output, error::Error> {
    let dir = config::psyops_dir();
    if !dir.exists() {
        return Ok(Output::Api("[]".into()));
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir()
            && path.join("psyop.json").exists()
            && path.join(".git").exists()
        {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(Output::Api(serde_json::to_string(&names)?))
}

// ---------------------------------------------------------------------------
// Run command
// ---------------------------------------------------------------------------

#[derive(Args)]
struct RunArgs {
    /// Psyop name
    name: String,
    /// Detach when agent needs input, printing PID
    #[arg(long)]
    detach_stdin: bool,
}

impl RunArgs {
    async fn handle(self) -> Result<Output, error::Error> {
        run_psyop(&self.name).await?;
        Ok(Output::Empty)
    }
}

async fn run_psyop(name: &str) -> Result<(), error::Error> {
    let cfg = config::load();
    let psyop_dir = config::psyops_dir().join(name);
    let config_path = psyop_dir.join("psyop.json");

    if !config_path.exists() {
        return Err(error::Error::PsyopNotFound(config_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&config_path)?;
    let psyop: crate::psyop::PsyOp = serde_json::from_str(&data)?;
    psyop.validate()?;

    let repo = git2::Repository::open(&psyop_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    let target_count = psyop.stages.first()
        .and_then(|s| s.count)
        .unwrap_or(100) as usize;
    let now = chrono::Utc::now();

    let db = crate::db::Db::open()?;

    let already_queued = db.count_unscored(name)?;
    let shortfall = target_count.saturating_sub(already_queued);

    if shortfall > 0 {
        // TODO: scraping has been split into its own `scrape` module; this
        // run loop will be rewired to operate on tagged posts pulled by
        // `psyop.filters[].tag`. Left unbuilt here so the rest of the
        // crate keeps compiling while the wiring is in flight.
        unimplemented!("psyop run scraping path is being rewired around the new scrape module");
        #[allow(unreachable_code)]
        let urls: Vec<String> = Vec::new();
        let filter_by_url: std::collections::HashMap<&str, &crate::psyop::Filter> =
            std::collections::HashMap::new();

        let mut pw = crate::playwright::Playwright::spawn()?;

        let states = pw.open_tabs(&urls)?;
        for (url, state) in &states {
            if state == "unexpected" {
                return Err(error::Error::Playwright(format!("unexpected page state for filter url \"{url}\"")));
            }
        }

        let mut collected = 0;
        while collected < shortfall {
            let Some((tweet, url)) = pw.next_tweet()? else { break };

            let filter = match filter_by_url.get(url.as_str()) {
                Some(f) => *f,
                None => continue,
            };

            let validation = crate::psyop::valid_for_psyop(
                &psyop, filter, &tweet.created,
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

            let inserted = db.insert_post(&post, name, &commit_sha, &url)?;
            if !inserted {
                let prior_query = db.existing_post_query(&post.id, name, &commit_sha)?;
                if prior_query.as_deref() == Some(url.as_str()) {
                    pw.close_query(&url)?;
                }
                continue;
            }

            collected += 1;
        }

        pw.close()?;
    }

    let entries = db.get_oldest_unscored(name, target_count)?;
    let mut scored_posts: Vec<crate::score::ScoredPost> = Vec::new();
    if !entries.is_empty() {
        let stage_results = crate::score::score(&psyop, entries)?;
        for stage_result in &stage_results {
            let ids: Vec<String> = stage_result.scored.iter().map(|s| s.post.id.clone()).collect();
            let scores: Vec<f64> = stage_result.scored.iter().map(|s| s.score).collect();
            db.set_scores(name, &commit_sha, stage_result.stage, &ids, &scores)?;
        }
        if let Some(last) = stage_results.into_iter().last() {
            scored_posts = last.scored;
        }
    }

    // Apply top-level psyop threshold + count to produce the final output set.
    // scored_posts is already sorted by score descending (per score::score).
    let mut output: Vec<&crate::score::ScoredPost> = scored_posts.iter().collect();
    if let Some(threshold) = psyop.threshold {
        output.retain(|s| s.score >= threshold);
    }
    if let Some(count) = psyop.count {
        output.truncate(count as usize);
    }

    let mut destinations = cfg.notifications.clone();
    destinations.extend(psyop.notifications.iter().cloned());
    config::notifications::destinations::notify(
        &destinations,
        name,
        &psyop,
        &output,
    ).await;

    Ok(())
}
