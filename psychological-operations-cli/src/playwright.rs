use serde::Deserialize;
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
}

#[derive(Debug, Deserialize)]
struct RunQueryResponse {
    state: String,
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
        let binary_path = crate::playwright_binary::extract()?;
        let mut child = Command::new(&binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

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

    /// Open the single shared Chrome session at x.com. Must be called once
    /// before any `run_query`.
    pub async fn start_session(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "start_session" })).await?;
        Ok(())
    }

    /// Type `query` into the in-page X search bar (with human-paced jitter)
    /// and click the Latest tab. Returns the post-search page state:
    /// `"results"`, `"empty"`, or `"unexpected"`.
    pub async fn run_query(&mut self, query: &str) -> Result<String, crate::error::Error> {
        let resp = self.send(&serde_json::json!({
            "cmd": "run_query",
            "query": query,
        })).await?;
        let parsed: RunQueryResponse = serde_json::from_value(resp)?;
        Ok(parsed.state)
    }

    pub async fn next_tweet(&mut self) -> Result<Option<TweetData>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "next_tweet" })).await?;
        let parsed: NextTweetResponse = serde_json::from_value(resp)?;
        if parsed.done {
            return Ok(None);
        }
        Ok(parsed.tweet)
    }

    /// Mark the current query as done. Next `run_query` will start fresh.
    pub async fn close_query(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "close_query" })).await?;
        Ok(())
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

    pub async fn get_page_url(&mut self) -> Result<Option<String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "get_page_url" })).await?;
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
