use clap::{Parser, Subcommand};

use crate::agent;
use crate::error;
use crate::invent;
use crate::notifications;
use crate::psyops;
use crate::scrapes;
use crate::agent_timeout;
use crate::agent_max_attempts;

#[derive(Parser)]
#[command(name = "psychological-operations")]
#[command(about = "Agentic X scraper with ObjectiveAI scoring pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage psyops (list/get/enable/disable/publish/run/notifications)
    Psyops {
        #[command(subcommand)]
        command: psyops::Commands,
    },
    /// Manage scrapes (list/get/enable/disable/publish/run/notifications/agent overrides)
    Scrapes {
        #[command(subcommand)]
        command: scrapes::Commands,
    },
    /// Global notification destinations
    Notifications {
        #[command(subcommand)]
        command: notifications::Commands,
    },
    /// Global agent intervention timeout (seconds)
    AgentTimeout {
        #[command(subcommand)]
        command: agent_timeout::Commands,
    },
    /// Global agent intervention max retry attempts
    AgentMaxAttempts {
        #[command(subcommand)]
        command: agent_max_attempts::Commands,
    },
    /// Invent a function for scoring posts
    Invent {
        #[command(subcommand)]
        command: invent::Commands,
    },
    /// Interact with a running agent intervention
    Agent {
        #[command(subcommand)]
        command: agent::Commands,
    },
}

pub enum Output {
    ConfigGet(String),
    ConfigSet,
    Api(String),
    Empty,
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::ConfigGet(s) => write!(f, "{s}"),
            Output::ConfigSet => write!(f, "ok"),
            Output::Api(s) => write!(f, "{s}"),
            Output::Empty => Ok(()),
        }
    }
}

pub async fn run() -> Result<Output, error::Error> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Psyops { command } => command.handle().await,
        Commands::Scrapes { command } => command.handle(),
        Commands::Notifications { command } => command.handle(),
        Commands::AgentTimeout { command } => command.handle(),
        Commands::AgentMaxAttempts { command } => command.handle(),
        Commands::Invent { command } => command.handle(),
        Commands::Agent { command } => command.handle().await,
    }
}
