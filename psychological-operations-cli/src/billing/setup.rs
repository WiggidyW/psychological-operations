//! `billing setup` — open chromium against the master billing
//! profile so the operator can sign into x.com / configure
//! console.x.com / click the extension to save credentials.

use std::process::Command;

use crate::chrome::extract::ensure_extracted;
use crate::chrome::native_host;
use crate::chrome::paths::billing_profile_dir;
use crate::error::Error;

pub async fn run() -> Result<crate::Output, Error> {
    let materialized = ensure_extracted()?;

    let profile = billing_profile_dir();
    std::fs::create_dir_all(&profile)?;

    // Same native-host registration the per-psyop browse path uses.
    // The extension on this profile needs the messaging bridge so
    // its "Save credentials" button can ship to billing.json.
    native_host::install(&profile)?;

    let extension_id = crate::chrome::bundles::extension_id();

    let mut cmd = Command::new(&materialized.chrome_binary);
    cmd.arg(format!("--user-data-dir={}", profile.display()));
    cmd.arg(format!("--load-extension={}", materialized.extension_dir.display()));
    cmd.arg(format!("--allowlisted-extension-id={extension_id}"));
    cmd.arg("--no-first-run");
    cmd.arg("--no-default-browser-check");
    cmd.arg("--disable-component-update");
    cmd.arg("--disable-features=ChromeWhatsNewUI,DefaultBrowserPromptRefresh");
    cmd.arg("https://console.x.com/");

    // No PSYOP_NAME / PSYOP_COMMIT_SHA — billing isn't a psyop. The
    // native host's psyop-identity resolver isn't called on the
    // BillingSave path; only Init / Ingest care about identity.

    let child = cmd.spawn().map_err(|e| {
        Error::Other(format!("failed to spawn chromium for billing setup: {e}"))
    })?;

    eprintln!(
        "psychological-operations: spawned chromium (pid {}) for billing setup",
        child.id(),
    );
    eprintln!("  profile: {}", profile.display());
    eprintln!("  - sign into your X account, then visit console.x.com");
    eprintln!("    to create the master App and provision credits.");
    eprintln!("  - on the App's credentials page, click the extension toolbar");
    eprintln!("    icon and choose 'Save credentials' to write billing.json.");
    eprintln!("  - this profile persists; future runs reuse the session.");

    Ok(crate::Output::Empty)
}
