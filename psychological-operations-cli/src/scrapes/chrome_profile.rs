//! Per-scrape Chrome profile snapshots so concurrent scrapes don't fight
//! over the shared `chrome-data/` profile lock. Each run copies the base
//! profile out to `chrome-data-runs/<name>/`, runs against the copy, and
//! overwrites the base with the copy on exit. Last-finished wins; the user
//! has accepted that tradeoff.
//!
//! All public APIs are async on tokio::fs so big profile copies don't tie
//! up tokio worker threads. `Drop` falls back to std::fs because drop
//! itself can't be async — that path only runs if the caller didn't reach
//! `ProfileSession::finalize().await` (i.e. error or panic).

use std::collections::HashSet;
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::OnceLock;

use tokio::fs;
use tokio::sync::Mutex;

/// Single global mutex that serialises copy-back so two siblings finishing
/// simultaneously don't half-overwrite the base profile. Async so awaiting
/// it doesn't block the worker.
fn copyback_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn base_profile_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".psychological-operations")
        .join("chrome-data")
}

fn runs_root() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".psychological-operations")
        .join("chrome-data-runs")
}

pub fn working_dir(name: &str) -> PathBuf {
    runs_root().join(name)
}

/// Copy the base `chrome-data/` profile into `chrome-data-runs/<name>/`.
/// Skips Chrome's profile-lock files / Singleton sockets so the snapshot can
/// itself be opened by a fresh `launchPersistentContext`.
pub async fn copy_profile_out(name: &str) -> Result<PathBuf, crate::error::Error> {
    let base = base_profile_dir();
    let dest = working_dir(name);
    if path_exists(&dest).await {
        remove_dir_all_best_effort_async(&dest).await;
    }
    fs::create_dir_all(&dest).await?;
    if path_exists(&base).await {
        copy_dir_filtered_async(&base, &dest).await?;
    }
    Ok(dest)
}

/// Overwrite the base profile with the contents of the working copy. Files
/// in the base that no longer exist in the working copy are removed (so
/// Chrome session-state deletions propagate). Held under a process-global
/// async mutex so concurrent siblings don't tear each other.
pub async fn copy_profile_back(name: &str) -> Result<(), crate::error::Error> {
    let _guard = copyback_lock().lock().await;
    let base = base_profile_dir();
    let src = working_dir(name);
    if !path_exists(&src).await {
        return Ok(());
    }
    if !path_exists(&base).await {
        fs::create_dir_all(&base).await?;
    }
    sync_dir_async(&src, &base).await?;
    Ok(())
}

/// Delete the working copy. Best-effort: failures are ignored because
/// scrape-run cleanup shouldn't mask the actual run result.
pub async fn cleanup(name: &str) {
    let dir = working_dir(name);
    if path_exists(&dir).await {
        remove_dir_all_best_effort_async(&dir).await;
    }
}

/// `Drop` guard that runs `copy_profile_back` + `cleanup` when it goes out
/// of scope, so a panicking scrape still propagates session state and
/// removes its working copy. The happy path should call `finalize().await`
/// to do the copy-back asynchronously and disarm the guard.
pub struct ProfileSession {
    name: String,
    armed: bool,
}

impl ProfileSession {
    pub async fn begin(name: &str) -> Result<(Self, PathBuf), crate::error::Error> {
        let dir = copy_profile_out(name).await?;
        Ok((Self { name: name.to_string(), armed: true }, dir))
    }

    /// Async copy-back + cleanup. Disarms the Drop guard so it won't redo
    /// the work synchronously. Call this on the success path of run_scrape.
    pub async fn finalize(mut self) {
        if let Err(e) = copy_profile_back(&self.name).await {
            eprintln!("scrape \"{}\" copy-back failed: {e}", self.name);
        }
        cleanup(&self.name).await;
        self.armed = false;
    }
}

impl Drop for ProfileSession {
    fn drop(&mut self) {
        if !self.armed { return; }
        // Sync fallback for the panic/error path. Drop can't be async.
        if let Err(e) = sync_copy_profile_back_blocking(&self.name) {
            eprintln!("scrape \"{}\" copy-back failed (drop fallback): {e}", self.name);
        }
        sync_cleanup_blocking(&self.name);
    }
}

// ── async internals ───────────────────────────────────────────────────────

/// Names that Chrome creates as profile locks / IPC sockets. They must not
/// be present in a fresh snapshot or `launchPersistentContext` refuses to
/// start. On Windows there is no Singleton* set, but the `lockfile` ones
/// can still appear depending on Chrome version, so we filter on every OS.
fn is_lock_entry(file_name: &str) -> bool {
    matches!(
        file_name,
        "SingletonLock" | "SingletonCookie" | "SingletonSocket" | "lockfile" | "Singleton"
    ) || file_name.starts_with("Singleton")
}

async fn path_exists(p: &Path) -> bool {
    fs::try_exists(p).await.unwrap_or(false)
}

fn copy_dir_filtered_async<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy().into_owned();
            if is_lock_entry(&name_str) { continue; }
            let from = entry.path();
            let to = dst.join(&file_name);
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                fs::create_dir_all(&to).await?;
                copy_dir_filtered_async(&from, &to).await?;
            } else if ft.is_file() {
                // Copy may fail if Chrome has the file open — skip those rather
                // than aborting, since they're typically caches.
                if let Err(e) = fs::copy(&from, &to).await {
                    eprintln!("warning: skipping {}: {e}", from.display());
                }
            }
            // Symlinks are skipped — Chrome's profile shouldn't contain them.
        }
        Ok(())
    })
}

/// Mirror `src` → `dst` asynchronously: copy/overwrite everything in src
/// into dst, and delete anything in dst that isn't in src. Skips lock-style
/// entries on the src side so Chrome's running-instance markers don't leak
/// into the base profile.
fn sync_dir_async<'a>(
    src: &'a Path,
    dst: &'a Path,
) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut src_names: HashSet<OsString> = HashSet::new();
        let mut entries = fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy().into_owned();
            if is_lock_entry(&name_str) { continue; }
            src_names.insert(file_name.clone());
            let from = entry.path();
            let to = dst.join(&file_name);
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                if !path_exists(&to).await {
                    fs::create_dir_all(&to).await?;
                } else {
                    let meta = fs::metadata(&to).await?;
                    if meta.is_file() {
                        fs::remove_file(&to).await?;
                        fs::create_dir_all(&to).await?;
                    }
                }
                sync_dir_async(&from, &to).await?;
            } else if ft.is_file() {
                let to_is_dir = fs::metadata(&to).await.map(|m| m.is_dir()).unwrap_or(false);
                if to_is_dir {
                    remove_dir_all_best_effort_async(&to).await;
                }
                if let Err(e) = fs::copy(&from, &to).await {
                    eprintln!("warning: copy-back skipped {}: {e}", from.display());
                }
            }
        }
        if path_exists(dst).await {
            let mut dst_entries = fs::read_dir(dst).await?;
            while let Some(entry) = dst_entries.next_entry().await? {
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy().into_owned();
                if is_lock_entry(&name_str) { continue; }
                if src_names.contains(&file_name) { continue; }
                let path = entry.path();
                let ft = entry.file_type().await?;
                if ft.is_dir() {
                    remove_dir_all_best_effort_async(&path).await;
                } else {
                    let _ = fs::remove_file(&path).await;
                }
            }
        }
        Ok(())
    })
}

async fn remove_dir_all_best_effort_async(path: &Path) {
    if let Err(e) = fs::remove_dir_all(path).await {
        eprintln!("warning: failed to remove {}: {e}", path.display());
    }
}

// ── sync Drop fallbacks (panic/error path only) ────────────────────────────

fn sync_copy_profile_back_blocking(name: &str) -> Result<(), crate::error::Error> {
    let base = base_profile_dir();
    let src = working_dir(name);
    if !src.exists() { return Ok(()); }
    if !base.exists() { std::fs::create_dir_all(&base)?; }
    sync_dir_blocking(&src, &base)?;
    Ok(())
}

fn sync_cleanup_blocking(name: &str) {
    let dir = working_dir(name);
    if dir.exists() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn sync_dir_blocking(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut src_names: HashSet<OsString> = HashSet::new();
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy().into_owned();
        if is_lock_entry(&name_str) { continue; }
        src_names.insert(file_name.clone());
        let from = entry.path();
        let to = dst.join(&file_name);
        let ft = entry.file_type()?;
        if ft.is_dir() {
            if !to.exists() {
                std::fs::create_dir_all(&to)?;
            } else if to.is_file() {
                std::fs::remove_file(&to)?;
                std::fs::create_dir_all(&to)?;
            }
            sync_dir_blocking(&from, &to)?;
        } else if ft.is_file() {
            if to.is_dir() {
                let _ = std::fs::remove_dir_all(&to);
            }
            let _ = std::fs::copy(&from, &to);
        }
    }
    if dst.exists() {
        for entry in std::fs::read_dir(dst)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy().into_owned();
            if is_lock_entry(&name_str) { continue; }
            if src_names.contains(&file_name) { continue; }
            let path = entry.path();
            let ft = entry.file_type()?;
            if ft.is_dir() {
                let _ = std::fs::remove_dir_all(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}
