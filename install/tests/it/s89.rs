//! S89: install.sh tells the truth at each interruption, pipe, path and rustup line
//! (install/TECHNICAL 3.6 to 3.9, 8.1, 8.5; IN-44 to IN-48).

use crate::support::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::{Command, Stdio};

const HINT: [&str; 2] = [
    "watane 0.1.0 is installed; the run was interrupted before it finished.",
    "Run ~/.local/bin/watane-dev doctor to check the maker tools.",
];

fn fake_tool(fx: &Fx, name: &str, body: &str) {
    let p = fx.fakes.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).expect("fake tool");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

/// TECHNICAL 3.8, 3.9 (IN-45): an INT after the commit ends the run with the hint that watane is installed.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn ac1_int_after_the_commit_says_watane_is_installed() {
    let fx = Fx::new();
    // the quarantine step runs after the receipt moved into place
    fake_tool(
        &fx,
        "xattr",
        "echo up > \"$FAKE_STATE/xattr_sleeping\"\nexec \"$FAKE_REAL/sleep\" 600",
    );
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("xattr_sleeping");
    signal_group(child.id(), "INT");
    let out = finish(child);
    assert_eq!(out.code, 130, "{out:?}");
    assert_eq!(lines(&out.stderr), ["", HINT[0], HINT[1]], "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(fx.tmp_tree().is_empty());
}

/// TECHNICAL 3.8, 3.7 (IN-45): an INT at the toolchain question says watane is installed and prints the install-it-later block.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac2_int_at_the_toolchain_question_says_watane_is_installed() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "\x03")]);
    assert_eq!(out.code, 130, "{out:?}");
    let all = lines(&out.stdout);
    let at = all
        .iter()
        .position(|l| *l == HINT[0])
        .unwrap_or_else(|| panic!("no hint: {out:?}"));
    assert_eq!(all[at + 1], HINT[1], "{out:?}");
    assert_eq!(all[at + 2], "To install it later, run:", "{out:?}");
    assert_eq!(
        all[at + 3],
        format!(
            "  curl --tlsv1.2 -sSf '{url}' | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2"
        ),
        "{out:?}"
    );
    assert!(
        !out.has_line("watane 0.1.0 is installed."),
        "no success block: {out:?}"
    );
    fx.assert_installed("0.1.0");
}

/// TECHNICAL 3.9 (IN-46): `--status` to a closed reader is quiet (exit 141 or 0, nothing on stderr).
#[test]
#[ignore = "slow: five runs of `--status` whose reader went away before the write, which only whole runs show (TECHNICAL 3.9)"]
fn ac3_status_to_a_closed_reader_is_quiet() {
    let fx = Fx::installed();
    for _ in 0..5 {
        let mut c = fx.cmd(&["--status"]);
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_run(&mut c);
        // the reader goes away before the script writes
        drop(child.stdout.take());
        let out = finish(child);
        assert_eq!(out.stderr, "", "{out:?}");
        assert!(out.code == 141 || out.code == 0, "{out:?}");
    }
}

/// TECHNICAL 3.9: a reader can consume one status line and close the pipe quietly.
#[test]
fn ac3_status_piped_to_head_is_quiet() {
    let fx = Fx::installed();
    let mut child = spawn_run(
        fx.cmd(&["--status"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    );
    let head = Command::new("head")
        .arg("-1")
        .stdin(child.stdout.take().expect("status stdout"))
        .output()
        .expect("head");
    assert!(head.status.success(), "{head:?}");
    assert_eq!(head.stdout, format!("watane 0.1.0 ({TARGET})\n").as_bytes());
    let out = finish(child);
    assert_eq!(out.stderr, "", "{out:?}");
    assert!(out.code == 141 || out.code == 0, "{out:?}");
}

/// TECHNICAL 8.1, 8.5 (IN-47): paths in a linked home print as `~` and never as the physical path.
#[test]
#[ignore = "slow: an install and an uninstall with `HOME` itself a link, which only whole runs show (TECHNICAL 8.1)"]
fn ac4_paths_in_a_linked_home_print_as_tilde() {
    let fx = Fx::new();
    let link = fx.root.path().join("link");
    symlink(&fx.home, &link).expect("link");
    let h = link.display().to_string();
    let env = [("HOME", h.as_str())];
    let out = fx.run_env(&["--no-toolchain"], &env);
    assert_eq!(out.code, 0, "{out:?}");
    // a path the script never installs, and a folder that now resolves elsewhere
    let receipt = fx.read(&fx.receipt());
    fs::write(
        fx.receipt(),
        format!("{receipt}path {h}/.local/share/fish\n"),
    )
    .expect("receipt");
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).expect("mv");
    write_file(&fx.home.join("Documents/vendor/marker"), "mine");
    symlink(link.join("Documents"), fx.share("watane-dev")).expect("link");
    let out = fx.run_env(&["--uninstall"], &env);
    assert_eq!(out.code, 0, "{out:?}");
    let real = fs::canonicalize(&fx.home).expect("real");
    assert!(
        out.stderr
            .lines()
            .any(|l| l
                == "watane install: skipped ~/.local/share/fish: not a path that watane installs"),
        "{out:?}"
    );
    assert!(
        out.stderr.lines().any(|l| l
            == "watane install: skipped ~/.local/share/watane-dev/vendor: ~/.local/share/watane-dev resolves to ~/Documents, not to ~/.local/share/watane-dev as at install"),
        "{out:?}"
    );
    assert!(!out.stderr.contains(&real.display().to_string()), "{out:?}");
    assert!(!out.stderr.contains(&h), "{out:?}");
    assert!(out.has_line(
        "partly uninstalled watane 0.1.0 (some entries were skipped or could not be removed)"
    ));
    assert!(!out.has_line("uninstalled watane 0.1.0"));
}

/// TECHNICAL 3.6 step 4, 3.8 (IN-47): an INT or TERM in the swap of a fresh install leaves no folder the run made.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn ac6_an_interrupted_fresh_install_leaves_no_folder_it_made() {
    for sig in ["INT", "TERM"] {
        let fx = Fx::new();
        fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
        let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
        fx.wait_state("mv_sleeping");
        signal_group(child.id(), sig);
        let out = finish(child);
        // 3.8: macOS sh can lose an INT that lands just after the `mv` it waited on died of it
        let lost = sig == "INT"
            && out.code == 5
            && out.stderr.contains("could not put the files in place");
        let want = if sig == "INT" { 130 } else { 143 };
        assert!(out.code == want || lost, "{sig}: {out:?}");
        assert_eq!(fx.home_tree(), Vec::<String>::new(), "{sig}: {out:?}");
        assert!(!out.stderr.contains("is installed"), "{out:?}");
    }
}

/// TECHNICAL 8.4: an upgrade and uninstall preserve the general-purpose folders.
#[test]
fn ac2_uninstall_keeps_general_purpose_folders_after_upgrade() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    assert_eq!(fx.run(&["--release", "0.2.0", "--no-toolchain"]).code, 0);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("uninstalled watane 0.2.0"), "{out:?}");
    assert_eq!(
        fx.home_tree(),
        vec![".local/", ".local/bin/", ".local/share/", "Applications/"]
    );
}

/// TECHNICAL 8.4: a creation record does not authorize removing a relinked root.
#[test]
fn ac2_uninstall_keeps_a_relinked_created_folder() {
    let fx = Fx::new();
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 0, "{out:?}");
    add_legacy_created_lines(&fx);
    let app_folder = fx.home.join("Applications");
    let original = fx.home.join("old-applications");
    fs::remove_dir_all(fx.app()).expect("empty the original folder");
    fs::rename(&app_folder, &original).expect("move the original aside");
    symlink(&original, &app_folder).expect("link to the original directory");

    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fs::read_link(app_folder).expect("kept link"), original);
    assert!(original.is_dir(), "{out:?}");
}

/// TECHNICAL 8.1: a reinstall must drop creation records for relinked folders.
#[test]
fn ac2_reinstall_drops_creation_records_for_relinked_folders() {
    let fx = Fx::installed();
    add_legacy_created_lines(&fx);
    let before = fx.read(&fx.receipt());

    // Relink a leaf and a parent. Only the former is itself a symlink.
    fs::remove_dir_all(fx.app()).expect("empty applications");
    let apps = fx.home.join("other-apps");
    fs::rename(fx.home.join("Applications"), &apps).expect("move applications");
    symlink(&apps, fx.home.join("Applications")).expect("relink applications");

    // Leave the recorded inode in place under a different physical path.
    for entry in ["watane", "watane-dev", "watane-node"] {
        fs::remove_file(fx.bin(entry)).expect("empty bin");
    }
    for entry in ["watane", "watane-dev"] {
        fs::remove_dir_all(fx.share(entry)).expect("empty share");
    }
    let local = fx.home.join("other-local");
    // Retain the old receipt so the reinstall can try to carry its ownership records.
    write_file(&fx.receipt(), &before);
    fs::rename(fx.home.join(".local"), &local).expect("move local");
    symlink(&local, fx.home.join(".local")).expect("relink local");

    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(
        !fx.read(&fx.receipt())
            .lines()
            .any(|line| line.starts_with("created "))
    );
}

/// TECHNICAL 8.4: uninstall leaves empty user folders and a replacement at the same path.
#[test]
fn ac2_uninstall_keeps_preexisting_and_replaced_folders() {
    let fx = Fx::new();
    fs::create_dir_all(fx.home.join(".local/bin")).expect("pre-existing bin");
    fs::create_dir_all(fx.home.join(".local/share")).expect("pre-existing share");
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let app_folder = fx.home.join("Applications");
    let original = fx.home.join("old-applications");
    fs::rename(&app_folder, &original).expect("move the original aside");
    fs::create_dir(&app_folder).expect("create an empty replacement");
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.home.join(".local/bin").is_dir());
    assert!(fx.home.join(".local/share").is_dir());
    assert!(app_folder.is_dir());
    assert!(original.join("watane.app").is_dir());

    let fx = Fx::new();
    fs::create_dir(fx.home.join("Applications")).expect("pre-existing Applications");
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        fx.home_tree(),
        vec![".local/", ".local/bin/", ".local/share/", "Applications/"]
    );
}

/// TECHNICAL 3.6 step 4 (IN-47): a failed swap of a fresh install leaves no folder the run made.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn ac6_a_failed_fresh_install_leaves_no_folder_it_made() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(fx.home_tree(), Vec::<String>::new(), "{out:?}");
}

/// TECHNICAL 3.6 step 4 (IN-47): a folder that existed before a failed install is never removed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn ac6_a_folder_that_existed_is_never_removed() {
    let fx = Fx::new();
    fs::create_dir_all(fx.home.join(".local/share")).expect("mkdir");
    fs::create_dir_all(fx.home.join("Applications")).expect("mkdir");
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        fx.home_tree(),
        vec![".local/", ".local/share/", "Applications/"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>(),
        "{out:?}"
    );
}

/// TECHNICAL 3.7 (IN-44): rustup's own output is set apart from the script's lines.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac7_rustups_output_is_set_apart() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    let body = fs::read_to_string(path).expect("installer");
    fs::write(
        path,
        body.replacen("#!/bin/sh\n", "#!/bin/sh\necho 'info: from rustup'\n", 1)
            + "echo 'warn: also rustup' >&2\n",
    )
    .expect("installer");
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 0, "{out:?}");
    let all = lines(&out.stdout);
    let at = |l: &str| {
        all.iter()
            .position(|x| *x == l)
            .unwrap_or_else(|| panic!("{l:?} missing: {out:?}"))
    };
    let begin = at("--- rustup's own output follows ---");
    let end = at("--- end of rustup's output ---");
    assert!(begin < at("info: from rustup") && at("info: from rustup") < end);
    assert_eq!(at("warn: also rustup") + 1, end, "{out:?}");
    assert!(end < at("toolchain: installed"), "{out:?}");
    assert!(at(&format!("using WATANE_RUSTUP_URL={url}")) < begin);
}

/// TECHNICAL 3.7, 3.9 (IN-44): a failing rustup ends the run with exit 6 after the closing line of its output.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac7_a_failing_rustup_still_decides_exit_6() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    fs::write(path, "#!/bin/sh\nprintf oops >&2\nexit 3\n").expect("installer");
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 6, "{out:?}");
    let all = lines(&out.stdout);
    let end = all
        .iter()
        .position(|l| *l == "--- end of rustup's output ---")
        .unwrap_or_else(|| panic!("no end line: {out:?}"));
    assert_eq!(all[end - 1], "oops");
    assert!(
        all[end + 1]
            == "watane install: the toolchain step failed: install Rust with rustup (rustup exited 3)",
        "{out:?}"
    );
    assert!(
        all.iter()
            .any(|l| l.contains("~/.local/bin/watane-dev doctor")),
        "{out:?}"
    );
}

/// TECHNICAL 3.7: an existing rustup needs no bootstrap URL and names its failed command.
#[test]
#[ignore = "slow: needs a pseudo-terminal to accept the existing rustup offer"]
fn ac4_existing_rustup_names_the_failing_default_step() {
    let fx = Fx::new();
    fake_tool(
        &fx,
        "rustup",
        r#"case "$1" in
  which) exit 1 ;;
  toolchain) echo 'stable toolchain installed' ;;
  default) exit 4 ;;
esac"#,
    );
    let out = fx.run_pty(&[], &[], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 6, "{out:?}");
    assert!(
        out.stdout
            .contains("Install it with rustup (a few hundred MB, into"),
        "{out:?}"
    );
    assert!(!out.stdout.contains("sh.rustup.rs"), "{out:?}");
    assert!(out.has_line("stable toolchain installed"), "{out:?}");
    assert!(
        out.has_line(
            "watane install: the toolchain step failed: rustup default stable (rustup exited 4)"
        ),
        "{out:?}"
    );
    assert!(out.has_line("  rustup default stable"), "{out:?}");
    fx.assert_installed("0.1.0");
}

/// TECHNICAL 3.8, 3.7: a rustup download that fails or is interrupted leaves the mirror retry command.
#[test]
#[ignore = "slow: needs a pseudo-terminal to consent and to interrupt the running download"]
fn ac4_the_rustup_download_names_its_step_and_mirror_on_retry() {
    for (outcome, code) in [("fail", 3), ("wait", 130)] {
        let fx = Fx::new();
        symlink(which("curl"), fx.real.join("curl")).expect("link real curl");
        let url = "https://example.invalid/rustup-init.sh";
        fake_tool(
            &fx,
            "curl",
            &format!(
                r#"case "$*" in
  *example.invalid*)
    if [ {outcome} = fail ]; then
      printf '%s\n' 'could not fetch the installer' >&2
      exit 22
    fi
    printf '%s\n' 'waiting for the rustup installer' >/dev/tty
    exec "$FAKE_REAL/sleep" 600
    ;;
esac
exec "$FAKE_REAL/curl" "$@""#
            ),
        );
        let steps = if outcome == "wait" {
            vec![
                ("[y/N] ", "y"),
                ("waiting for the rustup installer", "\x03"),
            ]
        } else {
            vec![("[y/N] ", "y")]
        };
        let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", url)], &steps);
        assert_eq!(out.code, code, "{out:?}");
        assert!(out.stdout.contains("watane 0.1.0 is installed"), "{out:?}");
        assert!(
            out.stdout
                .contains(&format!("  curl --tlsv1.2 -sSf {url} | sh")),
            "{out:?}"
        );
        if outcome == "fail" {
            assert!(out.stdout.contains("cannot download the rustup installer from https://example.invalid/rustup-init.sh"), "{out:?}");
            assert!(out.has_line("could not fetch the installer"), "{out:?}");
        }
        fx.assert_installed("0.1.0");
    }
}

/// TECHNICAL 8.4: Ctrl-C at the uninstall question leaves the data and confirms removal on a fresh line.
#[test]
#[ignore = "slow: needs a pseudo-terminal to interrupt the uninstall question"]
fn ac1_int_at_the_uninstall_question_says_watane_was_uninstalled() {
    let fx = Fx::installed();
    write_file(&fx.home.join(".watane/identity"), "kept identity");
    let out = fx.run_pty(&["--uninstall"], &[], &[("keep it: ", "\x03")]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(
        out.has_line("watane 0.1.0 was uninstalled; the watane home was kept."),
        "{out:?}"
    );
    assert!(!fx.bin("watane").exists());
    assert_eq!(fx.read(&fx.home.join(".watane/identity")), "kept identity");
}

/// TECHNICAL 3.8, 3.7 (IN-45): an INT after the typed yes prints the hint and no install-it-later lines.
#[test]
#[ignore = "slow: needs a pseudo-terminal from script(1)"]
fn ac2_int_after_the_answer_prints_no_install_it_later_lines() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    fs::write(
        path,
        "#!/bin/sh\nprintf 'info: rustup waits'\nexec \"$FAKE_REAL/sleep\" 600\n",
    )
    .expect("installer");
    let out = fx.run_pty(
        &[],
        &[("WATANE_RUSTUP_URL", &url)],
        &[("[y/N] ", "y"), ("info: rustup waits", "\x03")],
    );
    assert_eq!(out.code, 130, "{out:?}");
    let all = lines(&out.stdout);
    let at = all
        .iter()
        .position(|l| *l == HINT[0])
        .unwrap_or_else(|| panic!("no hint: {out:?}"));
    assert_eq!(all[at + 1], HINT[1], "{out:?}");
    // the question was answered: the hint is not followed by the commands of 3.7
    assert!(!all[at..].contains(&"To install it later, run:"), "{out:?}");
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "needs /dev/full, which only Linux has"
)]
fn fix1_say_to_a_full_disk_ends_the_run_with_exit_1() {
    let fx = Fx::installed();
    let mut c = fx.cmd(&["--status"]);
    c.stdin(Stdio::null())
        .stdout(
            fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .expect("full"),
        )
        .stderr(Stdio::piped());
    let out = finish(spawn_run(&mut c));
    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(out.stderr, "", "{out:?}");
}

/// TECHNICAL 3.4.1, 3.6 step 4 (IN-47): a link swapped in for a folder the run made is not followed out of the home by the cleanup.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn fix1_a_link_swapped_in_after_note_created_is_not_followed_out_of_the_home() {
    let fx = Fx::new();
    let outside = fx.root.path().join("outside");
    fs::create_dir_all(outside.join("share/watane-dev")).expect("outside");
    symlink(which("rmdir"), fx.real.join("rmdir")).expect("link real rmdir");
    // the first rmdir of the cleanup (the lock's) swaps the run-created ~/.local for a link
    fake_tool(
        &fx,
        "rmdir",
        &format!(
            "if [ ! -e \"$FAKE_STATE/swapped\" ]; then echo 1 > \"$FAKE_STATE/swapped\"; \"$FAKE_REAL/mv\" \"$HOME/.local\" \"$HOME/.moved\" && '{}' -s '{}' \"$HOME/.local\"; fi\nexec \"$FAKE_REAL/rmdir\" \"$@\"",
            which("ln").display(),
            outside.display()
        ),
    );
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!fx.state_read("swapped").is_empty(), "{out:?}");
    assert!(outside.join("share/watane-dev").is_dir(), "{out:?}");
}

/// TECHNICAL 3.4.1, 3.6 step 4 (IN-47): a folder the run made, swapped for a link into the home, is kept.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn fix1_a_created_folder_swapped_for_a_link_into_the_home_is_kept() {
    let fx = Fx::new();
    let keep = fx.home.join("keep");
    fs::create_dir_all(&keep).expect("keep");
    symlink(which("rmdir"), fx.real.join("rmdir")).expect("link real rmdir");
    // the first rmdir of the cleanup swaps the run-created watane-dev for a link to ~/keep
    fake_tool(
        &fx,
        "rmdir",
        &format!(
            "if [ ! -e \"$FAKE_STATE/swapped\" ]; then echo 1 > \"$FAKE_STATE/swapped\"; \"$FAKE_REAL/mv\" \"$HOME/.local/share/watane-dev\" \"$HOME/.moved\" && '{}' -s \"$HOME/keep\" \"$HOME/.local/share/watane-dev\"; fi\nexec \"$FAKE_REAL/rmdir\" \"$@\"",
            which("ln").display(),
        ),
    );
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!fx.state_read("swapped").is_empty(), "{out:?}");
    assert!(keep.is_dir(), "a folder that existed was removed: {out:?}");
}

/// TECHNICAL 3.8 (IN-45): a signal while staging prints no hint, because nothing is installed yet.
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn fix1_a_signal_while_staging_prints_no_hint() {
    let fx = Fx::new();
    // staging copies run before the receipt's `.new.<pid>` and the journal exist
    fake_tool(
        &fx,
        "cp",
        "for a; do case $a in *.new.*) echo up > \"$FAKE_STATE/cp_sleeping\"; exec \"$FAKE_REAL/sleep\" 600 ;; esac; done\nexec \"$FAKE_REAL/cp\" \"$@\"",
    );
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("cp_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stderr.contains("is installed"), "{out:?}");
    assert!(!fx.receipt().exists(), "{out:?}");
}

const CP_FAILS_ON_NEW: &str =
    "for a; do case $a in *.new.*) exit 1 ;; esac; done\nexec \"$FAKE_REAL/cp\" \"$@\"";

/// TECHNICAL 3.9 (IN-46): a reinstall that fails while staging says nothing was changed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix2_a_reinstall_failing_while_staging_says_nothing_was_changed() {
    let fx = Fx::installed();
    fake_tool(&fx, "cp", CP_FAILS_ON_NEW);
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        lines(&out.stderr),
        [
            "watane install: could not put the files in place",
            "Nothing was changed."
        ],
        "{out:?}"
    );
    fx.assert_installed("0.1.0");
}

/// TECHNICAL 3.9 (IN-46): a fresh install that fails while staging says nothing was installed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix2_a_fresh_install_failing_while_staging_says_nothing_was_installed() {
    let fx = Fx::new();
    fake_tool(&fx, "cp", CP_FAILS_ON_NEW);
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        lines(&out.stderr),
        [
            "watane install: could not put the files in place",
            "Nothing was installed."
        ],
        "{out:?}"
    );
    assert!(!fx.receipt().exists(), "{out:?}");
}

/// TECHNICAL 3.9 (IN-46): a receipt move that fails after the rename says watane is installed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix2_a_receipt_move_failing_after_the_rename_says_watane_is_installed() {
    let fx = Fx::new();
    fake_tool(
        &fx,
        "mv",
        "for a; do case $a in *receipt.new.*) \"$FAKE_REAL/mv\" \"$@\"; exit 1 ;; esac; done\nexec \"$FAKE_REAL/mv\" \"$@\"",
    );
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        lines(&out.stderr),
        [
            "watane install: a step after the install failed",
            "watane 0.1.0 is installed.",
            HINT[1]
        ],
        "{out:?}"
    );
    fx.assert_installed("0.1.0");
}

const MV_FAILS_ON_COMMIT_AND_UNDO: &str = "for a; do case $a in *receipt.new.*) exit 1 ;; esac; done\nfor last; do :; done\ncase $1 in *.new.*) ;; *) case $last in *.new.[0-9]*) exit 1 ;; esac ;; esac\nexec \"$FAKE_REAL/mv\" \"$@\"";

/// TECHNICAL 3.9 (IN-46): a reinstall whose undo cannot remove a new entry does not say nothing changed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix3_ac2_a_reinstall_whose_undo_cannot_remove_a_new_entry_does_not_say_nothing_changed() {
    let fx = Fx::installed();
    // the first install's receipt lacks the program entry: it is new to the reinstall
    let r = fx.receipt();
    let kept: Vec<String> = fs::read_to_string(&r)
        .expect("receipt")
        .lines()
        .filter(|l| !l.ends_with(".local/bin/watane"))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&r, kept.concat()).expect("receipt");
    fs::remove_file(fx.home.join(".local/bin/watane")).expect("remove program");
    fake_tool(&fx, "mv", MV_FAILS_ON_COMMIT_AND_UNDO);
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(
        fx.home.join(".local/bin/watane").exists(),
        "setup: undo was meant to leave the new program: {out:?}"
    );
    assert!(!out.stderr.contains("Nothing was"), "{out:?}");
    assert!(
        out.stderr
            .contains("The files could not be put back. Run this script again to restore the installed version."),
        "{out:?}"
    );
}

/// TECHNICAL 3.9 (IN-46): a fresh install whose undo fails says the files could not be removed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix3_a_fresh_install_whose_undo_fails_says_the_files_could_not_be_removed() {
    let fx = Fx::new();
    fake_tool(&fx, "mv", MV_FAILS_ON_COMMIT_AND_UNDO);
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        lines(&out.stderr),
        [
            "watane install: could not put the files in place",
            "The files could not be removed. Run this script again: it removes them and installs watane."
        ],
        "{out:?}"
    );
}

/// TECHNICAL 3.9 (IN-46): a checksum failure on a reinstall does not say nothing was installed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix3_a_checksum_failure_on_a_reinstall_does_not_say_nothing_was_installed() {
    let fx = Fx::installed();
    fx.rel
        .put(&archive_name("0.1.0"), b"not the archive".to_vec());
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 4, "{out:?}");
    assert_eq!(
        out.stderr.lines().last(),
        Some("Nothing was changed."),
        "{out:?}"
    );
    fx.assert_installed("0.1.0");
}

/// TECHNICAL 3.9 (IN-46): an archive failure on a reinstall does not say nothing was installed.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn fix3_an_archive_failure_on_a_reinstall_does_not_say_nothing_was_installed() {
    let fx = Fx::installed();
    // a well-formed archive whose sum matches but whose VERSION is another release's
    let bytes = build_archive_with("0.1.0", &["serde-1.0.0"], &|t| {
        fs::write(t.join("VERSION"), "0.2.0\n").expect("version");
    });
    let name = archive_name("0.1.0");
    fx.rel.put(
        "SHA256SUMS",
        format!("{}  {name}\n", sha256(&bytes)).into_bytes(),
    );
    fx.rel.put(&name, bytes);
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        lines(&out.stderr),
        [
            format!("watane install: {name} is not a watane archive of version 0.1.0").as_str(),
            "Nothing was changed."
        ],
        "{out:?}"
    );
    fx.assert_installed("0.1.0");
}

/// Write the optional records emitted by previous installers to exercise old receipts.
pub(crate) fn add_legacy_created_lines(fx: &Fx) {
    use std::os::unix::fs::MetadataExt;
    let mut receipt = fx.read(&fx.receipt());
    for folder in [".local", ".local/bin", ".local/share", "Applications"] {
        let inode = fs::metadata(fx.home.join(folder))
            .expect("directory inode")
            .ino();
        receipt.push_str(&format!("created {folder} {inode}\n"));
    }
    write_file(&fx.receipt(), &receipt);
}
