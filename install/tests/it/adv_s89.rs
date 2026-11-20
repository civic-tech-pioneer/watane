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

/// 3.8 decides "committed" from undo's state: the journal is there and `receipt.new.<pid>` is
/// gone. When the receipt's `mv` renames the file and then fails (killed, or an error after the
/// rename), the failure path of `install_files` runs rollback, which finds the commit done and
/// keeps the new version, and then dies with exit 5 and "Nothing was changed: the version that
/// was installed still works." Alice reads that nothing changed while 0.1.0 is installed.
#[test]
fn adv_s89_receipt_move_that_fails_after_the_rename_says_nothing_was_changed() {
    let fx = Fx::new();
    let p = fx.fakes.join("mv");
    fs::write(
        &p,
        "#!/bin/sh\nfor a; do case $a in *receipt.new.*) \"$FAKE_REAL/mv\" \"$@\"; exit 1 ;; esac; done\nexec \"$FAKE_REAL/mv\" \"$@\"\n",
    )
    .expect("fake mv");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&["--no-toolchain"]);
    fx.assert_installed("0.1.0");
    assert!(
        !out.stderr.contains("Nothing was changed"),
        "installed, yet the run says nothing changed: {out:?}"
    );
}

/// 3.8 (fix2): a fresh install says "Nothing was installed." when the swap fails, decided only by
/// what existed before the swap. When the swap fails (the receipt's `mv` fails without renaming)
/// and undo then cannot take a fresh entry back out (its `mv` back to `.new.<pid>` fails, twice),
/// the new `watane` stays at `~/.local/bin/watane`, on Alice's PATH, and the run still ends with
/// "Nothing was installed." The old-path check that says "could not be put back" never fires on
/// a fresh install, since there is no old path.
#[test]
fn adv_s89_fresh_install_whose_undo_fails_says_nothing_was_installed() {
    let fx = Fx::new();
    let p = fx.fakes.join("mv");
    fs::write(
        &p,
        "#!/bin/sh\nfor a; do case $a in *receipt.new.*) exit 1 ;; esac; done\nfor last; do :; done\ncase $1 in *.new.*) ;; *) case $last in *.new.[0-9]*) exit 1 ;; esac ;; esac\nexec \"$FAKE_REAL/mv\" \"$@\"\n",
    )
    .expect("fake mv");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    let left = fx.home.join(".local/bin/watane");
    assert!(
        left.exists(),
        "setup: undo was meant to leave the new program: {out:?}"
    );
    assert!(
        !out.stderr.contains("Nothing was installed."),
        "~/.local/bin/watane is in place, yet the run says nothing was installed: {out:?}"
    );
}
