use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Get current max attempts
    Get,
    /// Set max attempts
    Set { value: String },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        let mut cfg = crate::config::load();
        match self {
            Commands::Get => Ok(crate::Output::ConfigGet(
                serde_json::to_string(&cfg.agent_max_attempts)?,
            )),
            Commands::Set { value } => {
                cfg.agent_max_attempts = value.parse()
                    .map_err(|_| crate::error::Error::Other("invalid number".into()))?;
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
