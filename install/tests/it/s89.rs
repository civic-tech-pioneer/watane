//! S89: install.sh tells the truth at each interruption, pipe, path and rustup line
//! (install/TECHNICAL 3.6 to 3.9, 8.1, 8.5; IN-44 to IN-48).

use crate::support::*;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::Stdio;

const HINT: [&str; 2] = [
    "watane 0.1.0 is installed; the run was interrupted before it finished.",
    "Run watane-dev doctor to check the maker tools.",
];

fn fake_tool(fx: &Fx, name: &str, body: &str) {
    let p = fx.fakes.join(name);
    fs::write(&p, format!("#!/bin/sh\n{body}\n")).expect("fake tool");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

#[test]
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
    assert_eq!(lines(&out.stderr), HINT, "{out:?}");
    fx.assert_installed("0.1.0");
    assert!(fx.tmp_tree().is_empty());
}

#[test]
fn ac2_int_at_the_toolchain_question_says_watane_is_installed() {
    let fx = Fx::new();
    let out = fx.run_pty(&[], &[], &[("[y/N] ", "\x03")]);
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
        "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2",
        "{out:?}"
    );
    assert!(
        !out.stdout.contains("is installed.\n"),
        "no success block: {out:?}"
    );
    fx.assert_installed("0.1.0");
}

#[test]
fn ac2_a_signal_before_the_commit_prints_no_hint() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "sleep\n*watane.app.new.*\n");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(!out.stderr.contains("is installed"), "{out:?}");
}

#[test]
fn ac3_status_to_a_closed_reader_is_quiet() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
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

#[test]
fn ac3_status_prints_its_lines_when_read() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 0);
    assert_eq!(out.out_lines()[0], "watane 0.1.0 (aarch64-apple-darwin)");
    assert_eq!(out.stderr, "");
}

#[test]
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
}

#[test]
fn ac4_a_path_outside_the_home_stays_absolute() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    // the install recorded the folder somewhere outside the home
    let real = fs::canonicalize(&fx.home).expect("real");
    let was = format!(
        "resolved .local/share/watane-dev {}/.local/share/watane-dev",
        real.display()
    );
    let receipt = fx.read(&fx.receipt());
    assert!(receipt.contains(&was), "{receipt}");
    fs::write(
        fx.receipt(),
        receipt.replace(&was, "resolved .local/share/watane-dev /Volumes/Off/dev"),
    )
    .expect("receipt");
    let out = fx.run(&["--uninstall"]);
    assert!(
        out.stderr.lines().any(|l| l
            == "watane install: skipped ~/.local/share/watane-dev/vendor: ~/.local/share/watane-dev resolves to ~/.local/share/watane-dev, not to /Volumes/Off/dev as at install"),
        "{out:?}"
    );
}

#[test]
fn ac5_install_it_later_names_the_url_of_the_offer() {
    let fx = Fx::new();
    let url = "https://example.invalid/rustup-init.sh";
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", url)], &[("[y/N] ", "N")]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.stdout.contains(&format!("from {url},")), "{out:?}");
    let l = format!(
        "  curl --proto '=https' --tlsv1.2 -sSf {url} | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2"
    );
    assert!(out.stdout.lines().any(|x| x == l), "{out:?}");
    assert!(!out.stdout.contains("sh.rustup.rs"), "{out:?}");
    // unset: unchanged
    let fx = Fx::new();
    let out = fx.run_pty(&[], &[], &[("[y/N] ", "N")]);
    assert!(out.stdout.lines().any(|x| x == "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2"), "{out:?}");
}

#[test]
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

#[test]
fn ac6_a_failed_fresh_install_leaves_no_folder_it_made() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "fail\n*receipt.new.*\n");
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(fx.home_tree(), Vec::<String>::new(), "{out:?}");
}

#[test]
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

#[test]
fn ac7_rustups_output_is_set_apart() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    let body = fs::read_to_string(path).expect("installer");
    fs::write(
        path,
        body.replacen(
            "#!/bin/sh\n",
            "#!/bin/sh\necho 'info: from rustup'\necho 'warn: also rustup' >&2\n",
            1,
        ),
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
    assert!(begin < at("warn: also rustup") && at("warn: also rustup") < end);
    assert!(end < at("toolchain: installed"), "{out:?}");
    assert!(at(&format!("using WATANE_RUSTUP_URL={url}")) < begin);
}

#[test]
fn ac7_a_failing_rustup_still_decides_exit_6() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    fs::write(path, "#!/bin/sh\necho oops\nexit 3\n").expect("installer");
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", &url)], &[("[y/N] ", "y")]);
    assert_eq!(out.code, 6, "{out:?}");
    let all = lines(&out.stdout);
    let end = all
        .iter()
        .position(|l| *l == "--- end of rustup's output ---")
        .unwrap_or_else(|| panic!("no end line: {out:?}"));
    assert_eq!(all[end - 1], "oops");
    assert!(
        all[end + 1].starts_with("watane install: the toolchain step failed (rustup exited 3)"),
        "{out:?}"
    );
}

#[test]
fn ac2_int_after_the_answer_prints_no_install_it_later_lines() {
    let fx = Fx::new();
    let url = fx.rustup_installer();
    let path = url.trim_start_matches("file://");
    fs::write(
        path,
        "#!/bin/sh\necho 'info: rustup waits'\nexec \"$FAKE_REAL/sleep\" 600\n",
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
fn fix1_say_to_a_full_disk_ends_the_run_with_exit_1() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
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

#[test]
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

#[test]
fn fix1_install_it_later_quotes_an_odd_url() {
    let fx = Fx::new();
    let url = "http://x/a b;c";
    let out = fx.run_pty(&[], &[("WATANE_RUSTUP_URL", url)], &[("[y/N] ", "N")]);
    assert_eq!(out.code, 0, "{out:?}");
    let l = "  curl --proto '=https' --tlsv1.2 -sSf 'http://x/a b;c' | sh -s -- -y --profile minimal --no-modify-path -t wasm32-wasip2";
    assert!(out.stdout.lines().any(|x| x == l), "{out:?}");
}

#[test]
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

#[test]
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
