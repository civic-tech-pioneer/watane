//! Regressions found by the S55 user-sim on the two-machine driver (TECHNICAL 12.6).

use std::path::Path;
use std::process::Command;

#[test]
fn regress_s55_missing_from_dir_is_named() {
    let temp = tempfile::tempdir().expect("temp");
    let bin = temp.path().join("bin");
    std::fs::create_dir(&bin).expect("bin");
    // A tart that is never reached: the --from check comes first.
    let tart = bin.join("tart");
    std::fs::write(&tart, "#!/bin/sh\nexit 99\n").expect("tart");
    Command::new("/bin/chmod")
        .arg("755")
        .arg(&tart)
        .status()
        .expect("chmod");
    let missing = temp.path().join("no-such-release");
    let out = Command::new("/bin/sh")
        .arg(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("vm_two_machines.sh"),
        )
        .arg("--from")
        .arg(&missing)
        .env_clear()
        .env("HOME", temp.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .current_dir(temp.path())
        .output()
        .expect("two machines");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    // The driver's own line must name the folder the maintainer passed,
    // not print an empty `cd `.
    let own = stderr
        .lines()
        .find(|line| line.starts_with("vm_two_machines.sh: "))
        .unwrap_or_default();
    assert!(own.contains(missing.to_str().unwrap()), "{stderr}");
}
