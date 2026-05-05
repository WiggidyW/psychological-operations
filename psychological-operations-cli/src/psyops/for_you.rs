use serde::{Deserialize, Serialize};

use super::psyop::Filter;

/// Personalized "For You" timeline input on a psyop. Ingestion
/// mechanism is TBD — the X v2 API has no public algorithmic-feed
/// endpoint; the most likely candidate is the chronological home
/// timeline `/2/users/{id}/timelines/reverse_chronological`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ForYou {
    /// Higher = preferred when the deduped union is truncated by
    /// `PsyOp.max_posts`. `None` ranks below every `Some(_)`,
    /// regardless of the `Some` value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u64>,
    /// Per-tweet eligibility applied after fetch.
    #[serde(default)]
    pub filter: Filter,
}
