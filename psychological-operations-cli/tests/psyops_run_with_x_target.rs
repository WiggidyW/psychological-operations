//! `config.json` declares an X target with `action: like`. With
//! mock-X enabled, `Http::for_psyop` returns a mock client (no
//! tokens.json needed) and the like POST is dispatched against
//! the mock for each survivor.

mod common;

use common::TestEnv;

#[test]
fn psyops_run_with_x_target() {
    let env = TestEnv::new("psyops_run_with_x_target");

    let out = env.run(&[
        "psyops", "run",
        "--name", "test-psyop",
        "--seed", "42",
    ]);
    assert!(
        out.status.success(),
        "run failed: stderr={}",
        out.stderr,
    );

    common::snapshot::assert_snapshot(
        &common::snapshot::normalize(out.stdout_trimmed()),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_with_x_target/stdout.txt"),
        include_str!("../assets/psyops_run_with_x_target/stdout.txt"),
    );
    common::snapshot::assert_snapshot(
        &common::snapshot::normalize(out.stderr_trimmed()),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_with_x_target/stderr.txt"),
        include_str!("../assets/psyops_run_with_x_target/stderr.txt"),
    );
}
