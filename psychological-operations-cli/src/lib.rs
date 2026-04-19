pub mod error;
pub mod config;
pub mod db;
pub mod psyop;
pub mod input;
pub mod publish;
pub mod playwright;
pub mod playwright_binary;
pub mod score;
mod run;
mod agent;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "psychological-operations")]
#[command(about = "Agentic X scraper with ObjectiveAI scoring pipeline")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a psyop
    Run {
        #[command(flatten)]
        args: run::RunArgs,
    },
    /// Interact with a running agent intervention
    Agent {
        #[command(subcommand)]
        command: agent::Commands,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: config::Commands,
    },
}

pub enum Output {
    ConfigGet(String),
    ConfigSet,
    Empty,
}

impl std::fmt::Display for Output {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Output::ConfigGet(s) => write!(f, "{s}"),
            Output::ConfigSet => write!(f, "ok"),
            Output::Empty => Ok(()),
        }
    }
}

pub async fn run() -> Result<Output, error::Error> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { args } => args.handle().await,
        Commands::Agent { command } => command.handle().await,
        Commands::Config { command } => command.handle(),
    }
}
