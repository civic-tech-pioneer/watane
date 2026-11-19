//! Adversary tests of S50b-fix: deferred signals before the swap (install/TECHNICAL 3.8).

use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::support::*;

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

/// Writes a fake `tool` first on the PATH: when its arguments match `when`, it sends `signal`
/// to the whole process group (as Ctrl-C does) and exits with `code` itself, as a program
/// that handles INT does; otherwise it runs `otherwise` (a shell body).
fn group_signal_fake(fx: &Fx, tool: &str, when: &str, signal: &str, code: u8, otherwise: &str) {
    let p = fx.fakes.join(tool);
    fs::write(
        &p,
        format!(
            "#!/bin/sh\ncase \"$*\" in {when}) kill -s {signal} -- -\"$FAKE_INSTALL_PID\"; exit {code} ;; esac\n{otherwise}\n"
        ),
    )
    .expect("fake");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// 3.8: "Signals end the run at once at every step". check_signal maps a child's 130 only on
/// the failure paths that print a cause; where a status is dropped, macOS sh's lost INT is
/// never seen. A Ctrl-C during the toolchain detection (`rustup which rustc` exits 130) is lost: the
/// run reports the toolchain as missing, goes on, prints that watane is installed and exits 0.
#[test]
fn adv_s50b_fix_ctrl_c_in_toolchain_detection_finishes_with_success() {
    let fx = Fx::new();
    fx.add_rustup(true);
    group_signal_fake(&fx, "rustup", "which*", "INT", 130, "exit 0");
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stdout.contains("is installed."), "{out:?}");
}

/// 3.8: a Ctrl-C during quarantine removal (`xattr ... || true`, right after the commit) is
/// lost: the run goes on to the toolchain step and exits 0.
#[test]
fn adv_s50b_fix_ctrl_c_in_xattr_reaches_the_toolchain_step() {
    let fx = Fx::new();
    group_signal_fake(&fx, "xattr", "-dr*", "INT", 130, "exit 0");
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stdout.contains("no terminal to ask on"), "{out:?}");
    assert!(!out.stdout.contains("is installed."), "{out:?}");
}

#[test]
fn ac2_ctrl_c_in_platform_substitution() {
    let fx = Fx::new();
    group_signal_fake(&fx, "sysctl", "*", "INT", 130, "exit 0");
    fs::write(
        fx.fakes.join("uname"),
        "#!/bin/sh\ncase $1 in -s) echo Darwin ;; -m) echo x86_64 ;; esac\n",
    )
    .expect("uname");
    fs::set_permissions(fx.fakes.join("uname"), fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("unsupported platform"), "{out:?}");
}

#[test]
fn ac2_ctrl_c_in_ignored_rmdir() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    group_signal_fake(&fx, "rmdir", "*", "INT", 130, "exit 0");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stdout.contains("uninstalled watane"), "{out:?}");
}

#[test]
fn ac2_ctrl_c_in_answer_pipeline() {
    for tool in ["dd", "od"] {
        let fx = Fx::new();
        group_signal_fake(&fx, tool, "*", "INT", 130, "exit 0");
        let out = fx.run_pty(&[], &[], &[("[y/N] ", "n")]);
        assert_eq!(out.code, 130, "{tool}: {out:?}");
        assert!(!out.stdout.contains("toolchain: not installed"), "{out:?}");
    }
}

/// 3.8: a Ctrl-C during the staging copy (`cp -R`, the longest file step) is not a failure:
/// the swap's `|| return 1` once replaced cp's 130, and the run printed "could not put the
/// files in place" with exit 5.
#[test]
fn ac2_ctrl_c_in_staging_copy_has_no_cause() {
    let fx = Fx::new();
    let real = which("cp");
    group_signal_fake(
        &fx,
        "cp",
        "-R*",
        "INT",
        130,
        &format!("exec {} \"$@\"", real.display()),
    );
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("watane install:"), "{out:?}");
    assert!(!fx.receipt().exists(), "{out:?}");
}

/// A fake `mv` first on the PATH: its body runs `rules` (a shell `case` on the arguments),
/// and otherwise the real mv. `fire` sends INT to the whole group once (as Ctrl-C does) and
/// exits 130, as a program that handles INT does.
fn mv_fake(fx: &Fx, rules: &str) {
    let real = which("mv");
    let p = fx.fakes.join("mv");
    fs::write(
        &p,
        format!(
            "#!/bin/sh\nfire() {{ if [ ! -e \"$0.fired\" ]; then : >\"$0.fired\"; kill -s INT -- -\"$FAKE_INSTALL_PID\"; exit 130; fi; }}\n{rules}\nexec {} \"$@\"\n",
            real.display()
        ),
    )
    .expect("fake");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// 3.8: a Ctrl-C while recovery undoes a dead install's swap (step 0) is lost: undo_swap's
/// subshell turns mv's 130 into `exit 1`, and recover prints "could not recover interrupted
/// install" and exits 5, a cause line after the person stopped the run.
#[test]
fn adv_s50b_fix_ctrl_c_in_recovery_undo_prints_a_cause() {
    let fx = Fx::new();
    let bin = fx.bin("watane");
    fs::create_dir_all(bin.parent().expect("bin")).expect("bin dir");
    fs::create_dir_all(fx.receipt().parent().expect("share")).expect("share dir");
    fs::write(bin.with_file_name("watane.old.99999999"), "old\n").expect("old");
    fs::write(
        fx.receipt().with_file_name("receipt.new.99999999"),
        "pending\n",
    )
    .expect("new");
    fs::write(
        fx.receipt().with_file_name("journal.99999999"),
        format!("old {}\n", bin.display()),
    )
    .expect("journal");
    mv_fake(&fx, "case \"$1\" in *.old.99999999) fire ;; esac");
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("watane install:"), "{out:?}");
}

/// 3.8: after a swap fails, a Ctrl-C during its undo is lost: the undo subshell exits (1, or
/// 130 from its own check_signal), rollback drops that status, and install_files checks only
/// the swap's status, so the run prints "could not put the files in place" and exits 5.
#[test]
fn adv_s50b_fix_ctrl_c_in_swap_undo_prints_a_cause() {
    let fx = Fx::new();
    mv_fake(
        &fx,
        "case \"$1\" in *watane.app.new.*) exit 1 ;; esac\ncase \"$2\" in *.new.*) fire ;; esac",
    );
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("watane install:"), "{out:?}");
}

/// AC3: a failed journal append retains its signal status through swap and rollback.
#[test]
fn ac3_journal_append_preserves_signal_status() {
    let fx = Fx::new();
    let source = baked_script("0.1.0").replace(
        "main \"$@\"",
        r#"
printf() {
  case $1 in 'moved %s\n') return 130 ;; esac
  command printf "$@"
}
main "$@"
"#,
    );
    fs::write(&fx.script, source).expect("journal fixture");
    let out = fx.run(&[]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("watane install:"), "{out:?}");
}

/// AC3: permanent undo failure reports recovery rather than claiming an intact install.
#[test]
fn ac3_failed_undo_reports_recovery_and_next_run_installs() {
    let fx = Fx::new();
    assert_eq!(fx.run(&[]).code, 0);
    mv_fake(
        &fx,
        "case \"$1\" in *watane.app.new.*|*.old.*) exit 1 ;; esac",
    );
    let out = fx.run(&[]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(!out.stderr.contains("Nothing was changed"), "{out:?}");
    assert!(out.stderr.contains("The files could not be put back. Run this script again to restore the installed version."), "{out:?}");
    assert!(!fx.bin("watane").exists());
    assert!(!fx.app().exists());
    mv_fake(&fx, "");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    fx.assert_installed("0.1.0");
}

/// 3.9: "The files could not be put back" is printed only when the undo failed. Here every
/// installed file is put back and the installed version works, but removing a staged `.new.`
/// entry fails afterwards (an undo cleanup step), so the run claims the files could not be put
/// back and tells the person the installed version needs restoring: a false report.
#[test]
fn adv_s50b_fix_staging_cleanup_failure_claims_files_not_put_back() {
    let fx = Fx::new();
    assert_eq!(fx.run(&[]).code, 0);
    mv_fake(&fx, "case \"$1\" in *watane.app.new.*) exit 1 ;; esac");
    let real = which("rm");
    let p = fx.fakes.join("rm");
    fs::write(
        &p,
        format!(
            "#!/bin/sh\ncase \"$*\" in *.new.[0-9]*) exit 1 ;; esac\nexec {} \"$@\"\n",
            real.display()
        ),
    )
    .expect("fake");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&[]);
    assert_eq!(out.code, 5, "{out:?}");
    // the installed version is back in place and works
    fx.assert_installed("0.1.0");
    assert!(
        !out.stderr.contains("could not be put back"),
        "files are in place, yet: {out:?}"
    );
}

/// AC2: a transient restore failure is retried before reporting the old version whole.
#[test]
fn ac2_transient_undo_failure_retries_and_restores_old_version() {
    let fx = Fx::new();
    assert_eq!(fx.run(&[]).code, 0);
    mv_fake(
        &fx,
        r#"case "$1" in
          *watane.app.new.*) exit 1 ;;
          *.old.*) if [ ! -f "$HOME/undo_failed_once" ]; then
            : >"$HOME/undo_failed_once"
            exit 1
          fi ;;
        esac"#,
    );
    let out = fx.run(&[]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(fx.home.join("undo_failed_once").exists());
    assert!(
        out.stderr
            .contains("Nothing was changed: the version that was installed still works."),
        "{out:?}"
    );
    assert!(!out.stderr.contains("could not be put back"), "{out:?}");
    fx.assert_installed("0.1.0");
}

/// 3.9: an old entry still under its `.old.<pid>` name was not put back, even when its
/// path holds the new copy (removing the new copy failed, so undo stopped before the move).
#[test]
fn validator_undo_stopped_before_restoring_reports_files_not_put_back() {
    let fx = Fx::new();
    assert_eq!(fx.run(&[]).code, 0);
    mv_fake(&fx, "case \"$1\" in *receipt.new.*) exit 1 ;; esac");
    let real = which("rm");
    let p = fx.fakes.join("rm");
    fs::write(
        &p,
        format!(
            "#!/bin/sh\ncase \"$*\" in *\"/.local/bin/watane\") exit 1 ;; esac\nexec {} \"$@\"\n",
            real.display()
        ),
    )
    .expect("fake");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
    let out = fx.run(&[]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(!out.stderr.contains("Nothing was changed"), "{out:?}");
    assert!(
        out.stderr.contains("The files could not be put back."),
        "{out:?}"
    );
}
