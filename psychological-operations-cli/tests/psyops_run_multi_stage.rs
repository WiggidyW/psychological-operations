//! psyop with two scoring stages (both pointing at the same mock
//! function/profile). The first stage's surviving outputs feed the
//! second; final output reports survivor count `… survived all 2
//! stages`.

mod common;

use common::TestEnv;

#[test]
fn psyops_run_multi_stage() {
    let env = TestEnv::new("psyops_run_multi_stage");

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
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_multi_stage/stdout.txt"),
        include_str!("../assets/psyops_run_multi_stage/stdout.txt"),
    );
    common::snapshot::assert_snapshot(
        &common::snapshot::normalize(out.stderr_trimmed()),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/psyops_run_multi_stage/stderr.txt"),
        include_str!("../assets/psyops_run_multi_stage/stderr.txt"),
    );
}
