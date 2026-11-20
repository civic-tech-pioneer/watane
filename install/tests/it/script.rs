//! INST-S-1 to 22, 26 to 28, 30, 31, 34 and 35: runs `install.sh` against a fake release, one whole
//! run per behaviour that only a whole run shows; the rules a function decides are in `unit.rs`
//! (TECHNICAL 12.1).

use crate::support;

use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::support::*;

const CURL_LINE: &str = "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2";

fn archive() -> String {
    archive_name("0.1.0")
}

fn installed() -> Fx {
    Fx::installed()
}

fn names(fx: &Fx) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(&fx.home)
        .expect("home")
        .map(|e| e.expect("e").file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn leftovers(fx: &Fx) -> Vec<String> {
    fx.home_tree()
        .into_iter()
        .filter(|n| n.contains(".new.") || n.contains(".old.") || n.contains("journal"))
        .collect()
}

#[test]
fn ac6_checkout_copy_refuses_to_run() {
    let fx = Fx::new();
    fx.use_checkout_script();
    let out = fx.run(&[]);
    assert_eq!(out.code, 1);
    assert_eq!(out.first_err(), "watane install: not a version: @RELEASE@");
    assert!(fx.home_tree().is_empty());
}

/// A pair `(name, value)` of environment for `run_env`.
fn url_env(fx: &Fx) -> (String, String) {
    ("WATANE_RUSTUP_URL".to_string(), fx.rustup_installer())
}

/// `0.1.0` installed, `0.2.0` served, and a copy of every byte of the home.
fn swap_start() -> (Fx, Vec<(String, Vec<u8>)>) {
    let fx = installed();
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    let before = snapshot(&fx.home);
    (fx, before)
}

fn assert_old_receipt(fx: &Fx) {
    let r = fx.read(&fx.receipt());
    assert!(r.contains("\nversion 0.1.0\n"), "{r}");
    assert_eq!(
        r.lines().filter(|l| l.starts_with("path ")).count(),
        7,
        "{r}"
    );
}

fn assert_all_new(fx: &Fx) {
    fx.assert_installed("0.2.0");
    assert!(fx.share("watane-dev/vendor/new-crate-1.0.0").is_dir());
    assert!(!fx.share("watane-dev/vendor/serde-1.0.0").exists());
    assert!(leftovers(fx).is_empty(), "{:?}", leftovers(fx));
}

/// Starts `0.2.0` with the fake `mv` or `rm` set to sleep, waits until it sleeps, kills the group.
fn kill_at(fx: &Fx, tool: &str, glob: &str, args: &[&str]) {
    kill_at_checking(fx, tool, glob, args, false);
}

/// `kill_at`, and with `check_lock` the next run is stopped by the lock the killed run left
/// (TECHNICAL 3.6.2: a lock found is never broken) and prints the command that removes it.
fn kill_at_checking(fx: &Fx, tool: &str, glob: &str, args: &[&str], check_lock: bool) {
    fx.state_write(&format!("{tool}_fault"), &format!("sleep\n{glob}\n"));
    let child = spawn(&mut fx.cmd(args));
    fx.wait_state(&format!("{tool}_sleeping"));
    let child_pid = child.id();
    signal_group(child_pid, "KILL");
    let out = finish(child);
    assert_eq!(out.code, -1, "killed: {out:?}");
    fx.unfault(&format!("{tool}_fault"));
    fs::remove_file(fx.state.join(format!("{tool}_sleeping"))).expect("unsleep");
    // A killed run keeps its lock (TECHNICAL 3.6.2); direct undo fixtures never take one.
    if !check_lock {
        let _ = fs::remove_dir_all(fx.home.join(".watane-install.lock"));
        return;
    }
    let before = snapshot(&fx.home);
    let requests = fx.rel.requests();
    let blocked = fx.run(args);
    assert_eq!(blocked.code, 5, "{blocked:?}");
    assert_eq!(
        blocked.first_err(),
        format!(
            "watane install: another install may be running (pid {})",
            child_pid
        )
    );
    assert_eq!(snapshot(&fx.home), before, "blocked run changes nothing");
    assert_eq!(fx.rel.requests(), requests, "blocked run does not download");
    let command = blocked.stderr.lines().nth(2).expect("printed rm command");
    assert!(
        Command::new("/bin/sh")
            .args(["-c", command])
            .status()
            .expect("run printed lock removal")
            .success()
    );
}

fn has_leftover(fx: &Fx, what: &str) -> bool {
    leftovers(fx).iter().any(|n| n.contains(what))
}

/// Sends `sig` to the run's group while the swap sleeps in its `mv` of the app; the home then holds
/// every byte of before (3.6 step 4, 3.8). Returns the run for its exit code.
fn signal_in_the_swap_undoes_it(sig: &str) -> Out {
    let (fx, before) = swap_start();
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    let child = spawn(&mut fx.cmd(&["--release", "0.2.0"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), sig);
    let out = finish(child);
    assert_eq!(snapshot(&fx.home), before, "{out:?}");
    assert!(fx.tmp_tree().is_empty());
    out
}

const XCODE_LINES: [&str; 2] = [
    "watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:",
    "  xcode-select --install",
];

/// INST-S-1, 4, 5c, 19, 22, 34 and TECHNICAL 3.4 to 3.10: one fresh install of a whole run shows the
/// order of its steps, what it fetched, the files it wrote, the quarantine step, the PATH block for
/// the shell, and that with no terminal nothing answers the toolchain offer (not a variable that
/// names a file of answers either): the commands to run later name the installer's URL.
#[test]
fn inst_s1_fresh_install() {
    let fx = Fx::new();
    fx.state_write("xcode_missing", "");
    let (k, url) = url_env(&fx);
    let tty = fx.root.path().join("tty");
    fs::write(&tty, "y\n").expect("write");
    let out = fx.run_env(
        &[],
        &[
            (&k, &url),
            ("SHELL", "/bin/zsh"),
            ("WATANE_INSTALL_TTY", &tty.display().to_string()),
        ],
    );
    assert_eq!(out.code, 0, "{out:?}");
    fx.assert_installed("0.1.0");
    out.assert_in_order(&[
        "watane 0.1.0: installing for aarch64-apple-darwin",
        "downloading watane-0.1.0-aarch64-apple-darwin.tar.gz",
        "checked: sha256 matches SHA256SUMS of release v0.1.0",
        "installed:",
        "  ~/.local/bin/watane",
        "  ~/.local/bin/watane-dev",
        "  ~/.local/bin/watane-node",
        "  ~/.local/share/watane-dev/vendor",
        "  ~/Applications/watane.app",
        "no terminal to ask on: the toolchain was not installed",
        "watane 0.1.0 is installed.",
    ]);
    assert_eq!(names(&fx), [".local", "Applications"]);
    assert!(fx.tmp_tree().is_empty(), "temporary folder is gone");
    assert_eq!(
        fx.rel.requests(),
        [
            "/release/SHA256SUMS".to_string(),
            format!("/release/{}", archive())
        ]
    );
    assert!(out.stdout.contains("using WATANE_RELEASE_BASE="));
    assert!(leftovers(&fx).is_empty());
    // INST-S-4 and 5c: no terminal, no question; the URL is quoted as a file URL is not http(s)
    assert!(!out.stdout.contains("[y/N]"));
    let q = format!("'{url}'");
    assert!(
        out.has_line(
            &CURL_LINE
                .replace("--proto '=https' ", "")
                .replace("https://sh.rustup.rs", &q)
        )
    );
    assert_eq!(fx.state_read("installer.log"), "");
    // INST-S-22: the quarantine mark is removed from the app, and the person is told how
    let want = format!(
        "-dr com.apple.quarantine {}/Applications/watane.app\n",
        fx.home.display()
    );
    assert_eq!(fx.state_read("xattr.log"), want);
    assert!(out.has_line(r#"If macOS says "watane" cannot be opened, or could not verify that it is free of malware:"#));
    assert!(out.has_line("    xattr -dr com.apple.quarantine ~/Applications/watane.app"));
    // INST-S-19: ~/.local/bin is not on the PATH: the lines are printed, no shell file is edited
    assert!(out.has_line(
        "~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet."
    ));
    assert!(out.has_line(r#"  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc"#));
    for f in [".zshrc", ".bash_profile", ".profile"] {
        assert!(!fx.home.join(f).exists());
    }
    // INST-S-34: with the command line tools missing the output ends with how to get them, and the
    // script never runs `xcode-select --install` itself
    assert_xcode_tail(&out);
    assert_no_xcode_call(&fx);
}

/// INST-S-5b, 18 and TECHNICAL 3.1, 3.10: no flag says yes to the toolchain, a flag the run does
/// not know or a pair that conflict ends it with exit 1 and usage, `--help` prints the usage, and
/// `--status` and `--uninstall` of nothing exit 7 with their texts; `--status` of an install lists
/// the receipt.
#[test]
fn inst_s5b_flags_status_and_help() {
    let fx = Fx::new();
    let out = fx.run(&["--toolchain"]);
    assert_eq!(out.code, 1);
    assert_eq!(
        out.first_err(),
        "watane install: unknown or conflicting flag --toolchain"
    );
    assert_eq!(fx.run(&["--no-toolchain", "--status"]).code, 1);
    let out = fx.run(&["--help"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("usage: install.sh"));
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 7);
    assert_eq!(
        out.first_err(),
        "watane install: not installed: no receipt at ~/.local/share/watane/receipt"
    );
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 7);
    assert_eq!(
        out.first_err(),
        "watane install: nothing to uninstall: no receipt at ~/.local/share/watane/receipt"
    );
    assert!(fx.home_tree().is_empty());
    let fx = Fx::installed();
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 0);
    assert_eq!(out.stderr, "");
    assert_eq!(
        out.stdout,
        "watane 0.1.0 (aarch64-apple-darwin)\n~/.local/bin/watane\n~/.local/bin/watane-dev\n~/.local/bin/watane-node\n~/.local/share/watane-dev/vendor\n~/.local/share/watane/install.sh\n~/.local/share/watane/VERSION\n~/Applications/watane.app\n"
    );
}

/// INST-S-20, 21, 26 and TECHNICAL 3.2, 3.4 steps 1 to 3: the run refuses, before it asks the
/// network for anything and with nothing written, a platform that is not macOS 14 on Apple
/// silicon (exit 2), root (exit 1: step 2 of `main`, which only a whole run reaches), a tool that
/// is missing (exit 2) and a version that is a path (exit 1, even from the environment).
#[test]
fn inst_s20_s21_s26_the_run_refuses_before_it_fetches() {
    let fx = Fx::new();
    fx.state_write("uname_s", "Linux");
    fx.state_write("uname_m", "x86_64");
    let out = fx.run(&[]);
    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(
        out.first_err(),
        "watane install: unsupported platform: Linux x86_64"
    );
    assert_eq!(
        out.stderr.lines().nth(1),
        Some("watane installs on macOS 14.0 or later, on Apple silicon.")
    );
    assert!(fx.rel.requests().is_empty() && fx.home_tree().is_empty());

    let fx = Fx::new();
    fx.state_write("id_u", "0");
    let out = fx.run(&[]);
    assert_eq!(out.code, 1);
    assert_eq!(
        out.first_err(),
        "watane install: do not run this as root or with sudo"
    );
    assert!(fx.rel.requests().is_empty() && fx.home_tree().is_empty());

    let fx = Fx::new();
    fx.remove_tool("curl");
    let out = fx.run(&[]);
    assert_eq!(out.code, 2);
    assert_eq!(out.first_err(), "watane install: the tool curl is missing");
    assert!(fx.rel.requests().is_empty() && fx.home_tree().is_empty());

    let evil = "1/../../../../../evil/watane/releases/download/v1";
    let fx = Fx::new();
    fake_curl(&fx);
    let mut c = fx.cmd(&["--release", evil]);
    c.env_remove("WATANE_RELEASE_BASE");
    let out = exec(c);
    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(
        out.first_err(),
        format!("watane install: not a version: {evil}")
    );
    assert_eq!(
        out.stderr.lines().nth(1),
        Some("a version looks like 0.1.0")
    );
    assert_eq!(fx.state_read("curl.log"), "");
    assert!(fx.home_tree().is_empty());
    let mut c = fx.cmd(&[]);
    c.env_remove("WATANE_RELEASE_BASE");
    c.env("WATANE_RELEASE", evil);
    let out = exec(c);
    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(fx.state_read("curl.log"), "");
    // a version with a suffix is one: the run goes on to the URL of its release
    let mut c = fx.cmd(&["--release", "0.2.0-rc1"]);
    c.env_remove("WATANE_RELEASE_BASE");
    let out = exec(c);
    assert!(!out.stderr.contains("not a version"), "{out:?}");
    assert!(
        fx.state_read("curl.log")
            .contains("releases/download/v0.2.0-rc1/SHA256SUMS")
    );
}

/// INST-S-8, 10 and TECHNICAL 3.4 steps 7 and 8: a sum that does not match ends the run with exit 4
/// and a download that fails with exit 3 and curl's reason, each with nothing installed and no
/// temporary folder left. The checks themselves are `unit::unit_verify_archive_needs_one_matching_line`.
#[test]
fn inst_s8_s10_a_bad_sum_and_a_failed_download() {
    let fx = Fx::new();
    fx.rel.put(&archive(), b"not the archive".to_vec());
    let out = fx.run(&[]);
    assert_eq!(out.code, 4);
    let l: Vec<&str> = out.stderr.lines().collect();
    assert_eq!(
        l[0],
        "watane install: checksum mismatch for watane-0.1.0-aarch64-apple-darwin.tar.gz"
    );
    assert!(l[1].starts_with("expected "));
    assert!(l[2].starts_with("got "));
    assert_eq!(l[3], "Nothing was installed.");
    assert!(fx.home_tree().is_empty());
    assert!(fx.tmp_tree().is_empty());

    let fx = Fx::new();
    fx.rel.remove(&archive());
    let out = fx.run(&[]);
    assert_eq!(out.code, 3);
    assert_eq!(
        out.first_err(),
        format!(
            "watane install: cannot download {}/{}",
            fx.rel.base(),
            archive()
        )
    );
    assert!(out.stderr.lines().count() > 1, "curl's reason follows");
    assert!(fx.home_tree().is_empty());
    assert!(fx.tmp_tree().is_empty());
}

/// INST-S-12, 13 and TECHNICAL 3.6, 3.9: the same version again says it reinstalls and a path that
/// only the old receipt held is removed with it; a newer one says it upgrades, replaces the old
/// vendored crates, keeps what the person put beside it, and says to open the app again.
#[test]
fn inst_s12_s13_a_reinstall_then_an_upgrade() {
    let fx = Fx::installed_with(&["old-crate-1.0.0"]);
    write_file(&fx.bin("mine"), "mine");
    write_file(&fx.home.join(".watane/marker"), "marker");
    write_file(&fx.bin("watane-gone"), "old");
    let line = format!("path {}\n", fx.bin("watane-gone").display());
    fs::write(fx.receipt(), fx.read(&fx.receipt()) + &line).expect("write");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("reinstalling watane 0.1.0"));
    assert!(!out.has_line("If watane is open, quit it and open it again."));
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty());
    assert!(
        !fx.bin("watane-gone").exists(),
        "a path only the old receipt held"
    );
    assert!(!fx.read(&fx.receipt()).contains("watane-gone"));

    assert!(fx.share("watane-dev/vendor/old-crate-1.0.0").is_dir());
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    fx.assert_installed("0.2.0");
    assert!(!fx.share("watane-dev/vendor/old-crate-1.0.0").exists());
    assert!(fx.share("watane-dev/vendor/new-crate-1.0.0").is_dir());
    assert_eq!(fx.read(&fx.bin("mine")), "mine");
    assert_eq!(fx.read(&fx.home.join(".watane/marker")), "marker");
    assert!(leftovers(&fx).is_empty());
    assert!(out.has_line("If watane is open, quit it and open it again."));
}

/// INST-S-17, 27 and TECHNICAL 8.4, 8.5: an uninstall with no terminal removes what the install
/// wrote and keeps the home, and removes nothing a receipt names that the install does not write:
/// a path outside the install set, a `.mine` name, a path through a link to the home, a stale
/// `.old.<n>` name of a dead run goes. Each skipped line is on stderr.
#[test]
fn inst_s17_s27_an_uninstall_removes_only_what_the_install_wrote() {
    let (fx, named, other) = s27_start();
    write_file(&fx.home.join(".watane/marker"), "marker");
    let h = fx.home.display().to_string();
    write_file(&fx.bin("watane.mine"), "mine");
    append(&fx.receipt(), &format!("path {h}/.local/bin/watane.mine\n"));
    write_file(&fx.home.join("Documents/marker"), "m");
    symlink(&fx.home, fx.share("watane/x")).expect("link");
    append(
        &fx.receipt(),
        &format!("path {h}/.local/share/watane/x/Documents\n"),
    );
    let tty = fx.root.path().join("tty");
    fs::write(&tty, "delete\n").expect("write");
    let out = fx.run_env(
        &["--uninstall"],
        &[("WATANE_INSTALL_TTY", &tty.display().to_string())],
    );
    assert_eq!(out.code, 0, "{out:?}");
    s27_kept(&fx, other.path());
    for p in &named {
        let shown = p.replacen(&h, "~", 1);
        let line = format!("watane install: skipped {shown}: not a path that watane installs");
        assert!(
            out.stderr.lines().any(|l| l == line),
            "{line}\n{}",
            out.stderr
        );
    }
    assert!(!fx.bin("watane").exists() && !fx.app().exists());
    for p in [
        fx.bin("watane-dev"),
        fx.bin("watane-node"),
        fx.share("watane-dev"),
        fx.share("watane/install.sh"),
    ] {
        assert!(!p.exists(), "{} should be gone", p.display());
    }
    // INST-S-17: no terminal, no question, the home is kept, the answer file answers nothing
    assert!(fx.home.join(".watane/marker").is_file());
    assert!(!out.stdout.contains("keep it"));
    assert!(out.has_line("kept: ~/.watane (your home: identity, mods, channels)"));
    // 8.5 rule 2 (`.mine`) and rule 3 (a link inside the install folder to the home)
    assert!(fx.bin("watane.mine").exists());
    let line = "watane install: skipped ~/.local/bin/watane.mine: not a path that watane installs";
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);
    assert!(fx.home.join("Documents/marker").exists(), "{out:?}");
    let line = "watane install: skipped ~/.local/share/watane/x/Documents: not a path that watane installs";
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);
}

#[test]
fn ac1_toolchain_matches_the_raw_answer() {
    let script = fs::read_to_string(checkout_script()).expect("script");
    let step = script.split("  ts_ans=\"\"").nth(1).expect("answer step");
    let decision = step.split("  case $TC_STATE in").next().expect("decision");
    assert!(decision.contains("case $ts_ans in\n    [Yy] | [Yy][Ee][Ss])"));
    assert!(!decision.contains("tr "));
}

#[test]
fn inst_s11_interrupted_during_the_download() {
    let fx = Fx::new();
    fx.rel.stall_on(&archive());
    let child = spawn(&mut fx.cmd(&[]));
    fx.rel.wait_stalled();
    signal_group(child.id(), "INT");
    let out = finish(child);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(fx.home_tree().is_empty());
    assert!(fx.tmp_tree().is_empty());
}

#[test]
fn inst_s14_failed_upgrade_rolls_back() {
    let fx = installed();
    let before = snapshot(&fx.home);
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    fx.state_write("mv_fault", "fail\n*watane.app.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    // 3.9: only an undo that ended well says nothing changed
    assert!(
        out.stderr
            .contains("Nothing was changed: the version that was installed still works."),
        "{out:?}"
    );
    assert!(!out.stderr.contains("could not be put back"), "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    assert_eq!(snapshot(&fx.home), before);
    assert!(fx.tmp_tree().is_empty());
}

/// INST-S-14 and TECHNICAL 3.6 step 4, 3.9 (IN-46, IN-47): a swap that fails in a fresh install removes
/// what it moved in and the folders it made, and its last line is that nothing was installed, never
/// that "the version that was installed still works".
#[test]
fn inst_s14_failed_fresh_install_removes_what_it_moved_in() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "fail\n*watane.app.new.*\n");
    let out = fx.run(&[]);
    assert_eq!(out.code, 5, "{out:?}");
    let files: Vec<String> = fx
        .home_tree()
        .into_iter()
        .filter(|n| !n.ends_with('/'))
        .collect();
    assert!(files.is_empty(), "{files:?}");
    assert!(fx.home_tree().is_empty(), "{:?}", fx.home_tree());
    assert!(out.stderr.contains("Nothing was installed."), "{out:?}");
    assert!(
        !out.stderr
            .contains("the version that was installed still works"),
        "{}",
        out.stderr
    );
}

fn fake_curl(fx: &Fx) {
    let p = fx.fakes.join("curl");
    fs::write(
        &p,
        "#!/bin/sh\necho \"$*\" >> \"$FAKE_STATE/curl.log\"\nexit 22\n",
    )
    .expect("write");
    let _ = Command::new("chmod").arg("755").arg(&p).status();
}

fn append(p: &Path, text: &str) {
    let mut f = fs::OpenOptions::new().append(true).open(p).expect("open");
    f.write_all(text.as_bytes()).expect("append");
}

/// INST-S-27's start: installed, and a receipt that names other files.
fn s27_start() -> (Fx, Vec<String>, tempfile::TempDir) {
    let fx = installed();
    let other = tempfile::tempdir().expect("tempdir");
    let h = fx.home.display().to_string();
    let named = [
        format!("{h}/.local/../Documents"),
        format!("{h}/.local/share/fish"),
        format!("{h}/.local/bin/mine"),
        format!("{h}/.local/share/watane"),
        format!("{}/x", other.path().display()),
        String::new(),
    ];
    for p in &named {
        if p.is_empty() {
            continue;
        }
        let real = if p.contains("/.local/../") {
            format!("{h}/Documents")
        } else {
            p.clone()
        };
        if p.ends_with("/mine") {
            write_file(Path::new(&real), "mine");
        } else {
            write_file(&Path::new(&real).join("marker"), "marker");
        }
        append(&fx.receipt(), &format!("path {p}\n"));
    }
    append(&fx.receipt(), "path \n");
    write_file(&fx.bin("watane.old.mine"), "x");
    write_file(&fx.bin("watane.old.99999999"), "x");
    (fx, named.to_vec(), other)
}

fn s27_kept(fx: &Fx, other: &Path) {
    for m in [
        fx.home.join("Documents/marker"),
        fx.share("fish/marker"),
        fx.share("watane/marker"),
        other.join("x/marker"),
        fx.bin("mine"),
        fx.bin("watane.old.mine"),
    ] {
        assert!(m.exists(), "{} must stay", m.display());
    }
    assert!(!fx.bin("watane.old.99999999").exists());
}

fn foreign_lines(out: &Out, shown: &[&str]) {
    let l: Vec<&str> = out.stderr.lines().collect();
    assert_eq!(
        l[0],
        "watane install: found files that watane did not install:"
    );
    let n = shown.len();
    for (i, p) in shown.iter().enumerate() {
        assert_eq!(l[1 + i], format!("  {p}"));
        assert_eq!(l[2 + n + i], format!("  mv {p} {p}.mine"));
    }
    assert_eq!(
        l[1 + n],
        "Move them aside, then run this again, for example:"
    );
}

/// INST-S-30 and TECHNICAL 3.4.2: a file at a name the install writes that watane did not install
/// (a file, a link, a folder) ends the run with exit 8 and the `mv` that moves it aside, before any
/// request and with nothing written. The other shapes (a receipt without one path, a receipt that
/// is a folder, a dead run's journal) are `unit::unit_check_foreign_refuses_a_file_watane_did_not_install`.
#[test]
fn inst_s30_files_that_watane_did_not_install() {
    let fx = Fx::new();
    write_file(&fx.bin("watane"), "mine");
    fs::create_dir_all(fx.app()).expect("app");
    symlink("/nonexistent/target", fx.bin("watane-dev")).expect("link");
    let before = snapshot(&fx.home);
    let out = fx.run(&[]);
    assert_eq!(out.code, 8, "{out:?}");
    foreign_lines(
        &out,
        &[
            "~/.local/bin/watane",
            "~/.local/bin/watane-dev",
            "~/Applications/watane.app",
        ],
    );
    assert!(fx.rel.requests().is_empty());
    assert_eq!(snapshot(&fx.home), before);
}

/// INST-S-30 and TECHNICAL 3.6 step 2: a stale `<name>.new.<pid>` of this very run's pid is removed,
/// not copied into (a dead run's pid came back).
#[test]
#[ignore = "slow: the pid of the run is only known once it has started, so the name is planted in a started whole run"]
fn inst_s30_a_stale_name_with_this_runs_own_pid_is_removed() {
    let fx = installed();
    let mut c = std::process::Command::new("/bin/sh");
    let inner = fx.cmd(&[]);
    let mut args = vec![
        "-c".to_string(),
        "read x; exec \"$@\"".to_string(),
        "sh".to_string(),
        PERL.to_string(),
    ];
    args.extend(inner.get_args().map(|a| a.to_string_lossy().into_owned()));
    c.args(&args).env_clear().envs(fx.env_pairs());
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_run(&mut c);
    write_file(
        &fx.bin(&format!("watane.new.{}", child.id())).join("marker"),
        "m",
    );
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(b"go\n")
        .expect("go");
    let out = finish(child);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.bin("watane").is_file());
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
}

/// INST-S-31 and TECHNICAL 3.4.1: an install folder that resolves outside the home ends the run with exit 2
/// and names where it resolves, before any request and with nothing written outside the home. The
/// other folders and links are `unit::unit_check_folders_refuses_an_install_folder_outside_the_home`.
#[test]
fn inst_s31_an_install_folder_outside_the_home() {
    let check = |fx: &Fx, other: &Path, folder: &str, target: &str| {
        let out = fx.run(&[]);
        assert_eq!(out.code, 2, "{out:?}");
        let home = fs::canonicalize(&fx.home)
            .expect("canon")
            .display()
            .to_string();
        assert_eq!(
            out.first_err(),
            format!(
                "watane install: {folder} resolves to {target}, outside your home folder {home}"
            )
        );
        assert_eq!(
            out.stderr.lines().nth(1),
            Some(
                "watane writes only under your home folder. Make it a real folder there, then run this again."
            )
        );
        assert!(fx.rel.requests().is_empty());
        let left: Vec<_> = fs::read_dir(other).expect("other").collect();
        assert!(left.is_empty(), "the second folder is empty: {left:?}");
    };
    let new = || {
        let fx = Fx::new();
        let other = tempfile::tempdir().expect("other");
        let o = fs::canonicalize(other.path()).expect("canon");
        (fx, other, o)
    };
    let (fx, other, o) = new();
    symlink(&o, fx.home.join(".local")).expect("link");
    check(
        &fx,
        other.path(),
        "~/.local/bin",
        &format!("{}/bin", o.display()),
    );
}

/// INST-S-31 and TECHNICAL 3.4.1, 8.5: each other install folder linked outside the home is refused, an
/// uninstall leaves what a link leads to, and a link to a folder inside the home is fine for an
/// install, an upgrade and an uninstall. One whole run shows the refusal (a dangling `~/.local/bin`
/// link, exit 2 before any request); the other install folders are
/// `unit::unit_check_folders_refuses_an_install_folder_outside_the_home` (`o1` to `o4`, `self0` to
/// `self3`), under `set -eu` as the run is, and the default
/// `adv_s94::adv_s94_a_dangling_install_folder_link_out_of_the_home_exits_2`.
#[test]
#[ignore = "slow: an install, an upgrade and an uninstall through each link, which only whole runs show"]
fn inst_s31_every_install_folder_outside_the_home_and_a_link_inside_it() {
    let check = |fx: &Fx, other: &Path, folder: &str, target: &str| {
        let out = fx.run(&[]);
        assert_eq!(out.code, 2, "{out:?}");
        let home = fs::canonicalize(&fx.home)
            .expect("canon")
            .display()
            .to_string();
        assert_eq!(
            out.first_err(),
            format!(
                "watane install: {folder} resolves to {target}, outside your home folder {home}"
            )
        );
        assert_eq!(
            out.stderr.lines().nth(1),
            Some(
                "watane writes only under your home folder. Make it a real folder there, then run this again."
            )
        );
        assert!(fx.rel.requests().is_empty());
        let left: Vec<_> = fs::read_dir(other).expect("other").collect();
        assert!(left.is_empty(), "the second folder is empty: {left:?}");
    };
    let new = || {
        let fx = Fx::new();
        let other = tempfile::tempdir().expect("other");
        let o = fs::canonicalize(other.path()).expect("canon");
        (fx, other, o)
    };
    let new_installed = || {
        let fx = Fx::installed();
        let other = tempfile::tempdir().expect("other");
        let o = fs::canonicalize(other.path()).expect("canon");
        (fx, other, o)
    };
    let (fx, other, o) = new();
    fs::create_dir_all(fx.home.join(".local")).expect("local");
    symlink(o.join("bin"), fx.home.join(".local/bin")).expect("link");
    check(
        &fx,
        other.path(),
        "~/.local/bin",
        &format!("{}/bin", o.display()),
    );
    assert!(!o.join("bin").exists());

    // uninstall leaves what a link leads to
    let (fx, _other, o) = new_installed();

    fs::remove_dir_all(fx.share("watane-dev")).expect("rm");
    write_file(&o.join("vendor/marker"), "m");
    symlink(&o, fx.share("watane-dev")).expect("link");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(o.join("vendor/marker").exists());
    let line =
        "watane install: skipped ~/.local/share/watane-dev/vendor: not a path that watane installs";
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);

    // uninstall removes nothing through a link at the folder of the receipt and the journals
    let (fx, _other, o) = new_installed();

    fs::rename(fx.share("watane"), o.join("watane")).expect("move");
    symlink(o.join("watane"), fx.share("watane")).expect("link");
    write_file(&o.join("watane/journal.99999999"), "old x\n");
    write_file(&o.join("watane/receipt.old.99999999"), "x");
    let before = fx.tree(&o);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fx.tree(&o), before, "nothing outside the home was removed");

    // a link to a folder inside the home is fine
    let fx = Fx::new();
    fs::create_dir(fx.home.join("real_local")).expect("real");
    symlink(fx.home.join("real_local"), fx.home.join(".local")).expect("link");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.home.join("real_local/bin/watane").is_file());
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    fx.assert_installed("0.2.0");
    assert!(
        fx.home
            .join("real_local/share/watane-dev/vendor/new-crate-1.0.0")
            .is_dir()
    );
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("uninstalled watane 0.2.0"), "{out:?}");
    assert_uninstalled(&fx);
    assert!(!fx.home.join("real_local/bin").exists());
    assert!(!fx.home.join("real_local/share").exists());
    assert!(!fx.receipt().exists());
}

/// INST-S-2 and TECHNICAL 3.7: a typed `y` installs the toolchain (offer A).
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s2_toolchain_answered_y() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout.matches("[y/N]").count(), 1);
    assert_eq!(
        fx.state_read("installer.log"),
        "-y --profile minimal --no-modify-path -t wasm32-wasip2\n"
    );
    out.assert_in_order(&["toolchain: installed", "watane 0.1.0 is installed."]);
    assert_eq!(names(&fx), [".cargo", ".local", ".rustup", "Applications"]);
}

/// TECHNICAL 3.7 (S50b-fix ac2): an answer that is not UTF-8 is no.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac2_toolchain_rejects_invalid_utf8_answer() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = support::pty::run_bytes(
        &fx,
        &[],
        &[
            ("WATANE_RUSTUP_URL", &url),
            ("LANG", "en_US.UTF-8"),
            ("LC_ALL", "en_US.UTF-8"),
        ],
        &[("[y/N] ", b"y\xffnope")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fx.state_read("installer.log"), "", "{out:?}");
    assert!(out.has_line("toolchain: not installed"), "{out:?}");
}

/// TECHNICAL 3.7 (S50b-fix): an answer with a NUL byte is no.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn adv_s50b_fix_toolchain_nul_in_answer_is_no() {
    // 3.7: only `y` and `yes` count; `y` then a NUL (Ctrl-@) is another answer (the other NUL
    // shapes are `unit::unit_read_answer_refuses_a_nul_and_a_long_answer`)
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = support::pty::run_bytes(
        &fx,
        &[],
        &[("WATANE_RUSTUP_URL", &url)],
        &[("[y/N] ", b"y\0")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fx.state_read("installer.log"), "", "{out:?}");
    assert!(out.has_line("toolchain: not installed"), "{out:?}");
}

/// TECHNICAL 3.7 (S50b-fix ac2): `YeS` is yes, whatever the locale.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac2_toolchain_accepts_mixed_case_yes() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_pty(
        &[],
        &[
            ("WATANE_RUSTUP_URL", &url),
            ("LANG", "en_US.UTF-8"),
            ("LC_ALL", "en_US.UTF-8"),
        ],
        &[("[y/N] ", "YeS")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        fx.state_read("installer.log"),
        "-y --profile minimal --no-modify-path -t wasm32-wasip2\n"
    );
    assert!(out.has_line("toolchain: installed"), "{out:?}");
}

/// INST-S-3 and TECHNICAL 3.7: `n` or Enter does not install the toolchain and prints the commands to run later.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s3_toolchain_answered_n() {
    for answer in ["n", ""] {
        let fx = Fx::new();
        let url = fx.rustup_installer();
        let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", answer)]);
        assert_eq!(out.code, 0, "{out:?}");
        assert_eq!(out.stdout.matches("[y/N]").count(), 1);
        assert_eq!(fx.state_read("installer.log"), "");
        assert!(out.has_line("toolchain: not installed"));
        assert!(
            out.has_line(
                &CURL_LINE
                    .replace("--proto '=https' ", "")
                    .replace("https://sh.rustup.rs", &format!("'{url}'"))
            )
        );
        fx.assert_installed("0.1.0");
    }
}

/// INST-S-5 (a) and TECHNICAL 3.1, 3.7: `--no-toolchain` asks nothing, whatever is typed.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s5a_no_toolchain_flag_beats_a_typed_y() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_pty(
        &["--no-toolchain"],
        &[("WATANE_RUSTUP_URL", &url)],
        &[("checked: sha256", "y")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("[y/N]"));
    assert!(out.has_line("toolchain: skipped (--no-toolchain)"));
    // a URL that is not http(s) is printed in single quotes (3.7)
    let q = format!("'{url}'");
    assert!(
        out.has_line(
            &CURL_LINE
                .replace("--proto '=https' ", "")
                .replace("https://sh.rustup.rs", &q)
        )
    );
    assert_eq!(fx.state_read("installer.log"), "");
}

/// INST-S-7 and TECHNICAL 3.7: with the target missing, offer B adds it, from the home and with auto-install off.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s7_target_missing() {
    let fx = Fx::new();
    fx.add_rustup(false);
    let out = fx.run_pty(&[], &[], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        out.stdout
            .contains("Add the wasm32-wasip2 target with rustup? [y/N] ")
    );
    assert!(!out.stdout.contains("Install it with rustup"));
    let log = fx.state_read("rustup.log");
    assert_eq!(
        log.lines()
            .filter(|l| *l == "target add wasm32-wasip2")
            .count(),
        1,
        "{log}"
    );
    assert!(out.has_line("toolchain: installed"));
    // 3.7: offer B's rustup runs from $HOME (the test's cwd is not), auto-install off
    let home = fs::canonicalize(&fx.home).expect("home");
    let pwds = fx.state_read("rustup.pwd");
    assert!(pwds.lines().count() >= 2, "{pwds}");
    for l in pwds.lines() {
        assert_eq!(fs::canonicalize(l).expect("pwd"), home, "{pwds}");
    }
    let auto = fx.state_read("rustup.auto");
    assert!(auto.lines().all(|l| l == "0"), "{auto}");
}

fn uninstall_setup() -> Fx {
    let fx = installed();
    write_file(&fx.home.join(".watane/marker"), "marker");
    fx
}

fn assert_uninstalled(fx: &Fx) {
    for p in [
        fx.bin("watane"),
        fx.bin("watane-dev"),
        fx.bin("watane-node"),
        fx.share("watane-dev"),
        fx.share("watane"),
        fx.app(),
    ] {
        assert!(!p.exists(), "{} should be gone", p.display());
    }
}

/// INST-S-15, 16 and TECHNICAL 8.4: an uninstall deletes the home only for the typed `delete`; any other answer
/// (`n`, `DELETE`, `yes`) keeps it, says so, and the programs are removed all the same.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s16_uninstall_home_deleted() {
    let fx = uninstall_setup();
    let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", "delete")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!fx.home.join(".watane").exists());
    assert!(out.has_line("deleted: ~/.watane"));
    for other in ["n", "DELETE", "yes"] {
        let fx = uninstall_setup();
        let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", other)]);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            fx.home.join(".watane/marker").is_file(),
            "{other} keeps the home"
        );
        assert_uninstalled(&fx);
        assert!(out.has_line("kept: ~/.watane (your home: identity, mods, channels)"));
        assert!(out.has_line("uninstalled watane 0.1.0"));
    }
}

/// TECHNICAL 8.4 via 3.7: a NUL anywhere in the answer, or a trailing space, is not `delete`.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac3_uninstall_nul_in_answer_keeps_home() {
    // 8.4 via 3.7's reader: a NUL anywhere, or a trailing space, is not `delete`
    for answer in [&b"de\0lete"[..], &b"\0delete"[..], &b"delete "[..]] {
        let fx = uninstall_setup();
        let out = support::pty::run_bytes(&fx, &["--uninstall"], &[], &[("keep it: ", answer)]);
        assert_eq!(out.code, 0, "{answer:?}: {out:?}");
        assert!(
            fx.home.join(".watane/marker").is_file(),
            "{answer:?}: {out:?}"
        );
        assert!(
            out.has_line("kept: ~/.watane (your home: identity, mods, channels)"),
            "{answer:?}"
        );
    }
}

/// INST-S-28 and TECHNICAL 8.4 step 5: for a `WATANE_HOME` that is not a watane home the question is not asked and the
/// home is kept; for one that is, the typed `delete` deletes it. The other values are
/// `unit::unit_home_deletable_refuses_what_is_not_a_watane_home`.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s28_a_home_that_is_not_a_watane_home() {
    let cases = ["$HOME", "$HOME/link"];
    for case in cases {
        let fx = installed();
        write_file(&fx.home.join("keep"), "keep");
        fs::create_dir(fx.home.join("x")).expect("x");
        symlink(&fx.home, fx.home.join("link")).expect("link");
        let value = case.replace("$HOME", &fx.home.display().to_string());
        let out = fx.run_pty(
            &["--uninstall"],
            &[("WATANE_HOME", &value)],
            &[("uninstalled watane 0.1.0", "delete")],
        );
        assert_eq!(out.code, 0, "{case}: {out:?}");
        assert!(!out.stdout.contains("Type delete"), "{case}");
        let shown = match case {
            "$HOME" => "~".to_string(),
            "/" => "/".to_string(),
            c => c.replace("$HOME", "~"),
        };
        let line = format!("kept: {shown} (not deleted: not a folder that watane can delete)");
        assert!(out.has_line(&line), "{line}\n{}", out.stdout);
        assert!(fx.home.join("keep").exists(), "{case}");
    }
    let fx = installed();
    write_file(&fx.home.join("keep"), "keep");
    write_file(&fx.home.join("elsewhere/marker"), "m");
    let value = fx.home.join("elsewhere").display().to_string();
    let out = fx.run_pty(
        &["--uninstall"],
        &[("WATANE_HOME", &value)],
        &[("keep it: ", "delete")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!fx.home.join("elsewhere").exists());
    assert!(fx.home.join("keep").exists());
}

/// INST-S-27 and TECHNICAL 8.5: an upgrade whose old receipt names files the install does not write
/// keeps them, and replaces the version.
#[test]
#[ignore = "slow: an upgrade (a second whole install) over a receipt that names other files; the uninstall goes through do_uninstall's own receipt_allowed call, and finish_commit's is proven by adv_s94::adv_s94_an_upgrade_commit_keeps_what_the_old_receipt_names_beyond_the_install"]
fn inst_s27_an_upgrade_keeps_what_a_receipt_names_beyond_the_install() {
    let (fx, _, other) = s27_start();
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    s27_kept(&fx, other.path());
    assert_eq!(fx.read(&fx.share("watane/VERSION")).trim(), "0.2.0");
}

/// INST-S-32 and TECHNICAL 3.6 step 4, 3.9: a failure at the receipt move undoes the swap (every byte as before), and the next run installs.
#[test]
#[ignore = "slow: the swap fails at its last move and the next run upgrades; only two whole runs show the home as before and then new"]
fn inst_s32_a_failure_at_the_receipt() {
    let (fx, before) = swap_start();
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    fx.assert_installed("0.1.0");
    assert_old_receipt(&fx);
    assert!(fx.share("watane-dev/vendor/serde-1.0.0").is_dir());
    assert_eq!(snapshot(&fx.home), before, "every byte as before");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    assert!(fx.tmp_tree().is_empty());

    fx.unfault("mv_fault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert!(
        !out.stderr
            .contains("found files that watane did not install")
    );
    assert_all_new(&fx);
}

/// INST-S-33 and TECHNICAL 3.6 step 0, 3.8: a kill at the receipt move leaves the old receipt and a journal; the next run recovers and installs.
#[test]
#[ignore = "slow: a SIGKILL at the receipt move, then a recovery run, a failing swap and an upgrade, which only whole runs show"]
fn inst_s33_a_kill_at_the_receipt() {
    let (fx, before) = swap_start();
    kill_at_checking(&fx, "mv", "*receipt.new.*", &["--release", "0.2.0"], true);
    assert_old_receipt(&fx);
    assert!(has_leftover(&fx, "journal."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, "receipt.new."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, ".old."), "{:?}", leftovers(&fx));
    // the recovery alone, then the install: a failing swap after it shows the old version whole
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(snapshot(&fx.home), before);
    fx.unfault("mv_fault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert_all_new(&fx);
}

/// INST-S-33, 35 and TECHNICAL 3.6 step 0: a kill at the receipt move of a fresh install leaves no
/// receipt and a journal; the next run takes the files that journal moved in as watane's, not as
/// foreign, recovers and installs, and leaves nothing behind.
#[test]
#[ignore = "slow: a SIGKILL at the receipt move of a fresh install, then the run that recovers it; the foreign rule alone is `unit::unit_check_foreign_refuses_a_file_watane_did_not_install` (killed)"]
fn inst_s33_a_kill_at_the_receipt_of_a_fresh_install() {
    let fx = Fx::new();
    kill_at(&fx, "mv", "*receipt.new.*", &[]);
    assert!(!fx.receipt().exists());
    assert!(has_leftover(&fx, "journal."));
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stderr.contains("did not install"), "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
}

/// INST-S-33 and TECHNICAL 3.6 step 5: a kill after the receipt move (in step 5, at a path only the old receipt held)
/// leaves the new version and the journal, and keeps that path until the next run removes it and finishes the cleanup.
#[test]
#[ignore = "slow: a SIGKILL in step 5 at a path only the old receipt held, then a whole run that finishes it"]
fn inst_s33_a_kill_after_the_receipt_move_keeps_what_the_old_receipt_held() {
    // the old receipt is the only record of this path; the killed run cut step 5 at its removal
    let (fx, _) = swap_start();
    write_file(&fx.bin("watane-gone"), "old");
    let line = format!("path {}\n", fx.bin("watane-gone").display());
    fs::write(fx.receipt(), fx.read(&fx.receipt()) + &line).expect("write");
    kill_at(&fx, "rm", "*watane-gone*", &["--release", "0.2.0"]);
    assert!(fx.read(&fx.receipt()).contains("\nversion 0.2.0\n"));
    assert!(has_leftover(&fx, "journal."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, ".old."), "{:?}", leftovers(&fx));
    assert!(fx.bin("watane-gone").exists());
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        !fx.bin("watane-gone").exists(),
        "the next run finished step 5"
    );
    assert_all_new(&fx);
}

/// INST-S-33 and TECHNICAL 3.6 step 0, 3.8: a kill at the app move leaves a half-swapped home; the next run undoes it whole and the one after installs.
#[test]
#[ignore = "slow: a SIGKILL at the app move, then a failing swap and an upgrade, which only whole runs show"]
fn inst_s33_a_kill_at_the_app() {
    let (fx, before) = swap_start();
    kill_at(&fx, "mv", "*watane.app.new.*", &["--release", "0.2.0"]);
    assert_old_receipt(&fx);
    assert!(has_leftover(&fx, "journal."));
    // install.sh and VERSION were swapped, the app and the receipt not
    assert!(fx.read(&fx.share("watane/VERSION")).contains("0.2.0"));
    assert!(!fx.app().exists() && has_leftover(&fx, "watane.app.old."));
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(snapshot(&fx.home), before, "the undo put 0.1.0 back whole");
    fx.unfault("mv_fault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert_all_new(&fx);
}

/// INST-S-33 and TECHNICAL 3.6 step 4, 3.8: a TERM while the swap moves the app undoes the swap (the home holds every
/// byte of before), and prints no hint that watane is installed.
#[test]
fn inst_s33_term_in_the_swap_undoes_it() {
    let out = signal_in_the_swap_undoes_it("TERM");
    assert_eq!(out.code, 143, "{out:?}");
    // TECHNICAL 3.8 (S89 ac2): a signal before the commit prints no hint that watane is installed
    assert!(!out.stderr.contains("is installed"), "{out:?}");
}

/// 3.8: signal rollback needs only a staged journal, not a download or package build.
#[test]
fn ac1_term_rollback_from_a_prebuilt_journal() {
    let fx = Fx::new();
    let paths = [
        fx.bin("watane"),
        fx.bin("watane-dev"),
        fx.bin("watane-node"),
        fx.share("watane-dev/vendor"),
        fx.share("watane/install.sh"),
        fx.share("watane/VERSION"),
        fx.app(),
        fx.receipt(),
    ];
    for path in &paths {
        write_file(path, "old");
    }
    let before = snapshot(&fx.home);
    let mut journal = String::new();
    for path in &paths {
        write_file(
            &path.with_file_name(format!(
                "{}.old.99999999",
                path.file_name().expect("name").to_string_lossy()
            )),
            "old",
        );
        write_file(path, "new");
        journal.push_str(&format!("old {}\n", path.display()));
    }
    fs::remove_file(fx.app()).expect("app moved aside");
    write_file(&fx.app().with_file_name("watane.app.new.99999999"), "new");
    write_file(
        &fx.receipt().with_file_name("receipt.new.99999999"),
        "pending",
    );
    // The prebuilt receipt backup is complete and the new receipt was installed.
    // The swap records that proof before replacing the receipt (TECHNICAL 3.6).
    journal.push_str(&format!("moved {}\n", fx.receipt().display()));
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    write_file(&fx.share("watane/journal.99999999"), &journal);
    let source = baked_script("0.1.0").replace(
        "main \"$@\"",
        r#"
HOME_R=$(cd "$HOME" && pwd -P)
set_entries
SWAP_PID=99999999
SWAP_ACTIVE=1
trap cleanup EXIT
trap 'on_signal 143' TERM
mv "$HOME/Applications/watane.app.new.99999999" "$HOME/Applications/watane.app"
"#,
    );
    fs::write(&fx.script, source).expect("fixture script");
    let child = spawn(&mut fx.cmd(&[]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert_eq!(snapshot(&fx.home), before, "{out:?}");
}

/// INST-S-33 and TECHNICAL 3.6 step 4, 3.8: `INT` and `PIPE` (the reader went away) in the swap undo it as `TERM` does.
#[test]
#[ignore = "slow: an INT and a PIPE at the app move of two whole upgrades; `inst_s33_term_in_the_swap_undoes_it` is the default one"]
fn inst_s33_int_and_pipe_in_the_swap_undo_it() {
    let out = signal_in_the_swap_undoes_it("INT");
    // 3.8: macOS sh can lose an INT that lands just after the `mv` it waited on died of it; the
    // run then ends as the failed swap it saw
    let lost = out.code == 5 && out.stderr.contains("could not put the files in place");
    assert!(out.code == 130 || lost, "{out:?}");
    let out = signal_in_the_swap_undoes_it("PIPE");
    assert_eq!(out.code, 141, "{out:?}");
}

/// 3.8: a signal that kills the undo's subshell (macOS sh forks it with the signal pending) leaves
/// the swap in progress, so the handler that runs next undoes it again.
#[test]
#[ignore = "slow: a TERM inside the undo of a failed upgrade, which only a whole run reaches"]
fn inst_s33_a_signal_that_kills_the_undo_is_undone_by_the_handler() {
    let (fx, before) = swap_start();
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    // the undo removes the new app before it moves the old one back: TERM the group there
    fx.state_write("rm_fault", "term\n*/Applications/watane.app\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(
        !fx.state.join("rm_fault").exists(),
        "the undo reached its rm"
    );
    assert_eq!(snapshot(&fx.home), before, "{out:?}");
    assert!(fx.tmp_tree().is_empty());
}

/// The undo of 3.6 step 4 must be safe to repeat (rollback runs it again when a signal killed
/// it). The first undo removes `<entry>.new.<pid>` before the journal, so a repeat reads a
/// `new` + `moved` entry with no `.new` as "moved in" and removes what is at the path now.
#[test]
#[ignore = "slow: a TERM inside the undo of a failed fresh install, which only a whole run reaches"]
fn adv_s50b_fix_a_repeated_undo_removes_a_file_it_never_moved() {
    let fx = Fx::new();
    // fresh install: every entry is `new`; the app's swap `mv` fails after its `moved` line
    fx.state_write("mv_fault", "fail\n*/Applications/watane.app\n");
    // at the undo's `rm` of receipt.new (after it removed watane.app.new.<pid>): the person
    // drags a watane.app into ~/Applications, and the undo's subshell dies of TERM (the
    // macOS sh pending-trap death that `rollback` repeats the undo for)
    let rm = fx.fakes.join("rm");
    fs::write(
        &rm,
        "#!/bin/sh\n\
         for a; do case $a in */share/watane/receipt.new.*)\n\
         if [ -f \"$FAKE_STATE/armed\" ]; then\n\
         \"$FAKE_REAL/rm\" -f \"$FAKE_STATE/armed\"\n\
         /bin/mkdir -p \"$HOME/Applications/watane.app\"\n\
         echo mine > \"$HOME/Applications/watane.app/marker\"\n\
         kill -s TERM $PPID; exit 1\n\
         fi ;; esac; done\n\
         exec \"$FAKE_REAL/rm\" \"$@\"\n",
    )
    .expect("fake rm");
    fs::set_permissions(&rm, fs::Permissions::from_mode(0o755)).expect("chmod");
    fx.state_write("armed", "1");
    let out = fx.run(&[]);
    // TERM in undo now retains its signal status instead of becoming exit 5.
    assert_eq!(out.code, 143, "{out:?}");
    assert!(
        !fx.state.join("armed").exists(),
        "the undo reached its rm: {out:?}"
    );
    assert_eq!(
        fs::read_to_string(fx.app().join("marker")).unwrap_or_default(),
        "mine\n",
        "the person's watane.app is kept: {out:?}"
    );
}

/// AC4 (3.6 step 4, 3.8): interrupt one removal of each kind in a staged undo, both with
/// TERM (the handler repeats rollback) and KILL (the next run recovers).
#[test]
#[ignore = "slow: a TERM and a KILL at one removal of each kind of the undo, then the recoveries: sourced runs that only a signal at a real removal proves"]
fn ac4_each_kind_of_undo_removal_can_be_repeated_or_recovered() {
    let entries = [
        ".local/bin/watane",
        ".local/bin/watane-dev",
        ".local/bin/watane-node",
        ".local/share/watane-dev/vendor",
        ".local/share/watane/install.sh",
        ".local/share/watane/VERSION",
        "Applications/watane.app",
        ".local/share/watane/receipt",
    ];
    let removals: Vec<_> = entries[..6]
        .iter()
        .map(|p| format!("*/{p}"))
        .chain(std::iter::once(
            "*/share/watane/journal.99999999".to_string(),
        ))
        .chain(entries.iter().map(|p| format!("*/{p}.new.99999999")))
        .collect();
    // One removal of each kind (the entries and the staging names of a kind differ only by name, and
    // the undo removes them in one loop); a TERM at each shows the handler repeating the undo, a KILL
    // the next run recovering.
    let same_as_another = |removal: &str| {
        ["watane-dev", "watane-node", "VERSION"].iter().any(|n| {
            removal.ends_with(&format!("/bin/{n}"))
                || removal == format!("*/.local/share/watane/{n}")
        }) || [
            "bin/watane-dev",
            "bin/watane-node",
            "share/watane/install.sh",
            "share/watane/VERSION",
        ]
        .iter()
        .any(|n| removal.ends_with(&format!("{n}.new.99999999")))
    };
    for mode in ["term", "sleep"] {
        for removal in removals.iter().filter(|r| !same_as_another(r)) {
            let fx = Fx::new();
            for p in &entries[..6] {
                write_file(&fx.home.join(p), "old");
            }
            write_file(&fx.receipt(), "old receipt");
            fs::create_dir(fx.home.join("Applications")).expect("applications");
            let before = snapshot(&fx.home);
            let mut journal = String::new();
            for p in &entries {
                let path = fx.home.join(p);
                let kind = if *p == "Applications/watane.app" {
                    "new"
                } else {
                    "old"
                };
                journal.push_str(&format!(
                    "{kind} {}\nmoved {}\n",
                    path.display(),
                    path.display()
                ));
                write_file(&fx.home.join(format!("{p}.new.99999999")), "staged");
                if kind == "old" {
                    write_file(&fx.home.join(format!("{p}.old.99999999")), &fx.read(&path));
                    if path != fx.receipt() {
                        fs::remove_file(&path).expect("moved aside");
                    }
                }
            }
            write_file(&fx.share("watane/journal.99999999"), &journal);
            let setup = r#"
HOME_R=$(cd "$HOME" && pwd -P)
set_entries
trap cleanup EXIT
trap 'on_signal 143' TERM
"#;
            let source = baked_script("0.1.0").replace(
                "main \"$@\"",
                &format!("{setup}\nSWAP_PID=99999999\nSWAP_ACTIVE=1\nrollback\n"),
            );
            fs::write(&fx.script, source).expect("undo fixture");
            fx.state_write("rm_fault", &format!("{mode}\n{removal}\n"));
            if mode == "term" {
                let out = fx.run(&[]);
                assert_eq!(out.code, 143, "{removal}: {out:?}");
                assert!(!fx.state.join("rm_fault").exists(), "{removal} was reached");
                assert_eq!(snapshot(&fx.home), before, "{removal}: {out:?}");
            } else {
                let child = spawn(&mut fx.cmd(&[]));
                fx.wait_state("rm_sleeping");
                signal_group(child.id(), "KILL");
                let out = finish(child);
                assert_eq!(out.code, -1, "{removal}: {out:?}");
                fx.unfault("rm_fault");
            }
            fx.unfault("rm_fault");
            // A person puts their own app here after the interruption. Its staged
            // .new file never moved in; neither rollback nor recovery owns this app.
            write_file(&fx.app().join("marker"), "mine");
            let recover =
                baked_script("0.1.0").replace("main \"$@\"", &format!("{setup}\nrecover\n"));
            fs::write(&fx.script, recover).expect("recovery fixture");
            // after a TERM the handler's repeat finished the undo (the home is as before, asserted
            // above), so one recovery shows it has nothing left to do; after a KILL one recovery
            // finishes it, and at every fourth removal a second one shows that recovery can be repeated
            for _ in 0..if mode == "sleep"
                && removals.iter().position(|r| r == removal).unwrap_or(0) % 4 == 0
            {
                2
            } else {
                1
            } {
                let out = fx.run(&[]);
                assert_eq!(out.code, 0, "{mode}, {removal}: {out:?}");
                let mut expected = before.clone();
                expected.push(("Applications/watane.app/".to_string(), Vec::new()));
                expected.push((
                    "Applications/watane.app/marker".to_string(),
                    b"mine".to_vec(),
                ));
                expected.sort();
                let mut actual = snapshot(&fx.home);
                actual.sort();
                assert_eq!(actual, expected, "{mode}, {removal}: {out:?}");
                assert!(
                    leftovers(&fx).is_empty(),
                    "{mode}, {removal}: {:?}",
                    leftovers(&fx)
                );
            }
        }
    }
}

/// AC4: a fresh entry already removed by undo cannot claim a later replacement.
/// TECHNICAL 3.6 step 0, 3.8 (S50b-fix ac4): a fresh entry already removed by undo cannot claim a later replacement.
#[test]
#[ignore = "slow: a KILL at the undo's last removal and a later recovery, which only a killed run shows"]
fn ac4_recovery_keeps_replacement_at_an_undone_new_entry() {
    // the fresh entry still in place at the undo (moved back to staging), and already
    // gone (an empty staging directory stands in): both are proof for the retry
    for present in [true, false] {
        let fx = Fx::new();
        if present {
            write_file(&fx.bin("watane"), "installed by this run");
        } else {
            fs::create_dir_all(fx.home.join(".local/bin")).expect("bin");
        }
        write_file(&fx.share("watane/receipt.new.99999999"), "pending");
        write_file(
            &fx.share("watane/journal.99999999"),
            &format!(
                "new {}\nmoved {}\n",
                fx.bin("watane").display(),
                fx.bin("watane").display()
            ),
        );
        let setup = r#"
HOME_R=$(cd "$HOME" && pwd -P)
set_entries
"#;
        fs::write(
            &fx.script,
            baked_script("0.1.0").replace("main \"$@\"", &format!("{setup}\nundo_swap 99999999\n")),
        )
        .expect("undo fixture");
        kill_at(&fx, "rm", "*/share/watane/journal.99999999", &[]);
        write_file(&fx.bin("watane"), "mine");
        fs::write(
            &fx.script,
            baked_script("0.1.0").replace("main \"$@\"", &format!("{setup}\nrecover\n")),
        )
        .expect("recovery fixture");
        let out = fx.run(&[]);
        assert_eq!(out.code, 0, "present {present}: {out:?}");
        assert_eq!(
            fx.read(&fx.bin("watane")),
            "mine",
            "present {present}: {out:?}"
        );
        assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    }
}

/// Validator R1 (AC4): signals during xattr finish the committed swap's cleanup.
/// TECHNICAL 3.8, 3.6 step 5 (S50b-fix ac4): a signal during the quarantine step finishes the committed swap's cleanup.
#[test]
#[ignore = "slow: a TERM and an INT while the quarantine step sleeps after the commit, which only two whole upgrades show"]
fn ac4_signals_during_xattr_leave_the_new_version_without_a_journal() {
    for (signal, code) in [("TERM", 143), ("INT", 130)] {
        let (fx, _) = swap_start();
        fx.state_write("xattr_fault", "sleep\n");
        let child = spawn(&mut fx.cmd(&["--release", "0.2.0"]));
        fx.wait_state("xattr_sleeping");
        assert!(fx.read(&fx.receipt()).contains("\nversion 0.2.0\n"));
        assert!(has_leftover(&fx, "journal."));
        signal_group(child.id(), signal);
        let out = finish(child);
        assert_eq!(out.code, code, "{signal}: {out:?}");
        assert_all_new(&fx);
        assert!(fx.tmp_tree().is_empty(), "{signal}: {out:?}");
    }
}

fn plant_s35(fx: &Fx, n: u32) {
    let h = fx.home.display();
    let mut j = String::new();
    for l in fx.read(&fx.receipt()).lines() {
        if let Some(p) = l.strip_prefix("path ") {
            j.push_str(&format!("old {p}\n"));
        }
    }
    j.push_str(&format!("old {}\n", fx.receipt().display()));
    j.push_str(&format!("new {h}/Applications/watane.app\n"));
    j.push_str(&format!("old {h}/Documents/d\n"));
    j.push_str(&format!("moved {h}/Documents/e\n"));
    j.push_str(&format!("new {h}/.local/bin/watane-x"));
    let sw = fx.share("watane");
    fs::write(
        sw.join(format!("receipt.new.{n}")),
        "watane-receipt 1\nversion 0.2.0\n",
    )
    .expect("w");
    fs::write(sw.join(format!("journal.{n}")), j).expect("w");
    write_file(
        &fx.home.join(format!("Documents/d.old.{n}/marker")),
        "marker",
    );
    write_file(&fx.home.join("Documents/e/marker"), "marker");
    write_file(&fx.bin("watane-x"), "x");
}

/// INST-S-35 and TECHNICAL 3.6.1: a dead run's journal that names more than it moved changes nothing it does not own: the
/// next run recovers and reinstalls, and keeps the other files. The live journal and the unmoved `new`
/// line are `unit::unit_take_lock_and_the_live_journal_stop_a_second_run` and
/// `unit::unit_check_foreign_refuses_a_file_watane_did_not_install`.
#[test]
fn inst_s35_a_journal_that_names_more_than_it_moved() {
    let fx = installed();
    let n = dead_pid();
    plant_s35(&fx, n);
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("reinstalling watane 0.1.0"));
    fx.assert_installed("0.1.0");
    for p in [
        format!("Documents/d.old.{n}/marker"),
        "Documents/e/marker".to_string(),
        ".local/bin/watane-x".to_string(),
    ] {
        assert!(fx.home.join(&p).is_file(), "{p} is kept");
    }
    assert!(!fx.home.join("Documents/d").exists());
    assert!(!fx.share("watane").join(format!("journal.{n}")).exists());
    assert!(!fx.share("watane").join(format!("receipt.new.{n}")).exists());
    assert!(
        leftovers(&fx).iter().all(|l| l.contains("Documents")),
        "{:?}",
        leftovers(&fx)
    );
}

fn assert_xcode_tail(out: &Out) {
    let l = out.out_lines();
    assert_eq!(l[l.len() - 2..], XCODE_LINES, "{out:?}");
}

fn assert_no_xcode_call(fx: &Fx) {
    assert!(!fx.state_read("xcode-select.log").contains("--install"));
}

/// Runs a fresh install that waits at the first `chmod` of the staging, and returns the pid
/// of the script (the child is the script itself: `perl` execs `sh`).
fn install_held_at_staging(fx: &Fx) -> (RunChild, u32) {
    fx.state_write("chmod_wait", "");
    let child = spawn(&mut fx.cmd(&[]));
    fx.wait_state("chmod_waiting");
    let pid = child.id();
    (child, pid)
}

/// AC2 (3.6 step 2): a link planted at a name the script creates is never written through.
#[test]
#[ignore = "slow: a link planted at a name the script is about to create in a run held at its staging, which only a started whole run allows"]
fn ac2_a_planted_link_is_not_written_through() {
    for name in ["journal", "receipt.new"] {
        let fx = Fx::new();
        let victim = fx.root.path().join("victim");
        write_file(&victim, "victim\n");
        let (child, pid) = install_held_at_staging(&fx);
        let sw = fx.share("watane");
        fs::create_dir_all(&sw).expect("share");
        symlink(&victim, sw.join(format!("{name}.{pid}"))).expect("plant");
        fx.unfault("chmod_wait");
        let out = finish(child);
        assert_eq!(out.code, 5, "{name}: {out:?}");
        assert_eq!(fx.read(&victim), "victim\n", "{name}: written through");
        assert!(!fx.bin("watane").exists(), "{name}: nothing was swapped in");
    }
}

/// AC4 (3.7): the question names the installer that `WATANE_RUSTUP_URL` points at.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac4_the_toolchain_question_names_the_rustup_url() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "n")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        out.stdout.contains(&format!("from {url}, into ~/.rustup")),
        "{out:?}"
    );
    assert!(
        !out.stdout.contains("from https://sh.rustup.rs, into"),
        "{out:?}"
    );
}

/// AC3 (8.4 step 5): a home that resolves outside the home is not deleted; the command is printed.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac3_a_home_that_resolves_outside_the_home_is_not_deleted() {
    let fx = installed();
    let other = tempfile::tempdir().expect("other");
    let o = fs::canonicalize(other.path()).expect("canon");
    write_file(&o.join("marker"), "m");
    symlink(&o, fx.home.join("link")).expect("link");
    let value = fx.home.join("link").display().to_string();
    let out = fx.run_pty(
        &["--uninstall"],
        &[("WATANE_HOME", &value)],
        &[("uninstalled watane 0.1.0", "delete")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("Type delete"), "{out:?}");
    assert!(o.join("marker").exists(), "{out:?}");
    assert!(
        out.has_line(&format!("  rm -rf '{}'", o.display())),
        "{out:?}"
    );
    let kept = format!(
        "kept: ~/link (not deleted: it resolves to {}, outside your home folder)",
        o.display()
    );
    assert!(out.has_line(&kept), "{out:?}");
    assert!(out.has_line("To delete it yourself, run:"), "{out:?}");
}

/// AC3: the question names the resolved path.
/// TECHNICAL 8.4 step 5: the question names the resolved path of the home.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac3_the_question_names_the_resolved_path() {
    let fx = installed();
    write_file(&fx.home.join("real/marker"), "m");
    symlink(fx.home.join("real"), fx.home.join("link")).expect("link");
    let value = fx.home.join("link").display().to_string();
    let out = fx.run_pty(
        &["--uninstall"],
        &[("WATANE_HOME", &value)],
        &[("keep it: ", "n")],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        out.stdout
            .contains("Delete the watane home ~/real? It holds"),
        "{out:?}"
    );
    assert!(fx.home.join("real/marker").exists());
}

/// INST-S-31 and TECHNICAL 8.5: an uninstall through `bin` or `Applications` linked to the home removes nothing of the home's own.
#[test]
#[ignore = "slow: an uninstall (a whole run) through each folder linked to the home"]
fn inst_s31_uninstall_through_bin_or_applications_linked_to_the_home() {
    for (rel, line, victim) in [
        (".local/bin", ".local/bin/watane-x", "watane-x"),
        ("Applications", "Applications/watane.app", "watane.app"),
    ] {
        let fx = Fx::installed();
        let link = fx.home.join(rel);
        fs::remove_dir_all(&link).expect("rm");
        symlink(&fx.home, &link).expect("link");
        write_file(&fx.home.join(victim), "mine");
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(fx.receipt())
            .expect("open");
        std::io::Write::write_all(
            &mut f,
            format!("path {}/{line}\n", fx.home.display()).as_bytes(),
        )
        .expect("append");
        drop(f);
        let out = fx.run(&["--uninstall"]);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(fx.home.join(victim).exists(), "{victim} removed: {out:?}");
    }
}

/// INST-S-34 and TECHNICAL 3.9: the command line tools lines end the output of a run that asked the toolchain
/// question, for each answer; `xcode-select --install` is never run (the lines themselves are
/// `unit::unit_what_the_end_of_an_install_says` and the default `inst_s1_fresh_install`).
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s34_command_line_tools_for_each_answer() {
    for answer in ["y", "n"] {
        let fx = Fx::new();
        fx.state_write("xcode_missing", "");
        let url = fx.rustup_installer();
        let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", answer)]);
        assert_eq!(out.code, 0, "{answer}: {out:?}");
        assert_xcode_tail(&out);
        assert_no_xcode_call(&fx);
    }
}
