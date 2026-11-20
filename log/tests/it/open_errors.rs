#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::os::unix::fs::{PermissionsExt, symlink};
use watane_log::{Error, Log};

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn ac1_regular_file_is_not_a_directory() {
    let t = dir();
    let f = t.path().join("f");
    std::fs::write(&f, b"x").unwrap();
    let e = Log::open(&f).unwrap_err();
    assert!(matches!(
        e,
        Error::NotPlain {
            expected: "directory",
            link: false,
            ..
        }
    ));
    assert_eq!(e.to_string(), format!("{} is not a directory", f.display()));
}

#[test]
fn ac2_symlink_dir_and_directory_key() {
    let t = dir();
    let link = t.path().join("l");
    symlink(t.path(), &link).unwrap();
    let e = Log::open(&link).unwrap_err();
    assert!(matches!(
        e,
        Error::NotPlain {
            expected: "directory",
            link: true,
            ..
        }
    ));
    assert_eq!(
        e.to_string(),
        format!(
            "{} is a symbolic link; a directory is required (a link is refused)",
            link.display()
        )
    );
    let d = t.path().join("d");
    std::fs::create_dir(&d).unwrap();
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::create_dir(d.join("identity.key")).unwrap();
    let e = Log::open(&d).unwrap_err();
    assert!(matches!(
        e,
        Error::NotPlain {
            expected: "regular file",
            link: false,
            ..
        }
    ));
    assert_eq!(
        e.to_string(),
        format!("{} is not a regular file", d.join("identity.key").display())
    );
}

#[test]
fn ac3_empty_path() {
    let e = Log::open(std::path::Path::new("")).unwrap_err();
    assert!(matches!(e, Error::EmptyPath));
    assert_eq!(e.to_string(), "the log directory path is empty");
}

#[test]
fn s91_2_ac2_fifo_key_is_not_plain_and_does_not_hang() {
    let d = dir();
    let path = d.path().join("log");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(path.join("identity.key"))
        .status()
        .unwrap();
    assert!(status.success());
    let err = Log::open(&path).err().unwrap();
    assert!(matches!(
        err,
        Error::NotPlain {
            expected: "regular file",
            link: false,
            ..
        }
    ));
}

#[test]
fn s91_2_ac1_dangling_key_link_is_not_plain() {
    let d = dir();
    let path = d.path().join("log");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    symlink(d.path().join("none"), path.join("identity.key")).unwrap();
    let err = Log::open(&path).err().unwrap();
    assert!(matches!(
        err,
        Error::NotPlain {
            expected: "regular file",
            link: true,
            ..
        }
    ));
}
