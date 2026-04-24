use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::notifications::destinations::Destination;

// ---------------------------------------------------------------------------
// Paths
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

// ---------------------------------------------------------------------------
// Per-name overrides
// ---------------------------------------------------------------------------

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

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ScrapeConfig {
    /// Per-scrape notification destinations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<Destination>,
    /// When `true`, automatic execution skips this scrape.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// Optional override of the global agent intervention timeout for this
    /// scrape. `None` means inherit `Config.agent_timeout`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_timeout: Option<u64>,
    /// Optional override of the global agent intervention max retry attempts
    /// for this scrape. `None` means inherit `Config.agent_max_attempts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_max_attempts: Option<u64>,
}

fn is_false(b: &bool) -> bool { !*b }

impl ScrapeConfig {
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
            && !self.disabled
            && self.agent_timeout.is_none()
            && self.agent_max_attempts.is_none()
    }
}

impl PsyopConfig {
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty() && !self.disabled
    }
}

// ---------------------------------------------------------------------------
// Config root
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout: u64,
    #[serde(default = "default_agent_max_attempts")]
    pub agent_max_attempts: u64,
    /// Global notification destinations — fire on every psyop run.
    #[serde(default)]
    pub notifications: Vec<Destination>,
    /// Per-psyop overrides keyed by psyop name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub psyops: BTreeMap<String, PsyopConfig>,
    /// Per-scrape overrides keyed by scrape name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scrapes: BTreeMap<String, ScrapeConfig>,
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
            scrapes: BTreeMap::new(),
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
