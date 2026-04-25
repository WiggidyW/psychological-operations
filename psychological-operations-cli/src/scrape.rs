use serde::{Deserialize, Serialize};

use objectiveai::agent::InlineAgentBaseWithFallbacksOrRemoteCommitOptional;

/// One search filter for a Scrape. The query maps to an X.com `/search?q=…`
/// URL; per-filter min_* values combine with the Scrape's root-level min_*
/// values by taking the greater of the two when validating each tweet at
/// scrape time.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Filter {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_likes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_retweets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_replies: Option<u64>,
}

impl Filter {
    /// Build the X.com search URL this filter targets.
    pub fn url(&self) -> String {
        let q_enc = urlencoding::encode(&self.query);
        format!("https://x.com/search?q={q_enc}&src=typed_query&f=live")
    }
}

/// A scrape job. Defines what to scrape (filters), how many tweets to
/// collect, scrape-time validation thresholds, and the tags to apply to
/// every tweet stored by this run. Psyops then operate on these tags.
#[derive(Debug, Serialize, Deserialize)]
pub struct Scrape {
    /// Agent used for intervention when scraping fails (e.g. login wall,
    /// unexpected page state).
    pub agent: InlineAgentBaseWithFallbacksOrRemoteCommitOptional,
    pub filters: Vec<Filter>,
    /// Tags applied to every tweet stored by this scrape run. Must contain
    /// at least one tag.
    pub tags: Vec<String>,
    /// How many tweets to collect per run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Reject tweets whose `created` is older than this many seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u64>,
    /// Reject tweets whose `created` is younger than this many seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_age: Option<u64>,
    /// Root-level engagement floors. Per-filter values are combined by max.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_likes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_retweets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_replies: Option<u64>,
}

/// Read a scrape's JSON definition from disk.
pub fn load(name: &str) -> Result<Scrape, crate::error::Error> {
    let path = crate::config::scrapes_dir().join(name).join("scrape.json");
    if !path.exists() {
        return Err(crate::error::Error::PsyopNotFound(path.display().to_string()));
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Write a scrape's JSON definition back to disk (pretty-printed).
pub fn save(name: &str, scrape: &Scrape) -> Result<(), crate::error::Error> {
    let path = crate::config::scrapes_dir().join(name).join("scrape.json");
    let json = serde_json::to_string_pretty(scrape)?;
    std::fs::write(&path, json + "\n")?;
    Ok(())
}

impl Scrape {
    pub fn validate(&self) -> Result<(), crate::error::Error> {
        if self.filters.is_empty() {
            return Err(crate::error::Error::InvalidPsyop("filters must not be empty".into()));
        }
        if self.tags.is_empty() {
            return Err(crate::error::Error::InvalidPsyop("tags must not be empty".into()));
        }
        Ok(())
    }
}

pub struct ValidationResult {
    pub valid: bool,
    pub reason: Option<&'static str>,
}

/// Combine root and per-filter minimums by taking the greater of the two.
fn effective_min(root: Option<u64>, per_filter: Option<u64>) -> u64 {
    root.unwrap_or(0).max(per_filter.unwrap_or(0))
}

/// Per-tweet validation against a Scrape + the filter that produced it.
pub fn valid_for_scrape(
    scrape: &Scrape,
    filter: &Filter,
    created: &str,
    likes: u64,
    retweets: u64,
    replies: u64,
    now: &chrono::DateTime<chrono::Utc>,
) -> ValidationResult {
    if let Ok(created_time) = chrono::DateTime::parse_from_rfc3339(created) {
        let age_seconds = (*now - created_time.with_timezone(&chrono::Utc)).num_seconds();
        if let Some(max_age) = scrape.max_age {
            if age_seconds > max_age as i64 {
                return ValidationResult { valid: false, reason: Some("max_age") };
            }
        }
        if let Some(min_age) = scrape.min_age {
            if age_seconds < min_age as i64 {
                return ValidationResult { valid: false, reason: Some("min_age") };
            }
        }
    }
    let min_likes = effective_min(scrape.min_likes, filter.min_likes);
    if likes < min_likes {
        return ValidationResult { valid: false, reason: Some("min_likes") };
    }
    let min_retweets = effective_min(scrape.min_retweets, filter.min_retweets);
    if retweets < min_retweets {
        return ValidationResult { valid: false, reason: Some("min_retweets") };
    }
    let min_replies = effective_min(scrape.min_replies, filter.min_replies);
    if replies < min_replies {
        return ValidationResult { valid: false, reason: Some("min_replies") };
    }
    ValidationResult { valid: true, reason: None }
}
