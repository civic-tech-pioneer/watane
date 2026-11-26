//! Regressions found by the S135 review (PR #49, r1), each red before its fix.

use std::fs;
use std::os::unix::fs::symlink;

use crate::support::unit::*;
use crate::support::*;

/// TECHNICAL 3.6 step 3, 3.9 (S135 r1 P1.1): `~/.local/bin` relinked to the home itself after the
/// check says "your home folder itself", as `check_folders` does, never a bare `~`.
#[test]
fn regress_s135_changed_during_install_to_the_home_has_no_bare_tilde() {
    let fx = Fx::new();
    let home = fx.root.path().join("h");
    fs::create_dir_all(home.join(".local/bin")).expect("bin");
    fs::create_dir_all(home.join(".local/share/watane")).expect("share");
    symlink(&home, home.join("binlink")).expect("link");
    let units = fx.units(
        "",
        &[],
        &[(
            "relinked",
            r#"HOME=$ROOT/h; check_home; set_entries; check_folders
rmdir "$HOME/.local/bin" && mv "$HOME/binlink" "$HOME/.local/bin" || exit 99
if swap_entries; then rc=0; else rc=$?; fi; echo "rc=$rc"; printf 'changed<%s>\n' "$SWAP_CHANGED""#,
        )],
    );
    let u = case(&units, "relinked");
    assert!(u.has_out_line("rc=1"), "{u:?}");
    assert!(
        u.has_out_line(
            "changed<~/.local/bin changed during the install: it now resolves to your home folder itself>"
        ),
        "{u:?}"
    );
}
