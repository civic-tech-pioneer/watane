//! INST-S-1 to 22, 26 to 28, 30 and 31: runs `install.sh` against a fake release (TECHNICAL 12.1).

mod support;

use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Stdio};

use support::*;

const CURL_LINE: &str = "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2";

fn archive() -> String {
    archive_name("0.1.0")
}

fn installed() -> Fx {
    let fx = Fx::new();
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    fx
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

#[test]
fn inst_s1_fresh_install() {
    let fx = Fx::new();
    let out = fx.run(&[]);
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
}

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
        assert!(out.has_line(CURL_LINE));
        fx.assert_installed("0.1.0");
    }
}

#[test]
fn inst_s4_no_terminal() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_env(&[], &[("WATANE_RUSTUP_URL", &url)]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("[y/N]"));
    assert!(out.has_line("no terminal to ask on: the toolchain was not installed"));
    assert!(out.has_line(CURL_LINE));
    assert_eq!(fx.state_read("installer.log"), "");
}

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
    assert!(out.has_line(CURL_LINE));
    assert_eq!(fx.state_read("installer.log"), "");
}

#[test]
fn inst_s5b_there_is_no_flag_that_says_yes() {
    let fx = Fx::new();
    let out = fx.run(&["--toolchain"]);
    assert_eq!(out.code, 1);
    assert_eq!(
        out.first_err(),
        "watane install: unknown or conflicting flag --toolchain"
    );
    assert!(fx.home_tree().is_empty());
}

#[test]
fn inst_s5c_no_variable_says_yes() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let file = fx.root.path().join("tty");
    fs::write(&file, "y\n").expect("write");
    let out = fx.run_env(
        &[],
        &[
            ("WATANE_INSTALL_TTY", &file.display().to_string()),
            ("WATANE_RUSTUP_URL", &url),
        ],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("no terminal to ask on: the toolchain was not installed"));
    assert_eq!(fx.state_read("installer.log"), "");
}

#[test]
fn inst_s6_toolchain_complete() {
    let fx = Fx::new();
    fx.add_rustup(true);
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("toolchain: ok (rustup, wasm32-wasip2)"));
    assert!(!out.stdout.contains("[y/N]"));
    assert_eq!(fx.state_read("installer.log"), "");
}

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
}

#[test]
fn inst_s8_checksum_mismatch() {
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
}

#[test]
fn inst_s9_no_checksum_line() {
    let fx = Fx::new();
    fx.rel.put(
        "SHA256SUMS",
        format!("{}  other.tar.gz\n", "0".repeat(64)).into_bytes(),
    );
    let out = fx.run(&[]);
    assert_eq!(out.code, 4);
    assert_eq!(
        out.first_err(),
        format!("watane install: SHA256SUMS has no line for {}", archive())
    );
    assert!(fx.home_tree().is_empty());
    assert!(fx.tmp_tree().is_empty());
}

#[test]
fn two_checksum_lines_are_refused() {
    let fx = Fx::new();
    let line = format!("{}  {}\n", "0".repeat(64), archive());
    fx.rel
        .put("SHA256SUMS", format!("{line}{line}").into_bytes());
    let out = fx.run(&[]);
    assert_eq!(out.code, 4, "{out:?}");
    assert_eq!(
        out.first_err(),
        format!(
            "watane install: SHA256SUMS has more than one line for {}",
            archive()
        )
    );
    assert!(fx.home_tree().is_empty());
}

#[test]
fn inst_s10_download_fails() {
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
fn inst_s12_upgrade() {
    let fx = Fx::new();
    fx.rel.set_version("0.1.0", &["old-crate-1.0.0"]);
    assert_eq!(fx.run(&[]).code, 0);
    write_file(&fx.bin("mine"), "mine");
    write_file(&fx.home.join(".watane/marker"), "marker");
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

#[test]
fn a_path_only_the_old_receipt_held_is_removed() {
    let fx = installed();
    write_file(&fx.bin("watane-gone"), "old");
    let line = format!("path {}\n", fx.bin("watane-gone").display());
    fs::write(fx.receipt(), fx.read(&fx.receipt()) + &line).expect("write");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!fx.bin("watane-gone").exists());
    assert!(!fx.read(&fx.receipt()).contains("watane-gone"));
}

#[test]
fn inst_s13_same_version_again() {
    let fx = installed();
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("reinstalling watane 0.1.0"));
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty());
}

#[test]
fn inst_s14_failed_upgrade_rolls_back() {
    let fx = installed();
    let before = snapshot(&fx.home);
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    fx.state_write("mv_fault", "fail\n*watane.app.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    assert_eq!(snapshot(&fx.home), before);
    assert!(fx.tmp_tree().is_empty());
}

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

#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s15_uninstall_home_kept() {
    let fx = uninstall_setup();
    let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", "n")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_uninstalled(&fx);
    assert!(fx.home.join(".watane/marker").is_file());
    assert!(out.has_line("kept: ~/.watane (your home: identity, mods, channels)"));
    assert!(out.has_line("uninstalled watane 0.1.0"));
}

#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s16_uninstall_home_deleted() {
    let fx = uninstall_setup();
    let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", "delete")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!fx.home.join(".watane").exists());
    assert!(out.has_line("deleted: ~/.watane"));
    for other in ["DELETE", "yes", "y"] {
        let fx = uninstall_setup();
        let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", other)]);
        assert_eq!(out.code, 0, "{out:?}");
        assert!(
            fx.home.join(".watane/marker").is_file(),
            "{other} keeps the home"
        );
    }
}

#[test]
fn inst_s17_uninstall_no_terminal() {
    let fx = uninstall_setup();
    let file = fx.root.path().join("tty");
    fs::write(&file, "delete\n").expect("write");
    let out = fx.run_env(
        &["--uninstall"],
        &[("WATANE_INSTALL_TTY", &file.display().to_string())],
    );
    assert_eq!(out.code, 0, "{out:?}");
    assert_uninstalled(&fx);
    assert!(fx.home.join(".watane/marker").is_file());
    assert!(!out.stdout.contains("keep it"));
    assert!(out.has_line("kept: ~/.watane (your home: identity, mods, channels)"));
}

#[test]
fn inst_s18_uninstall_of_nothing() {
    let fx = Fx::new();
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 7);
    assert_eq!(
        out.first_err(),
        "watane install: nothing to uninstall: no receipt at ~/.local/share/watane/receipt"
    );
    assert!(fx.home_tree().is_empty());
}

#[test]
fn inst_s19_bin_not_on_path() {
    let fx = Fx::new();
    let out = fx.run_env(&[], &[("SHELL", "/bin/zsh")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line(
        "~/.local/bin is not on your PATH, so a new terminal will not find watane-dev yet."
    ));
    assert!(out.has_line(r#"  echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc"#));
    for f in [".zshrc", ".bash_profile", ".profile"] {
        assert!(!fx.home.join(f).exists());
    }
    let fx = Fx::new();
    let path = format!(
        "{}:{}",
        fx.path_value(),
        fx.home.join(".local/bin").display()
    );
    let out = fx.run_env(&[], &[("SHELL", "/bin/zsh"), ("PATH", &path)]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("is not on your PATH"));
}

#[test]
fn inst_s20_unsupported_platform() {
    let cases: [(&str, &str, &str, &str); 3] = [
        ("Linux", "x86_64", "14.5", "Linux x86_64"),
        ("Darwin", "x86_64", "14.5", "Darwin x86_64"),
        (
            "Darwin",
            "arm64",
            "12.7",
            "macOS 12.7 (needs 14.0 or later)",
        ),
    ];
    for (s, m, v, shown) in cases {
        let fx = Fx::new();
        fx.state_write("uname_s", s);
        fx.state_write("uname_m", m);
        fx.state_write("sw_vers", v);
        let out = fx.run(&[]);
        assert_eq!(out.code, 2, "{out:?}");
        assert_eq!(
            out.first_err(),
            format!("watane install: unsupported platform: {shown}")
        );
        assert_eq!(
            out.stderr.lines().nth(1),
            Some("watane installs on macOS 14.0 or later, on Apple silicon.")
        );
        assert!(fx.rel.requests().is_empty());
        assert!(fx.home_tree().is_empty());
    }
}

#[test]
fn inst_s20_rosetta_shell_is_apple_silicon() {
    let fx = Fx::new();
    fx.state_write("uname_m", "x86_64");
    fx.state_write("rosetta", "1");
    assert_eq!(fx.run(&[]).code, 0);
}

#[test]
fn inst_s21_root_and_missing_tools() {
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
}

#[test]
fn inst_s22_gatekeeper_step() {
    let fx = Fx::new();
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    let want = format!(
        "-dr com.apple.quarantine {}/Applications/watane.app\n",
        fx.home.display()
    );
    assert_eq!(fx.state_read("xattr.log"), want);
    assert!(out.has_line(r#"If macOS says "watane" cannot be opened, or could not verify that it is free of malware:"#));
    assert!(out.has_line("    xattr -dr com.apple.quarantine ~/Applications/watane.app"));
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

#[test]
fn inst_s26_a_version_that_is_a_path() {
    let evil = "1/../../../../../evil/watane/releases/download/v1";
    let cases: [(&[&str], &str, &str); 5] = [
        (&["--release", evil], "", evil),
        (&[], evil, evil),
        (&["--release", "0.1.0/x"], "", "0.1.0/x"),
        (&["--release", ""], "", ""),
        (&["--release", "1.0"], "", "1.0"),
    ];
    for (args, env, shown) in cases {
        let fx = Fx::new();
        fake_curl(&fx);
        let mut c = fx.cmd(args);
        c.env_remove("WATANE_RELEASE_BASE");
        if !env.is_empty() {
            c.env("WATANE_RELEASE", env);
        }
        let out = exec(c);
        assert_eq!(out.code, 1, "{out:?}");
        assert_eq!(
            out.first_err(),
            format!("watane install: not a version: {shown}")
        );
        assert_eq!(
            out.stderr.lines().nth(1),
            Some("a version looks like 0.1.0")
        );
        assert_eq!(fx.state_read("curl.log"), "");
        assert!(fx.home_tree().is_empty());
    }
    let fx = Fx::new();
    fake_curl(&fx);
    let mut c = fx.cmd(&["--release", "0.2.0-rc1"]);
    c.env_remove("WATANE_RELEASE_BASE");
    let out = exec(c);
    assert!(!out.stderr.contains("not a version"), "{out:?}");
    assert!(
        fx.state_read("curl.log")
            .contains("releases/download/v0.2.0-rc1/SHA256SUMS")
    );
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

#[test]
fn inst_s27_a_receipt_that_names_other_files() {
    let (fx, named, other) = s27_start();
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    s27_kept(&fx, other.path());
    for p in &named {
        let line = format!("watane install: skipped {p}: not a path that watane installs");
        assert!(
            out.stderr.lines().any(|l| l == line),
            "{line}\n{}",
            out.stderr
        );
    }
    assert!(!fx.bin("watane").exists() && !fx.app().exists());
    let (fx, _, other) = s27_start();
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    s27_kept(&fx, other.path());
    assert_eq!(fx.read(&fx.share("watane/VERSION")).trim(), "0.2.0");
}

#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn inst_s28_a_home_that_is_not_a_watane_home() {
    let cases = ["$HOME", "/", "$HOME/x/..", "$HOME/link", "$HOME/link/"];
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

    let fx = installed();
    let r = fx.read(&fx.receipt());
    let kept: Vec<&str> = r
        .lines()
        .filter(|l| !l.ends_with("watane-dev/vendor"))
        .collect();
    fs::write(fx.receipt(), format!("{}\n", kept.join("\n"))).expect("write");
    let before = snapshot(&fx.home);
    let n = fx.rel.requests().len();
    let out = fx.run(&[]);
    assert_eq!(out.code, 8, "{out:?}");
    foreign_lines(&out, &["~/.local/share/watane-dev/vendor"]);
    assert_eq!(fx.rel.requests().len(), n);
    assert_eq!(snapshot(&fx.home), before);

    // a stale name with this run's own pid is removed, not copied into
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
    let mut child = c.spawn().expect("spawn");
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

    let fx = Fx::new();
    fs::create_dir_all(fx.receipt()).expect("receipt dir");
    let out = fx.run(&[]);
    assert_eq!(out.code, 8, "{out:?}");
    foreign_lines(&out, &["~/.local/share/watane/receipt"]);
}

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

    let (fx, other, o) = new();
    fs::create_dir_all(fx.home.join(".local")).expect("local");
    symlink(&o, fx.home.join("Applications")).expect("link");
    check(
        &fx,
        other.path(),
        "~/Applications",
        &o.display().to_string(),
    );

    let (fx, other, o) = new();
    fs::create_dir_all(fx.home.join(".local/bin")).expect("bin");
    symlink(&o, fx.home.join(".local/share")).expect("link");
    check(
        &fx,
        other.path(),
        "~/.local/share/watane",
        &format!("{}/watane", o.display()),
    );

    let (fx, other, o) = new();
    fs::create_dir_all(fx.home.join(".local/share")).expect("share");
    symlink(&o, fx.home.join(".local/share/watane-dev")).expect("link");
    check(
        &fx,
        other.path(),
        "~/.local/share/watane-dev",
        &o.display().to_string(),
    );

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
    let (fx, _other, o) = new();
    assert_eq!(fx.run(&[]).code, 0);
    fs::remove_dir_all(fx.share("watane-dev")).expect("rm");
    write_file(&o.join("vendor/marker"), "m");
    symlink(&o, fx.share("watane-dev")).expect("link");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(o.join("vendor/marker").exists());
    let line = format!(
        "watane install: skipped {}/.local/share/watane-dev/vendor: not a path that watane installs",
        fx.home.display()
    );
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);

    // uninstall removes nothing through a link at the folder of the receipt and the journals
    let (fx, _other, o) = new();
    assert_eq!(fx.run(&[]).code, 0);
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
}

#[test]
fn status_and_help() {
    let fx = Fx::new();
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 7);
    assert_eq!(
        out.first_err(),
        "watane install: not installed: no receipt at ~/.local/share/watane/receipt"
    );
    assert_eq!(fx.run(&["--no-toolchain", "--status"]).code, 1);
    let out = fx.run(&["--help"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.starts_with("usage: install.sh"));
    assert_eq!(fx.run(&[]).code, 0);
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 0);
    assert_eq!(
        out.stdout,
        "watane 0.1.0 (aarch64-apple-darwin)\n~/.local/bin/watane\n~/.local/bin/watane-dev\n~/.local/bin/watane-node\n~/.local/share/watane-dev/vendor\n~/.local/share/watane/install.sh\n~/.local/share/watane/VERSION\n~/Applications/watane.app\n"
    );
}

// --- INST-S-32 to 35: the swap's failure, kill and journal scenarios (TECHNICAL 3.6, 3.6.1) ---

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
    fx.state_write(&format!("{tool}_fault"), &format!("sleep\n{glob}\n"));
    let child = spawn(&mut fx.cmd(args));
    fx.wait_state(&format!("{tool}_sleeping"));
    signal_group(child.id(), "KILL");
    let out = finish(child);
    assert_eq!(out.code, -1, "killed: {out:?}");
    fs::remove_file(fx.state.join(format!("{tool}_fault"))).expect("unfault");
    fs::remove_file(fx.state.join(format!("{tool}_sleeping"))).expect("unsleep");
}

fn has_leftover(fx: &Fx, what: &str) -> bool {
    leftovers(fx).iter().any(|n| n.contains(what))
}

#[test]
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

    fs::remove_file(fx.state.join("mv_fault")).expect("unfault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert!(
        !out.stderr
            .contains("found files that watane did not install")
    );
    assert_all_new(&fx);
}

#[test]
fn inst_s33_a_kill_at_the_receipt() {
    let (fx, before) = swap_start();
    kill_at(&fx, "mv", "*receipt.new.*", &["--release", "0.2.0"]);
    assert_old_receipt(&fx);
    assert!(has_leftover(&fx, "journal."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, "receipt.new."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, ".old."), "{:?}", leftovers(&fx));
    // the recovery alone, then the install: a failing swap after it shows the old version whole
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(snapshot(&fx.home), before);
    fs::remove_file(fx.state.join("mv_fault")).expect("unfault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert_all_new(&fx);
}

#[test]
fn inst_s33_a_kill_at_the_receipt_of_a_fresh_install() {
    // no receipt yet: the files that the journal says were moved in are not foreign
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

#[test]
fn inst_s33_a_kill_after_the_receipt_move() {
    let (fx, _) = swap_start();
    kill_at(&fx, "rm", "*.old.*", &["--release", "0.2.0"]);
    assert!(fx.read(&fx.receipt()).contains("\nversion 0.2.0\n"));
    assert!(has_leftover(&fx, "journal."), "{:?}", leftovers(&fx));
    assert!(has_leftover(&fx, ".old."), "{:?}", leftovers(&fx));
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_all_new(&fx);
}

#[test]
fn inst_s33_a_kill_after_the_receipt_move_keeps_what_the_old_receipt_held() {
    // the old receipt is the only record of this path; the killed run cut step 5 at its removal
    let (fx, _) = swap_start();
    write_file(&fx.bin("watane-gone"), "old");
    let line = format!("path {}\n", fx.bin("watane-gone").display());
    fs::write(fx.receipt(), fx.read(&fx.receipt()) + &line).expect("write");
    kill_at(&fx, "rm", "*watane-gone*", &["--release", "0.2.0"]);
    assert!(fx.read(&fx.receipt()).contains("\nversion 0.2.0\n"));
    assert!(fx.bin("watane-gone").exists());
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        !fx.bin("watane-gone").exists(),
        "the next run finished step 5"
    );
    assert_all_new(&fx);
}

#[test]
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
    fs::remove_file(fx.state.join("mv_fault")).expect("unfault");
    let out = fx.run(&["--release", "0.2.0"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("upgrading watane 0.1.0 to 0.2.0"));
    assert_all_new(&fx);
}

#[test]
fn inst_s33_term_in_the_swap_undoes_it() {
    let (fx, before) = swap_start();
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    let child = spawn(&mut fx.cmd(&["--release", "0.2.0"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert_eq!(snapshot(&fx.home), before);
    assert!(fx.tmp_tree().is_empty());
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

    // the same files of a live process: nothing is done
    let fx = installed();
    let mut live = Command::new(fx.real.join("sleep"))
        .arg("600")
        .spawn()
        .expect("sleep");
    plant_s35(&fx, live.id());
    let before = snapshot(&fx.home);
    let asked = fx.rel.requests().len();
    let out = fx.run(&[]);
    live.kill().expect("kill");
    live.wait().expect("wait");
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        out.first_err(),
        format!(
            "watane install: another install is running (pid {})",
            live.id()
        )
    );
    assert!(out.stderr.contains(&format!("journal.{}", live.id())));
    assert_eq!(fx.rel.requests().len(), asked, "no request to the release");
    assert_eq!(snapshot(&fx.home), before);

    // no receipt: a `new` line with no `moved` line does not make a foreign app watane's
    let fx = Fx::new();
    let n = dead_pid();
    write_file(
        &fx.share("watane").join(format!("journal.{n}")),
        &format!("new {}\n", fx.app().display()),
    );
    fs::create_dir_all(fx.app()).expect("app");
    write_file(&fx.app().join("marker"), "m");
    let before = snapshot(&fx.home);
    let out = fx.run(&[]);
    assert_eq!(out.code, 8, "{out:?}");
    foreign_lines(&out, &["~/Applications/watane.app"]);
    assert_eq!(snapshot(&fx.home), before);
}

// --- AC6: a member that leaves the folder ---

#[test]
fn ac6_a_member_that_leaves_its_folder_is_refused() {
    let fx = Fx::new();
    let abs = fx.root.path().join("evil-abs");
    for name in [
        abs.display().to_string(),
        "watane-0.1.0-aarch64-apple-darwin/../../evil-rel".to_string(),
        "../evil-up".to_string(),
    ] {
        let bytes = archive_with_member(&name);
        fx.rel.put(&archive(), bytes.clone());
        fx.rel.put(
            "SHA256SUMS",
            format!("{}  {}\n", sha256(&bytes), archive()).into_bytes(),
        );
        let out = fx.run(&[]);
        assert_eq!(out.code, 5, "{name}: {out:?}");
        assert!(out.stderr.contains("leaves its folder"), "{name}: {out:?}");
        assert!(fx.home_tree().is_empty(), "{name}: {:?}", fx.home_tree());
        assert!(fx.tmp_tree().is_empty(), "{name}: {:?}", fx.tmp_tree());
        let outside: Vec<String> = fx
            .tree(fx.root.path())
            .into_iter()
            .filter(|n| n.contains("evil"))
            .collect();
        assert!(outside.is_empty(), "{name}: {outside:?}");
    }
}

// --- INST-S-34: the command line tools ---

const XCODE_LINES: [&str; 2] = [
    "watane-dev needs the macOS command line tools (a C linker and git). Run this and follow macOS's own dialog:",
    "  xcode-select --install",
];

fn assert_xcode_tail(out: &Out) {
    let l = out.out_lines();
    assert_eq!(l[l.len() - 2..], XCODE_LINES, "{out:?}");
}

fn assert_no_xcode_call(fx: &Fx) {
    assert!(!fx.state_read("xcode-select.log").contains("--install"));
}

#[test]
fn inst_s34_command_line_tools() {
    let fx = Fx::new();
    fx.state_write("xcode_missing", "");
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_xcode_tail(&out);
    assert_no_xcode_call(&fx);

    let fx = Fx::new();
    fx.state_write("xcode_missing", "");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_xcode_tail(&out);
    assert_no_xcode_call(&fx);

    let fx = Fx::new();
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("xcode-select"));
    assert!(!out.stdout.contains("command line tools"));
    assert_no_xcode_call(&fx);
}

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
