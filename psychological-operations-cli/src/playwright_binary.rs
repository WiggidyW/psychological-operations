use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::path::PathBuf;

pub const PLAYWRIGHT_BINARY: &[u8] = include_bytes!(env!("PSYOPS_PLAYWRIGHT_BINARY_PATH"));

/// Extract the embedded playwright binary to a temp directory.
/// Uses content-hash caching so the binary is only written once per version.
pub fn extract() -> Result<PathBuf, std::io::Error> {
    // Hash: length + first 4KB + last 4KB
    let mut hasher = DefaultHasher::new();
    hasher.write_usize(PLAYWRIGHT_BINARY.len());
    hasher.write(&PLAYWRIGHT_BINARY[..PLAYWRIGHT_BINARY.len().min(4096)]);
    if PLAYWRIGHT_BINARY.len() > 4096 {
        hasher.write(&PLAYWRIGHT_BINARY[PLAYWRIGHT_BINARY.len() - 4096..]);
    }
    let hash = hasher.finish();

    let dir = std::env::temp_dir().join(format!("psyops-playwright-{hash:016x}"));
    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let path = dir.join(format!("psychological-operations-playwright{ext}"));

    if !path.exists() {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(&path, PLAYWRIGHT_BINARY)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    Ok(path)
}
