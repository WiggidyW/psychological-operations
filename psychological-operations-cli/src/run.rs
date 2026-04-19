use clap::{Args, Parser, Subcommand};

use crate::agent;
use crate::config;
use crate::publish;
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
            return Err(error::Error::Playwright(format!("unexpected page state for query \"{query}\"")));
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
    config::notifications::destinations::notify(&cfg.notifications, &format!("PsyOp \"{name}\": scraped {collected} posts.")).await;

    Ok(())
}
