//! Master X dev-account / billing-account setup. The chrome
//! extension captures credentials from `console.x.com` and ships
//! them via the native messaging port; the host writes them to
//! `~/.psychological-operations/billing.json`. Per-psyop OAuth
//! (next commit) reads that file to drive the user-context flow.

pub mod config;
pub mod setup;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Open chromium against the master billing-account profile.
    /// User signs into x.com / configures console.x.com / clicks
    /// the extension to save credentials to billing.json.
    Setup,
}

impl Commands {
    pub async fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::Setup => setup::run().await,
        }
    }
}
