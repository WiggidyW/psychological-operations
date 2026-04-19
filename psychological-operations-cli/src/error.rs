#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("psyop not found: {0}")]
    PsyopNotFound(String),
    #[error("playwright error: {0}")]
    Playwright(String),
    #[error("objectiveai cli error: {0}")]
    ObjectiveAiCli(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid psyop: {0}")]
    InvalidPsyop(String),
    #[error("stage {stage}: {message}")]
    Stage { stage: usize, message: String },
    #[error("{0}")]
    Other(String),
}
