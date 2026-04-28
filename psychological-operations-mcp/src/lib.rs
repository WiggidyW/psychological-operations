//! Psychological Operations MCP CLI library.
//!
//! Other crates can `use psychological_operations_mcp::{ConfigBuilder, run}` and
//! spawn the server in-process; the binary at `main.rs` is a thin wrapper
//! that reads `Config` from the environment and calls [`run`].

mod psychological_operations;
mod run;

pub use run::*;
