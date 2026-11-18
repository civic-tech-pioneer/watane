//! Adversary tests of S50b-fix: deferred signals before the swap (install/TECHNICAL 3.8).

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;

use support::*;

/// Writes a wrapper first on the PATH: it sends `signal` to the installer shell only (as
/// `kill <pid>` from another program does) when its arguments match `when`, then runs the
/// real tool. The wrapper only places the signal in time; the tool still does its work.
fn signal_wrapper(fx: &Fx, tool: &str, when: &str, signal: &str) {
    let real = which(tool);
    let p = fx.fakes.join(tool);
    fs::write(
        &p,
        format!(
            "#!/bin/sh\ncase \"$*\" in {when}) kill -s {signal} \"$FAKE_INSTALL_PID\" ;; esac\nexec {} \"$@\"\n",
            real.display()
        ),
    )
    .expect("wrapper");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// 3.8: "An INT, HUP, TERM or PIPE before the swap ends the run". A TERM that reaches the
/// shell while it downloads SHA256SUMS is only recorded; the run goes on and fetches the
/// archive (a new network request after the person stopped it), and is ended only later at
/// verify_archive's checkpoint. Before S50b-fix the trap ended the run once curl returned.
#[test]
fn adv_s50b_fix_term_during_download_still_fetches_the_archive() {
    let fx = Fx::new();
    signal_wrapper(&fx, "curl", "*SHA256SUMS*", "TERM");
    let out = fx.run(&[]);
    assert_eq!(out.code, 143, "{out:?}");
    let reqs = fx.rel.requests();
    assert!(
        !reqs.iter().any(|r| r.contains(".tar.gz")),
        "the archive was requested after the TERM: {reqs:?} {out:?}"
    );
}

/// 3.8: a signal before the swap ends the run "with its signal code and no cause line".
/// A TERM recorded during step 5 (mktemp) is not checked before the foreign-file check, which
/// exits 8 with its own lines; the run prints a cause after the person stopped it.
#[test]
fn adv_s50b_fix_term_before_the_foreign_check_prints_a_cause() {
    let fx = Fx::new();
    fs::create_dir_all(fx.home.join(".local/bin")).expect("bin");
    fs::write(fx.home.join(".local/bin/watane-dev"), "mine\n").expect("foreign");
    signal_wrapper(&fx, "mktemp", "*", "TERM");
    let out = fx.run(&[]);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stderr.contains("watane install:"), "{out:?}");
}

/// 3.8: deferral must not outlive the checks. A TERM whose trap runs right after do_swap's
/// `check_signal` returned and before `SIGNAL_DEFER=0` is recorded and never looked at again:
/// the swap, the commit and the toolchain step all run, and the run prints that watane is
/// installed, then exits 143. The `kill` line only fixes the moment a signal can land; bash
/// runs a trap between any two commands.
#[test]
fn adv_s50b_fix_term_after_the_swap_checkpoint_installs_and_reaches_the_toolchain() {
    let fx = Fx::new();
    let from = "  check_signal\n  SWAP_PID=$$\n";
    let s = baked_script("0.1.0");
    assert!(s.contains(from));
    fs::write(
        &fx.script,
        s.replacen(
            from,
            "  check_signal\n  kill -s TERM $$\n  SWAP_PID=$$\n",
            1,
        ),
    )
    .expect("script");
    let out = fx.run(&[]);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stdout.contains("no terminal to ask on"), "{out:?}");
    assert!(!out.stdout.contains("is installed."), "{out:?}");
    assert!(!fx.receipt().exists(), "{out:?}");
}
