//! Snapshot assertion modeled on
//! `objectiveai-api/tests/common/stream_harness.rs::assert_snapshot`.
//!
//! Each test compares actual stdout / stderr against a committed
//! file under `tests/assets/<test_name>/{stdout,stderr}.txt`.
//! Set `UPDATE_PSYOPS_SNAPSHOTS=1` to regenerate.

const SNAPSHOT_ENV: &str = "UPDATE_PSYOPS_SNAPSHOTS";

/// Compare `actual` against `expected_static` (from `include_str!`),
/// or write `actual` to `path` when `UPDATE_PSYOPS_SNAPSHOTS=1`.
///
/// `path` is the absolute path to the snapshot file (typically
/// built via `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/...")`)
/// so update-mode can open and rewrite it.
pub fn assert_snapshot(actual: &str, path: &str, expected_static: &str) {
    if std::env::var(SNAPSHOT_ENV).as_deref() == Ok("1") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).expect("create snapshot parent dir");
        }
        std::fs::write(path, actual).expect("write snapshot");
        eprintln!("Updated snapshot: {path}");
        let written = std::fs::read_to_string(path).expect("re-read snapshot");
        assert_eq!(actual, written.trim_end_matches('\n'));
    } else {
        assert_eq!(
            actual,
            expected_static.trim_end_matches('\n'),
            "snapshot mismatch at {path}: re-run with {SNAPSHOT_ENV}=1",
        );
    }
}
