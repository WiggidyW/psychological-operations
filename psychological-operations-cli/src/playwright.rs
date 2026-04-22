use serde::Deserialize;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use crate::db::MediaUrl;

#[derive(Debug, Deserialize)]
pub struct TweetData {
    pub id: String,
    pub handle: String,
    pub text: String,
    pub images: Vec<MediaUrl>,
    pub videos: Vec<MediaUrl>,
    pub created: String,
    pub community: Option<String>,
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
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
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

    fn send(&mut self, cmd: &serde_json::Value) -> Result<serde_json::Value, crate::error::Error> {
        let line = serde_json::to_string(cmd)? + "\n";
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;

        let mut response = String::new();
        self.reader.read_line(&mut response)?;
        let value: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(err) = value.get("error").and_then(|e| e.as_str()) {
            return Err(crate::error::Error::Playwright(err.to_string()));
        }

        Ok(value)
    }

    pub fn open_tabs(&mut self, urls: &[String]) -> Result<std::collections::HashMap<String, String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({
            "cmd": "open_tabs",
            "urls": urls,
        }))?;
        let parsed: OpenTabsResponse = serde_json::from_value(resp)?;
        Ok(parsed.states)
    }

    pub fn next_tweet(&mut self) -> Result<Option<(TweetData, String)>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "next_tweet" }))?;
        let parsed: NextTweetResponse = serde_json::from_value(resp)?;
        if parsed.done {
            return Ok(None);
        }
        match (parsed.tweet, parsed.query) {
            (Some(tweet), Some(query)) => Ok(Some((tweet, query))),
            _ => Ok(None),
        }
    }

    pub fn close_query(&mut self, query: &str) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "close_query", "query": query }))?;
        Ok(())
    }

    pub fn has_open_tabs(&mut self) -> Result<bool, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "has_open_tabs" }))?;
        Ok(resp.get("open").and_then(|v| v.as_bool()).unwrap_or(false))
    }

    pub fn start_mcp(&mut self) -> Result<u16, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "start_mcp" }))?;
        let parsed: McpResponse = serde_json::from_value(resp)?;
        parsed.mcp_port.ok_or_else(|| crate::error::Error::Playwright("no mcp_port returned".into()))
    }

    pub fn stop_mcp(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "stop_mcp" }))?;
        Ok(())
    }

    pub fn install_browser(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "install_browser" }))?;
        Ok(())
    }

    pub fn get_page_url(&mut self, query: &str) -> Result<Option<String>, crate::error::Error> {
        let resp = self.send(&serde_json::json!({ "cmd": "get_page_url", "query": query }))?;
        let parsed: PageUrlResponse = serde_json::from_value(resp)?;
        Ok(parsed.url)
    }

    pub fn close(&mut self) -> Result<(), crate::error::Error> {
        self.send(&serde_json::json!({ "cmd": "close" }))?;
        let _ = self.child.wait();
        Ok(())
    }
}

impl Drop for Playwright {
    fn drop(&mut self) {
        let _ = self.send(&serde_json::json!({ "cmd": "close" }));
        let _ = self.child.wait();
    }
}
