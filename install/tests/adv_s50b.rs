//! Adversary tests of S50b: attacks on the Install boundary (spec/TECHNICAL 6, install/TECHNICAL 8.5, 10).

mod support;

use std::fs;
use std::os::unix::fs::symlink;

use support::*;

/// 8.4 step 2 `rmdir`s `<prefix>/bin`, `<prefix>/share/watane-dev`, `<prefix>/share` and
/// `<prefix>/share/watane` without the folder check of 8.5 rule 3: through a link at `~/.local`
/// an uninstall removes empty folders outside the home ("removals only of paths that watane
/// installs"; "no write outside $HOME").
#[test]
fn adv_s50b_uninstall_rmdirs_outside_the_home_through_a_link() {
    let fx = Fx::new();
    let other = tempfile::tempdir().expect("other");
    let o = fs::canonicalize(other.path()).expect("canon");
    let h = fx.home.display().to_string();
    // the person's ~/.local is a link to another disk; there a receipt, and empty folders
    // that are not watane's
    fs::create_dir_all(o.join("bin")).expect("bin");
    fs::create_dir_all(o.join("share/watane-dev")).expect("dev");
    write_file(
        &o.join("share/watane/receipt"),
        &format!(
            "watane-receipt 1\nversion 0.1.0\ntarget aarch64-apple-darwin\npath {h}/.local/bin/watane\n"
        ),
    );
    symlink(&o, fx.home.join(".local")).expect("link");
    let before = fx.tree(&o);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fx.tree(&o), before, "nothing outside the home was removed");
}

/// The real `rustup`, when the machine that runs the tests has one.
fn real_rustup() -> Option<std::path::PathBuf> {
    let out = std::process::Command::new("sh")
        .args(["-c", "command -v rustup"])
        .output()
        .ok()?;
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!p.is_empty()).then(|| std::path::PathBuf::from(p))
}

/// 3.7 detects with `<rustup> which rustc` in the folder the person runs the one-liner from.
/// In a folder with a `rust-toolchain.toml` (any Rust project), rustup installs the toolchain
/// that file names to answer `which`: the toolchain is downloaded with no question asked and
/// no terminal at all ("the toolchain only after a typed yes"). The dist server is the test's
/// own fake server, so the attempt is seen in its request log and nothing leaves the machine.
#[test]
fn adv_s50b_toolchain_installed_without_a_yes_from_a_project_folder() {
    let Some(rustup) = real_rustup() else {
        eprintln!("no rustup on this machine: the attack needs the real one");
        return;
    };
    let fx = Fx::new();
    // rustup present, no toolchain: state `none` B of 3.7
    fs::create_dir_all(fx.home.join(".cargo/bin")).expect("cargo bin");
    symlink(&rustup, fx.home.join(".cargo/bin/rustup")).expect("rustup");
    let project = fx.root.path().join("project");
    write_file(
        &project.join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"stable\"\n",
    );
    let dist = fx.rel.base().replace("/release", "");
    let mut c = fx.cmd(&[]);
    c.current_dir(&project).env("RUSTUP_DIST_SERVER", &dist);
    let out = exec(c);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("no terminal to ask on: the toolchain was not installed"));
    let fetched: Vec<String> = fx
        .rel
        .requests()
        .into_iter()
        .filter(|r| r.starts_with("/dist/"))
        .collect();
    assert!(
        fetched.is_empty(),
        "rustup was asked to download a toolchain with no yes: {fetched:?}"
    );
}
