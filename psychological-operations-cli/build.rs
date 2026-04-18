use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let playwright_dir = manifest_dir.parent().unwrap().join("psychological-operations-playwright");

    let target = env::var("TARGET").unwrap();
    let profile = if env::var("PROFILE").unwrap() == "release" { "release" } else { "debug" };

    // Validate the playwright binary
    let validate_script = playwright_dir.join("validate.sh");
    let mut args = vec![];
    if profile == "release" {
        args.push("--release");
    }

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "bash"])
            .arg(&validate_script)
            .args(&args)
            .status()
    } else {
        Command::new("bash")
            .arg(&validate_script)
            .args(&args)
            .status()
    };

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            panic!(
                "\n\npsychological-operations-playwright binary is missing or stale.\n\
                 Run: bash psychological-operations-playwright/build.sh{}\n\
                 Exit code: {}\n",
                if profile == "release" { " --release" } else { "" },
                s.code().unwrap_or(-1),
            );
        }
        Err(e) => {
            panic!("Failed to run validate.sh: {e}. Make sure bash is available.");
        }
    }

    // Compute binary path
    let ext = if target.contains("windows") { ".exe" } else { "" };
    let binary_path = playwright_dir
        .join("embed")
        .join(&target)
        .join(profile)
        .join(format!("psychological-operations-playwright{ext}"));

    println!("cargo:rustc-env=PSYOPS_PLAYWRIGHT_BINARY_PATH={}", binary_path.display());
    println!("cargo:rerun-if-changed={}", playwright_dir.join("embed").display());
}
