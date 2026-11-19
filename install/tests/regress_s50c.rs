//! Regressions for install-folder changes during staging and cleanup (TECHNICAL 3.6).
mod support;

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use support::*;

#[test]
fn ac2_folder_changed_during_staging_aborts_before_swap() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    let receipt = fs::read(fx.share("watane/receipt")).unwrap();
    let program = fs::read(fx.home.join(".local/bin/watane")).unwrap();
    fx.rel.set_version("0.2.0", &["serde-1.0.1"]);
    fx.bake_script("0.2.0");
    write_file(&fx.home.join("Documents/vendor/marker"), "mine");

    fx.state_write("chmod_wait", "");
    let child = spawn(&mut fx.cmd(&["--no-toolchain"]));
    fx.wait_state("chmod_waiting");
    fs::rename(fx.share("watane-dev"), fx.home.join("old-dev")).unwrap();
    symlink(fx.home.join("Documents"), fx.share("watane-dev")).unwrap();
    fs::remove_file(fx.state.join("chmod_wait")).unwrap();
    let out = finish(child);

    assert_eq!(out.code, 5, "{out:?}");
    assert_eq!(
        out.first_err(),
        format!(
            "watane install: ~/.local/share/watane-dev changed during the install: it now resolves to {}",
            fx.home.join("Documents").canonicalize().unwrap().display()
        )
    );
    assert_eq!(fs::read(fx.share("watane/receipt")).unwrap(), receipt);
    assert_eq!(
        fs::read(fx.home.join(".local/bin/watane")).unwrap(),
        program
    );
    assert_eq!(
        fs::read_to_string(fx.share("watane/VERSION")).unwrap(),
        "0.1.0\n"
    );
    assert!(fx.home.join("old-dev/vendor/serde-1.0.0").exists());
    assert_eq!(
        fs::read_to_string(fx.home.join("Documents/vendor/marker")).unwrap(),
        "mine"
    );
    assert!(!fx.home.join(".watane-install.lock").exists());
    assert!(fs::read_dir(fx.share("watane")).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("journal.")
    }));
}

#[test]
fn ac1_cleanup_keeps_old_name_when_folder_changes_after_commit() {
    let fx = Fx::new();
    assert_eq!(fx.run(&["--no-toolchain"]).code, 0);
    fx.rel.set_version("0.2.0", &["serde-1.0.1"]);
    fx.bake_script("0.2.0");
    let xattr = fx.fakes.join("xattr");
    fs::write(
        &xattr,
        format!(
            r#"#!/bin/sh
set -eu
"$FAKE_REAL/mv" "$HOME/.local/share/watane-dev" "$HOME/saved-dev"
"{}" -s "$HOME/Documents" "$HOME/.local/share/watane-dev"
mkdir -p "$HOME/Documents/vendor.old.$FAKE_INSTALL_PID"
printf mine > "$HOME/Documents/vendor.old.$FAKE_INSTALL_PID/marker"
printf '%s' "$FAKE_INSTALL_PID" > "$FAKE_STATE/cleanup_pid"
"#,
            which("ln").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&xattr, fs::Permissions::from_mode(0o755)).unwrap();
    let out = fx.run(&["--no-toolchain"]);
    assert_eq!(out.code, 0, "{out:?}");
    let pid = fx.state_read("cleanup_pid");
    assert_eq!(
        fs::read_to_string(fx.home.join(format!("Documents/vendor.old.{pid}/marker"))).unwrap(),
        "mine"
    );
    assert!(
        out.stderr.contains(&format!(
            "skipped {}: ~/.local/share/watane-dev resolves to",
            fx.share("watane-dev/vendor").display()
        )),
        "{out:?}"
    );
    assert_eq!(fx.read(&fx.share("watane/VERSION")), "0.2.0\n");
    assert!(!fx.home.join(".watane-install.lock").exists());
    assert!(!fx.share(&format!("watane/journal.{pid}")).exists());
}
