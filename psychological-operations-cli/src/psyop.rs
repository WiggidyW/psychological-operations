use serde::{Deserialize, Serialize};
use objectiveai::functions::{
    FullInlineFunctionOrRemoteCommitOptional,
    FullInlineFunction,
    AlphaInlineFunction,
    InlineFunction,
    InlineProfileOrRemoteCommitOptional,
};
use objectiveai::functions::executions::request::Strategy;
use objectiveai::agent::InlineAgentBaseWithFallbacksOrRemoteCommitOptional;

use crate::config::notifications::destinations::Destination;

#[derive(Debug, Serialize, Deserialize)]
pub struct Stage {
    pub function: FullInlineFunctionOrRemoteCommitOptional,
    pub profile: InlineProfileOrRemoteCommitOptional,
    pub strategy: Strategy,
    pub count: Option<u64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PsyOp {
    pub agent: InlineAgentBaseWithFallbacksOrRemoteCommitOptional,
    pub queries: Vec<String>,
    pub count: Option<u64>,
    pub threshold: Option<f64>,
    pub max_age: Option<u64>,
    pub min_likes: Option<u64>,
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub notifications: Vec<Destination>,
}

/// Read a psyop's JSON definition from disk.
pub fn load(name: &str) -> Result<PsyOp, crate::error::Error> {
    let path = crate::config::psyops_dir().join(name).join("psyop.json");
    if !path.exists() {
        return Err(crate::error::Error::PsyopNotFound(path.display().to_string()));
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Write a psyop's JSON definition back to disk (pretty-printed).
pub fn save(name: &str, psyop: &PsyOp) -> Result<(), crate::error::Error> {
    let path = crate::config::psyops_dir().join(name).join("psyop.json");
    let json = serde_json::to_string_pretty(psyop)?;
    std::fs::write(&path, json + "\n")?;
    Ok(())
}

impl PsyOp {
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        if self.stages.is_empty() {
            return Err(crate::error::Error::InvalidPsyop("stages must not be empty".into()));
        }
        let first = &self.stages[0];
        if first.count.is_none() {
            return Err(crate::error::Error::InvalidPsyop("first stage must have a count".into()));
        }
        if first.threshold.is_some() {
            return Err(crate::error::Error::InvalidPsyop("first stage must not have a threshold".into()));
        }
        Ok(())
    }
}

pub struct ValidationResult {
    pub valid: bool,
    pub reason: Option<&'static str>,
}

pub fn valid_for_psyop(psyop: &PsyOp, created: &str, likes: u64, now: &chrono::DateTime<chrono::Utc>) -> ValidationResult {
    if let Some(max_age) = psyop.max_age {
        if let Ok(created_time) = chrono::DateTime::parse_from_rfc3339(created) {
            let age_seconds = (*now - created_time.with_timezone(&chrono::Utc)).num_seconds();
            if age_seconds > max_age as i64 {
                return ValidationResult { valid: false, reason: Some("max_age") };
            }
        }
    }
    if let Some(min_likes) = psyop.min_likes {
        if likes < min_likes {
            return ValidationResult { valid: false, reason: Some("min_likes") };
        }
    }
    ValidationResult { valid: true, reason: None }
}

/// Determine if a function is a vector function.
/// If the function is remote, it must be fetched first (caller resolves it).
pub fn is_vector_function(function: &FullInlineFunction) -> bool {
    match function {
        FullInlineFunction::Alpha(alpha) => matches!(alpha, AlphaInlineFunction::Vector(_)),
        FullInlineFunction::Standard(standard) => matches!(standard, InlineFunction::Vector { .. }),
    }
}
