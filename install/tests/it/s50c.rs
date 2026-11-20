//! S50c receipt, lock and recovery regressions (TECHNICAL 3.6, 8.5).
use crate::support::*;
use std::fs;
use std::os::unix::fs::symlink;

/// TECHNICAL 8.5 rule 4, 3.6 (S50c ac1): the folder identity recorded in the receipt guards its removals; the rule itself is `unit::unit_receipt_allowed_skips_what_the_install_never_wrote`.
#[test]
#[ignore = "slow: two whole uninstalls, one with a folder relinked and one with a receipt that records no folders, which only whole runs show (TECHNICAL 8.5)"]
fn ac1_resolved_folders_guard_receipt_removals() {
    for recorded in [true, false] {
        let fx = Fx::installed();
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
            assert!(out.stderr.lines().any(|l| l == "watane install: skipped ~/.local/share/watane-dev/vendor: ~/.local/share/watane-dev resolves to ~/Documents, not to ~/.local/share/watane-dev as at install"));
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

/// TECHNICAL 3.6.2 (INST-S-35): a planted lock stops an install and an uninstall (exit 5, nothing written, nothing fetched) and is never broken.
#[test]
#[ignore = "slow: nine whole runs (install, uninstall, status) at three planted locks; the lock texts are `unit::unit_take_lock_and_the_live_journal_stop_a_second_run`"]
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
    fx.unfault("mv_fault");
    assert_eq!(finish(child).code, 0);
    assert!(!fx.home.join(".watane-install.lock").exists());
}

/// TECHNICAL 3.6 step 3 (S50c ac3): a receipt backup cut short leaves the complete old receipt.
#[test]
#[ignore = "slow: a fault injected into one step of a whole run; only a whole run shows what it then tells and leaves (TECHNICAL 3.9)"]
fn ac3_partial_receipt_copy_keeps_complete_old_receipt() {
    let fx = Fx::installed();
    let before = fs::read(fx.receipt()).unwrap();
    fx.rel.set_version("0.2.0", &["new-crate-1.0.0"]);
    fx.state_write("cp_fault", "half\n*receipt.old.*\n");
    let out = fx.run(&["--release", "0.2.0", "--no-toolchain"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(fs::read(fx.receipt()).unwrap(), before);
    fx.assert_installed("0.1.0");
}

/// TECHNICAL 8.5, 3.8 (S50c ac3): a receipt in a folder that resolves out of the install folder is not used and its staging names stay.
#[test]
#[ignore = "slow: a whole uninstall whose share folder is a link out, which only a whole run shows (TECHNICAL 8.5)"]
fn ac3_disallowed_receipt_preserves_staging_evidence() {
    let fx = Fx::installed();
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

/// TECHNICAL 3.6 step 5, 8.5 rule 4 (S50c ac1): the cleanup after an upgrade checks that the folder resolves as the old receipt recorded.
#[test]
#[ignore = "slow: `finish_commit` over a prebuilt committed swap, whose folder was relinked, in a whole sourced run (TECHNICAL 3.6 step 5)"]
fn ac1_upgrade_cleanup_checks_old_receipt_resolution() {
    let fx = Fx::installed();
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

/// TECHNICAL 3.6.2 (S50c ac2): the lock is removed after a platform refusal, a checksum failure and an interrupted download.
#[test]
#[ignore = "slow: three whole runs that end at a platform refusal, a checksum failure and an interrupted download, each of which must remove its lock (TECHNICAL 3.6.2)"]
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

/// TECHNICAL 3.8, 3.6.2 (S50c ac5): a signal during the lock creation keeps its status.
#[test]
#[ignore = "slow: a whole run whose lock `mkdir` is signalled (TECHNICAL 3.8)"]
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
    let fx = Fx::installed();
    fs::create_dir(fx.home.join(".watane-install.lock")).unwrap();
    let before = snapshot(&fx.home);
    let out = fx.run(&["--uninstall"]);
    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(snapshot(&fx.home), before);
}

/// 3.6.2: a lock whose `pid` cannot be written stops the run (exit 5) and is removed. The fake
/// `mkdir` plants the `pid` file in the new lock, so that the write (noclobber) fails for root as
/// well as for any other user, which a 500-mode folder does not.
#[test]
#[ignore = "slow: a whole run whose lock cannot hold its pid file; the function is `unit::unit_take_lock_and_the_live_journal_stop_a_second_run` (nowrite)"]
fn ac2_unwritable_lock_pid_exits_5_and_removes_the_lock() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new();
    fs::write(
        fx.fakes.join("mkdir"),
        "#!/bin/sh\n\"$FAKE_REAL/mkdir\" \"$@\" || exit\ncase $1 in *.watane-install.lock) : > \"$1/pid\" ;; esac\n",
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

/// TECHNICAL 3.6.2, 3.8 (S50c ac2): the lock goes even when a signal's undo fails.
/// 3.6.2: the lock goes even when a signal's undo fails (the journal stays for the next run).
#[test]
#[ignore = "slow: a signal sent to a whole run at one point of its work; only a whole run shows what it then tells and leaves (TECHNICAL 3.8)"]
fn ac2_lock_is_removed_after_a_failed_undo_under_a_signal() {
    let fx = Fx::installed();
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

/// TECHNICAL 3.4.2, 8.5 rule 4 (S50c ac1): an upgrade refuses a remapped or unrecorded receipt path before it downloads.
#[test]
#[ignore = "slow: whole upgrades over folders relinked after the install, refused before any download (TECHNICAL 3.4.2, 8.5)"]
fn ac1_upgrade_refuses_remapped_or_unrecorded_receipt_paths_before_download() {
    for recorded in [true, false] {
        let fx = Fx::installed();
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
        let old = if recorded {
            "~/.local/share/watane-dev"
        } else {
            "an unrecorded path"
        };
        let now = if recorded {
            "~/Documents"
        } else {
            "~/.local/share/watane-dev"
        };
        assert!(out.stderr.contains(&format!(
            "watane install: ~/.local/share/watane-dev resolves to {now}, not to {old} as at install"
        )), "{out:?}");
        assert!(out.stderr.contains(
            "Make ~/.local/share/watane-dev a real folder again (or point it where it was), then run this again."
        ), "{out:?}");
        assert!(
            !out.stderr.contains("mv ~/.local/share/watane-dev/vendor"),
            "{out:?}"
        );
        assert!(!out.stderr.contains("did not install"), "{out:?}");
        let fixes: Vec<_> = out
            .stderr
            .lines()
            .filter(|l| l.starts_with("Make ~/"))
            .collect();
        let once: std::collections::BTreeSet<_> = fixes.iter().collect();
        assert_eq!(fixes.len(), once.len(), "one fix line per folder: {out:?}");
        if !recorded {
            assert!(
                fixes.contains(
                    &"Make ~/.local/bin a real folder again (or point it where it was), then run this again."
                ),
                "{out:?}"
            );
        }
        assert_eq!(snapshot(&fx.home), before);
        assert_eq!(fx.rel.requests(), requests);
        if recorded {
            assert_eq!(fx.read(&fx.home.join("Documents/vendor/marker")), "mine");
        }
    }
}

/// TECHNICAL 3.4.2 (S50c ac1): a relinked folder and an ordinary foreign path are both reported, exit 8.
#[test]
#[ignore = "slow: a whole install over a relinked folder and an ordinary foreign file (TECHNICAL 3.4.2)"]
fn ac1_relinked_folder_and_ordinary_foreign_path_are_both_reported() {
    let fx = Fx::installed();
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
    write_file(&fx.home.join("Documents/vendor/marker"), "mine");
    symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
    let dev = format!("path {}\n", fx.bin("watane-dev").display());
    let receipt = fx.read(&fx.receipt());
    assert!(receipt.contains(&dev));
    fs::write(fx.receipt(), receipt.replace(&dev, "")).unwrap();
    let before = snapshot(&fx.home);
    let requests = fx.rel.requests();
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 8, "{out:?}");
    assert!(
        out.stderr
            .contains("found files that watane did not install:"),
        "{out:?}"
    );
    assert!(
        out.stderr
            .contains("  mv ~/.local/bin/watane-dev ~/.local/bin/watane-dev.mine"),
        "{out:?}"
    );
    assert!(
        !out.stderr.contains("mv ~/.local/share/watane-dev/vendor"),
        "{out:?}"
    );
    assert!(
        out.stderr
            .contains("Make ~/.local/share/watane-dev a real folder again"),
        "{out:?}"
    );
    assert_eq!(snapshot(&fx.home), before);
    assert_eq!(fx.rel.requests(), requests);
}
