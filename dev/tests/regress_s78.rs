//! Regressions found by the S78 acceptance run.

mod support;

use std::process::Command;

/// Runs only as the child of the test below: builds the warm cache.
#[test]
#[ignore = "slow: builds the template cold; runs only as the child of the test below"]
fn regress_s78_child_builds_cache() {
    if std::env::var_os("WATANE_REGRESS_CHILD").is_none() {
        return;
    }
    let _ = support::cache();
}

/// The warm cache used to sit in a `static TempDir`, which is never dropped:
/// every test binary left a folder of GBs in `$TMPDIR`. It lives under
/// `CARGO_TARGET_TMPDIR` now (S78-1 AC2).
#[test]
#[ignore = "slow: builds the template cold; only it proves the warm cache leaves nothing in TMPDIR after exit"]
fn regress_s78_warm_cache_leaves_temp_dir_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "regress_s78_child_builds_cache",
            "--include-ignored",
            "--test-threads=1",
        ])
        .env("TMPDIR", tmp.path())
        .env("WATANE_REGRESS_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let left: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "temp folders left after exit: {left:?}");
    let expected = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("watane-dev-warm");
    assert!(expected.join("counter").is_dir(), "{expected:?}");
}
