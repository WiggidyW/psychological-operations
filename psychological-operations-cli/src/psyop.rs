use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Stage {
    pub function: serde_json::Value,
    pub profile: serde_json::Value,
    pub strategy: serde_json::Value,
    pub count: Option<u64>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PsyOp {
    pub agent: serde_json::Value,
    pub queries: Vec<String>,
    pub count: Option<u64>,
    pub threshold: Option<f64>,
    pub max_age: Option<u64>,
    pub min_likes: Option<u64>,
    pub stages: Vec<Stage>,
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
