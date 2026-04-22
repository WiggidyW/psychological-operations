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
    eprintln!("[run_psyop] loading config");
    let cfg = config::load();
    let psyop_dir = config::psyops_dir().join(name);
    let config_path = psyop_dir.join("psyop.json");

    if !config_path.exists() {
        return Err(error::Error::PsyopNotFound(config_path.display().to_string()));
    }

    let data = std::fs::read_to_string(&config_path)?;
    let psyop: crate::psyop::PsyOp = serde_json::from_str(&data)?;
    psyop.validate()?;

    eprintln!("[run_psyop] resolving commit SHA");
    let repo = git2::Repository::open(&psyop_dir)?;
    let head = repo.head()?.peel_to_commit()?;
    let commit_sha = head.id().to_string();

    let target_count = psyop.stages.first()
        .and_then(|s| s.count)
        .unwrap_or(100) as usize;
    let now = chrono::Utc::now();

    let db = crate::db::Db::open()?;

    let already_queued = db.count_queued(name)?;
    let shortfall = target_count.saturating_sub(already_queued);
    eprintln!("[run_psyop] target={target_count} queued={already_queued} shortfall={shortfall}");

    if shortfall > 0 {
        eprintln!("[run_psyop] spawning playwright");
        let mut pw = crate::playwright::Playwright::spawn()?;

        eprintln!("[run_psyop] opening tabs for {} queries", psyop.queries.len());
        let states = pw.open_tabs(&psyop.queries)?;
        eprintln!("[run_psyop] tab states: {states:?}");
        for (query, state) in &states {
            if state == "unexpected" {
                return Err(error::Error::Playwright(format!("unexpected page state for query \"{query}\"")));
            }
        }

        eprintln!("[run_psyop] scraping tweets (need={shortfall})");
        let mut collected = 0;
        while collected < shortfall {
            eprintln!("[run_psyop] next_tweet (collected={collected}/{shortfall})");
            let Some((tweet, query)) = pw.next_tweet()? else {
                eprintln!("[run_psyop] next_tweet returned None — done");
                break
            };
            eprintln!("[run_psyop] got tweet id={} query={} likes={}", tweet.id, query, tweet.likes);

            let validation = crate::psyop::valid_for_psyop(&psyop, &tweet.created, tweet.likes, &now);
            if !validation.valid {
                eprintln!("[run_psyop] tweet invalid: {:?}", validation.reason);
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

        eprintln!("[run_psyop] closing playwright");
        pw.close()?;
    }

    // Score the target_count oldest queued posts only
    let posts = db.get_oldest_queued(name, target_count)?;
    let scored_count = posts.len();
    eprintln!("[run_psyop] scoring {scored_count} posts");
    let mut scored_posts: Vec<crate::score::ScoredPost> = Vec::new();
    if !posts.is_empty() {
        scored_posts = crate::score::score(&psyop, posts)?;
        let ids: Vec<String> = scored_posts.iter().map(|s| s.post.id.clone()).collect();
        let scores: Vec<f64> = scored_posts.iter().map(|s| s.score).collect();
        db.finish_posts(&ids, &scores)?;
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
