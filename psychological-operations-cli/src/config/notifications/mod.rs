pub mod destinations;

use clap::{Args, Subcommand};

use destinations::Destination;

/// Mutually exclusive `--psyop` / `--scrape` selectors that pick which
/// per-name notification list to operate on. Both unset = the global list.
#[derive(Args, Clone)]
#[group(multiple = false)]
pub struct Target {
    /// Target a specific psyop's notifications instead of the global list
    #[arg(long)]
    psyop: Option<String>,
    /// Target a specific scrape's notifications instead of the global list
    #[arg(long)]
    scrape: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Get all notifications or one by index
    Get {
        index: Option<usize>,
        #[command(flatten)]
        target: Target,
    },
    /// Add a notification target (JSON string)
    Add {
        json: String,
        #[command(flatten)]
        target: Target,
    },
    /// Remove a notification target by index
    Del {
        index: usize,
        #[command(flatten)]
        target: Target,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { index, target } => {
                let list = read_notifications(&target)?;
                match index {
                    Some(i) => {
                        let entry = list.get(i)
                            .ok_or_else(|| crate::error::Error::Other(format!("no notification at index {i}")))?;
                        Ok(crate::Output::ConfigGet(serde_json::to_string(entry)?))
                    }
                    None => Ok(crate::Output::ConfigGet(serde_json::to_string(&list)?)),
                }
            }
            Commands::Add { json, target } => {
                let parsed: Destination = serde_json::from_str(&json)?;
                mutate_notifications(&target, |list| {
                    list.push(parsed);
                    Ok(())
                })?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Del { index, target } => {
                mutate_notifications(&target, |list| {
                    if index >= list.len() {
                        return Err(crate::error::Error::Other(format!("no notification at index {index}")));
                    }
                    list.remove(index);
                    Ok(())
                })?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}

fn read_notifications(target: &Target) -> Result<Vec<Destination>, crate::error::Error> {
    let cfg = crate::config::load();
    if let Some(name) = &target.psyop {
        return Ok(cfg.psyops.get(name).map(|p| p.notifications.clone()).unwrap_or_default());
    }
    if let Some(name) = &target.scrape {
        return Ok(cfg.scrapes.get(name).map(|s| s.notifications.clone()).unwrap_or_default());
    }
    Ok(cfg.notifications)
}

fn mutate_notifications<F>(target: &Target, f: F) -> Result<(), crate::error::Error>
where
    F: FnOnce(&mut Vec<Destination>) -> Result<(), crate::error::Error>,
{
    let mut cfg = crate::config::load();
    if let Some(name) = &target.psyop {
        let entry = cfg.psyops.entry(name.clone()).or_default();
        f(&mut entry.notifications)?;
        if entry.notifications.is_empty() && !entry.disabled {
            cfg.psyops.remove(name);
        }
    } else if let Some(name) = &target.scrape {
        let entry = cfg.scrapes.entry(name.clone()).or_default();
        f(&mut entry.notifications)?;
        if entry.notifications.is_empty() && !entry.disabled {
            cfg.scrapes.remove(name);
        }
    } else {
        f(&mut cfg.notifications)?;
    }
    crate::config::save(&cfg)?;
    Ok(())
}
