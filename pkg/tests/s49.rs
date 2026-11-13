use std::{fs, path::Path};
use watane_pkg::*;
#[allow(dead_code)]
#[path = "support/release.rs"]
mod support;

const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);
const LINK: &str = "replace the symbolic link with a regular entry";

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn only_link(dir: &Path, part: &str) {
    match Mod::read(dir) {
        Err(ModError::Invalid(v)) => assert_eq!(
            v,
            [Violation {
                rule: Rule::Path,
                file: part.into(),
                message: LINK.into()
            }],
            "{part}"
        ),
        other => panic!("expected invalid, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn ac1_s49_linked_ui_one_violation() {
    for target in ["ui2", "/etc", "missing"] {
        let dir = support::built_fixture("hello");
        let ui = dir.path().join("ui");
        if target == "ui2" {
            fs::rename(&ui, dir.path().join("ui2")).unwrap();
        } else {
            fs::remove_dir_all(&ui).unwrap();
        }
        std::os::unix::fs::symlink(target, &ui).unwrap();
        only_link(dir.path(), "ui");
    }
}

#[cfg(unix)]
#[test]
fn ac2_s49_linked_wit_one_violation() {
    for target in ["wit2", "/etc", "missing"] {
        let dir = support::built_fixture("hello");
        let wit = dir.path().join("wit");
        if target == "wit2" {
            fs::rename(&wit, dir.path().join("wit2")).unwrap();
        } else {
            fs::remove_dir_all(&wit).unwrap();
        }
        std::os::unix::fs::symlink(target, &wit).unwrap();
        only_link(dir.path(), "wit");
    }
}

fn message(bytes: &[u8]) -> String {
    Release::decode(bytes).unwrap_err().to_string()
}

#[test]
fn ac3_s49_cut_short_values() {
    let g = hex(GOLDEN);
    assert_eq!(
        message(&g[..4]),
        "bad release body: slug: expected text, found a text string cut short by the end of input"
    );
    let id = 1 + 1 + 1 + 5 + 1 + 5 + 1 + 5 + 1 + 1 + 1 + 11;
    assert_eq!(
        message(&g[..id + 3]),
        "bad release body: package id: expected a 32-byte key, found a byte string cut short by the end of input"
    );
    assert_eq!(
        message(&[0x8c, 0x19]),
        "bad release body: format: expected an unsigned integer, found an unsigned integer cut short by the end of input"
    );
}

#[test]
fn ac4_s49_nothing_there_stays_end_of_input() {
    for b in [&[][..], &[0x8c][..]] {
        assert!(message(b).ends_with("found end of input"), "{}", message(b));
    }
}

#[cfg(unix)]
#[test]
fn ac1_s49_linked_logic_wasm_one_violation() {
    // A mod without logic exports, and one with exports but no logic/ source.
    for (fixture, drop) in [("http-valid", Some("logic")), ("hello", None)] {
        let dir = support::built_fixture(fixture);
        if let Some(d) = drop {
            fs::remove_dir_all(dir.path().join(d)).unwrap();
        }
        std::os::unix::fs::symlink("missing", dir.path().join("logic.wasm")).unwrap();
        match Mod::read(dir.path()) {
            Err(ModError::Invalid(v)) => assert_eq!(
                v,
                [Violation {
                    rule: Rule::Path,
                    file: "logic.wasm".into(),
                    message: "replace the symbolic link with a regular file".into()
                }],
                "{fixture}"
            ),
            other => panic!("expected invalid, got {other:?}"),
        }
    }
}
