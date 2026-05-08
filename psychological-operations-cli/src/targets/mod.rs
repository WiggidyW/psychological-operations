pub mod destinations;

use clap::Subcommand;

use destinations::Destination;

#[derive(Subcommand)]
pub enum Commands {
    /// Get all global targets, or one by index
    Get {
        index: Option<usize>,
    },
    /// Add a global target (JSON string)
    Add {
        json: String,
    },
    /// Remove a global target by index
    Del {
        index: usize,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { index } => {
                let cfg = crate::config::load();
                match index {
                    Some(i) => {
                        let entry = cfg.targets.get(i)
                            .ok_or_else(|| crate::error::Error::Other(format!("no target at index {i}")))?;
                        Ok(crate::Output::ConfigGet(serde_json::to_string(entry)?))
                    }
                    None => Ok(crate::Output::ConfigGet(serde_json::to_string(&cfg.targets)?)),
                }
            }
            Commands::Add { json } => {
                let parsed: Destination = serde_json::from_str(&json)?;
                let mut cfg = crate::config::load();
                cfg.targets.push(parsed);
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Del { index } => {
                let mut cfg = crate::config::load();
                if index >= cfg.targets.len() {
                    return Err(crate::error::Error::Other(format!("no target at index {index}")));
                }
                cfg.targets.remove(index);
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
