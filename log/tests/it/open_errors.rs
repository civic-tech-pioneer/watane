#![allow(clippy::unwrap_used, reason = "tests may panic")]

#[cfg(unix)]
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

#[cfg(unix)] // Uses POSIX symlinks and mode bits.
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

#[cfg(unix)] // Uses a POSIX FIFO.
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
    // A blocking open of a FIFO waits for a writer: fail instead of hanging.
    let (tx, rx) = std::sync::mpsc::channel();
    let opener = path.clone();
    std::thread::spawn(move || tx.send(Log::open(&opener).err()).unwrap());
    let err = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("Log::open on a FIFO key returned promptly")
        .unwrap();
    assert!(matches!(
        err,
        Error::NotPlain {
            expected: "regular file",
            link: false,
            ..
        }
    ));
}

#[cfg(unix)] // Uses a POSIX dangling symlink.
#[test]
fn s91_2_ac1_dangling_key_link_is_not_plain() {
    let d = dir();
    let path = d.path().join("log");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = path.join("identity.key");
    symlink(d.path().join("none"), &key).unwrap();
    let err = Log::open(&path).err().unwrap();
    assert!(matches!(
        &err,
        Error::NotPlain {
            expected: "regular file",
            link: true,
            ..
        }
    ));
    assert_eq!(
        err.to_string(),
        format!(
            "{} is a symbolic link; a regular file is required (a link is refused)",
            key.display()
        )
    );
}

#[cfg(unix)] // Uses an AF_UNIX socket pathname.
#[test]
fn s91_2_ac2_socket_key_is_not_plain() {
    let d = dir();
    let path = d.path().join("log");
    std::fs::create_dir(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(path.join("identity.key")).unwrap();
    let err = Log::open(&path).err().unwrap();
    assert!(
        matches!(
            err,
            Error::NotPlain {
                expected: "regular file",
                link: false,
                ..
            }
        ),
        "{err:?}"
    );
}
