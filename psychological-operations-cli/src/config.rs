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

/// Notification destinations + flags that apply to one psyop. Used both as
/// the `base` of a `PsyopOverrides` and as the value of each commit-specific
/// entry under `commits`.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PsyopConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<Destination>,
    /// `Some(true)`  → disabled, `Some(false)` → forced enabled,
    /// `None`        → inherit from the next layer (base, then default
    /// behaviour, which is enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ScrapeConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notifications: Vec<Destination>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_max_attempts: Option<u64>,
}

impl PsyopConfig {
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty() && self.disabled.is_none()
    }
}

impl ScrapeConfig {
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
            && self.disabled.is_none()
            && self.agent_timeout.is_none()
            && self.agent_max_attempts.is_none()
    }
}

/// Two-level overrides for a psyop: a `base` that applies to every commit
/// of that psyop, plus an optional `commits` map keyed by SHA. When
/// resolving a value for a specific commit, the commit-level entry shadows
/// `base`. For `notifications`, the rule is replace-or-fall-back (never
/// merged); for scalar fields the commit-level wins only if it's `Some(_)`.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PsyopOverrides {
    #[serde(default, skip_serializing_if = "PsyopConfig::is_empty")]
    pub base: PsyopConfig,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commits: BTreeMap<String, PsyopConfig>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ScrapeOverrides {
    #[serde(default, skip_serializing_if = "ScrapeConfig::is_empty")]
    pub base: ScrapeConfig,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commits: BTreeMap<String, ScrapeConfig>,
}

impl PsyopOverrides {
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && self.commits.is_empty()
    }

    /// Notifications that apply at `commit_sha`. If a commit-level entry
    /// exists with non-empty notifications, those are used exclusively;
    /// otherwise the base notifications are used. Never a concatenation.
    pub fn notifications_for(&self, commit_sha: &str) -> &[Destination] {
        if let Some(c) = self.commits.get(commit_sha) {
            if !c.notifications.is_empty() {
                return &c.notifications;
            }
        }
        &self.base.notifications
    }

    pub fn disabled_for(&self, commit_sha: &str) -> bool {
        if let Some(c) = self.commits.get(commit_sha) {
            if let Some(d) = c.disabled { return d; }
        }
        self.base.disabled.unwrap_or(false)
    }
}

impl ScrapeOverrides {
    pub fn is_empty(&self) -> bool {
        self.base.is_empty() && self.commits.is_empty()
    }

    pub fn notifications_for(&self, commit_sha: &str) -> &[Destination] {
        if let Some(c) = self.commits.get(commit_sha) {
            if !c.notifications.is_empty() {
                return &c.notifications;
            }
        }
        &self.base.notifications
    }

    pub fn disabled_for(&self, commit_sha: &str) -> bool {
        if let Some(c) = self.commits.get(commit_sha) {
            if let Some(d) = c.disabled { return d; }
        }
        self.base.disabled.unwrap_or(false)
    }

    pub fn agent_timeout_for(&self, commit_sha: &str) -> Option<u64> {
        if let Some(c) = self.commits.get(commit_sha) {
            if let Some(v) = c.agent_timeout { return Some(v); }
        }
        self.base.agent_timeout
    }

    pub fn agent_max_attempts_for(&self, commit_sha: &str) -> Option<u64> {
        if let Some(c) = self.commits.get(commit_sha) {
            if let Some(v) = c.agent_max_attempts { return Some(v); }
        }
        self.base.agent_max_attempts
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
    /// Seconds to wait between spawning consecutive scrape tasks in
    /// `scrapes run`. Staggers Chrome opens against X's IP-level rate
    /// limits when many scrapes start at once.
    #[serde(default = "default_scraper_spawn_delay_secs")]
    pub scraper_spawn_delay_secs: u64,
    /// Global notification destinations — fire on every psyop run.
    #[serde(default)]
    pub notifications: Vec<Destination>,
    /// Per-psyop overrides keyed by psyop name. Each entry has a `base`
    /// applied to every commit, plus an optional `commits` map that can
    /// shadow base values for a specific SHA.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub psyops: BTreeMap<String, PsyopOverrides>,
    /// Per-scrape overrides keyed by scrape name. Same shape as `psyops`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub scrapes: BTreeMap<String, ScrapeOverrides>,
}

fn default_agent_timeout() -> u64 { 180 }
fn default_agent_max_attempts() -> u64 { 3 }
fn default_scraper_spawn_delay_secs() -> u64 { 10 }

impl Default for Config {
    fn default() -> Self {
        Self {
            agent_timeout: default_agent_timeout(),
            agent_max_attempts: default_agent_max_attempts(),
            scraper_spawn_delay_secs: default_scraper_spawn_delay_secs(),
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
