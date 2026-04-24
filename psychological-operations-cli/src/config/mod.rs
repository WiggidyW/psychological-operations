pub mod agent_timeout;
pub mod agent_max_attempts;
pub mod notifications;
pub mod psyops;

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use notifications::destinations::Destination;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum Commands {
    /// Agent intervention timeout in seconds
    AgentTimeout {
        #[command(subcommand)]
        command: agent_timeout::Commands,
    },
    /// Agent intervention max retry attempts
    AgentMaxAttempts {
        #[command(subcommand)]
        command: agent_max_attempts::Commands,
    },
    /// Manage notification targets
    Notifications {
        #[command(subcommand)]
        command: notifications::Commands,
    },
    /// Manage psyops (enable/disable/list)
    Psyops {
        #[command(subcommand)]
        command: psyops::Commands,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::AgentTimeout { command } => command.handle(),
            Commands::AgentMaxAttempts { command } => command.handle(),
            Commands::Notifications { command } => command.handle(),
            Commands::Psyops { command } => command.handle(),
        }
    }
}

// ---------------------------------------------------------------------------
// Config I/O
// ---------------------------------------------------------------------------

fn base_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".psychological-operations")
}

pub fn config_path() -> PathBuf {
    base_dir().join("config.json")
}

pub fn psyops_dir() -> PathBuf {
    base_dir().join("psyops")
}

pub fn scrapes_dir() -> PathBuf {
    base_dir().join("scrapes")
}

pub fn db_path() -> PathBuf {
    base_dir().join("data.db")
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PsyopConfig {
    /// Per-psyop notification destinations. Stored here rather than inside
    /// `psyop.json` so shared psyop definitions don't have to carry
    /// user-specific webhook URLs / tokens / auth headers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<Destination>,
    /// When `true`, automatic execution skips this psyop.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
}

fn is_false(b: &bool) -> bool { !*b }

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout: u64,
    #[serde(default = "default_agent_max_attempts")]
    pub agent_max_attempts: u64,
    /// Global notification destinations — fire on every psyop run.
    #[serde(default)]
    pub notifications: Vec<Destination>,
    /// Per-psyop overrides keyed by psyop name. Holds notification
    /// destinations and the enable/disable flag.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub psyops: BTreeMap<String, PsyopConfig>,
}

fn default_agent_timeout() -> u64 { 180 }
fn default_agent_max_attempts() -> u64 { 3 }

impl Default for Config {
    fn default() -> Self {
        Self {
            agent_timeout: default_agent_timeout(),
            agent_max_attempts: default_agent_max_attempts(),
            notifications: Vec::new(),
            psyops: BTreeMap::new(),
        }
    }
}

pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        return Config::default();
    }
    let data = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save(config: &Config) -> Result<(), crate::error::Error> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json + "\n")?;
    Ok(())
}
