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
