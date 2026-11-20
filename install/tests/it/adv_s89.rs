//! Adversarial tests of S89: the exit-trap hint, the folders a run created, and what Alice reads
//! (install/TECHNICAL 3.6 step 4, 3.8, 3.9; IN-45, IN-47).

use crate::support::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;

const HINT: [&str; 2] = [
    "watane 0.1.0 is installed; the run was interrupted before it finished.",
    "Run watane-dev doctor to check the maker tools.",
];

/// 3.8: "A signal after the commit (the receipt was moved into place ...) ... prints on stderr
/// the two lines". A TERM that reaches the shell while the `mv` of the receipt runs is handled
/// once that `mv` returns, before `COMMITTED=1` runs: the rollback finds the commit done and
/// keeps the new version, yet the trap stays silent, so Alice reads an interrupted run with
/// no word that watane is installed.
#[test]
fn adv_s89_signal_during_the_receipt_move_keeps_the_install_but_prints_no_hint() {
    let fx = Fx::new();
    let p = fx.fakes.join("mv");
    fs::write(
        &p,
        "#!/bin/sh\nfor a; do case $a in *receipt.new.*) \"$FAKE_REAL/mv\" \"$@\" || exit; kill -s TERM $PPID; exit 0 ;; esac; done\nexec \"$FAKE_REAL/mv\" \"$@\"\n",
    )
    .expect("fake mv");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 143, "{out:?}");
    fx.assert_installed("0.1.0");
    let lines: Vec<&str> = out.stderr.lines().collect();
    assert_eq!(lines, HINT, "installed, but the hint is missing: {out:?}");
}
