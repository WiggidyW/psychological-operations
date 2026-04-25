use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::intervention;

/// Where to find the listener: by-PID (legacy) or by-scrape-name (new).
pub enum Address {
    Pid(u32),
    Scrape(String),
}

impl Address {
    fn port_file(&self) -> PathBuf {
        match self {
            Address::Pid(pid) => intervention::pid_port_file(*pid),
            Address::Scrape(name) => intervention::scrape_port_file(name),
        }
    }
}

pub async fn send_reply(addr: Address, message: &str) -> Result<(), crate::error::Error> {
    let port_file = addr.port_file();
    if !port_file.exists() {
        return Err(crate::error::Error::Other(format!(
            "no agent waiting for input (port file not found: {})", port_file.display()
        )));
    }

    let port_str = tokio::fs::read_to_string(&port_file).await?;
    let port: u16 = port_str.trim().parse()
        .map_err(|_| crate::error::Error::Other("invalid port in agent port file".into()))?;

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).await?;
    stream.write_all(message.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    stream.shutdown().await?;

    // Read and print output until the server closes the connection
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 { break; }
        tokio::io::stdout().write_all(&buf[..n]).await?;
        tokio::io::stdout().flush().await?;
    }

    // Listener owns its port files via a Drop guard, so we don't remove
    // them here; just trying to is harmless if the listener is gone.
    Ok(())
}
