use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::notifications::NotificationConfig;

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

pub fn db_path() -> PathBuf {
    base_dir().join("data.db")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_agent_timeout")]
    pub agent_timeout: u64,
    #[serde(default = "default_agent_max_attempts")]
    pub agent_max_attempts: u64,
    #[serde(default)]
    pub notifications: Vec<NotificationConfig>,
}

fn default_agent_timeout() -> u64 { 180 }
fn default_agent_max_attempts() -> u64 { 3 }

impl Default for Config {
    fn default() -> Self {
        Self {
            agent_timeout: default_agent_timeout(),
            agent_max_attempts: default_agent_max_attempts(),
            notifications: Vec::new(),
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
