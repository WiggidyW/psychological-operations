//! `config.json` defines a single global stdout target with mode
//! `urls_with_scores`. After scoring, the run loop should
//! synchronously fire the target, printing one
//! `score — https://x.com/.../status/<id>` line per survivor.

mod common;

use common::TestEnv;

#[test]
fn psyops_run_with_stdout_target() {
    let env = TestEnv::new("psyops_run_with_stdout_target");

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
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_with_stdout_target/stdout.txt"),
        include_str!("../assets/psyops_run_with_stdout_target/stdout.txt"),
    );
    common::snapshot::assert_snapshot(
        &common::snapshot::normalize(out.stderr_trimmed()),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_with_stdout_target/stderr.txt"),
        include_str!("../assets/psyops_run_with_stdout_target/stderr.txt"),
    );
}
