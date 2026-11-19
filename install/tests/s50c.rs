//! S50c receipt, lock and recovery regressions (TECHNICAL 3.6, 8.5).
mod support;
use std::fs;
use std::os::unix::fs::symlink;
use support::*;

#[test]
fn ac1_resolved_folders_guard_receipt_removals() {
    for recorded in [true, false] {
        let fx = Fx::new();
        assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
        let receipt = fx.read(&fx.receipt());
        for folder in [
            ".local/bin",
            ".local/share/watane",
            ".local/share/watane-dev",
            "Applications",
        ] {
            let physical = fs::canonicalize(fx.home.join(folder)).unwrap();
            assert!(
                receipt.contains(&format!("resolved {folder} {}\n", physical.display())),
                "{receipt}"
            );
        }
        let lines: Vec<&str> = receipt.lines().collect();
        assert_eq!(lines.len(), 14, "{receipt}");
        assert!(
            lines[3..10].iter().all(|l| l.starts_with("path ")),
            "{receipt}"
        );
        assert!(
            lines[10..].iter().all(|l| l.starts_with("resolved ")),
            "{receipt}"
        );
        if recorded {
            fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
            write_file(&fx.home.join("Documents/vendor/marker"), "mine");
            symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
        } else {
            fs::write(
                fx.receipt(),
                receipt
                    .lines()
                    .filter(|l| !l.starts_with("resolved "))
                    .map(|l| format!("{l}\n"))
                    .collect::<String>(),
            )
            .unwrap();
        }
        let before = snapshot(&fx.home);
        let out = fx.run(&["--uninstall"]);
        assert_eq!(out.code, 0, "{out:?}");
        if recorded {
            assert!(fx.home.join("Documents/vendor/marker").exists());
            for l in lines.iter().filter(|l| l.starts_with("path ")) {
                let p = std::path::Path::new(&l["path ".len()..]);
                if !l.ends_with("/watane-dev/vendor") {
                    assert!(fs::symlink_metadata(p).is_err(), "{l} left: {out:?}");
                }
            }
            assert!(!fx.receipt().exists(), "{out:?}");
            assert!(out.stderr.lines().any(|l| l == format!("watane install: skipped {}: ~/.local/share/watane-dev resolves to {}, not to {} as at install", fx.share("watane-dev/vendor").display(), fs::canonicalize(fx.home.join("Documents")).unwrap().display(), fs::canonicalize(&fx.home).unwrap().join(".local/share/watane-dev").display())));
        } else {
            assert_eq!(snapshot(&fx.home), before);
            assert!(fx.bin("watane").exists());
            assert!(fx.receipt().exists());
            assert_eq!(
                out.stderr
                    .matches("not to an unrecorded path as at install")
                    .count(),
                8
            );
        }
    }
}

#[test]
fn ac2_lock_excludes_runs_and_preserves_planted_locks() {
    for pid in [Some("99999999\n"), None, Some("12\n34\n")] {
        let fx = Fx::new();
        let lock = fx.home.join(".watane-install.lock");
        fs::create_dir(&lock).unwrap();
        if let Some(pid) = pid {
            fs::write(lock.join("pid"), pid).unwrap();
        }
        for args in [&["--no-toolchain"][..], &["--uninstall"][..]] {
            let before = snapshot(&fx.home);
            let out = fx.run(args);
            assert_eq!(out.code, 5, "{out:?}");
            let suffix = if pid == Some("99999999\n") {
                " (pid 99999999)"
            } else {
                ""
            };
            assert_eq!(
                out.stderr,
                format!(
                    "watane install: another install may be running{suffix}\nIf no install is running, remove its lock and run this again:\n  rm -rf '{}'\n",
                    lock.display()
                )
            );
            assert_eq!(snapshot(&fx.home), before);
            assert!(fx.rel.requests().is_empty());
        }
        assert_eq!(fx.run(&["--status"]).code, 7);
    }
}

#[test]
fn ac2_concurrent_install_and_uninstall_stop_before_download() {
    let fx = Fx::new();
    fx.state_write("mv_fault", "wait\n*watane.new.*\n");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("mv_sleeping");
    let requests = fx.rel.requests();
    let before = snapshot(&fx.home);
    for args in [&["--no-toolchain"][..], &["--uninstall"][..]] {
        let out = fx.run(args);
        assert_eq!(out.code, 5, "{out:?}");
        assert_eq!(
            out.first_err(),
            format!(
                "watane install: another install may be running (pid {})",
                child.id()
            )
        );
        assert_eq!(fx.rel.requests(), requests);
        assert_eq!(snapshot(&fx.home), before);
    }
    fs::remove_file(fx.state.join("mv_fault")).unwrap();
    assert_eq!(finish(child).code, 0);
    assert!(!fx.home.join(".watane-install.lock").exists());
}

#[test]
fn ac3_partial_receipt_copy_keeps_complete_old_receipt() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let before = fs::read(fx.receipt()).unwrap();
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    fx.state_write("cp_fault", "half\n*receipt.old.*\n");
    let out = fx.run(&["--release", "0.2.0", "--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(fs::read(fx.receipt()).unwrap(), before);
    fx.assert_installed("0.1.0");
}

#[test]
fn ac3_disallowed_receipt_preserves_staging_evidence() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    write_file(&fx.share("watane/journal.99999999"), "journal evidence");
    write_file(&fx.share("watane/receipt.new.99999999"), "receipt evidence");
    write_file(&fx.bin("watane.new.99999999"), "staging evidence");
    let other = tempfile::tempdir().unwrap();
    fs::rename(fx.share("watane"), other.path().join("watane")).unwrap();
    symlink(other.path().join("watane"), fx.share("watane")).unwrap();
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(fx.read(&fx.bin("watane.new.99999999")), "staging evidence");
}

#[test]
fn ac1_upgrade_cleanup_checks_old_receipt_resolution() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let receipt = fx.read(&fx.receipt());
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
    write_file(&fx.home.join("Documents/vendor/marker"), "mine");
    symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
    // Prebuilt committed swap: the next version no longer installs vendor.
    fs::write(fx.share("watane/receipt.old.99999999"), &receipt).unwrap();
    fs::write(
        fx.receipt(),
        receipt
            .lines()
            .filter(|l| !l.ends_with("/watane-dev/vendor"))
            .map(|l| format!("{l}\n"))
            .collect::<String>()
            .replace("version 0.1.0", "version 0.2.0")
            .replace(
                &format!(
                    "resolved .local/share/watane-dev {}",
                    fs::canonicalize(&fx.home)
                        .unwrap()
                        .join(".local/share/watane-dev")
                        .display()
                ),
                &format!(
                    "resolved .local/share/watane-dev {}",
                    fs::canonicalize(fx.home.join("Documents"))
                        .unwrap()
                        .display()
                ),
            ),
    )
    .unwrap();
    fs::write(
        &fx.script,
        baked_script("0.1.0").replace(
            "main \"$@\"",
            "check_home; set_entries; finish_commit 99999999",
        ),
    )
    .unwrap();
    let out = fx.run(&[]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(fx.home.join("Documents/vendor/marker").exists());
    assert!(
        out.stderr.contains("~/.local/share/watane-dev resolves to"),
        "{out:?}"
    );
}

#[test]
fn ac2_lock_is_removed_after_platform_checksum_and_download_interrupt() {
    use std::os::unix::fs::PermissionsExt;
    for (tool, body, code) in [
        ("uname", "echo Linux", 2),
        ("shasum", "echo bad", 4),
        (
            "curl",
            "kill -s INT -- -\"$FAKE_INSTALL_PID\"; exit 130",
            130,
        ),
    ] {
        let fx = Fx::new();
        fs::write(fx.fakes.join(tool), format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(fx.fakes.join(tool), fs::Permissions::from_mode(0o755)).unwrap();
        let out = fx.run(&["--no-toolchain"]);
        assert_eq!(out.code, code, "{tool}: {out:?}");
        assert!(!fx.home.join(".watane-install.lock").exists());
    }
}

#[test]
fn ac5_signalled_lock_creation_keeps_signal_status() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new();
    fs::write(fx.fakes.join("mkdir"), "#!/bin/sh\nexit 130\n").unwrap();
    fs::set_permissions(fx.fakes.join("mkdir"), fs::Permissions::from_mode(0o755)).unwrap();
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 130, "{out:?}");
    assert!(!out.stderr.contains("watane install:"));
    assert!(fx.rel.requests().is_empty());
}

#[test]
fn ac5_status_reads_receipt_while_lock_is_held() {
    let fx = Fx::new();
    write_file(
        &fx.receipt(),
        "watane-receipt 1\nversion 0.1.0\ntarget aarch64-apple-darwin\n",
    );
    fs::create_dir(fx.home.join(".watane-install.lock")).unwrap();
    let before = snapshot(&fx.home);
    let out = fx.run(&["--status"]);
    assert_eq!(out.code, 0, "{out:?}");
    assert!(out.has_line("watane 0.1.0 (aarch64-apple-darwin)"));
    assert_eq!(snapshot(&fx.home), before);
}

/// 3.6.2: an uninstall at a planted lock removes nothing of an installed home.
#[test]
fn ac2_uninstall_at_a_planted_lock_removes_nothing() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fs::create_dir(fx.home.join(".watane-install.lock")).unwrap();
    let before = snapshot(&fx.home);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(snapshot(&fx.home), before);
}

/// 3.6.2: a lock whose `pid` cannot be written stops the run (exit 5) and is removed.
#[test]
fn ac2_unwritable_lock_pid_exits_5_and_removes_the_lock() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new();
    fs::write(
        fx.fakes.join("mkdir"),
        "#!/bin/sh\n\"$FAKE_REAL/mkdir\" \"$@\" || exit\ncase $1 in *.watane-install.lock) \"$FAKE_REAL/chmod\" 500 \"$1\" ;; esac\n",
    )
    .unwrap();
    fs::set_permissions(fx.fakes.join("mkdir"), fs::Permissions::from_mode(0o755)).unwrap();
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert!(
        out.stderr
            .lines()
            .any(|l| l == "watane install: could not write install lock"),
        "{out:?}"
    );
    assert!(!fx.home.join(".watane-install.lock").exists());
    assert!(fx.rel.requests().is_empty());
}

/// 3.6.2: the lock goes even when a signal's undo fails (the journal stays for the next run).
#[test]
fn ac2_lock_is_removed_after_a_failed_undo_under_a_signal() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    fx.state_write("mv_fault", "sleep\n*receipt.new.*\n");
    fx.state_write("rm_fault", "fail\n*/Applications/watane.app\n");
    let child = spawn(&mut fx.cmd(&["--release", "0.2.0", "--no-toolchain"]));
    fx.wait_state("mv_sleeping");
    signal_group(child.id(), "TERM");
    let out = finish(child);
    assert_eq!(out.code, 143, "{out:?}");
    assert!(out.stderr.contains("fake rm: failing on"), "{out:?}");
    assert!(!fx.home.join(".watane-install.lock").exists());
    assert!(fx.tmp_tree().is_empty());
    assert!(fx.home_tree().iter().any(|p| p.contains("journal.")));
}

#[test]
fn ac1_upgrade_refuses_remapped_or_unrecorded_receipt_paths_before_download() {
    for recorded in [true, false] {
        let fx = Fx::new();
        assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
        if recorded {
            fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
            write_file(&fx.home.join("Documents/vendor/marker"), "mine");
            symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
        } else {
            let receipt = fx.read(&fx.receipt());
            fs::write(
                fx.receipt(),
                receipt
                    .lines()
                    .filter(|l| !l.starts_with("resolved "))
                    .map(|l| format!("{l}\n"))
                    .collect::<String>(),
            )
            .unwrap();
        }
        fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
        let before = snapshot(&fx.home);
        let requests = fx.rel.requests();
        let out = fx.run(&["--release", "0.2.0", "--no-toolchain"]);
        assert_eq!(out.code, 8, "{out:?}");
        assert_eq!(
            out.first_err(),
            "watane install: found files that watane did not install:"
        );
        assert!(
            out.stderr
                .lines()
                .any(|l| l == "  ~/.local/share/watane-dev/vendor"),
            "{out:?}"
        );
        assert_eq!(snapshot(&fx.home), before);
        assert_eq!(fx.rel.requests(), requests);
        if recorded {
            assert_eq!(fx.read(&fx.home.join("Documents/vendor/marker")), "mine");
        }
    }
}
