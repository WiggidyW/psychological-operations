//! Compile-time-embedded Chromium zip + extension assets.
//!
//! Paths come from `build.rs`, which calls
//! `psychological-operations-chromium/validate.sh` to confirm the
//! sister-bundle is present and fresh, then emits `cargo:rustc-env`
//! lines pointing at each artifact in `embed/<target>/<profile>/`.
//!
//! Two extensions ship side-by-side, never loaded into the same
//! Chromium profile: `scrape` (For-You DOM walker on x.com) and
//! `auth` (X-App credentials form on developer.x.com).

pub const CHROMIUM_BUNDLE: &[u8] = include_bytes!(env!("PSYOPS_CHROMIUM_BUNDLE_PATH"));

/// Relative path inside the extracted Chromium zip to the launchable
/// binary (e.g. `chrome-win/chrome.exe`).
pub const LAUNCH_ENTRY: &str = include_str!(env!("PSYOPS_CHROMIUM_LAUNCH_ENTRY_PATH"));

pub fn launch_entry() -> &'static str {
    LAUNCH_ENTRY.trim()
}

// ── Scrape extension ───────────────────────────────────────────────────

pub const SCRAPE_EXTENSION_TAR: &[u8] =
    include_bytes!(env!("PSYOPS_SCRAPE_EXTENSION_TAR_PATH"));

/// 32-char extension ID derived from the SPKI public key in
/// `psychological-operations-chromium/extension-key-scrape.pem`.
/// Stable across every build because the key is committed.
pub const SCRAPE_EXTENSION_ID: &str =
    include_str!(env!("PSYOPS_SCRAPE_EXTENSION_ID_PATH"));

pub fn scrape_extension_id() -> &'static str {
    SCRAPE_EXTENSION_ID.trim()
}

// ── Auth extension ─────────────────────────────────────────────────────

pub const AUTH_EXTENSION_TAR: &[u8] =
    include_bytes!(env!("PSYOPS_AUTH_EXTENSION_TAR_PATH"));

/// 32-char extension ID derived from the SPKI public key in
/// `psychological-operations-chromium/extension-key-auth.pem`.
/// Stable across every build because the key is committed.
pub const AUTH_EXTENSION_ID: &str =
    include_str!(env!("PSYOPS_AUTH_EXTENSION_ID_PATH"));

pub fn auth_extension_id() -> &'static str {
    AUTH_EXTENSION_ID.trim()
}

// ── Native messaging ───────────────────────────────────────────────────

/// Reserved native-messaging host name used by both extensions and the
/// `psychological-operations native-host` subcommand. Same string is
/// hard-coded in each extension's `background.js`.
pub const NATIVE_HOST_NAME: &str = "com.objectiveai.psychological_operations";
