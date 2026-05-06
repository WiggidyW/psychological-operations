use serde::{Deserialize, Serialize};

/// Per-tweet eligibility filter. Shared by `Query` and `ForYou` —
/// both attach an `Option<Filter>` so a source with no filter accepts
/// every tweet that the source itself produces.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Filter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_likes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_retweets: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_replies: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_impressions: Option<u64>,
    /// Reject tweets whose `created` is older than this many seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u64>,
    /// Reject tweets whose `created` is younger than this many seconds.
    /// Useful for letting engagement settle before scoring.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_age: Option<u64>,
}
