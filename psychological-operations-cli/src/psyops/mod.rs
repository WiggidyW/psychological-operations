pub mod notifications;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::psyop::PsyOp;

#[derive(Subcommand)]
pub enum Commands {
    /// List all psyops on disk (every dir under `psyops/` that has both
    /// `psyop.json` and `.git`). Each entry includes its `enabled` state
    /// and current commit. `--enabled` and `--disabled` are mutually
    /// exclusive filters.
    List {
        #[arg(long, conflicts_with = "disabled")]
        enabled: bool,
        #[arg(long)]
        disabled: bool,
    },
    /// Print the on-disk JSON definition of a psyop.
    Get {
        name: String,
    },
    /// Mark a psyop as enabled (clear the disabled flag).
    Enable {
        name: String,
    },
    /// Mark a psyop as disabled.
    Disable {
        name: String,
    },
    /// Publish a psyop definition (writes psyop.json + commits in its repo).
    Publish {
        #[command(flatten)]
        args: PublishArgs,
    },
    /// Run a psyop end-to-end (score eligible tagged posts, notify).
    Run {
        name: String,
    },
    /// Manage per-psyop notification destinations.
    Notifications {
        #[command(subcommand)]
        command: notifications::Commands,
    },
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct PsyopSource {
    /// Inline JSON psyop definition
    #[arg(long)]
    psyop_inline: Option<String>,
    /// Path to a JSON file containing the psyop definition
    #[arg(long)]
    psyop_file: Option<std::path::PathBuf>,
}

#[derive(Args)]
pub struct PublishArgs {
    /// Psyop name
    #[arg(long)]
    pub name: String,
    #[command(flatten)]
    pub source: PsyopSource,
    /// Commit message
    #[arg(long)]
    pub message: String,
}

#[derive(Serialize)]
struct PsyopEntry {
    name: String,
    enabled: bool,
    commit_sha: String,
}

impl Commands {
    pub async fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::List { enabled, disabled } => list(enabled, disabled),
            Commands::Get { name } => get(&name),
            Commands::Enable { name } => enable(&name),
            Commands::Disable { name } => disable(&name),
            Commands::Publish { args } => publish(args),
            Commands::Run { name } => crate::run_psyop(&name).await.map(|_| crate::Output::Empty),
            Commands::Notifications { command } => command.handle(),
        }
    }
}

fn list(enabled: bool, disabled: bool) -> Result<crate::Output, crate::error::Error> {
    let cfg = crate::config::load();
    let dir = crate::config::psyops_dir();
    let mut entries: Vec<PsyopEntry> = Vec::new();
    if dir.exists() {
        for ent in std::fs::read_dir(&dir)? {
            let ent = ent?;
            let path = ent.path();
            if !path.is_dir()
                || !path.join("psyop.json").exists()
                || !path.join(".git").exists()
            {
                continue;
            }
            let Some(name) = ent.file_name().to_str().map(|s| s.to_string()) else { continue };
            let is_enabled = !cfg.psyops.get(&name).map(|p| p.disabled).unwrap_or(false);
            if enabled && !is_enabled { continue; }
            if disabled && is_enabled { continue; }
            let commit_sha = (|| -> Result<String, git2::Error> {
                let repo = git2::Repository::open(&path)?;
                let head = repo.head()?.peel_to_commit()?;
                Ok(head.id().to_string())
            })().unwrap_or_default();
            entries.push(PsyopEntry { name, enabled: is_enabled, commit_sha });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(crate::Output::ConfigGet(serde_json::to_string(&entries)?))
}

fn get(name: &str) -> Result<crate::Output, crate::error::Error> {
    let psyop = crate::psyop::load(name)?;
    Ok(crate::Output::ConfigGet(serde_json::to_string(&psyop)?))
}

fn enable(name: &str) -> Result<crate::Output, crate::error::Error> {
    let mut cfg = crate::config::load();
    if let Some(entry) = cfg.psyops.get_mut(name) {
        entry.disabled = false;
        if entry.is_empty() {
            cfg.psyops.remove(name);
        }
    }
    crate::config::save(&cfg)?;
    Ok(crate::Output::ConfigSet)
}

fn disable(name: &str) -> Result<crate::Output, crate::error::Error> {
    let mut cfg = crate::config::load();
    cfg.psyops.entry(name.to_string()).or_default().disabled = true;
    crate::config::save(&cfg)?;
    Ok(crate::Output::ConfigSet)
}

fn publish(args: PublishArgs) -> Result<crate::Output, crate::error::Error> {
    let psyop: PsyOp = if let Some(inline) = args.source.psyop_inline {
        serde_json::from_str(&inline)?
    } else if let Some(path) = args.source.psyop_file {
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data)?
    } else {
        unreachable!("clap group ensures one is set")
    };
    psyop.validate()?;
    let dir = crate::config::psyops_dir().join(&args.name);
    let json = serde_json::to_string_pretty(&psyop)? + "\n";
    let sha = crate::publish::publish_file(&dir, "psyop.json", &json, &args.message)?;
    Ok(crate::Output::Api(sha))
}
