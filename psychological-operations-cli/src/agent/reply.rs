use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn port_file_path(pid: u32) -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".psychological-operations")
        .join(format!("agent-{pid}.port"))
}

pub async fn send_reply(pid: u32, message: &str) -> Result<(), crate::error::Error> {
    let port_file = port_file_path(pid);
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

    // Clean up port file
    let _ = tokio::fs::remove_file(&port_file).await;

    Ok(())
}
