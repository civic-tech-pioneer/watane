//! Clean-Mac preflight and host VM driver contracts (TECHNICAL 12.3–12.5).

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
    let temp = tempfile::tempdir().expect("temp");
    let home = temp.path().join("home");
    fs::create_dir(&home).expect("home");
    fs::write(temp.path().join("Cargo.toml"), "[workspace]\n").expect("manifest");
    let output = run_clean(&home);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("12.4: checkout at"),
        "{output:?}"
    );
    assert!(!home.join(".local").exists());
}

#[test]
fn ac3_scripts_are_posix_and_shellcheck_clean() {
    for name in ["clean_mac.sh", "vm_mac.sh"] {
        let out = Command::new("/bin/sh")
            .arg("-n")
            .arg(script(name))
            .output()
            .expect("sh -n");
        assert!(out.status.success(), "{name}: {out:?}");
        if Command::new("shellcheck").arg("--version").output().is_ok() {
            let out = Command::new("shellcheck")
                .args(["-s", "sh"])
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
    for (fail_guest, prepare) in [(false, false), (true, false), (false, true)] {
        let temp = tempfile::tempdir().expect("temp");
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).expect("bin");
        let log = temp.path().join("calls");
        let fake = r#"#!/bin/sh
printf '%s %s\n' "${0##*/}" "$*" >> "$CALLS"
case ${0##*/}:$1 in
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
            let expected = count + usize::from(prepare && verb == "clone") * 2;
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
            assert!(
                calls.contains("ghcr.io/cirruslabs/macos-golden-gate-vanilla:latest watane-clean"),
                "{calls}"
            );
            assert!(calls.contains("softwareupdate --install"), "{calls}");
            assert!(
                calls.contains("--from /Users/admin/watane-release"),
                "{calls}"
            );
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
