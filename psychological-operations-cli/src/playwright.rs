use serde::Deserialize;
use std::path::Path;
use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::db::MediaUrl;

#[derive(Debug, Deserialize)]
pub struct TweetData {
    pub id: String,
    pub handle: String,
    pub text: String,
    pub images: Vec<MediaUrl>,
    pub videos: Vec<MediaUrl>,
    pub created: String,
    pub likes: u64,
    #[serde(default)]
    pub retweets: u64,
    #[serde(default)]
    pub replies: u64,
}

#[derive(Debug, Deserialize)]
struct NextTweetResponse {
    done: bool,
    tweet: Option<TweetData>,
    query: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenTabsResponse {
    states: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct McpResponse {
    mcp_port: Option<u16>,
}

#[derive(Debug, Deserialize)]
struct PageUrlResponse {
    url: Option<String>,
}

pub struct Playwright {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl Playwright {
    pub fn spawn() -> Result<Self, crate::error::Error> {
        Self::spawn_inner(None)
    }

    /// Like `spawn`, but tells the playwright child to use `profile_dir` as
    /// the Chrome user-data directory (via `POPS_CHROME_DATA_DIR`) instead of
    /// the default shared `~/.psychological-operations/chrome-data`. Used by
    /// concurrent scrape runs that snapshot the base profile.
    pub fn spawn_with_profile(profile_dir: &Path) -> Result<Self, crate::error::Error> {
        Self::spawn_inner(Some(profile_dir))
    }

    fn spawn_inner(profile_dir: Option<&Path>) -> Result<Self, crate::error::Error> {
        let binary_path = crate::playwright_binary::extract()?;
        let mut cmd = Command::new(&binary_path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if let Some(dir) = profile_dir {
            cmd.env("POPS_CHROME_DATA_DIR", dir);
        }
        let mut child = cmd.spawn()?;

        let stdin = child.stdin.take().expect("failed to open stdin");
        let stdout = child.stdout.take().expect("failed to open stdout");
        let reader = BufReader::new(stdout);

        Ok(Self { child, stdin, reader })
    }

    async fn send(&mut self, cmd: &serde_json::Value) -> Result<serde_json::Value, crate::error::Error> {
        let line = serde_json::to_string(cmd)? + "\n";
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;

        let mut response = String::new();
        self.reader.read_line(&mut response).await?;
        let value: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
            return Err(crate::error::Error::Playwright(err.to_string()));
        }

        Ok(value)
    }

    pub async fn open_tabs(&mut self, urls: &[String]) -> Result<std::collections::HashMap<String, String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({
            "cmd": "open_tabs",
            "urls": urls,
        })).await?;
        let parsed: OpenTabsResponse = serde_json::from_value(resp)?;
        Ok(parsed.states)
    }

    /// Re-validate previously-unexpected URLs after agent intervention. The
    /// page handles for those URLs are kept open by the playwright side; this
    /// re-runs validation and promotes them into the scraping rotation if
    /// they now show results. Returns `url -> new state`.
    pub async fn retry_unexpected(&mut self, urls: &[String]) -> Result<std::collections::HashMap<String, String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({
            "cmd": "retry_unexpected",
            "urls": urls,
        })).await?;
        let parsed: OpenTabsResponse = serde_json::from_value(resp)?;
        Ok(parsed.states)
    }

    pub async fn next_tweet(&mut self) -> Result<Option<(TweetData, String)>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "next_tweet" })).await?;
        let parsed: NextTweetResponse = serde_json::from_value(resp)?;
        if parsed.done {
            return Ok(None);
        }
        match (parsed.tweet, parsed.query) {
            (Some(tweet), Some(query)) => Ok(Some((tweet, query))),
            _ => Ok(None),
        }
    }

    pub async fn close_query(&mut self, query: &str) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "close_query", "query": query })).await?;
        Ok(())
    }

    pub async fn has_open_tabs(&mut self) -> Result<bool, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "has_open_tabs" })).await?;
        Ok(resp.get("open").and_then(|v| v.as_bool()).unwrap_or(false))
    }

    pub async fn start_mcp(&mut self) -> Result<u16, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "start_mcp" })).await?;
        let parsed: McpResponse = serde_json::from_value(resp)?;
        parsed.mcp_port.ok_or_else(|| crate::error::Error::Playwright("no mcp_port returned".into()))
    }

    pub async fn stop_mcp(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "stop_mcp" })).await?;
        Ok(())
    }

    pub async fn install_browser(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "install_browser" })).await?;
        Ok(())
    }

    pub async fn get_page_url(&mut self, query: &str) -> Result<Option<String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "get_page_url", "query": query })).await?;
        let parsed: PageUrlResponse = serde_json::from_value(resp)?;
        Ok(parsed.url)
    }

    pub async fn close(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "close" })).await?;
        let _ = self.child.wait().await;
        Ok(())
    }
}

impl Drop for Playwright {
    fn drop(&mut self) {
        // Drop is sync; child is a tokio process but `start_kill` is sync.
        // Best-effort: signal the child to terminate. The parent's reaper
        // will collect it. We don't try to send the protocol "close" here
        // because that would require an async write — the happy path
        // already calls `close().await` before drop.
        let _ = self.child.start_kill();
    }
}
