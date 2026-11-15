//! Regressions found by the S60 acceptance run.

mod support;

use std::process::Command;

/// Runs only as the child of the test below: signs the shared fixture.
#[test]
fn regress_s60_child_signs_fixture() {
    if std::env::var_os("WATANE_REGRESS_CHILD").is_none() {
        return;
    }
    let _ = support::fixture();
}

/// The fixture signs its entries with `Log` in temp dirs, but keeps them in a
/// `static`, which is never dropped: every test binary that uses it leaves
/// its log dirs in the temp dir (about 4 MiB a run). Tests must clean their
/// temp dirs (S60-1 AC9).
#[test]
fn regress_s60_fixture_temp_dirs_removed() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "regress_s60_child_signs_fixture",
            "--test-threads=1",
        ])
        .env("TMPDIR", tmp.path())
        .env("WATANE_REGRESS_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let left: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(
        left.is_empty(),
        "temp dirs left after the test binary exits: {left:?}"
    );
}
