use clap::{Args, Subcommand};
use objectiveai::functions::inventions::{
    ParamsState, ParamsStateOrRemoteCommitOptional,
    state::{AlphaScalarState, AlphaVectorState, Params},
};

use crate::input;

#[derive(Args)]
pub struct InventionParams {
    /// Function name
    #[arg(long)]
    pub name: String,
    /// Specification/prompt for the invention
    #[arg(long)]
    pub spec: String,
    /// Nesting depth (0 for leaf-only)
    #[arg(long, default_value = "0")]
    pub depth: u64,
    /// Minimum branch width
    #[arg(long, default_value = "2")]
    pub min_branch_width: u64,
    /// Maximum branch width
    #[arg(long, default_value = "3")]
    pub max_branch_width: u64,
    /// Minimum leaf width (tasks per leaf)
    #[arg(long, default_value = "1")]
    pub min_leaf_width: u64,
    /// Maximum leaf width (tasks per leaf)
    #[arg(long, default_value = "5")]
    pub max_leaf_width: u64,
}

impl InventionParams {
    fn into_params(self) -> Params {
        Params {
            depth: self.depth,
            min_branch_width: self.min_branch_width,
            max_branch_width: self.max_branch_width,
            min_leaf_width: self.min_leaf_width,
            max_leaf_width: self.max_leaf_width,
            name: self.name,
            spec: self.spec,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Invent a scalar function for scoring individual posts
    AlphaScalar {
        #[command(flatten)]
        params: InventionParams,
        /// Agent reference (e.g. favorite=name or remote=github,owner=x,...)
        #[arg(long)]
        agent: String,
        /// Seed for deterministic mock responses
        #[arg(long)]
        seed: Option<i64>,
    },
    /// Invent a vector function for ranking posts
    AlphaVector {
        #[command(flatten)]
        params: InventionParams,
        /// Agent reference (e.g. favorite=name or remote=github,owner=x,...)
        #[arg(long)]
        agent: String,
        /// Seed for deterministic mock responses
        #[arg(long)]
        seed: Option<i64>,
    },
    /// Invent from existing state (remote reference)
    Remote {
        /// State reference (e.g. remote=mock,name=inv-good-sl)
        #[arg(long)]
        state: String,
        /// Agent reference
        #[arg(long)]
        agent: String,
        /// Seed for deterministic mock responses
        #[arg(long)]
        seed: Option<i64>,
    },
}

impl Commands {
    pub fn handle(self) -> Result<crate::Output, crate::error::Error> {
        match self {
            Commands::AlphaScalar { params, agent, seed } => {
                let p = params.into_params();
                let state = ParamsStateOrRemoteCommitOptional::Inline(
                    ParamsState::AlphaScalar(AlphaScalarState {
                        params: p,
                        input_schema: Some(input::scalar_input_schema()),
                    }),
                );
                run_invention(&state, &agent, seed)
            }
            Commands::AlphaVector { params, agent, seed } => {
                let p = params.into_params();
                let state = ParamsStateOrRemoteCommitOptional::Inline(
                    ParamsState::AlphaVector(AlphaVectorState {
                        params: p,
                        input_schema: Some(input::vector_input_schema()),
                    }),
                );
                run_invention(&state, &agent, seed)
            }
            Commands::Remote { state, agent, seed } => {
                // Fetch the remote state
                let fetched = fetch_state(&state)?;
                let filled = fill_schema_if_missing(fetched)?;
                let state = ParamsStateOrRemoteCommitOptional::Inline(filled);
                run_invention(&state, &agent, seed)
            }
        }
    }
}

/// Fetch a remote invention state via the CLI.
fn fetch_state(ref_str: &str) -> Result<ParamsState, crate::error::Error> {
    let output = std::process::Command::new("objectiveai")
        .args(["functions", "inventions", "state", "get", "--path", ref_str])
        .stderr(std::process::Stdio::inherit())
        .output()?;

    if !output.status.success() {
        return Err(crate::error::Error::ObjectiveAiCli("failed to fetch invention state".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let state: ParamsState = serde_json::from_str(stdout.trim())?;
    Ok(state)
}

/// Fill input_schema if it's missing, using our post schema.
fn fill_schema_if_missing(state: ParamsState) -> Result<ParamsState, crate::error::Error> {
    match state {
        ParamsState::AlphaScalar(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::scalar_input_schema());
            }
            Ok(ParamsState::AlphaScalar(s))
        }
        ParamsState::AlphaVector(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::vector_input_schema());
            }
            Ok(ParamsState::AlphaVector(s))
        }
        ParamsState::AlphaScalarBranch(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::scalar_input_schema());
            }
            Ok(ParamsState::AlphaScalarBranch(s))
        }
        ParamsState::AlphaScalarLeaf(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::scalar_input_schema());
            }
            Ok(ParamsState::AlphaScalarLeaf(s))
        }
        ParamsState::AlphaVectorBranch(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::vector_input_schema());
            }
            Ok(ParamsState::AlphaVectorBranch(s))
        }
        ParamsState::AlphaVectorLeaf(mut s) => {
            if s.input_schema.is_none() {
                s.input_schema = Some(input::vector_input_schema());
            }
            Ok(ParamsState::AlphaVectorLeaf(s))
        }
    }
}

/// Shell out to objectiveai CLI to run the invention.
fn run_invention(
    state: &ParamsStateOrRemoteCommitOptional,
    agent: &str,
    seed: Option<i64>,
) -> Result<crate::Output, crate::error::Error> {
    let state_json = serde_json::to_string(state)?;

    let mut args = vec![
        "functions".to_string(),
        "inventions".to_string(),
        "recursive".to_string(),
        "create".to_string(),
        "remote".to_string(),
        "--state-inline".to_string(),
        state_json,
        "--agent".to_string(),
        agent.to_string(),
    ];

    if let Some(s) = seed {
        args.push("--seed".to_string());
        args.push(s.to_string());
    }

    let output = std::process::Command::new("objectiveai")
        .args(&args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()?;

    if !output.success() {
        return Err(crate::error::Error::ObjectiveAiCli("invention failed".into()));
    }

    Ok(crate::Output::Empty)
}
