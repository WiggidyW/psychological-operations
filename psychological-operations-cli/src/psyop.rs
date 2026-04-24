use serde::{Deserialize, Serialize};
use objectiveai::functions::{
    FullInlineFunctionOrRemoteCommitOptional,
    FullInlineFunction,
    AlphaInlineFunction,
    InlineFunction,
    InlineProfileOrRemoteCommitOptional,
};
use objectiveai::functions::executions::request::Strategy;

/// A psyop pulls input by tag — every stored post under any of its
/// sources' tags becomes a candidate. Per-source thresholds further
/// narrow which tagged posts are eligible for scoring.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Source {
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_likes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_retweets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_replies: Option<u64>,
    /// Reject tweets whose `created` is older than this many seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u64>,
    /// Reject tweets whose `created` is younger than this many seconds.
    /// Useful for letting engagement settle before scoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_age: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PsyOp {
    pub sources: Vec<Source>,
    pub function: FullInlineFunctionOrRemoteCommitOptional,
    pub profile: InlineProfileOrRemoteCommitOptional,
    pub strategy: Strategy,
    /// Optional cap on how many tagged posts to feed into the function.
    /// `None` means score every eligible post.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    #[serde(default)]
    pub invert: bool,
    /// If `false`, scored posts are sent to the function with an empty
    /// `images` array regardless of what was scraped. Defaults to `true`.
    #[serde(default = "default_true")]
    pub images: bool,
    /// If `false`, scored posts are sent to the function with an empty
    /// `videos` array regardless of what was scraped. Defaults to `true`.
    #[serde(default = "default_true")]
    pub videos: bool,
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
        if self.sources.is_empty() {
            return Err(crate::error::Error::InvalidPsyop("sources must not be empty".into()));
        }
        Ok(())
    }
}

pub struct ValidationResult {
    pub valid: bool,
    pub reason: Option<&'static str>,
}

/// Per-tweet score-time eligibility check against a single Source.
pub fn valid_for_source(
    source: &Source,
    created: &str,
    likes: u64,
    retweets: u64,
    replies: u64,
    now: &chrono::DateTime<chrono::Utc>,
) -> ValidationResult {
    if let Ok(created_time) = chrono::DateTime::parse_from_rfc3339(created) {
        let age_seconds = (*now - created_time.with_timezone(&chrono::Utc)).num_seconds();
        if let Some(max_age) = source.max_age {
            if age_seconds > max_age as i64 {
                return ValidationResult { valid: false, reason: Some("max_age") };
            }
        }
        if let Some(min_age) = source.min_age {
            if age_seconds < min_age as i64 {
                return ValidationResult { valid: false, reason: Some("min_age") };
            }
        }
    }
    if let Some(min_likes) = source.min_likes {
        if likes < min_likes {
            return ValidationResult { valid: false, reason: Some("min_likes") };
        }
    }
    if let Some(min_retweets) = source.min_retweets {
        if retweets < min_retweets {
            return ValidationResult { valid: false, reason: Some("min_retweets") };
        }
    }
    if let Some(min_replies) = source.min_replies {
        if replies < min_replies {
            return ValidationResult { valid: false, reason: Some("min_replies") };
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
