#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn ac1_runner_mounts_caches_forwards_arguments_and_propagates_failures() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path();
    executable(
        &bin.join("docker"),
        r#"#!/bin/bash
printf '%s\n' "$@" >> "$DOCKER_LOG"
if [ "$1" = build ]; then exit "${BUILD_EXIT:-0}"; fi
while [ "$1" != bash ]; do shift; done
exec "$@"
"#,
    );
    executable(
        &bin.join("cargo"),
        r#"#!/bin/bash
printf '<%s>' "$@" >> "$CARGO_LOG"
printf '\n' >> "$CARGO_LOG"
exit "${DRIVER_EXIT:-0}"
"#,
    );
    for (name, body) in [
        ("dbus-run-session", "exec \"$@\""),
        ("xvfb-run", "shift; exec \"$@\""),
        ("pkg-config", "exit 0"),
        ("openbox", "exit 0"),
        (
            "xprop",
            "echo '_NET_SUPPORTING_WM_CHECK(WINDOW): window id # 0x123'",
        ),
    ] {
        executable(&bin.join(name), &format!("#!/bin/bash\n{body}\n"));
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/linux-test.sh");
    for (variable, code, calls) in [
        ("BUILD_EXIT", 7, 0),
        ("DRIVER_EXIT", 9, 1),
        ("DRIVER_EXIT", 0, 1),
    ] {
        let docker_log = bin.join("docker.log");
        let cargo_log = bin.join("cargo.log");
        fs::write(&docker_log, "").unwrap();
        fs::write(&cargo_log, "").unwrap();
        let status = Command::new(&script)
            .args(if code == 0 {
                vec![]
            } else {
                vec!["a filter with spaces"]
            })
            .current_dir(bin)
            .env(
                "PATH",
                format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            )
            .env("DOCKER_LOG", &docker_log)
            .env("CARGO_LOG", &cargo_log)
            .env(variable, code.to_string())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(code));
        let cargo = fs::read_to_string(cargo_log).unwrap();
        assert_eq!(cargo.lines().count(), calls);
        for line in cargo.lines() {
            assert!(line.starts_with("<test><-p><watane-app>"));
            if code != 0 {
                assert!(line.contains("<a filter with spaces>"));
            }
        }
        if calls == 1 {
            assert!(cargo.contains("<--features><driver><--><--include-ignored>"));
        }
        let docker = fs::read_to_string(docker_log).unwrap();
        assert!(docker.contains("linux/arm64"));
        if calls > 0 {
            assert!(docker.contains(
                "type=volume,source=watane-app-linux-registry,target=/usr/local/cargo/registry"
            ));
            assert!(
                docker.contains("type=volume,source=watane-app-linux-target,target=/linux-target")
            );
            assert!(docker.contains("CARGO_TARGET_DIR=/linux-target"));
            assert!(docker.contains("target=/repo"));
        }
    }
}

#[test]
fn ac2_front_tests_compile_but_runner_skips_every_front_name() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let script = fs::read_to_string(root.join("scripts/linux-test.sh")).unwrap();
    for name in ["gui_shell.rs", "gui_isolation.rs", "j2.rs"] {
        let source = fs::read_to_string(root.join("tests").join(name)).unwrap();
        for test in source.split("#[test]").skip(1) {
            if test.contains("#[ignore = \"front:") {
                let name = test.split("fn ").nth(1).unwrap().split('(').next().unwrap();
                assert!(script.contains(&format!("--skip {name}")), "missing {name}");
            }
        }
    }
    assert!(!script.contains("--skip ac5_journey_j2"));
}

#[test]
fn ac6_linux_suite_and_stage_platform_are_documented() {
    let spec =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    let plan = spec
        .split("## 11.")
        .nth(1)
        .unwrap()
        .split("## 13.")
        .next()
        .unwrap();
    assert!(plan.contains("scripts/linux-test.sh"));
    assert!(plan.contains("arm64 Linux in Docker"));
}
