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
    #[serde(default)]
    pub invert: bool,
}

/// A psyop selects scored input by tag — every post stored under any of
/// these tags becomes a candidate. Per-filter min_* values combine with the
/// PsyOp's root-level min_* values by taking the greater of the two when
/// deciding which tagged posts to actually score.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filter {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_likes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_retweets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_replies: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PsyOp {
    pub agent: InlineAgentBaseWithFallbacksOrRemoteCommitOptional,
    pub filters: Vec<Filter>,
    pub count: Option<u64>,
    pub threshold: Option<f64>,
    pub max_age: Option<u64>,
    pub min_likes: Option<u64>,
    pub min_retweets: Option<u64>,
    pub min_replies: Option<u64>,
    /// If `false`, scored posts are sent to the function with an empty
    /// `images` array regardless of what was scraped. Defaults to `true`.
    #[serde(default = "default_true")]
    pub images: bool,
    /// If `false`, scored posts are sent to the function with an empty
    /// `videos` array regardless of what was scraped. Defaults to `true`.
    #[serde(default = "default_true")]
    pub videos: bool,
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub notifications: Vec<Destination>,
}

fn default_true() -> bool { true }

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
        if self.filters.is_empty() {
            return Err(crate::error::Error::InvalidPsyop("filters must not be empty".into()));
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

/// Combine root and per-filter minimums by taking the greater of the two.
/// Root acts as a global floor; per-filter can raise but not lower it.
fn effective_min(root: Option<u64>, per_filter: Option<u64>) -> u64 {
    root.unwrap_or(0).max(per_filter.unwrap_or(0))
}

pub fn valid_for_psyop(
    psyop: &PsyOp,
    filter: &Filter,
    created: &str,
    likes: u64,
    retweets: u64,
    replies: u64,
    now: &chrono::DateTime<chrono::Utc>,
) -> ValidationResult {
    if let Some(max_age) = psyop.max_age {
        if let Ok(created_time) = chrono::DateTime::parse_from_rfc3339(created) {
            let age_seconds = (*now - created_time.with_timezone(&chrono::Utc)).num_seconds();
            if age_seconds > max_age as i64 {
                return ValidationResult { valid: false, reason: Some("max_age") };
            }
        }
    }
    let min_likes = effective_min(psyop.min_likes, filter.min_likes);
    if likes < min_likes {
        return ValidationResult { valid: false, reason: Some("min_likes") };
    }
    let min_retweets = effective_min(psyop.min_retweets, filter.min_retweets);
    if retweets < min_retweets {
        return ValidationResult { valid: false, reason: Some("min_retweets") };
    }
    let min_replies = effective_min(psyop.min_replies, filter.min_replies);
    if replies < min_replies {
        return ValidationResult { valid: false, reason: Some("min_replies") };
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
