pub mod notifications;
pub mod agent_timeout;
pub mod agent_max_attempts;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::scrape::Scrape;

#[derive(Subcommand)]
pub enum Commands {
    /// List all scrapes on disk (every dir under `scrapes/` that has both
    /// `scrape.json` and `.git`). Each entry includes its `enabled` state
    /// and current commit. `--enabled` and `--disabled` are mutually
    /// exclusive filters.
    List {
        #[arg(long, conflicts_with = "disabled")]
        enabled: bool,
        #[arg(long)]
        disabled: bool,
    },
    /// Print the on-disk JSON definition of a scrape.
    Get {
        name: String,
    },
    /// Mark a scrape as enabled (clear the disabled flag).
    Enable {
        name: String,
    },
    /// Mark a scrape as disabled.
    Disable {
        name: String,
    },
    /// Publish a scrape definition (writes scrape.json + commits in its repo).
    Publish {
        #[command(flatten)]
        args: PublishArgs,
    },
    /// Manage per-scrape notification destinations.
    Notifications {
        #[command(subcommand)]
        command: notifications::Commands,
    },
    /// Per-scrape override of the global agent intervention timeout.
    AgentTimeout {
        #[command(subcommand)]
        command: agent_timeout::Commands,
    },
    /// Per-scrape override of the global agent intervention max retry attempts.
    AgentMaxAttempts {
        #[command(subcommand)]
        command: agent_max_attempts::Commands,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct ScrapeSource {
    /// Inline JSON scrape definition
    #[arg(long)]
    scrape_inline: Option<String>,
    /// Path to a JSON file containing the scrape definition
    #[arg(long)]
    scrape_file: Option<std::path::PathBuf>,
}

#[derive(Args)]
pub struct PublishArgs {
    /// Scrape name
    #[arg(long)]
    pub name: String,
    #[command(flatten)]
    pub source: ScrapeSource,
    /// Commit message
    #[arg(long)]
    pub message: String,
}

#[derive(Serialize)]
struct ScrapeEntry {
    name: String,
    enabled: bool,
    commit_sha: String,
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::List { enabled, disabled } => list(enabled, disabled),
            Commands::Get { name } => get(&name),
            Commands::Enable { name } => enable(&name),
            Commands::Disable { name } => disable(&name),
            Commands::Publish { args } => publish(args),
            Commands::Notifications { command } => command.handle(),
            Commands::AgentTimeout { command } => command.handle(),
            Commands::AgentMaxAttempts { command } => command.handle(),
        }
    }
}

fn list(enabled: bool, disabled: bool) -> Result<crate::Output, crate::error::Error> {
    let cfg = crate::config::load();
    let dir = crate::config::scrapes_dir();
    let mut entries: Vec<ScrapeEntry> = Vec::new();
    if dir.exists() {
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
            let is_enabled = !cfg.scrapes.get(&name).map(|s| s.disabled).unwrap_or(false);
            if enabled && !is_enabled { continue; }
            if disabled && is_enabled { continue; }
            let commit_sha = (|| -> Result<String, git2::Error> {
                let repo = git2::Repository::open(&path)?;
                let head = repo.head()?.peel_to_commit()?;
                Ok(head.id().to_string())
            })().unwrap_or_default();
            entries.push(ScrapeEntry { name, enabled: is_enabled, commit_sha });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(crate::Output::ConfigGet(serde_json::to_string(&entries)?))
}

fn get(name: &str) -> Result<crate::Output, crate::error::Error> {
    let scrape = crate::scrape::load(name)?;
    Ok(crate::Output::ConfigGet(serde_json::to_string(&scrape)?))
}

fn enable(name: &str) -> Result<crate::Output, crate::error::Error> {
    let mut cfg = crate::config::load();
    if let Some(entry) = cfg.scrapes.get_mut(name) {
        entry.disabled = false;
        if entry.is_empty() {
            cfg.scrapes.remove(name);
        }
    }
    crate::config::save(&cfg)?;
    Ok(crate::Output::ConfigSet)
}

fn disable(name: &str) -> Result<crate::Output, crate::error::Error> {
    let mut cfg = crate::config::load();
    cfg.scrapes.entry(name.to_string()).or_default().disabled = true;
    crate::config::save(&cfg)?;
    Ok(crate::Output::ConfigSet)
}

fn publish(args: PublishArgs) -> Result<crate::Output, crate::error::Error> {
    let scrape: Scrape = if let Some(inline) = args.source.scrape_inline {
        serde_json::from_str(&inline)?
    } else if let Some(path) = args.source.scrape_file {
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data)?
    } else {
        unreachable!("clap group ensures one is set")
    };
    scrape.validate()?;
    let dir = crate::config::scrapes_dir().join(&args.name);
    let json = serde_json::to_string_pretty(&scrape)? + "\n";
    let sha = crate::publish::publish_file(&dir, "scrape.json", &json, &args.message)?;
    Ok(crate::Output::Api(sha))
}
