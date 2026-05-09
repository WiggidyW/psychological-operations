//! Pre-seed the per-profile `Default/Preferences` JSON to bake in
//! the launch-time UX defaults we want for every Chromium spawn.
//!
//! Two knobs today, both written before `cmd.spawn()`:
//!
//!  1. **`extensions.pinned_extensions`** (array of extension IDs) —
//!     the extension we just `--load-extension`'d shows up pinned to
//!     the toolbar on first launch instead of hiding in the puzzle-
//!     piece menu. (Post-M89 toolbar-pin storage.)
//!
//!  2. **`session.restore_on_startup` = 5** + empty
//!     **`session.startup_urls`** — Chromium opens nothing on launch
//!     beyond the URL we passed on the command line. Without this,
//!     the second time the operator opens a profile they'd get every
//!     tab from their previous session re-opened on top of our
//!     landing URL.
//!
//! Writing to Preferences before Chromium starts is the only
//! portable mechanism that doesn't require managed-policy file
//! placement at OS-specific paths. Idempotent: every spawn writes
//! the same merged shape, so existing profiles inherit the new
//! defaults on their next launch.

use std::fs;
use std::path::Path;

use serde_json::{json, Map, Value};

use crate::error::Error;

pub fn seed_profile_prefs(profile: &Path, pinned_extension_ids: &[&str]) -> Result<(), Error> {
    let default_dir = profile.join("Default");
    fs::create_dir_all(&default_dir)?;
    let prefs_path = default_dir.join("Preferences");

    let mut prefs: Value = if prefs_path.exists() {
        let bytes = fs::read(&prefs_path)?;
        // A corrupt or empty Preferences is recoverable — start fresh.
        // Chromium will rebuild the rest of the file on launch.
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let root = prefs
        .as_object_mut()
        .ok_or_else(|| Error::Other("Preferences root is not a JSON object".into()))?;

    seed_pinned_extensions(root, pinned_extension_ids)?;
    seed_no_session_restore(root)?;
    mark_clean_exit(root)?;

    let serialized = serde_json::to_vec(&prefs)?;
    fs::write(&prefs_path, serialized)?;

    wipe_session_files(&default_dir)?;
    Ok(())
}

fn seed_pinned_extensions(
    root: &mut Map<String, Value>,
    extension_ids: &[&str],
) -> Result<(), Error> {
    let extensions = root
        .entry("extensions")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| Error::Other(
            "Preferences \"extensions\" is not an object".into(),
        ))?;
    let pinned = extensions
        .entry("pinned_extensions")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| Error::Other(
            "Preferences \"extensions.pinned_extensions\" is not an array".into(),
        ))?;

    for id in extension_ids {
        let id_value = json!(id);
        if !pinned.iter().any(|v| v == &id_value) {
            pinned.push(id_value);
        }
    }
    Ok(())
}

// Chromium's "session crashed" detector keys off `profile.exit_type`.
// If the previous exit was anything other than "Normal" — including
// us forcibly killing Chromium on the parent's exit — Chromium
// triggers session restore on next launch *regardless* of the
// `restore_on_startup` pref. Mark the prior exit as Normal pre-launch
// so the crash heuristic doesn't override our no-restore intent.
fn mark_clean_exit(root: &mut Map<String, Value>) -> Result<(), Error> {
    let profile = root
        .entry("profile")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| Error::Other("Preferences \"profile\" is not an object".into()))?;
    profile.insert("exit_type".into(), json!("Normal"));
    profile.insert("exited_cleanly".into(), json!(true));
    Ok(())
}

// Even with the right prefs, Chromium will re-open the previous
// session if the on-disk session files are present. Wipe them so
// there's literally nothing to restore.
fn wipe_session_files(default_dir: &Path) -> Result<(), Error> {
    for name in ["Last Session", "Last Tabs", "Current Session", "Current Tabs"] {
        let _ = fs::remove_file(default_dir.join(name));
    }
    let sessions_dir = default_dir.join("Sessions");
    if sessions_dir.exists() {
        let _ = fs::remove_dir_all(&sessions_dir);
    }
    Ok(())
}

fn seed_no_session_restore(root: &mut Map<String, Value>) -> Result<(), Error> {
    // session.restore_on_startup values:
    //   1 = restore last session   (Chromium default w/ --user-data-dir)
    //   4 = open the new tab page
    //   5 = open URLs from session.startup_urls
    // We pick 5 with an empty list so nothing extra opens — Chromium
    // honors the URL we passed on the command line and that's it.
    let session = root
        .entry("session")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| Error::Other(
            "Preferences \"session\" is not an object".into(),
        ))?;
    session.insert("restore_on_startup".into(), json!(5));
    session.insert("startup_urls".into(), json!([]));
    Ok(())
}
