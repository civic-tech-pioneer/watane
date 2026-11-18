//! `watane-dev --version` prints `watane-dev <version>` (install TECHNICAL 11 stage 2).
use std::process::Command;

#[test]
fn ac3_version_prints_name_and_version() {
    let out = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("watane-dev {}\n", env!("CARGO_PKG_VERSION"))
    );
}
