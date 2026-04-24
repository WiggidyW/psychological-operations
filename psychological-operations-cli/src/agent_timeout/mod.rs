use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Get current agent timeout
    Get,
    /// Set agent timeout
    Set { value: String },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        let mut cfg = crate::config::load();
        match self {
            Commands::Get => Ok(crate::Output::ConfigGet(
                serde_json::to_string(&cfg.agent_timeout)?,
            )),
            Commands::Set { value } => {
                cfg.agent_timeout = value.parse()
                    .map_err(|_| crate::error::Error::Other("invalid number".into()))?;
                crate::config::save(&cfg)?;
                Ok(crate::Output::ConfigSet)
            }
        }
    }
}
