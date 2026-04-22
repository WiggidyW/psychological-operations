pub mod destinations;

use clap::Subcommand;

use destinations::Destination;

#[derive(Subcommand)]
pub enum Commands {
    /// Get all notifications or one by index
    Get {
        index: Option<usize>,
        /// Target a specific psyop's notifications instead of the global list
        #[arg(long)]
        psyop: Option<String>,
    },
    /// Add a notification target (JSON string)
    Add {
        json: String,
        /// Target a specific psyop's notifications instead of the global list
        #[arg(long)]
        psyop: Option<String>,
    },
    /// Remove a notification target by index
    Del {
        index: usize,
        /// Target a specific psyop's notifications instead of the global list
        #[arg(long)]
        psyop: Option<String>,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Get { index, psyop } => {
                let list = read_notifications(psyop.as_deref())?;
                match index {
                    Some(i) => {
                        let entry = list.get(i)
                            .ok_or_else(|| crate::error::Error::Other(format!("no notification at index {i}")))?;
                        Ok(crate::Output::ConfigGet(serde_json::to_string(entry)?))
                    }
                    None => Ok(crate::Output::ConfigGet(serde_json::to_string(&list)?)),
                }
            }
            Commands::Add { json, psyop } => {
                let parsed: Destination = serde_json::from_str(&json)?;
                mutate_notifications(psyop.as_deref(), |list| {
                    list.push(parsed);
                    Ok(())
                })?;
                Ok(crate::Output::ConfigSet)
            }
            Commands::Del { index, psyop } => {
                mutate_notifications(psyop.as_deref(), |list| {
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

fn read_notifications(psyop: Option<&str>) -> Result<Vec<Destination>, crate::error::Error> {
    match psyop {
        Some(name) => Ok(crate::psyop::load(name)?.notifications),
        None => Ok(crate::config::load().notifications),
    }
}

fn mutate_notifications<F>(psyop: Option<&str>, f: F) -> Result<(), crate::error::Error>
where
    F: FnOnce(&mut Vec<Destination>) -> Result<(), crate::error::Error>,
{
    match psyop {
        Some(name) => {
            let mut p = crate::psyop::load(name)?;
            f(&mut p.notifications)?;
            crate::psyop::save(name, &p)?;
        }
        None => {
            let mut cfg = crate::config::load();
            f(&mut cfg.notifications)?;
            crate::config::save(&cfg)?;
        }
    }
    Ok(())
}
