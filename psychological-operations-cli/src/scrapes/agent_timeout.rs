use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Get this scrape's agent_timeout override (or `null` if unset).
    /// `--commit <sha>` reads the commit-specific layer instead of base.
    Get {
        name: String,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Set this scrape's agent_timeout override (in seconds).
    Set {
        name: String,
        value: u64,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Remove this scrape's agent_timeout override (fall back to next layer).
    Unset {
        name: String,
        #[arg(long)]
        commit: Option<String>,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { name, commit } => {
                let cfg = crate::config::load();
                let v: Option<u64> = cfg.scrapes.get(&name).and_then(|o| match commit.as_deref() {
                    Some(sha) => o.commits.get(sha).and_then(|c| c.agent_timeout),
                    None => o.base.agent_timeout,
                });
                Ok(crate::Output::ConfigGet(serde_json::to_string(&v)?))
            }
            Commands::Set { name, value, commit } => {
                let mut cfg = crate::config::load();
                let overrides = cfg.scrapes.entry(name).or_default();
                match commit.as_deref() {
                    Some(sha) => {
                        overrides.commits.entry(sha.to_string()).or_default().agent_timeout = Some(value);
                    }
                    None => {
                        overrides.base.agent_timeout = Some(value);
                    }
                }
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Unset { name, commit } => {
                let mut cfg = crate::config::load();
                if let Some(overrides) = cfg.scrapes.get_mut(&name) {
                    match commit.as_deref() {
                        Some(sha) => {
                            if let Some(c) = overrides.commits.get_mut(sha) {
                                c.agent_timeout = None;
                                if c.is_empty() {
                                    overrides.commits.remove(sha);
                                }
                            }
                        }
                        None => { overrides.base.agent_timeout = None; }
                    }
                    if overrides.is_empty() {
                        cfg.scrapes.remove(&name);
                    }
                }
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
