//! Unpack modes under a restrictive umask (TECHNICAL.md §6.4 and §11 `unpack`):
//! directories are 0755 and files 0644 whatever the umask of the process.
//!
//! The umask is process-wide and the crate forbids `unsafe`, so the test runs
//! itself again in a child process started by `sh` after `umask 077`. This file
//! holds this one test so no other test shares the child's umask.
#![cfg(unix)]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use watane_pkg::Package;

const CHILD: &str = "WATANE_PKG_UMASK_CHILD";
const NAME: &str = "ac5_unpack_modes_ignore_umask";

// Deliberately no sorting, validation, or use of the package encoder.
fn raw(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = b"WATANEPK\x01".to_vec();
    bytes.extend_from_slice(&(files.len() as u32).to_be_bytes());
    for (path, data) in files {
        bytes.extend_from_slice(&(path.len() as u16).to_be_bytes());
        bytes.extend_from_slice(path.as_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
    }
    bytes
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn ac5_unpack_modes_ignore_umask() {
    if std::env::var_os(CHILD).is_none() {
        let status = Command::new("sh")
            .arg("-c")
            .arg("umask 077 && exec \"$0\" --exact \"$1\" --test-threads=1 --nocapture")
            .arg(std::env::current_exe().unwrap())
            .arg(NAME)
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success(), "the child run under umask 077 failed");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    // The umask is in effect: a plain new file gets no group or other bits.
    let probe = tmp.path().join("probe");
    fs::write(&probe, b"").unwrap();
    assert_eq!(mode(&probe), 0o600);

    let p = Package::from_bytes(raw(&[
        ("mod.toml", b"name = \"Hello\"\nsummary = \"Says hello.\"\n"),
        ("ui/css/site.css", b"p {}\n"),
        ("ui/index.html", b"<p>hello</p>\n"),
        (
            "wit/world.wit",
            b"package mods:hello@0.1.0;\n\nworld hello {}\n",
        ),
    ]))
    .unwrap();
    let dest = tmp.path().join("new");
    p.unpack(&dest).unwrap();
    for (path, expected) in [
        ("", 0o755),
        ("ui", 0o755),
        ("ui/css", 0o755),
        ("wit", 0o755),
        ("mod.toml", 0o644),
        ("ui/css/site.css", 0o644),
        ("ui/index.html", 0o644),
        ("wit/world.wit", 0o644),
    ] {
        assert_eq!(mode(&dest.join(path)), expected, "{path}");
    }
}
