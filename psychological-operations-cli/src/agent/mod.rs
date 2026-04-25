pub mod intervention;
pub mod reply;

use clap::{Args, Subcommand};
use serde::Serialize;

#[derive(Subcommand)]
pub enum Commands {
    /// Send a message to a waiting scrape intervention. Address by `--scrape
    /// <name>` (preferred) or by positional `<pid>` (legacy).
    Reply {
        #[command(flatten)]
        target: ReplyTarget,
        message: String,
    },
    /// List active interventions waiting on a reply.
    List,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
pub struct ReplyTarget {
    /// Scrape name to address (preferred — stable across processes).
    #[arg(long)]
    scrape: Option<String>,
    /// PID of the waiting process (legacy address form).
    #[arg(long)]
    pid: Option<u32>,
}

#[derive(Serialize)]
struct InterventionEntry {
    kind: &'static str,
    name: Option<String>,
    pid: Option<u32>,
    port: u16,
}

impl Commands {
    pub async fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Reply { target, message } => {
                let addr = target.resolve()?;
                reply::send_reply(addr, &message).await?;
                Ok(crate::Output::Empty)
            }
            Commands::List => list(),
        }
    }
}

impl ReplyTarget {
    fn resolve(self) -> Result<reply::Address, crate::error::Error> {
        // clap's group(required=true, multiple=false) guarantees exactly one.
        if let Some(name) = self.scrape {
            return Ok(reply::Address::Scrape(name));
        }
        if let Some(pid) = self.pid {
            return Ok(reply::Address::Pid(pid));
        }
        unreachable!("clap group ensures one is set")
    }
}

fn list() -> Result<crate::Output, crate::error::Error> {
    let dir = intervention::agent_dir_path();
    let mut entries: Vec<InterventionEntry> = Vec::new();
    if dir.exists() {
        for ent in std::fs::read_dir(&dir)? {
            let ent = ent?;
            let path = ent.path();
            let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else { continue };
            if !file_name.starts_with("agent-") || !file_name.ends_with(".port") { continue }
            let stem = &file_name["agent-".len()..file_name.len() - ".port".len()];
            let port_str = std::fs::read_to_string(&path).unwrap_or_default();
            let Ok(port) = port_str.trim().parse::<u16>() else { continue };
            if let Some(name) = stem.strip_prefix("scrape-") {
                entries.push(InterventionEntry {
                    kind: "scrape",
                    name: Some(name.to_string()),
                    pid: None,
                    port,
                });
            } else if let Ok(pid) = stem.parse::<u32>() {
                entries.push(InterventionEntry {
                    kind: "pid",
                    name: None,
                    pid: Some(pid),
                    port,
                });
            }
        }
    }
    Ok(crate::Output::ConfigGet(serde_json::to_string(&entries)?))
}
