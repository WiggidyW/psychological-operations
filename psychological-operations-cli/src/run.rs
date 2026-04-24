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

    let _ = (commit_sha, name);
    // TODO: psyop run is being rewired around the new (scrape, tags,
    // sources) model. The scrape side now lives in `crate::scrape`; the
    // psyop here will pull tagged posts from the DB via `psyop.sources`,
    // run a single function execution, and persist scores. Left unbuilt
    // here so the rest of the crate keeps compiling while the wiring is
    // in flight.
    unimplemented!("psyop run is being rewired around the new sources/tags model");
    #[allow(unreachable_code)]
    let scored_posts: Vec<crate::score::ScoredPost> = Vec::new();
    #[allow(unreachable_code)]
    let output: Vec<&crate::score::ScoredPost> = scored_posts.iter().collect();

    let mut destinations = cfg.notifications.clone();
    if let Some(per_psyop) = cfg.psyops.get(name) {
        destinations.extend(per_psyop.notifications.iter().cloned());
    }
    config::notifications::destinations::notify(
        &destinations,
        name,
        &psyop,
        &output,
    ).await;

    Ok(())
}
