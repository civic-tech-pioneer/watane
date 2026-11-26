//! Early-check signals and genuine failures on POSIX shells (install/TECHNICAL 3.2, 3.8, 3.9).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use crate::support::*;

const RETRY: &str = "\nwatane setup was interrupted. Run the same command to try again.\n";

/// `/bin/sh` and `/bin/dash`, when they are two shells: on a machine where one is the other
/// (most Linux), the second run of every case would prove nothing.
fn shells() -> Vec<&'static str> {
    let mut shells = vec!["/bin/sh"];
    let same = std::fs::canonicalize("/bin/dash").ok() == std::fs::canonicalize("/bin/sh").ok();
    if std::path::Path::new("/bin/dash").exists() && !same {
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

/// TECHNICAL 3.8 (S50b-fix ac2): a signal that kills `id`, `shasum` or `tar` has no cause line, for each of four signals.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
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
            assert_eq!(
                out.stderr,
                if code == 130 { RETRY } else { "" },
                "{shell}: {out:?}"
            );
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
            assert_eq!(
                out.stderr,
                if code == 130 { RETRY } else { "" },
                "{shell}: {out:?}"
            );
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

/// Validator R1 (3.8): a signal after the commit ends the run.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
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

/// Validator R1 (3.8): a signal stops an uninstall before its last steps.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn a_signal_during_the_uninstall_ends_it_at_once() {
    let fx = Fx::installed();
    fx.state_write("rm_fault", "term\n*watane-node\n");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stdout.contains("uninstalled watane"), "{out:?}");
}

/// Validator R1 (3.8): cleanup ends a run that recorded a signal with its code, whatever exit it saw.
#[test]
fn cleanup_keeps_a_recorded_signal_code() {
    for shell in shells() {
        let out = fixture(
            shell,
            "trap cleanup EXIT\ntrap 'on_signal 143' TERM\nkill -s TERM $$\nexit 0",
        );
        assert_eq!(out.code, 143, "{shell}: {out:?}");
        assert!(out.stderr.is_empty(), "{shell}: {out:?}");
    }
}

/// AC1: a trap dispatched between builtins must exit before the next check.
#[test]
fn ac1_a_signal_before_the_swap_ends_the_run_at_once() {
    for shell in shells() {
        for (signal, code) in [("INT", 130), ("HUP", 129), ("TERM", 143), ("PIPE", 141)] {
            let out = fixture(
                shell,
                &format!(
                    "trap cleanup EXIT\ntrap 'on_signal {code}' {signal}\nkill -s {signal} $$\nprintf 'continued after signal\\n'\ncheck_signal"
                ),
            );
            assert_eq!(out.code, code, "{shell}: {out:?}");
            assert!(out.stdout.is_empty(), "{shell}: {out:?}");
            assert_eq!(
                out.stderr,
                if code == 130 { RETRY } else { "" },
                "{shell}: {out:?}"
            );
        }
    }
}

/// AC3: failure branches must preserve a child's signal status before printing a cause.
#[test]
fn ac3_signalled_download_and_toolchain_have_no_cause() {
    for shell in shells() {
        for code in [129, 130, 141, 143] {
            for body in [
                format!(
                    "TMP=$TMPDIR\ncurl() {{ return {code}; }}\ndownload fixture \"$TMP/archive\" plain"
                ),
                format!("TMP=$TMPDIR\nrustup_step fixture /bin/sh -c 'exit {code}'"),
            ] {
                let out = fixture(shell, &format!("trap cleanup EXIT\n{body}"));
                assert_eq!(out.code, code, "{shell}: {body}: {out:?}");
                assert_eq!(
                    out.stderr,
                    if code == 130 { RETRY } else { "" },
                    "{shell}: {body}: {out:?}"
                );
            }
        }
    }
}

/// Validator (3.8): the flag and foreign-file failure paths let a recorded signal end the run first.
#[test]
fn a_recorded_signal_ends_the_flag_and_foreign_paths_without_a_cause() {
    for shell in shells() {
        for body in [
            "SIGNAL=143\nbad_flag --nope",
            "set_entries\nmkdir -p \"$PREFIX/bin\"\nprintf 'mine\\n' >\"$PREFIX/bin/watane-dev\"\nSIGNAL=143\ncheck_foreign",
        ] {
            let out = fixture(shell, &format!("trap cleanup EXIT\n{body}"));
            assert_eq!(out.code, 143, "{shell}: {body}: {out:?}");
            assert!(out.stderr.is_empty(), "{shell}: {body}: {out:?}");
            // Without the recorded signal the same body prints its cause.
            let out = fixture(
                shell,
                &format!("trap cleanup EXIT\n{}", body.replace("SIGNAL=143\n", "")),
            );
            assert_ne!(out.code, 143, "{shell}: {body}: {out:?}");
            assert!(
                out.first_err().starts_with("watane install: "),
                "{shell}: {body}: {out:?}"
            );
        }
    }
}

/// TECHNICAL 3.8: the trap works while the downloaded script is still arriving.
#[test]
fn ctrl_c_before_main_prints_the_retry_hint() {
    let script = include_str!("../../install.sh");
    let (startup, _) = script.split_once("usage() {").expect("startup");
    for shell in shells() {
        // Send INT at the first point after the early trap is installed.
        let mut cmd = Command::new(PERL);
        cmd.args(LAUNCH)
            .args([shell, "-c", &format!("{startup}kill -s INT $$\n")]);
        let out = exec(cmd);
        assert_eq!(out.code, 130, "{shell}: {out:?}");
        assert_eq!(out.stderr, RETRY, "{shell}: {out:?}");
    }
}

/// TECHNICAL 3.8: an INT detected during exit cleanup still prints a single hint.
#[test]
fn ctrl_c_during_cleanup_prints_the_retry_hint() {
    for shell in shells() {
        let out = fixture(
            shell,
            "trap cleanup EXIT\nTMP=$TMPDIR\nrm() { return 130; }\nexit 0",
        );
        assert_eq!(out.code, 130, "{shell}: {out:?}");
        assert_eq!(out.stderr, RETRY, "{shell}: {out:?}");
    }
}

/// TECHNICAL 3.8: the producer and shell receive INT, then the input pipe closes.
fn interrupt_stream(shell: &str, tail: &str) -> Out {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let source = include_str!("../../install.sh");
    let (startup, _) = source.split_once("usage() {").expect("startup");
    let mut cmd = Command::new(PERL);
    cmd.args(LAUNCH)
        .arg(shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_run(&mut cmd);
    let mut input = child.stdin.take().expect("input");
    writeln!(input, "{startup}printf 'ready\\n'\n{tail}").expect("write script prefix");
    let mut ready = String::new();
    BufReader::new(child.stdout.take().expect("output"))
        .read_line(&mut ready)
        .expect("read readiness");
    assert_eq!(ready, "ready\n");
    signal_group(child.id(), "INT");
    drop(input);
    finish(child)
}

#[test]
fn ctrl_c_in_a_stream_between_commands_exits_130() {
    for shell in shells() {
        let out = interrupt_stream(shell, "");
        assert_eq!(out.code, 130, "{shell}: {out:?}");
    }
}

#[test]
fn ctrl_c_in_a_stream_inside_a_function_exits_130() {
    for shell in shells() {
        let out = interrupt_stream(shell, "unfinished() {");
        assert_eq!(out.code, 130, "{shell}: {out:?}");
    }
}
