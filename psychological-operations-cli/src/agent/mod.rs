pub mod reply;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Send a message to a detached agent and reattach to its output
    Reply {
        pid: u32,
        message: String,
    },
}

impl Commands {
    pub async fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Reply { pid, message } => {
                reply::send_reply(pid, &message).await?;
                Ok(crate::Output::Empty)
            }
        }
    }
}
