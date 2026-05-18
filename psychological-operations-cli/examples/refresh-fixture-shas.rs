//! One-shot: refresh psyop_commit_sha references inside committed
//! test data.db fixtures so they match the commit_sha the harness's
//! `publish::publish_file` (git2) would produce from the current
//! psyop.json content.
//!
//! Why this exists: any change to a test psyop.json (e.g. adding the
//! "mock" field) shifts the deterministic commit_sha. Pre-seeded
//! data.db rows reference the OLD commit_sha; the cli looks up state
//! under the NEW one — yielding "0 accepted" / empty-queue errors.
//! Asset data.db is the static source of truth, so we fix it here.
//!
//! Uses git2 directly (NOT git CLI) so the commit_sha computation
//! matches the harness byte-for-byte — git CLI's blob/tree builder
//! diverges in subtle ways on Windows.
//!
//! Run from repo root:
//!     cargo run -p psychological-operations-cli --example refresh-fixture-shas

use std::path::{Path, PathBuf};

use git2::{Repository, Signature, Time};
use rusqlite::{Connection, params};

// Must mirror tests/common/mod.rs:git_init_psyops's Config.
const AUTHOR_NAME: &str = "psyops-test";
const AUTHOR_EMAIL: &str = "test@psyops.invalid";
const COMMIT_TIME_SECS: i64 = 1_767_225_600; // 2026-01-01T00:00:00Z
const COMMIT_MSG: &str = "init";

// Tables in data.db that carry psyop_commit_sha (src/db.rs:8-72).
const TABLES_WITH_COMMIT: &[&str] = &["posts", "for_you_queue", "delivery_queue"];

fn compute_commit_sha(psyop_dir: &Path) -> String {
    // Re-implement publish::publish_file's commit flow in a throwaway
    // temp dir. Using publish_file directly would mutate the input
    // dir; we want to leave assets untouched.
    let content = std::fs::read(psyop_dir.join("psyop.json"))
        .expect("read psyop.json");

    let temp_root = std::env::temp_dir().join(format!(
        "refresh-fixture-shas-{}-{}",
        std::process::id(),
        psyop_dir.file_name().unwrap().to_string_lossy(),
    ));
    let _ = std::fs::remove_dir_all(&temp_root);
    std::fs::create_dir_all(&temp_root).expect("mkdir temp");

    let sha = (|| -> String {
        let repo = Repository::init(&temp_root).expect("git init");
        let target = temp_root.join("psyop.json");
        std::fs::write(&target, &content).expect("write psyop.json");

        let mut index = repo.index().expect("get index");
        index.add_path(Path::new("psyop.json")).expect("add_path");
        index.write().expect("index.write");
        let tree_oid = index.write_tree().expect("write_tree");
        let tree = repo.find_tree(tree_oid).expect("find_tree");

        let sig = Signature::new(
            AUTHOR_NAME,
            AUTHOR_EMAIL,
            &Time::new(COMMIT_TIME_SECS, 0),
        ).expect("signature");

        let oid = repo
            .commit(Some("HEAD"), &sig, &sig, COMMIT_MSG, &tree, &[])
            .expect("commit");
        oid.to_string()
    })();

    let _ = std::fs::remove_dir_all(&temp_root);
    sha
}

fn refresh_fixture(fixture_root: &Path) {
    let plugin_root = fixture_root
        .join(".objectiveai")
        .join("plugins")
        .join("psychological-operations");
    let data_db = plugin_root.join("data.db");
    let psyops_dir = plugin_root.join("psyops");
    if !data_db.exists() || !psyops_dir.exists() {
        return;
    }

    // psyop_name -> new_sha
    let mut new_shas: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&psyops_dir).expect("read psyops dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("psyop.json").exists() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let sha = compute_commit_sha(&path);
        new_shas.push((name, sha));
    }

    let label = fixture_root.file_name().unwrap().to_string_lossy();
    println!("\n{label}:");
    for (n, s) in &new_shas {
        println!("  {n} -> {s}");
    }

    let conn = Connection::open(&data_db).expect("open data.db");
    for table in TABLES_WITH_COMMIT {
        for (psyop_name, new_sha) in &new_shas {
            let mut stmt = conn
                .prepare(&format!(
                    "SELECT DISTINCT psyop_commit_sha FROM {table} WHERE psyop = ?1",
                ))
                .expect("prepare select");
            let old_shas: Vec<String> = stmt
                .query_map(params![psyop_name], |row| row.get::<_, String>(0))
                .expect("query_map")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect old shas");
            for old in old_shas {
                if old == *new_sha {
                    continue;
                }
                let n = conn
                    .execute(
                        &format!(
                            "UPDATE {table} SET psyop_commit_sha = ?1 \
                             WHERE psyop = ?2 AND psyop_commit_sha = ?3",
                        ),
                        params![new_sha, psyop_name, &old],
                    )
                    .expect("update");
                println!(
                    "    {table}: {n} rows  {old_short} -> {new_short}  (psyop={psyop_name})",
                    old_short = &old[..7],
                    new_short = &new_sha[..7],
                );
            }
        }
    }
}

fn main() {
    // Resolve repo root from CARGO_MANIFEST_DIR ascend one level.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets = manifest_dir.join("assets");
    if !assets.exists() {
        eprintln!("error: {} not found", assets.display());
        std::process::exit(1);
    }

    let mut fixtures: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&assets).expect("read assets") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let data_db = path
            .join(".objectiveai")
            .join("plugins")
            .join("psychological-operations")
            .join("data.db");
        if data_db.exists() {
            fixtures.push(path);
        }
    }
    fixtures.sort();
    if fixtures.is_empty() {
        println!("no fixtures with data.db found");
        return;
    }
    for fix in &fixtures {
        refresh_fixture(fix);
    }
    println!("\ndone.");
}
