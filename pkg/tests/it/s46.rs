#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::fs;
use std::path::Path;
use watane_pkg::*;

const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);
const LINK: &str = "a symbolic link; replace it with a regular file or directory";

#[cfg(unix)]
fn mkfifo(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    assert!(
        std::process::Command::new("mkfifo")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
}
#[cfg(unix)]
fn violations(dir: &Path) -> Vec<Violation> {
    match Mod::read(dir) {
        Err(ModError::Invalid(v)) => v,
        other => panic!("expected invalid, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn ac1_s46_hidden_link_named() {
    for (path, target) in [
        ("ui/.git/x", "../index.html"),
        ("ui/.a/.b/y", "/etc/passwd"),
        ("wit/.hid/k", "../world.wit"),
        ("wit/.a/.b/k", "/etc/passwd"),
        ("wit/other/k", "../world.wit"),
        ("wit/.hid", "."),
    ] {
        let dir = support::built_fixture("hello");
        let link = dir.path().join(path);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
        let v = violations(dir.path());
        assert_eq!(
            v,
            [Violation {
                rule: Rule::Path,
                file: path.into(),
                message: LINK.into()
            }],
            "{path}"
        );
        match pack(dir.path(), false) {
            Err(PackageError::Symlink { path: p }) => assert_eq!(p, path),
            // pack never enters wit/other (not content, 6.1): Mod::read names the link.
            Err(PackageError::Mod(ModError::Invalid(pv))) => {
                assert_eq!((path, pv), ("wit/other/k", v));
            }
            other => panic!("{path}: {other:?}"),
        }
    }
}

#[cfg(unix)]
#[test]
fn ac1_s46_wit_directory_rule_without_link() {
    let dir = support::built_fixture("hello");
    fs::create_dir_all(dir.path().join("wit/.hidden")).unwrap();
    fs::write(dir.path().join("wit/.hidden/x.wit"), "x").unwrap();
    let v = violations(dir.path());
    assert_eq!(v.len(), 1, "{v:?}");
    assert_eq!((v[0].rule, v[0].file.as_str()), (Rule::Path, "wit/.hidden"));
}

#[cfg(unix)]
#[test]
fn ac2_s46_fifo_refused_by_read() {
    let dir = support::built_fixture("hello");
    mkfifo(&dir.path().join("ui/extra"));
    let v = violations(dir.path());
    assert_eq!(
        v,
        [Violation {
            rule: Rule::Path,
            file: "ui/extra".into(),
            message:
                "not a regular file or a directory (a FIFO, socket or device); replace or remove it"
                    .into()
        }]
    );
    assert!(matches!(
        pack(dir.path(), false),
        Err(PackageError::Path { path, .. }) if path == "ui/extra"
    ));
}

#[cfg(unix)]
#[test]
fn ac2_s46_hidden_fifo_ignored() {
    let dir = support::built_fixture("hello");
    let before = pack(dir.path(), false).unwrap();
    mkfifo(&dir.path().join("ui/.git/p"));
    Mod::read(dir.path()).unwrap();
    let after = pack(dir.path(), false).unwrap();
    assert_eq!(before.as_bytes(), after.as_bytes());
    assert_eq!(before.id(), after.id());
}

fn decode_error(bytes: &[u8]) -> String {
    let e = Release::decode(bytes).unwrap_err();
    assert!(matches!(e, ReleaseError::Malformed(_)), "{e}");
    e.to_string()
}

#[test]
fn ac4_s46_release_errors_say_what_was_expected() {
    let golden = data_encoding::HEXLOWER.decode(GOLDEN.as_bytes()).unwrap();
    for (bytes, field) in [
        (&[][..], ""),
        (&[0x8c][..], "format"),
        (&golden[..10], "version"),
    ] {
        let message = decode_error(bytes);
        assert_eq!(message.matches("release").count(), 1, "{message}");
        assert!(message.contains("expected"), "{message}");
        assert!(message.contains("end of input"), "{message}");
        assert!(message.contains(field), "{message}");
    }
    assert_eq!(
        decode_error(&[]),
        "bad release body: expected an array of 12 items, found end of input"
    );
    assert_eq!(
        decode_error(&[0x8c]),
        "bad release body: format: expected an unsigned integer, found end of input"
    );
}

#[test]
fn ac4_s46_wrong_type_says_expected() {
    // A text where the format number belongs.
    let message = decode_error(&[0x8c, 0x61, b'1']);
    assert_eq!(
        message,
        "bad release body: format: expected an unsigned integer, found a text string"
    );
}
