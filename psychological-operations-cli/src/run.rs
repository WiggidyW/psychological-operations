use clap::{Parser, Subcommand};

use crate::agent;
use crate::config;
use crate::error;
use crate::invent;
use crate::notifications;
use crate::psyops;
use crate::scrapes;
use crate::agent_timeout;
use crate::agent_max_attempts;

#[derive(Parser)]
#[command(name = "psychological-operations")]
#[command(about = "Agentic X scraper with ObjectiveAI scoring pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage psyops (list/get/enable/disable/publish/run/notifications)
    Psyops {
        #[command(subcommand)]
        command: psyops::Commands,
    },
    /// Manage scrapes (list/get/enable/disable/publish/notifications/agent overrides)
    Scrapes {
        #[command(subcommand)]
        command: scrapes::Commands,
    },
    /// Global notification destinations
    Notifications {
        #[command(subcommand)]
        command: notifications::Commands,
    },
    /// Global agent intervention timeout (seconds)
    AgentTimeout {
        #[command(subcommand)]
        command: agent_timeout::Commands,
    },
    /// Global agent intervention max retry attempts
    AgentMaxAttempts {
        #[command(subcommand)]
        command: agent_max_attempts::Commands,
    },
    /// Invent a function for scoring posts
    Invent {
        #[command(subcommand)]
        command: invent::Commands,
    },
    /// Interact with a running agent intervention
    Agent {
        #[command(subcommand)]
        command: agent::Commands,
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
        Commands::Psyops { command } => command.handle().await,
        Commands::Scrapes { command } => command.handle(),
        Commands::Notifications { command } => command.handle(),
        Commands::AgentTimeout { command } => command.handle(),
        Commands::AgentMaxAttempts { command } => command.handle(),
        Commands::Invent { command } => command.handle(),
        Commands::Agent { command } => command.handle().await,
    }
}

pub async fn run_psyop(name: &str) -> Result<(), error::Error> {
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
    // sources) model.
    unimplemented!("psyop run is being rewired around the new sources/tags model");
    #[allow(unreachable_code)]
    let scored_posts: Vec<crate::score::ScoredPost> = Vec::new();
    #[allow(unreachable_code)]
    let output: Vec<&crate::score::ScoredPost> = scored_posts.iter().collect();

    let mut destinations = cfg.notifications.clone();
    if let Some(per_psyop) = cfg.psyops.get(name) {
        destinations.extend(per_psyop.notifications_for(&commit_sha).iter().cloned());
    }
    crate::notifications::destinations::notify(
        &destinations,
        name,
        &psyop,
        &output,
    ).await;

    Ok(())
}
