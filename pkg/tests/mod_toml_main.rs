#[path = "support/release.rs"]
#[allow(dead_code)]
mod support;

use std::fs;
use watane_pkg::{Mod, ModError, Rule};

fn with_main(value: &str) -> tempfile::TempDir {
    let dir = support::built_fixture("hello");
    let path = dir.path().join("mod.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(path, format!("{text}main = {value}\n")).unwrap();
    dir
}

#[test]
fn ac1_main_proposal_and_default() {
    let plain = support::built_fixture("hello");
    assert!(!Mod::read(plain.path()).unwrap().main());
    let proposed = with_main("true");
    assert!(Mod::read(proposed.path()).unwrap().main());
    let declined = with_main("false");
    assert!(!Mod::read(declined.path()).unwrap().main());
}

#[test]
fn ac2_main_requires_boolean_and_page() {
    let wrong_type = with_main("\"yes\"");
    expect(wrong_type.path(), "`main` must be true or false");
    let no_page = with_main("true");
    fs::remove_file(no_page.path().join("ui/index.html")).unwrap();
    expect(no_page.path(), "`main` needs a page (ui/index.html)");
    fs::remove_dir(no_page.path().join("ui")).unwrap();
    expect(no_page.path(), "`main` needs a page (ui/index.html)");
}

#[test]
fn main_with_linked_ui_gets_only_the_path_violation() {
    let dir = with_main("true");
    let page = tempfile::tempdir().unwrap();
    fs::rename(dir.path().join("ui"), page.path().join("ui")).unwrap();
    std::os::unix::fs::symlink(page.path().join("ui"), dir.path().join("ui")).unwrap();
    let Err(ModError::Invalid(violations)) = Mod::read(dir.path()) else {
        panic!("a linked ui must be refused");
    };
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].rule, Rule::Path);
    assert_eq!(violations[0].file, "ui");
}

fn expect(dir: &std::path::Path, message: &str) {
    let Err(ModError::Invalid(violations)) = Mod::read(dir) else {
        panic!("expected a mod.toml violation: {message}");
    };
    assert!(
        violations
            .iter()
            .any(|v| { v.rule == Rule::ModToml && v.file == "mod.toml" && v.message == message }),
        "{violations:?}"
    );
}
