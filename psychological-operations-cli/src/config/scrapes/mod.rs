use clap::Subcommand;
use serde::Serialize;

#[derive(Subcommand)]
pub enum Commands {
    /// Mark a scrape as enabled (clear the disabled flag).
    Enable {
        name: String,
    },
    /// Mark a scrape as disabled.
    Disable {
        name: String,
    },
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
            Commands::Enable { name } => {
                let mut cfg = crate::config::load();
                if let Some(entry) = cfg.scrapes.get_mut(&name) {
                    entry.disabled = false;
                    if entry.notifications.is_empty() {
                        cfg.scrapes.remove(&name);
                    }
                }
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Disable { name } => {
                let mut cfg = crate::config::load();
                cfg.scrapes.entry(name).or_default().disabled = true;
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::List { enabled, disabled } => {
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
                        let Some(name) = ent.file_name().to_str().map(|s| s.to_string()) else {
                            continue;
                        };
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
        }
    }
}
