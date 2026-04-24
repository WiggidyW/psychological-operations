use clap::Subcommand;

use crate::notifications::destinations::Destination;

#[derive(Subcommand)]
pub enum Commands {
    /// Get all per-scrape notifications, or one by index
    Get {
        name: String,
        index: Option<usize>,
    },
    /// Add a notification target to this scrape (JSON string)
    Add {
        name: String,
        json: String,
    },
    /// Remove a notification target by index from this scrape
    Del {
        name: String,
        index: usize,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { name, index } => {
                let cfg = crate::config::load();
                let list = cfg.scrapes.get(&name)
                    .map(|s| s.notifications.clone())
                    .unwrap_or_default();
                match index {
                    Some(i) => {
                        let entry = list.get(i)
                            .ok_or_else(|| crate::error::Error::Other(format!("no notification at index {i}")))?;
                        Ok(crate::Output::ConfigGet(serde_json::to_string(entry)?))
                    }
                    None => Ok(crate::Output::ConfigGet(serde_json::to_string(&list)?)),
                }
            }
            Commands::Add { name, json } => {
                let parsed: Destination = serde_json::from_str(&json)?;
                let mut cfg = crate::config::load();
                cfg.scrapes.entry(name).or_default().notifications.push(parsed);
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Del { name, index } => {
                let mut cfg = crate::config::load();
                {
                    let entry = cfg.scrapes.get_mut(&name)
                        .ok_or_else(|| crate::error::Error::Other(format!("no scrape config entry for \"{name}\"")))?;
                    if index >= entry.notifications.len() {
                        return Err(crate::error::Error::Other(format!("no notification at index {index}")));
                    }
                    entry.notifications.remove(index);
                }
                if cfg.scrapes.get(&name).is_some_and(|e| e.is_empty()) {
                    cfg.scrapes.remove(&name);
                }
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
