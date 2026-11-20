//! S89 user-sim: what Alice reads after a run is true when she acts on it (install/TECHNICAL 3.7, 3.9).

use crate::support::*;
use std::process::Command;

const LATER: &str = " | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2";

/// With a mirror that is not https (3.2: a set `WATANE_RUSTUP_URL` gets no `--proto '=https'`),
/// the offer downloads it, but the "install it later" line keeps `--proto '=https'`, so pasting
/// it fails with curl's `Protocol "file" not supported or disabled in libcurl`.
#[test]
fn regress_s89_install_it_later_line_runs_for_the_mirror_it_names() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_env(&["--no-toolchain"], &[("WATANE_RUSTUP_URL", &url)]);
    assert_eq!(out.code, 0, "{out:?}");
    let line = out
        .stdout
        .lines()
        .find(|l| l.starts_with("  curl ") && l.ends_with(LATER))
        .unwrap_or_else(|| panic!("no install-it-later line: {out:?}"))
        .trim()
        .to_string();
    assert!(line.contains(&url), "{line}");
    // Alice pastes the line into her shell.
    let pasted = Command::new("/bin/sh")
        .args(["-c", &line])
        .env_clear()
        .env("HOME", &fx.home)
        .env("PATH", fx.path_value())
        .env("FAKE_STATE", &fx.state)
        .output()
        .expect("run pasted line");
    assert!(
        pasted.status.success(),
        "the pasted line failed: {}",
        String::from_utf8_lossy(&pasted.stderr)
    );
    // curl's failure is hidden by the pipe's status; the installer never ran
    assert!(
        fx.home.join(".cargo/bin/rustup").is_file(),
        "the pasted line installed nothing: {}",
        String::from_utf8_lossy(&pasted.stderr)
    );
}

/// A fresh install (nothing installed before) whose swap fails says "the version that was
/// installed still works": there was no installed version.
#[test]
fn regress_s89_failed_fresh_install_names_no_installed_version() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "fail\n*watane.app.new.*\n");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(fx.home_tree().is_empty(), "{out:?}");
    assert!(
        !out.stderr
            .contains("the version that was installed still works"),
        "{}",
        out.stderr
    );
}
