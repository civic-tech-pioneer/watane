//! Early-check signals and genuine failures on POSIX shells (install/TECHNICAL 3.2, 3.8, 3.9).

mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::*;

fn shells() -> Vec<&'static str> {
    let mut shells = vec!["/bin/sh"];
    if std::path::Path::new("/bin/dash").exists() {
        shells.push("/bin/dash");
    }
    shells
}

fn fixture(shell: &str, body: &str) -> Out {
    let fx = Fx::new();
    fixture_with(shell, &fx, body)
}

fn fixture_with(shell: &str, fx: &Fx, body: &str) -> Out {
    fs::write(
        &fx.script,
        baked_script("0.1.0").replace("main \"$@\"", body),
    )
    .expect("fixture");
    let mut cmd = Command::new(PERL);
    cmd.args(LAUNCH).arg(shell).arg(&fx.script);
    cmd.env_clear().envs(fx.env_pairs());
    exec(cmd)
}

#[test]
fn ac2_interrupted_id_tar_and_shasum_have_no_cause() {
    for shell in shells() {
        for (tool, arg) in [
            ("id", "-u"),
            ("shasum", "-a"),
            ("tar", "-tzf"),
            ("tar", "-tvzf"),
            ("tar", "-xzf"),
        ] {
            for (signal, code) in [("INT", 130), ("HUP", 129), ("TERM", 143), ("PIPE", 141)] {
                let fx = Fx::new();
                let fake = fx.fakes.join(tool);
                fs::write(
                    &fake,
                    format!(
                        r#"#!/bin/sh
if [ "$1" = '{arg}' ]; then
  kill -s {signal} -- -"$FAKE_INSTALL_PID"
  exit {code}
fi
exit 0
"#
                    ),
                )
                .expect("fake tool");
                fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("chmod");
                let body = match tool {
                    "id" => "main --release 0.1.0",
                    "shasum" => "verify_archive",
                    _ => "extract_archive",
                };
                let out = fixture_with(
                    shell,
                    &fx,
                    &format!(
                        r#"
trap cleanup EXIT
trap 'on_signal 129' HUP
trap 'on_signal 130' INT
trap 'on_signal 143' TERM
trap 'on_signal 141' PIPE
TMP=$TMPDIR
ARCHIVE=fixture
RELEASE=0.1.0
printf 'abc fixture\n' >"$TMP/SHA256SUMS"
{body}
"#
                    ),
                );
                assert_eq!(out.code, code, "{shell} {tool} {arg} {signal}: {out:?}");
                assert!(
                    !out.stderr.contains("watane install:"),
                    "{shell} {tool} {arg} {signal}: {out:?}"
                );
            }
        }
    }
}

#[test]
fn ac2_version_checks_need_only_builtins() {
    for shell in shells() {
        for version in ["0.1.0", "01.002.3", "1.2.3-rc1.A9.0"] {
            let out = fixture(
                shell,
                &format!("PATH=/nonexistent\ncheck_version '{version}'"),
            );
            assert_eq!(out.code, 0, "{shell}: {version}: {out:?}");
        }
    }
}

#[test]
fn ac3_bad_versions_keep_their_text_and_code() {
    for shell in shells() {
        for version in [
            "",
            "1.2",
            "1.2.3.4",
            "1.2.3-",
            "1.2.3-a..b",
            "1.2.3-a-1",
            "1.2.3/x",
            "1.2.3\n",
            "a.2.3",
        ] {
            let out = fixture(shell, &format!("check_version '{version}'"));
            assert_eq!(out.code, 1, "{shell}: {out:?}");
            assert_eq!(
                out.stderr,
                format!("watane install: not a version: {version}\na version looks like 0.1.0\n")
            );
        }
    }
}

#[test]
fn ac2_a_signal_before_a_check_failure_has_no_cause() {
    for shell in shells() {
        for (signal, code) in [("INT", 130), ("HUP", 129), ("TERM", 143), ("PIPE", 141)] {
            let out = fixture(
                shell,
                &format!(
                    "trap cleanup EXIT\ntrap 'on_signal {code}' {signal}\nkill -s {signal} $$\ndie 2 'the tool id is missing'"
                ),
            );
            assert_eq!(out.code, code, "{shell}: {out:?}");
            assert!(out.stderr.is_empty(), "{shell}: {out:?}");
        }
    }
}

/// A group INT can kill the child before bash 3.2 dispatches (or loses) the parent's trap.
#[test]
fn ac2_a_signalled_child_status_has_no_cause() {
    for shell in shells() {
        for code in [129, 130, 141, 143] {
            let out = fixture(
                shell,
                &format!("trap cleanup EXIT\n(exit {code}) || die 2 'the tool id is missing'"),
            );
            assert_eq!(out.code, code, "{shell}: {out:?}");
            assert!(out.stderr.is_empty(), "{shell}: {out:?}");
        }
    }
}

#[test]
fn ac2_checksum_lines_need_only_builtins() {
    for shell in shells() {
        for (lines, code, cause) in [
            ("abc other\n", 4, "SHA256SUMS has no line for fixture"),
            (
                "abc fixture\nabc fixture\n",
                4,
                "SHA256SUMS has more than one line for fixture",
            ),
            ("other other\n\tabc\tfixture", 0, ""),
            ("bad fixture\n", 4, "checksum mismatch for fixture"),
        ] {
            let out = fixture(
                shell,
                &format!(
                    r#"
TMP=$TMPDIR
ARCHIVE=fixture
RELEASE=0.1.0
printf '%s' '{lines}' >"$TMP/SHA256SUMS"
shasum() {{ printf 'abc  fixture\n'; }}
PATH=/nonexistent
verify_archive
"#
                ),
            );
            assert_eq!(out.code, code, "{shell}: {out:?}");
            if code == 0 {
                assert!(out.stderr.is_empty(), "{shell}: {out:?}");
            } else {
                assert_eq!(out.first_err(), format!("watane install: {cause}"));
            }
        }
    }
}

/// Validator R1 (3.8): only the checks before staging defer a signal; after the commit it ends the run.
#[test]
fn a_signal_after_the_commit_ends_the_run_at_once() {
    for (signal, code) in [("INT", 130), ("TERM", 143)] {
        let fx = Fx::new();
        // The fake exits 1: only the trap, not a signalled status, can end the run with the code.
        let fake = fx.fakes.join("rustup");
        fs::write(
            &fake,
            format!("#!/bin/sh\nkill -s {signal} \"$FAKE_INSTALL_PID\"\nexit 1\n"),
        )
        .expect("fake rustup");
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("chmod");
        let out = fx.run(&[]);
        assert_eq!(out.code, code, "{signal}: {out:?}");
        fx.assert_installed("0.1.0");
        assert!(!out.stdout.contains("no terminal"), "{signal}: {out:?}");
        assert!(!out.stdout.contains("is installed."), "{signal}: {out:?}");
    }
}

/// Validator R1 (3.8): an uninstall is not deferred; a signal stops it before its last steps.
#[test]
fn a_signal_during_the_uninstall_ends_it_at_once() {
    let fx = Fx::new();
    assert_eq!(fx.run(&[]).code, 0);
    fx.state_write("rm_fault", "term\n*watane-node\n");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stdout.contains("uninstalled watane"), "{out:?}");
}
