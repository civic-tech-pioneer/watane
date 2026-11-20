//! Clean-Mac preflight and host VM driver contracts (TECHNICAL 12.3–12.6).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn script(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn run_clean(home: &Path) -> Output {
    Command::new("/bin/sh")
        .arg(script("clean_mac.sh"))
        .args(["--from", "/nonexistent-release", "--part", "clean"])
        .env_clear()
        .env("HOME", home)
        .env(
            "PATH",
            format!(
                "{}/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                home.display()
            ),
        )
        .output()
        .expect("clean_mac")
}

#[test]
fn ac3_refuses_cargo_in_home_before_install() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("temp");
    let home = temp.path().join("home");
    fs::create_dir_all(home.join(".cargo/bin")).expect("cargo dir");
    fs::write(home.join(".cargo/bin/cargo"), "#!/bin/sh\nexit 0\n").expect("cargo");
    fs::set_permissions(
        home.join(".cargo/bin/cargo"),
        fs::Permissions::from_mode(0o755),
    )
    .expect("executable cargo");
    let output = run_clean(&home);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.contains("12.4:") && err.contains(".cargo exists"),
        "{output:?}"
    );
    assert!(!home.join(".local").exists());
}

#[test]
fn ac3_refuses_checkout_above_home_before_install() {
    // A manifest above the home, and a worktree's `.git` file (12.4 uses -e).
    for marker in ["Cargo.toml", ".git"] {
        let temp = tempfile::tempdir().expect("temp");
        let home = temp.path().join("home");
        fs::create_dir(&home).expect("home");
        fs::write(temp.path().join(marker), "[workspace]\n").expect("marker");
        let output = run_clean(&home);
        assert_eq!(output.status.code(), Some(1), "{marker}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("12.4: checkout at"),
            "{marker}: {output:?}"
        );
        assert!(!home.join(".local").exists());
    }
}

#[test]
fn ac3_scripts_are_posix_and_shellcheck_clean() {
    for name in [
        "clean_mac.sh",
        "vm_lib.sh",
        "vm_mac.sh",
        "vm_two_machines.sh",
    ] {
        let out = Command::new("/bin/sh")
            .arg("-n")
            .arg(script(name))
            .output()
            .expect("sh -n");
        assert!(out.status.success(), "{name}: {out:?}");
        if Command::new("shellcheck").arg("--version").output().is_ok() {
            let out = Command::new("shellcheck")
                .args(["-x", "-s", "sh"])
                .arg(script(name))
                .output()
                .expect("shellcheck");
            assert!(out.status.success(), "{name}: {out:?}");
        }
    }
}

#[test]
fn ac4_missing_tart_touches_nothing() {
    let temp = tempfile::tempdir().expect("temp");
    let out = Command::new("/bin/sh")
        .arg(script("vm_mac.sh"))
        .args(["--prepare", "--from", "/not-a-release"])
        .env_clear()
        .env("HOME", temp.path())
        .env("PATH", temp.path())
        .current_dir(temp.path())
        .output()
        .expect("vm driver");
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "vm_mac.sh: tart is not installed (see install/TECHNICAL.md 12.3)\n"
    );
    assert_eq!(fs::read_dir(temp.path()).expect("listing").count(), 0);
}

#[test]
fn ac2_two_machines_missing_tart_touches_nothing() {
    let temp = tempfile::tempdir().expect("temp");
    let out = Command::new("/bin/sh")
        .arg(script("vm_two_machines.sh"))
        .args(["--from", "/not-a-release"])
        .env_clear()
        .env("HOME", temp.path())
        .env("PATH", temp.path())
        .current_dir(temp.path())
        .output()
        .expect("two machines");
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "vm_two_machines.sh: tart is not installed (see install/TECHNICAL.md 12.6)\n"
    );
    assert_eq!(fs::read_dir(temp.path()).expect("listing").count(), 0);
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "slow: needs a release build"]
fn ac1_ac2_fresh_home_real_release() {
    let release = tempfile::tempdir().expect("release");
    let out = Command::new("/bin/sh")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("package.sh"))
        .args(["aarch64-apple-darwin", "0.1.0"])
        .arg(release.path())
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .expect("package prebuilt release");
    assert!(out.status.success(), "{out:?}");
    let out = Command::new("/bin/sh")
        .arg(script("clean_mac.sh"))
        .arg("--from")
        .arg(release.path())
        .arg("--fresh-home")
        .output()
        .expect("fresh-home");
    assert!(out.status.success(), "{out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).lines().last(),
        Some("CLEAN-MAC PASS")
    );
}

#[test]
fn ac4_vm_lifecycle_and_failure_cleanup_without_a_vm() {
    use std::os::unix::fs::PermissionsExt;
    for (fail_guest, prepare, has_clean) in [
        (false, false, false),
        (true, false, false),
        (false, true, false),
        (false, true, true),
    ] {
        let temp = tempfile::tempdir().expect("temp");
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        let log = temp.path().join("calls");
        let fake = r#"#!/bin/sh
printf '%s %s\n' "${0##*/}" "$*" >> "$CALLS"
case ${0##*/}:$1 in
  tart:get) [ "$HAS_CLEAN" = yes ] || exit 1 ;;
  tart:ip) echo 192.0.2.10 ;;
  ssh:*)
    [ "$SSH_ASKPASS_REQUIRE" = force ] && [ "$("$SSH_ASKPASS")" = admin ] || exit 3
    case "$*" in
      *clean_mac.sh*)
        if [ "$FAIL_GUEST" = yes ]; then echo 'guest assertion failed' >&2; exit 1; fi
        echo 'CLEAN-MAC PASS' ;;
    esac ;;
esac
"#;
        for name in ["tart", "ssh", "scp"] {
            let path = bin.join(name);
            fs::write(&path, fake).expect("fake");
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let mut command = Command::new("/bin/sh");
        command.arg(script("vm_mac.sh"));
        if prepare {
            let release = temp.path().join("release with spaces");
            fs::create_dir(&release).expect("release");
            for asset in ["install.sh", "SHA256SUMS"] {
                fs::write(release.join(asset), "fixture").expect("asset");
            }
            command.arg("--prepare").arg("--from").arg(release);
        }
        let out = command
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("CALLS", &log)
            .env("FAIL_GUEST", if fail_guest { "yes" } else { "no" })
            .env("HAS_CLEAN", if has_clean { "yes" } else { "no" })
            .output()
            .expect("vm driver");
        assert_eq!(out.status.code(), Some(i32::from(fail_guest)), "{out:?}");
        let calls = fs::read_to_string(log).expect("calls");
        let count = if fail_guest {
            1
        } else if prepare {
            3
        } else {
            2
        };
        for verb in ["clone", "run --no-graphics", "ip", "stop", "delete"] {
            // --prepare clones watane-clean only when it is not there yet, then watane-clean-clt.
            let expected =
                count + usize::from(prepare && verb == "clone") * (2 - usize::from(has_clean));
            assert_eq!(
                calls
                    .lines()
                    .filter(|line| line.starts_with(&format!("tart {verb} ")))
                    .count(),
                expected,
                "{calls}"
            );
        }
        if prepare {
            assert_eq!(
                calls.contains(
                    "clone ghcr.io/cirruslabs/macos-golden-gate-vanilla:latest watane-clean\n"
                ),
                !has_clean,
                "{calls}"
            );
            assert!(calls.contains("softwareupdate --install"), "{calls}");
            assert!(
                calls.contains("--from /Users/admin/watane-release"),
                "{calls}"
            );
        }
        for line in calls
            .lines()
            .filter(|l| l.starts_with("tart stop ") || l.starts_with("tart delete "))
        {
            let name = line.split(' ').nth(2).unwrap_or_default();
            assert!(name.starts_with("watane-run-"), "{calls}");
        }
        assert!(calls.contains("--wait"), "{calls}");
        assert!(calls.contains("ssh -tt"), "{calls}");
        assert!(!calls.contains("--dir"), "{calls}");
        if !fail_guest {
            assert!(
                calls.contains("--part clean") && calls.contains("--part clt"),
                "{calls}"
            );
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).lines().last(),
                Some("VM PASS")
            );
        }
    }
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac1_pty_delivers_answer_after_prompt_without_terminal() {
    let source = fs::read_to_string(script("clean_mac.sh")).expect("script");
    let function = source
        .split("pty() {")
        .nth(1)
        .expect("pty function")
        .split("install_release() {")
        .next()
        .expect("end of function");
    let temp = tempfile::tempdir().expect("temp");
    let helper = temp.path().join("answer.sh");
    fs::write(&helper, format!("pty() {{{function}\npty delete 'keep it: ' sh -c 'printf \"keep it: \"; sleep 1; read -r answer </dev/tty; [ \"$answer\" = delete ] && echo ANSWER-PASS'\n")).expect("helper");
    let out = Command::new("/usr/bin/perl")
        .args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV"])
        .arg("/bin/sh")
        .arg(helper)
        .env_clear()
        .env("HOME", temp.path())
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("TERM", "dumb")
        .output()
        .expect("pty");
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("ANSWER-PASS"),
        "{out:?}"
    );
}

#[test]
fn ac1_no_clt_linkage_rejects_non_system_libraries_and_bad_macho() {
    let source = fs::read_to_string(script("clean_mac.sh")).expect("source");
    let function = source
        .split("native_dependencies() {")
        .nth(1)
        .expect("native dependency reader")
        .split("installed() {")
        .next()
        .expect("end");
    let temp = tempfile::tempdir().expect("temp");
    let helper = temp.path().join("linkage.sh");
    fs::write(
        &helper,
        format!("native_dependencies() {{{function}\nnative_dependencies \"$1\"\n"),
    )
    .expect("helper");
    for (library, ok) in [
        ("/usr/lib/libSystem.B.dylib", true),
        ("/System/Library/Frameworks/AppKit.framework/AppKit", true),
        ("/opt/homebrew/lib/evil.dylib", false),
        ("@rpath/evil.dylib", false),
    ] {
        let size = (24 + library.len() + 1).next_multiple_of(8);
        let mut bytes = Vec::new();
        for value in [
            0xfeed_facf_u32,
            0x0100_000c,
            0,
            2,
            1,
            size as u32,
            0,
            0,
            12,
            size as u32,
            24,
            0,
            0,
            0,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(library.as_bytes());
        bytes.resize(32 + size, 0);
        let binary = temp.path().join("macho");
        fs::write(&binary, &bytes).expect("macho");
        let out = Command::new("/bin/sh")
            .arg(&helper)
            .arg(&binary)
            .output()
            .expect("reader");
        assert_eq!(out.status.success(), ok, "{library}: {out:?}");
        fs::write(&binary, &bytes[..bytes.len() - 1]).expect("truncated");
        let out = Command::new("/bin/sh")
            .arg(&helper)
            .arg(&binary)
            .output()
            .expect("reader");
        assert!(!out.status.success(), "truncated accepted: {out:?}");
    }
}

#[test]
fn ac1_preserves_existing_guest_home_paths_and_rejects_new_shell_files() {
    let source = fs::read_to_string(script("clean_mac.sh")).expect("source");
    let function = source
        .split("home_changes() {")
        .nth(1)
        .expect("home comparison")
        .split("native_dependencies() {")
        .next()
        .expect("end");
    let temp = tempfile::tempdir().expect("temp");
    let home = temp.path().join("home");
    fs::create_dir_all(home.join("Desktop")).expect("existing guest directory");
    let work = temp.path().join("work");
    fs::create_dir(&work).expect("work");
    let before = Command::new("find")
        .arg(&home)
        .arg("-print")
        .output()
        .expect("find");
    assert!(before.status.success());
    fs::write(work.join("home-before"), before.stdout).expect("snapshot");
    fs::create_dir_all(home.join(".local/bin")).expect("installed directory");
    let helper = temp.path().join("compare.sh");
    fs::write(&helper, format!("H=$1; work=$2; mode=clean\nfail() {{ echo \"$*\" >&2; exit 1; }}\ncheck() {{ \"$@\" || fail \"$*\"; }}\nhome_changes() {{{function}\nhome_changes\n")).expect("helper");
    let run = || {
        Command::new("/bin/sh")
            .arg(&helper)
            .arg(&home)
            .arg(&work)
            .output()
            .expect("comparison")
    };
    let out = run();
    assert!(out.status.success(), "{out:?}");
    fs::write(home.join(".zshrc"), "unexpected change").expect("shell file");
    let out = run();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(".zshrc"),
        "{out:?}"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn ac1_relative_release_path_survives_fresh_home() {
    let temp = tempfile::tempdir().expect("temp");
    fs::create_dir(temp.path().join("release")).expect("release");
    fs::write(
        temp.path().join("release/install.sh"),
        "echo RELATIVE-RELEASE\nexit 29\n",
    )
    .expect("fixture");
    let out = Command::new("/bin/sh")
        .arg(script("clean_mac.sh"))
        .args(["--from", "release", "--fresh-home"])
        .current_dir(temp.path())
        .output()
        .expect("relative release");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("RELATIVE-RELEASE"),
        "{out:?}"
    );
}

#[test]
fn ac4_term_during_guest_run_stops_and_deletes_only_the_run_vm() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().expect("temp");
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    let log = temp.path().join("calls");
    let marker = temp.path().join("guest-running");
    let fake = r#"#!/bin/sh
printf '%s %s\n' "${0##*/}" "$*" >> "$CALLS"
case ${0##*/}:$1:$* in
  tart:ip:*) echo 192.0.2.10 ;;
  ssh:*clean_mac.sh*) : > "$MARKER"; sleep 2 ;;
esac
"#;
    for name in ["tart", "ssh", "scp"] {
        let path = bin.join(name);
        fs::write(&path, fake).expect("fake");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let mut child = Command::new("/bin/sh")
        .arg(script("vm_mac.sh"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("CALLS", &log)
        .env("MARKER", &marker)
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("vm driver");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !marker.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "guest run never started"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let killed = Command::new("/bin/kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("kill");
    assert!(killed.success());
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(1), "{status:?}");
    let calls = fs::read_to_string(log).expect("calls");
    let run = format!("watane-run-{}", child.id());
    assert!(calls.contains(&format!("tart stop {run}\n")), "{calls}");
    assert!(calls.contains(&format!("tart delete {run}\n")), "{calls}");
    assert!(!calls.contains("--part clt"), "{calls}");
}

#[test]
fn ac2_ac4_two_machines_lifecycle_failure_and_term_cleanup() {
    two_machine_modes(&[
        "pass", "latest", "install", "mount", "copy", "ip", "journey", "missing", "term",
        "vanished", "empty",
    ]);
}

#[test]
fn ac3_two_machines_hung_ssh_ends_run() {
    two_machine_modes(&["hang-ssh"]);
}

fn two_machine_modes(modes: &[&str]) {
    use std::os::unix::fs::PermissionsExt;
    for &mode in modes {
        let temp = tempfile::Builder::new()
            .prefix("two machines' ")
            .tempdir()
            .expect("temp");
        let bin = temp.path().join("bin");
        let tests = temp.path().join("install/tests");
        fs::create_dir_all(&bin).expect("bin");
        fs::create_dir_all(&tests).expect("tests");
        fs::create_dir_all(temp.path().join("target/debug")).expect("target");
        fs::write(temp.path().join("target/debug/watane-app"), "fixture").expect("app");
        for name in ["vm_lib.sh", "vm_two_machines.sh"] {
            let source = fs::read_to_string(script(name)).expect("source");
            // Keep the production Apple-signed interpreter; only this offline fixture
            // substitutes the HTTP server so the test needs no listening socket.
            fs::write(
                tests.join(name),
                source.replace(
                    "/usr/bin/python3 -m http.server 0",
                    &format!(
                        "'{}' -m http.server 0",
                        bin.join("python3")
                            .display()
                            .to_string()
                            .replace('\'', "'\\''")
                    ),
                ),
            )
            .expect("copy script");
        }
        let fake = r#"#!/bin/sh
printf '%s %s\n' "${0##*/}" "$*" >> "$CALLS"
case ${0##*/}:$1 in
  plutil:*) [ "$1" != -extract ] || echo '{}' ;;
  cat:*)
    case "$1" in
      */bridge-*.pid)
        if [ "$MODE" = vanished ] || [ "$MODE" = empty ]; then
          pid=$(/bin/cat "$1")
          kill "$pid"
          if [ "$MODE" = vanished ]; then
            # The fake bridge removes its PID file in its TERM handler.
            while [ -f "$1" ]; do sleep 0.02; done
          else
            : > "$1"
          fi
        fi ;;
    esac
    exec /bin/cat "$@" ;;
  tart:ip) [ "$MODE" != ip ] || exit 17; echo 192.0.2.10 ;;
  ssh:*)
    [ "$SSH_ASKPASS_REQUIRE" = force ] && [ "$("$SSH_ASKPASS")" = admin ] || exit 3
    case "$*" in
      *bob.sh*)
        trap 'echo bridge-stopped >> "$CALLS"; if [ "$MODE" = vanished ]; then rm -f "${SSH_ASKPASS%/*}"/bridge-*.pid; fi; exit' TERM
        : > "$BRIDGE_READY"
        while :; do sleep 1; done ;;
      *mount*) [ "$MODE" != hang-ssh ] || exec /usr/bin/perl -e 'sleep 1 while 1' ; [ "$MODE" != mount ] || exit 19 ;;
      *install.sh*) [ "$MODE" != install ] || exit 21 ;;
      *'route -n get default'*) echo 127.0.0.1 ;;
    esac ;;
  scp:*) [ "$MODE" != copy ] || exit 23 ;;
  python3:*)
    trap 'echo web-stopped >> "$CALLS"; exit' TERM
    echo 'Serving HTTP on 127.0.0.1 port 54321 (http://127.0.0.1:54321/) ...'
    while :; do sleep 1; done ;;
  cargo:test)
    case "$*" in
      *two_machines::*)
        [ -d "$WATANE_VM_ALICE_HOME" ] || exit 25
        [ "$WATANE_VM_SITE_BASE" = http://127.0.0.1:54321/ ] || exit 26
        echo "alice=$WATANE_VM_ALICE_HOME" >> "$CALLS"
        sh -c "$WATANE_VM_BOB .watane" >/dev/null 2>&1 &
        # Wait for the fake SSH process to install its TERM handler.
        while [ ! -f "$BRIDGE_READY" ]; do sleep 0.1; done
        : > "$MARKER"
        if [ "$MODE" = term ]; then sleep 2; fi
        [ "$MODE" != journey ] || { echo 'journey assertion failed'; exit 27; }
        [ "$MODE" = missing ] || echo 'test two_machines::ac1_j3_two_machines ... ok' ;;
    esac ;;
esac
"#;
        for name in ["tart", "ssh", "scp", "cargo", "python3", "plutil", "cat"] {
            let path = bin.join(name);
            fs::write(&path, fake).expect("fake");
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let release = temp.path().join("release with spaces");
        fs::create_dir(&release).expect("release");
        for name in ["install.sh", "SHA256SUMS"] {
            fs::write(release.join(name), "fixture").expect("asset");
        }
        let calls = temp.path().join("calls");
        let marker = temp.path().join("ready");
        let tmp = temp.path().join("tmp");
        fs::create_dir(&tmp).expect("tmp");
        let mut command = Command::new("/usr/bin/perl");
        command
            .args(super::support::LAUNCH)
            .arg("/bin/sh")
            .arg(tests.join("vm_two_machines.sh"));
        if mode != "latest" {
            command.arg("--from").arg(&release);
        }
        command
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("CALLS", &calls)
            .env("MARKER", &marker)
            .env("BRIDGE_READY", temp.path().join("bridge-ready"))
            .env("MODE", mode)
            .env(
                "WATANE_VM_TEST_SCALE",
                if mode.starts_with("hang-") { "10" } else { "1" },
            )
            .env("TMPDIR", &tmp)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = super::support::spawn_run(&mut command);
        if mode == "term" {
            let deadline = std::time::Instant::now() + super::support::WAIT_TIMEOUT;
            while !marker.exists() {
                assert!(
                    child.try_wait().expect("poll").is_none(),
                    "driver exited early"
                );
                assert!(
                    std::time::Instant::now() < deadline,
                    "journey never started"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert!(
                Command::new("/bin/kill")
                    .args(["-TERM", &child.id().to_string()])
                    .status()
                    .expect("kill")
                    .success()
            );
        }
        let pid = child.id();
        let out = super::support::finish(child);
        assert_eq!(
            out.code == 0,
            ["pass", "latest", "vanished", "empty"].contains(&mode),
            "{mode}: {out:?}"
        );
        let deadline = std::time::Instant::now() + super::support::WAIT_TIMEOUT;
        let log = loop {
            let log = fs::read_to_string(&calls).expect("calls");
            if ![
                "pass", "latest", "journey", "missing", "term", "vanished", "empty",
            ]
            .contains(&mode)
                || log.contains("bridge-stopped")
            {
                break log;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "bridge did not stop: {log}"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        if mode.starts_with("hang-") {
            let (step, bound) = match mode {
                "hang-ssh" => ("remote_plain", 30),
                _ => unreachable!(),
            };
            assert!(
                out.stderr.contains(step)
                    && out.stderr.contains(&format!("timed out after {bound} s")),
                "{out:?}"
            );
        }
        let run = format!("watane-run-{pid}");
        assert!(
            log.contains(&format!("tart clone watane-clean {run}\n")),
            "{log}"
        );
        for verb in ["stop", "delete"] {
            assert!(log.contains(&format!("tart {verb} {run}\n")), "{log}");
            assert!(
                !log.contains(&format!("tart {verb} watane-clean\n")),
                "{log}"
            );
        }
        assert!(
            !log.lines()
                .filter(|line| line.starts_with("tart run "))
                .any(|line| line.contains("--dir")),
            "{log}"
        );
        assert_eq!(
            fs::read_dir(&tmp).expect("tmp listing").count(),
            0,
            "{out:?}"
        );
        if [
            "pass", "latest", "journey", "missing", "term", "vanished", "empty",
        ]
        .contains(&mode)
        {
            assert!(log.contains("web-stopped"), "{log}");
            assert!(log.contains("bridge-stopped"), "{log}");
            let alice = log
                .lines()
                .find_map(|line| line.strip_prefix("alice="))
                .expect("alice home");
            assert!(!Path::new(alice).exists());
        }
        if ["pass", "latest", "vanished", "empty"].contains(&mode) {
            assert!(
                log.contains("cargo build -p watane-app --features driver"),
                "{log}"
            );
            assert!(log.contains("--test it --no-run"), "{log}");
            let clone_at = log.find("tart clone").expect("clone");
            assert!(log.find("cargo build").expect("build") < clone_at, "{log}");
            assert!(
                log.find("--test it --no-run").expect("compile tests") < clone_at,
                "{log}"
            );
            assert!(log.contains("ssh -T"), "{log}");
            if mode == "latest" {
                assert!(log.contains("curl -fsSL https://github.com/civic-tech-pioneer/watane/releases/latest/download/install.sh | sh"), "{log}");
            } else {
                assert!(
                    log.contains("WATANE_RELEASE_BASE=file:///Users/admin/watane-release"),
                    "{log}"
                );
            }
            assert_eq!(out.stdout.lines().last(), Some("TWO-MACHINES PASS"));
        } else if mode != "term" {
            let command = match mode {
                "install" => "install.sh",
                "mount" => "mount",
                "copy" => "copy",
                "hang-ssh" => "remote_plain",
                "ip" => "tart ip",
                _ => "cargo test",
            };
            assert!(out.stderr.contains(command), "{out:?}");
            assert!(!out.stdout.contains("TWO-MACHINES PASS"));
        }
    }
}

#[test]
fn ac3_guest_bridge_streams_driver_and_reports_open_failure() {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let source = fs::read_to_string(script("vm_two_machines.sh")).expect("source");
    let bridge = source
        .split("<<'BOB'\n")
        .nth(1)
        .expect("bridge")
        .split("\nBOB\n")
        .next()
        .expect("end");
    for mode in ["pass", "fail"] {
        let temp = tempfile::tempdir().expect("temp");
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        let open = bin.join("open");
        fs::write(
            &open,
            r#"#!/bin/sh
[ "$MODE" != fail ] || { echo 'open launch failed' >&2; exit 31; }
while [ "$#" -gt 0 ]; do
  case $1 in -i) input=$2; shift ;; -o) output=$2; shift ;; --stderr) log=$2; shift ;; esac
  shift
done
exec 3<"$input"
exec 4>"$output"
echo '{"event":"ready","port":1234}' >&4
echo 'guest app stderr' > "$log"
while IFS= read -r line <&3; do
  case $line in *quit*) exit 0 ;; *) echo "$line" >&4 ;; esac
done
"#,
        )
        .expect("open");
        fs::set_permissions(open, fs::Permissions::from_mode(0o755)).expect("chmod");
        let helper = temp.path().join("bob.sh");
        fs::write(&helper, bridge).expect("helper");
        let tmp = temp.path().join("tmp");
        fs::create_dir(&tmp).expect("tmp");
        // Bound a broken FIFO handshake without opening a real app or window.
        let mut command = Command::new("/usr/bin/perl");
        command
            .args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV", "sh"])
            .arg(helper)
            .arg("bob-tamper")
            .env("HOME", temp.path())
            .env("TMPDIR", &tmp)
            .env("MODE", mode)
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = super::support::spawn_run(&mut command);
        child
            .stdin
            .take()
            .expect("input")
            .write_all(b"{\"id\":1}\n")
            .expect("write");
        let out = super::support::finish_within(child, super::support::WAIT_TIMEOUT)
            .expect("bridge deadline");
        if mode == "pass" {
            assert!(out.code == 0, "{out:?}");
            let output = &out.stdout;
            assert!(
                output.contains("ready") && output.contains("\"id\":1"),
                "{out:?}"
            );
            assert!(out.stderr.contains("guest app stderr"), "{out:?}");
        } else {
            assert_eq!(out.code, 1, "{out:?}");
            assert!(out.stderr.contains("open"), "{out:?}");
        }
        assert_eq!(fs::read_dir(tmp).expect("listing").count(), 0, "{out:?}");
    }
}

#[test]
fn ac6_two_machine_run_is_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let technical = fs::read_to_string(root.join("TECHNICAL.md")).expect("technical");
    let section = technical
        .split("### 12.6 Two machines (S55)")
        .nth(1)
        .expect("12.6");
    for text in [
        "WATANE_VM_BOB",
        "WATANE_VM_ALICE_HOME",
        "WATANE_VM_SITE_BASE",
        "NAT",
        "consent",
        "vm_two_machines.sh: tart is not installed (see install/TECHNICAL.md 12.6)",
    ] {
        assert!(section.contains(text), "missing {text}");
    }
    assert!(
        technical
            .split("### 12.3")
            .nth(1)
            .expect("12.3")
            .split("### 12.4")
            .next()
            .expect("end")
            .contains("vm_lib.sh")
    );
    assert!(
        fs::read_to_string(root.join("AGENTS.md"))
            .expect("agents")
            .contains("sh tests/vm_two_machines.sh [--from <release-dir>]")
    );
}

#[test]
fn ac3_guest_rejects_each_shared_filesystem() {
    use std::os::unix::fs::PermissionsExt;
    let source = fs::read_to_string(script("vm_two_machines.sh")).expect("source");
    let command = source
        .split("check remote_plain 'set -eu\n")
        .nth(1)
        .expect("mount check")
        .split("'\nif [ -n")
        .next()
        .expect("end");
    let temp = tempfile::tempdir().expect("temp");
    let mount = temp.path().join("mount");
    fs::write(&mount, "#!/bin/sh\nprintf '%s\\n' \"$MOUNT\"\n").expect("mount");
    fs::set_permissions(mount, fs::Permissions::from_mode(0o755)).expect("chmod");
    let shared = temp.path().join("My Shared Files");
    let helper = temp.path().join("check.sh");
    fs::write(
        &helper,
        format!(
            "set -eu\n{}",
            command
                .replace(
                    "/tmp/watane-mounts",
                    &temp.path().join("mounts").display().to_string()
                )
                .replace("/Volumes/My Shared Files", &shared.display().to_string())
        ),
    )
    .expect("helper");
    for filesystem in ["apfs", "virtiofs", "smbfs", "nfs", "afpfs", "webdav"] {
        let out = Command::new("sh")
            .arg(&helper)
            .env("PATH", format!("{}:/usr/bin:/bin", temp.path().display()))
            .env(
                "MOUNT",
                format!("share on /somewhere ({filesystem}, nodev, nosuid)"),
            )
            .output()
            .expect("check");
        assert_eq!(
            out.status.success(),
            filesystem == "apfs",
            "{filesystem}: {out:?}"
        );
    }
    fs::create_dir(shared).expect("shared folder");
    let out = Command::new("sh")
        .arg(helper)
        .env("PATH", format!("{}:/usr/bin:/bin", temp.path().display()))
        .env("MOUNT", "/dev/disk on / (apfs, local)")
        .output()
        .expect("check folder");
    assert!(!out.status.success(), "{out:?}");
}

#[test]
#[cfg(target_os = "macos")]
fn ac1_bob_bundle_copies_release_ats_at_runtime() {
    let source = fs::read_to_string(script("vm_two_machines.sh")).expect("source");
    let bundle = source
        .split("check mkdir -p \"$work/Bob.app/Contents/MacOS\"")
        .nth(1)
        .expect("bundle")
        .split("cat >\"$work/bob.sh\"")
        .next()
        .expect("end");
    let temp = tempfile::tempdir().expect("temp");
    fs::create_dir_all(temp.path().join("Bob.app/Contents/MacOS")).expect("bundle directory");
    let helper = temp.path().join("bundle.sh");
    fs::write(
        &helper,
        format!("set -eu\nwork=$1\ncheck() {{ \"$@\"; }}\n{bundle}"),
    )
    .expect("helper");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("root");
    // Substitute only the prebuilt executable; generate the real plist and copy its live ATS value.
    let helper_source = fs::read_to_string(&helper).expect("helper");
    fs::write(
        &helper,
        helper_source.replace("target/debug/watane-app", "/usr/bin/true"),
    )
    .expect("fixture");
    let out = Command::new("sh")
        .arg(helper)
        .arg(temp.path())
        .current_dir(root)
        .output()
        .expect("bundle");
    assert!(out.status.success(), "{out:?}");
    let extract = |path: &Path| {
        let out = Command::new("plutil")
            .args(["-extract", "NSAppTransportSecurity", "json", "-o", "-"])
            .arg(path)
            .output()
            .expect("plutil");
        assert!(out.status.success(), "{out:?}");
        // plutil's JSON key order is unspecified; XML output is canonical for this dictionary.
        let json = temp.path().join("ats.json");
        fs::write(&json, out.stdout).expect("json");
        let out = Command::new("plutil")
            .args(["-convert", "xml1", "-o", "-"])
            .arg(json)
            .output()
            .expect("xml");
        assert!(out.status.success(), "{out:?}");
        out.stdout
    };
    assert_eq!(
        extract(&temp.path().join("Bob.app/Contents/Info.plist")),
        extract(&root.join("app/Info.plist"))
    );
}

#[test]
fn ac5_two_machine_recovery_and_deadlines_are_documented() {
    let technical = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md"))
        .expect("technical");
    let section = technical
        .split("### 12.6 Two machines (S55)")
        .nth(1)
        .expect("12.6")
        .split("## 13.")
        .next()
        .expect("end");
    for text in [
        "NSAppTransportSecurity",
        "app/Info.plist",
        "bare test binary",
        "set +e",
        "vanished",
        "empty",
        "300 s",
        "900 s",
        "1200 s",
        "3600 s",
        "WATANE_VM_TEST_SCALE",
        "timed out after N s",
    ] {
        assert!(section.contains(text), "missing {text}");
    }
}

#[test]
fn ac3_remote_journey_and_whole_script_deadlines() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().expect("temp");
    let ssh = temp.path().join("ssh");
    fs::write(&ssh, "#!/bin/sh\nexec /usr/bin/perl -e 'sleep 1 while 1'\n").expect("ssh");
    fs::set_permissions(&ssh, fs::Permissions::from_mode(0o755)).expect("chmod");
    fs::copy(&ssh, temp.path().join("scp")).expect("scp");
    fs::copy(&ssh, temp.path().join("cargo")).expect("cargo");
    let source = fs::read_to_string(script("vm_two_machines.sh")).expect("source");
    let config = source
        .split("install_bound=")
        .nth(1)
        .expect("bounds")
        .split("boot watane-clean")
        .next()
        .expect("end bounds");
    let plain = source
        .split("remote_plain() {")
        .nth(1)
        .expect("remote functions")
        .split("check remote_plain")
        .next()
        .expect("end remote functions");
    // The script's own journey launch, so its supervisor and bound are what is tested.
    let journey = source
        .split("deadline_args \"$journey_bound\"")
        .nth(1)
        .expect("journey")
        .split("# Cargo succeeds")
        .next()
        .expect("end journey");
    let helper = temp.path().join("run.sh");
    fs::write(
        &helper,
        format!(
            r#"set -eu
driver=deadline-test
fail() {{ echo "$*" >&2; exit 1; }}
. "$1"
install_bound={config}
ip=192.0.2.1
remote_plain() {{{plain}
case $2 in
  whole) whole_end=0; check touch "$3" ;;
  build) bounded "$whole_bound" "cargo build" ssh ;;
  journey) deadline_args "$journey_bound"{journey} ;;
  remote) check remote true ;;
  copy) check copy fixture admin@192.0.2.1: ;;
  install) check remote_install fixture ;;
esac
"#
        ),
    )
    .expect("helper");
    for mode in ["remote", "copy", "install", "journey", "build", "whole"] {
        let marker = temp.path().join("must-not-run");
        let mut command = Command::new("/usr/bin/perl");
        command
            .args(super::support::LAUNCH)
            .arg("sh")
            .arg(&helper)
            .arg(script("vm_lib.sh"))
            .arg(mode)
            .arg(&marker)
            .env("PATH", format!("{}:/usr/bin:/bin", temp.path().display()))
            .env("WATANE_VM_TEST_SCALE", "100")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = super::support::spawn_run(&mut command);
        let out = super::support::finish_within(child, std::time::Duration::from_secs(90))
            .expect("deadline");
        assert_ne!(out.code, 0, "{out:?}");
        let message = match mode {
            "remote" => "remote timed out after 3 s",
            "copy" => "copy timed out after 3 s",
            "install" => "install timed out after 9 s",
            "journey" => "cargo test two_machines timed out after 12 s",
            _ => "whole script timed out after 36 s",
        };
        assert!(out.stderr.contains(message), "{out:?}");
        assert!(!marker.exists());
    }
}

#[test]
fn ac3_test_scale_cannot_disable_a_bound() {
    let temp = tempfile::tempdir().expect("temp");
    for scale in ["0", "00", "08", "abc", "-3", "1.5"] {
        let out = Command::new("/bin/sh")
            .args([
                "-c",
                "set -eu; driver=scale-test; fail() { echo \"$*\" >&2; exit 1; }; . \"$1\"; echo sourced",
                "sh",
            ])
            .arg(script("vm_lib.sh"))
            .env("WATANE_VM_TEST_SCALE", scale)
            .env("TMPDIR", temp.path())
            .output()
            .expect("source");
        assert!(!out.status.success(), "{scale}: {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("invalid WATANE_VM_TEST_SCALE"),
            "{scale}: {out:?}"
        );
    }
}
