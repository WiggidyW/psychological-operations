//! Smoke test: verifies the test harness wiring (binary build,
//! per-subprocess env, snapshot mechanism, Drop cleanup).
//!
//! Runs `psychological-operations psyops list` against an empty
//! per-test base dir → no psyops → empty `[]` JSON on stdout.
//! Snapshots stdout + stderr.
//!
//! First run: UPDATE_PSYOPS_SNAPSHOTS=1 cargo test --test harness_smoke
//! Then commit assets/harness_smoke/{stdout,stderr}.txt.

mod common;

use common::TestEnv;

#[test]
fn harness_smoke_psyops_list_empty() {
    let env = TestEnv::new("harness_smoke");

    let out = env.run(&["psyops", "list"]);
    assert!(
        out.status.success(),
        "psyops list failed: stderr={}",
        out.stderr,
    );

    common::snapshot::assert_snapshot(
        out.stdout_trimmed(),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/harness_smoke/stdout.txt"),
        include_str!("../assets/harness_smoke/stdout.txt"),
    );
    common::snapshot::assert_snapshot(
        out.stderr_trimmed(),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/harness_smoke/stderr.txt"),
        include_str!("../assets/harness_smoke/stderr.txt"),
    );
}
