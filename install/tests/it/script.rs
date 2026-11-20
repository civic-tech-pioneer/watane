//! INST-S-1 to 22, 26 to 28, 30 and 31: runs `install.sh` against a fake release (TECHNICAL 12.1).

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
fn ac1_toolchain_matches_the_raw_answer() {
    let script = fs::read_to_string(checkout_script()).expect("script");
    let step = script.split("  ts_ans=\"\"").nth(1).expect("answer step");
    let decision = step.split("  case $TC_STATE in").next().expect("decision");
    assert!(decision.contains("case $ts_ans in\n    [Yy] | [Yy][Ee][Ss])"));
    assert!(!decision.contains("tr "));
}

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

#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn adv_s50b_fix_toolchain_nul_in_answer_is_no() {
    // 3.7: only `y` and `yes` count; `y` then a NUL (Ctrl-@) is another answer
    for answer in [&b"y\0"[..], &b"\0yes"[..], &b"y\0\0\0"[..]] {
        let fx = Fx::new();
        let url = fx.rustup_installer();
        let out = support::pty::run_bytes(
            &fx,
            &[],
            &[("WATANE_RUSTUP_URL", &url)],
            &[("[y/N] ", answer)],
        );
        assert_eq!(out.code, 0, "{out:?}");
        assert_eq!(
            fx.state_read("installer.log"),
            "",
            "answer {answer:?}: {out:?}"
        );
        assert!(out.has_line("toolchain: not installed"), "{out:?}");
    }
}

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

#[test]
fn inst_s4_no_terminal() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_env(&[], &[("WATANE_RUSTUP_URL", &url)]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(!out.stdout.contains("[y/N]"));
    assert!(out.has_line("no terminal to ask on: the toolchain was not installed"));
    // IN-44: the command names the URL that the offer would have named;
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
    assert!(!out.has_line("If watane is open, quit it and open it again."));
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
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac3_uninstall_nul_in_answer_keeps_home() {
    // 8.4 via 3.7's reader: a NUL anywhere, or a trailing space, is not `delete`
    for answer in [
        &b"de\0lete"[..],
        &b"delete\0"[..],
        &b"\0delete"[..],
        &b"delete "[..],
    ] {
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
        let shown = p.replacen(&fx.home.display().to_string(), "~", 1);
        let line = format!("watane install: skipped {shown}: not a path that watane installs");
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
    let line =
        "watane install: skipped ~/.local/share/watane-dev/vendor: not a path that watane installs";
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
    let child_pid = child.id();
    signal_group(child_pid, "KILL");
    let out = finish(child);
    assert_eq!(out.code, -1, "killed: {out:?}");
    fs::remove_file(fx.state.join(format!("{tool}_fault"))).expect("unfault");
    fs::remove_file(fx.state.join(format!("{tool}_sleeping"))).expect("unsleep");
    // Direct undo fixtures do not run main and therefore never acquire a lock.
    if !fx.read(&fx.script).ends_with("main \"$@\"\n") {
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

#[test]
fn inst_s33_term_in_the_swap_undoes_it() {
    let out = signal_in_the_swap_undoes_it("TERM");
    assert_eq!(out.code, 143, "{out:?}");
}

#[test]
fn inst_s33_int_in_the_swap_undoes_it() {
    let out = signal_in_the_swap_undoes_it("INT");
    // 3.8: macOS sh can lose an INT that lands just after the `mv` it waited on died of it; the
    // run then ends as the failed swap it saw
    let lost = out.code == 5 && out.stderr.contains("could not put the files in place");
    assert!(out.code == 130 || lost, "{out:?}");
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

/// 3.6 step 4, 3.8: `PIPE` (the reader went away) in the swap undoes it as `TERM` does.
#[test]
fn inst_s33_pipe_in_the_swap_undoes_it() {
    let out = signal_in_the_swap_undoes_it("PIPE");
    assert_eq!(out.code, 141, "{out:?}");
}

/// 3.8: a signal that kills the undo's subshell (macOS sh forks it with the signal pending) leaves
/// the swap in progress, so the handler that runs next undoes it again.
#[test]
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

/// AC4 (3.6 step 4, 3.8): interrupt each removal in a staged undo, both with
/// TERM (the handler repeats rollback) and KILL (the next run recovers).
#[test]
fn ac4_every_undo_removal_can_be_repeated_or_recovered() {
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
    for mode in ["term", "sleep"] {
        for removal in &removals {
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
                fs::remove_file(fx.state.join("rm_fault")).expect("unfault");
            }
            // A person puts their own app here after the interruption. Its staged
            // .new file never moved in; neither rollback nor recovery owns this app.
            write_file(&fx.app().join("marker"), "mine");
            let recover =
                baked_script("0.1.0").replace("main \"$@\"", &format!("{setup}\nrecover\n"));
            fs::write(&fx.script, recover).expect("recovery fixture");
            for _ in 0..2 {
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
#[test]
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

/// Validator R1 (AC4): clearing both a fresh entry and its parent cannot wedge recovery.
#[test]
fn ac4_recovery_skips_a_fresh_entry_whose_parent_is_gone() {
    let fx = Fx::new();
    write_file(&fx.share("watane/receipt.new.99999999"), "pending");
    write_file(
        &fx.share("watane/journal.99999999"),
        &format!(
            "new {}\nmoved {}\n",
            fx.bin("watane").display(),
            fx.bin("watane").display()
        ),
    );
    assert!(!fx.home.join(".local/bin").exists());
    for _ in 0..2 {
        let out = fx.run(&["--no-toolchain"]);
        assert_eq!(out.code, 0, "{out:?}");
        fx.assert_installed("0.1.0");
        assert!(leftovers(&fx).is_empty(), "{:?}", leftovers(&fx));
    }
}

/// Validator R1 (AC4): recovery cannot continue after a real filesystem failure.
#[test]
fn ac4_recovery_errors_stop_with_a_message_and_exit_5() {
    for failure in [
        "marker",
        "move",
        "restore",
        "staging",
        "journal",
        "committed",
        "commit_journal",
        "remove",
        "receipt_old",
        "stale",
    ] {
        let fx = Fx::new();
        fs::create_dir_all(fx.home.join(".local/bin")).expect("bin");
        if failure != "stale" {
            write_file(
                &fx.share("watane/journal.99999999"),
                &format!(
                    "{} {}\nmoved {}\n",
                    if matches!(failure, "restore" | "remove") {
                        "old"
                    } else {
                        "new"
                    },
                    fx.bin("watane").display(),
                    fx.bin("watane").display(),
                ),
            );
        }
        if !matches!(failure, "committed" | "commit_journal") {
            write_file(&fx.share("watane/receipt.new.99999999"), "pending");
        } else {
            let home = fs::canonicalize(&fx.home).expect("physical home");
            let resolved = [
                ".local/bin",
                ".local/share/watane",
                ".local/share/watane-dev",
                "Applications",
            ]
            .map(|folder| format!("resolved {folder} {}\n", home.join(folder).display()))
            .join("");
            write_file(
                &fx.receipt(),
                &format!(
                    "watane-receipt 1\nversion 0.1.0\ntarget {TARGET}\npath {}\n{resolved}",
                    fx.bin("watane").display()
                ),
            );
        }
        match failure {
            "marker" => {
                let mkdir = fx.fakes.join("mkdir");
                write_file(
                    &mkdir,
                    "#!/bin/sh\ncase $1 in *.new.99999999) exit 1 ;; esac\nexec \"$FAKE_REAL/mkdir\" \"$@\"\n",
                );
                fs::set_permissions(mkdir, fs::Permissions::from_mode(0o755)).expect("chmod");
            }
            "move" => {
                write_file(&fx.bin("watane"), "new");
                fx.state_write("mv_fault", "fail\n*/bin/watane.new.99999999\n");
            }
            "restore" => {
                write_file(&fx.bin("watane"), "new");
                write_file(&fx.bin("watane.old.99999999"), "old");
                fx.state_write("mv_fault", "fail\n*/bin/watane.old.99999999\n");
            }
            "staging" | "stale" => {
                write_file(&fx.bin("watane.new.99999999"), "staged");
                fx.state_write("rm_fault", "fail\n*/bin/watane.new.99999999\n");
            }
            "journal" | "commit_journal" => {
                fx.state_write("rm_fault", "fail\n*/journal.99999999\n")
            }
            "remove" => {
                write_file(&fx.bin("watane"), "new");
                write_file(&fx.bin("watane.old.99999999"), "old");
                fx.state_write("rm_fault", "fail\n*/bin/watane\n");
            }
            "receipt_old" => {
                write_file(&fx.share("watane/receipt.old.99999999"), "old");
                fx.state_write("rm_fault", "fail\n*/receipt.old.99999999\n");
            }
            "committed" => {
                write_file(&fx.bin("watane.old.99999999"), "old");
                fx.state_write("rm_fault", "fail\n*/bin/watane.old.99999999\n");
            }
            _ => unreachable!(),
        }
        let source = baked_script("0.1.0").replace(
            "main \"$@\"",
            r#"
HOME_R=$(cd "$HOME" && pwd -P)
set_entries
recover
printf 'continued after recovery\n'
"#,
        );
        fs::write(&fx.script, source).expect("recovery fixture");
        let out = fx.run(&[]);
        assert_eq!(out.code, 5, "{failure}: {out:?}");
        let message = if failure == "stale" {
            "watane install: could not recover stale install files"
        } else {
            "watane install: could not recover interrupted install (pid 99999999)"
        };
        assert!(out.stderr.contains(message), "{failure}: {out:?}");
        assert!(
            out.stderr
                .contains("Recovery stopped; fix the filesystem error and run this again."),
            "{failure}: {out:?}"
        );
        assert!(
            !out.stdout.contains("continued after recovery"),
            "{failure}: {out:?}"
        );
    }
}

/// Validator R1 (AC4): signals during xattr finish the committed swap's cleanup.
#[test]
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

// --- S50b-fix2 ---

fn serve(fx: &Fx, bytes: Vec<u8>) {
    fx.rel.put(&archive(), bytes.clone());
    fx.rel.put(
        "SHA256SUMS",
        format!("{}  {}\n", sha256(&bytes), archive()).into_bytes(),
    );
}

/// AC1 (3.4 step 9): an archive with a link member is refused before anything is moved.
#[test]
fn ac1_an_archive_with_a_link_member_is_refused() {
    for kind in ["symlink", "hardlink", "dirlink"] {
        let fx = Fx::new();
        let outside = fx.root.path().join("outside");
        write_file(&outside, "outside\n");
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).expect("chmod");
        let o = outside.clone();
        let bytes = build_archive_with("0.1.0", &["serde-1.0.0"], &move |t| match kind {
            "symlink" => {
                fs::remove_file(t.join("bin/watane")).expect("rm");
                symlink(&o, t.join("bin/watane")).expect("link");
            }
            "hardlink" => {
                fs::remove_file(t.join("bin/watane")).expect("rm");
                fs::hard_link(t.join("bin/watane-dev"), t.join("bin/watane")).expect("hard");
            }
            _ => {
                symlink(&o, t.join("share/watane-dev/vendor/serde-1.0.0/elsewhere")).expect("link");
            }
        });
        serve(&fx, bytes);
        let out = fx.run(&[]);
        assert_eq!(out.code, 5, "{kind}: {out:?}");
        assert!(out.stderr.contains("link"), "{kind}: {out:?}");
        assert!(fx.home_tree().is_empty(), "{kind}: {:?}", fx.home_tree());
        assert!(fx.tmp_tree().is_empty(), "{kind}: {:?}", fx.tmp_tree());
        assert_eq!(fx.read(&outside), "outside\n", "{kind}");
        let mode = fs::metadata(&outside).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{kind}: mode of the outside file");
    }
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
fn ac2_a_planted_link_is_not_written_through() {
    for name in ["journal", "receipt.new"] {
        let fx = Fx::new();
        let victim = fx.root.path().join("victim");
        write_file(&victim, "victim\n");
        let (child, pid) = install_held_at_staging(&fx);
        let sw = fx.share("watane");
        fs::create_dir_all(&sw).expect("share");
        symlink(&victim, sw.join(format!("{name}.{pid}"))).expect("plant");
        fs::remove_file(fx.state.join("chmod_wait")).expect("release");
        let out = finish(child);
        assert_eq!(out.code, 5, "{name}: {out:?}");
        assert_eq!(fx.read(&victim), "victim\n", "{name}: written through");
        assert!(!fx.bin("watane").exists(), "{name}: nothing was swapped in");
    }
}

/// AC5 (8.5 rule 2): a `bin/` name ending in `.mine` is not removed.
#[test]
fn ac5_a_bin_name_ending_in_mine_is_skipped() {
    let fx = installed();
    let h = fx.home.display().to_string();
    write_file(&fx.bin("watane.mine"), "mine");
    append(&fx.receipt(), &format!("path {h}/.local/bin/watane.mine\n"));
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.bin("watane.mine").exists());
    let line = "watane install: skipped ~/.local/bin/watane.mine: not a path that watane installs";
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);
    assert!(!fx.bin("watane").exists());
}

/// AC5 (8.5 rule 3): the holding folder resolves inside the install folder, not just the home.
#[test]
fn ac5_a_link_inside_an_install_folder_does_not_reach_the_home() {
    let fx = installed();
    let h = fx.home.display().to_string();
    write_file(&fx.home.join("Documents/marker"), "m");
    symlink(&fx.home, fx.share("watane/x")).expect("link");
    append(
        &fx.receipt(),
        &format!("path {h}/.local/share/watane/x/Documents\n"),
    );
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.home.join("Documents/marker").exists(), "{out:?}");
    let line = "watane install: skipped ~/.local/share/watane/x/Documents: not a path that watane installs";
    assert!(out.stderr.lines().any(|l| l == line), "{}", out.stderr);
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

#[test]
fn inst_s31_an_install_folder_resolving_to_the_home() {
    let home_err = |fx: &Fx, folder: &str| {
        let out = fx.run(&[]);
        assert_eq!(out.code, 2, "{out:?}");
        assert_eq!(
            out.first_err(),
            format!("watane install: {folder} resolves to your home folder itself")
        );
        assert_eq!(
            out.stderr.lines().nth(1),
            Some(
                "watane writes only under your home folder. Make it a real folder there, then run this again."
            )
        );
        assert!(fx.rel.requests().is_empty());
    };
    for (rel, shown) in [
        (".local/bin", "~/.local/bin"),
        ("Applications", "~/Applications"),
        (".local/share/watane", "~/.local/share/watane"),
        (".local/share/watane-dev", "~/.local/share/watane-dev"),
    ] {
        let fx = Fx::new();
        fs::create_dir_all(fx.home.join(".local/share")).expect("share");
        symlink(&fx.home, fx.home.join(rel)).expect("link");
        home_err(&fx, shown);
    }
}

#[test]
fn inst_s31_uninstall_through_bin_or_applications_linked_to_the_home() {
    for (rel, line, victim) in [
        (".local/bin", ".local/bin/watane-x", "watane-x"),
        ("Applications", "Applications/watane.app", "watane.app"),
    ] {
        let fx = Fx::new();
        assert_eq!(fx.run(&[]).code, 0);
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
