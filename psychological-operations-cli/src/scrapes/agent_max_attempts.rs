use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Get this scrape's agent_max_attempts override (or `null` if unset).
    Get {
        name: String,
    },
    /// Set this scrape's agent_max_attempts override.
    Set {
        name: String,
        value: u64,
    },
    /// Remove this scrape's agent_max_attempts override (fall back to global).
    Unset {
        name: String,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { name } => {
                let cfg = crate::config::load();
                let v = cfg.scrapes.get(&name).and_then(|s| s.agent_max_attempts);
                Ok(crate::Output::ConfigGet(serde_json::to_string(&v)?))
            }
            Commands::Set { name, value } => {
                let mut cfg = crate::config::load();
                cfg.scrapes.entry(name).or_default().agent_max_attempts = Some(value);
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Unset { name } => {
                let mut cfg = crate::config::load();
                if let Some(entry) = cfg.scrapes.get_mut(&name) {
                    entry.agent_max_attempts = None;
                    if entry.is_empty() {
                        cfg.scrapes.remove(&name);
                    }
                }
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
