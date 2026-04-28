//! Pause-and-wait-for-the-user intervention used by `scrapes run` when a
//! typed search lands on an unexpected page state (login wall, captcha,
//! checkpoint, etc.).
//!
//! With the single-session sequential scraper there is at most one paused
//! scrape at any moment — the per-scrape port file naming is still
//! meaningful, since it tells the operator which scrape's filter is
//! currently waiting.
//!
//! Each invocation:
//!   1. Spawns a one-shot TCP listener on `127.0.0.1:0`.
//!   2. Writes two port files atomically:
//!        - `~/.psychological-operations/agent-<pid>.port` — legacy, by-PID
//!        - `~/.psychological-operations/agent-scrape-<name>.port` — by-name
//!   3. Prints the prompt to stderr (intervention messages are *local*
//!      operator UX — they intentionally do NOT fan out to the configured
//!      notification destinations like discord/telegram/http).
//!   4. Waits up to `timeout` for `agent reply --scrape <name>` to connect
//!      and send a line. The reply text itself is informational; the user
//!      resolves the page by interacting with the visible Chrome window.
//!      On reply the caller re-types the failing query into the search bar.
//!   5. Cleans up both port files via a `Drop` guard so a panicking caller
//!      doesn't leave stale files behind.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use crate::config::Config;
use crate::scrape::Scrape;

fn agent_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".psychological-operations")
}

pub fn pid_port_file(pid: u32) -> PathBuf {
    agent_dir().join(format!("agent-{pid}.port"))
}

pub fn scrape_port_file(name: &str) -> PathBuf {
    agent_dir().join(format!("agent-scrape-{name}.port"))
}

/// Glob-style helper used by `agent list` to enumerate active interventions.
pub fn agent_dir_path() -> PathBuf {
    agent_dir()
}

/// Owns the two port files; deletes both on drop.
struct PortFileGuard {
    pid_path: PathBuf,
    scrape_path: PathBuf,
}

impl PortFileGuard {
    fn write(pid: u32, scrape_name: &str, port: u16) -> std::io::Result<Self> {
        let dir = agent_dir();
        std::fs::create_dir_all(&dir)?;
        let pid_path = pid_port_file(pid);
        let scrape_path = scrape_port_file(scrape_name);
        std::fs::write(&pid_path, port.to_string())?;
        std::fs::write(&scrape_path, port.to_string())?;
        Ok(Self { pid_path, scrape_path })
    }
}

impl Drop for PortFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.pid_path);
        let _ = std::fs::remove_file(&self.scrape_path);
    }
}

/// Outcome of waiting on a single intervention attempt.
pub enum InterventionOutcome {
    /// The user replied. The text they sent is included for diagnostics.
    Reply(String),
    /// `timeout` elapsed before any client connected.
    Timeout,
}

/// Resolve `(timeout_secs, max_attempts)` for a scrape from per-scrape
/// overrides, falling back to the global config defaults.
pub fn resolve_limits(cfg: &Config, scrape_name: &str, commit_sha: &str) -> (u64, u64) {
    let overrides = cfg.scrapes.get(scrape_name);
    let timeout = overrides
        .and_then(|o| o.agent_timeout_for(commit_sha))
        .unwrap_or(cfg.agent_timeout);
    let max_attempts = overrides
        .and_then(|o| o.agent_max_attempts_for(commit_sha))
        .unwrap_or(cfg.agent_max_attempts);
    (timeout, max_attempts)
}

/// Run one intervention attempt: write port files, print the prompt to
/// stderr, wait up to `timeout_secs` for a single reply on the listener,
/// return the outcome. Intervention prompts intentionally do not fan out
/// to remote notification destinations — they're local operator UX.
pub async fn await_one(
    scrape_name: &str,
    _commit_sha: &str,
    _scrape: &Scrape,
    prompt: &str,
    timeout_secs: u64,
) -> Result<InterventionOutcome, crate::error::Error> {
    let listener = TcpListener::bind("127.0.0.1:0").await
        .map_err(|e| crate::error::Error::Other(format!("intervention bind failed: {e}")))?;
    let port = listener.local_addr()
        .map_err(|e| crate::error::Error::Other(format!("intervention addr failed: {e}")))?
        .port();

    let pid = std::process::id();
    let _guard = PortFileGuard::write(pid, scrape_name, port)
        .map_err(|e| crate::error::Error::Other(format!("intervention port file write failed: {e}")))?;

    eprintln!("{prompt}");

    let timeout = Duration::from_secs(timeout_secs);
    match tokio::time::timeout(timeout, listener.accept()).await {
        Err(_) => Ok(InterventionOutcome::Timeout),
        Ok(Err(e)) => Err(crate::error::Error::Other(format!("intervention accept failed: {e}"))),
        Ok(Ok((stream, _addr))) => {
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            // Best-effort read of one line; if the client closes without
            // sending one, we still treat it as a "go" signal.
            let _ = reader.read_line(&mut line).await;
            let body = line.trim().to_string();
            let _ = write_half.write_all(b"ok\n").await;
            let _ = write_half.shutdown().await;
            Ok(InterventionOutcome::Reply(body))
        }
    }
}
