//! Per-scrape Chrome profile snapshots so concurrent scrapes don't fight
//! over the shared `chrome-data/` profile lock. Each run copies the base
//! profile out to `chrome-data-runs/<name>/`, runs against the copy, and
//! overwrites the base with the copy on exit. Last-finished wins; the user
//! has accepted that tradeoff.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Single global mutex that serialises copy-back so two siblings finishing
/// simultaneously don't half-overwrite the base profile.
fn copyback_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
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
pub fn copy_profile_out(name: &str) -> Result<PathBuf, crate::error::Error> {
    let base = base_profile_dir();
    let dest = working_dir(name);
    if dest.exists() {
        remove_dir_all_best_effort(&dest);
    }
    std::fs::create_dir_all(&dest)?;
    if base.exists() {
        copy_dir_filtered(&base, &dest)?;
    }
    Ok(dest)
}

/// Overwrite the base profile with the contents of the working copy. Files
/// in the base that no longer exist in the working copy are removed (so
/// Chrome session-state deletions propagate). Held under a process-global
/// mutex so concurrent siblings don't tear each other.
pub fn copy_profile_back(name: &str) -> Result<(), crate::error::Error> {
    let _guard = copyback_lock().lock().unwrap_or_else(|e| e.into_inner());
    let base = base_profile_dir();
    let src = working_dir(name);
    if !src.exists() {
        // Nothing to copy back — treat as success.
        return Ok(());
    }
    if !base.exists() {
        std::fs::create_dir_all(&base)?;
    }
    sync_dir(&src, &base)?;
    Ok(())
}

/// Delete the working copy. Best-effort: failures are ignored because
/// scrape-run cleanup shouldn't mask the actual run result.
pub fn cleanup(name: &str) {
    let dir = working_dir(name);
    if dir.exists() {
        remove_dir_all_best_effort(&dir);
    }
}

/// `Drop` guard that runs `copy_profile_back` + `cleanup` when it goes out
/// of scope, so a panicking scrape still propagates session state and
/// removes its working copy.
pub struct ProfileSession {
    name: String,
    armed: bool,
}

impl ProfileSession {
    pub fn begin(name: &str) -> Result<(Self, PathBuf), crate::error::Error> {
        let dir = copy_profile_out(name)?;
        Ok((Self { name: name.to_string(), armed: true }, dir))
    }

    /// Disarm the guard — caller has already done copy-back / cleanup
    /// explicitly and doesn't want it run again on drop.
    #[allow(dead_code)]
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ProfileSession {
    fn drop(&mut self) {
        if !self.armed { return; }
        if let Err(e) = copy_profile_back(&self.name) {
            eprintln!("scrape \"{}\" copy-back failed: {e}", self.name);
        }
        cleanup(&self.name);
    }
}

// ── internals ──────────────────────────────────────────────────────────────

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

fn copy_dir_filtered(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy().into_owned();
        if is_lock_entry(&name_str) { continue; }
        let from = entry.path();
        let to = dst.join(&file_name);
        let ft = entry.file_type()?;
        if ft.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_filtered(&from, &to)?;
        } else if ft.is_file() {
            // Copy may fail if Chrome has the file open — skip those rather
            // than aborting, since they're typically caches.
            if let Err(e) = std::fs::copy(&from, &to) {
                eprintln!("warning: skipping {}: {e}", from.display());
            }
        }
        // Symlinks are skipped — Chrome's profile shouldn't contain them.
    }
    Ok(())
}

/// Mirror `src` → `dst`: copy/overwrite everything in src into dst, and
/// delete anything in dst that isn't in src. Skips lock-style entries on
/// the src side so Chrome's running-instance markers don't leak into the
/// base profile.
fn sync_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    let mut src_names: std::collections::HashSet<std::ffi::OsString> = std::collections::HashSet::new();
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
            sync_dir(&from, &to)?;
        } else if ft.is_file() {
            if to.is_dir() {
                remove_dir_all_best_effort(&to);
            }
            if let Err(e) = std::fs::copy(&from, &to) {
                eprintln!("warning: copy-back skipped {}: {e}", from.display());
            }
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
                remove_dir_all_best_effort(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

fn remove_dir_all_best_effort(path: &Path) {
    if let Err(e) = std::fs::remove_dir_all(path) {
        eprintln!("warning: failed to remove {}: {e}", path.display());
    }
}
