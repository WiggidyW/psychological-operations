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
