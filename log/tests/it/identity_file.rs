#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::oracle;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use watane_log::{Error, Log};
fn mode(p: &std::path::Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}
#[test]
fn ac1_create_and_reopen_identity() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("new/nested");
    let log = Log::open(&dir).unwrap();
    let path = dir.join("identity.key");
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&path), 0o600);
    assert_eq!(fs::read(&path).unwrap().len(), 32);
    assert_eq!(
        Log::open(&dir).unwrap().identity().unwrap(),
        log.identity().unwrap()
    );
    assert_eq!(
        format!("{log:?}"),
        format!("Log {{ identity: {} }}", log.identity().unwrap())
    );
}
#[test]
fn ac2_refuse_exposed_symlink_and_corrupt_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = temp.path().join("identity.key");
    fs::write(&path, oracle::SEED).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let e = Log::open(temp.path()).unwrap_err();
    assert!(matches!(&e,Error::Exposed{path:p,mode:0o640} if p==&path));
    assert!(e.to_string().contains("exposed"));
    assert!(e.to_string().contains("0640"));
    assert!(e.to_string().contains(path.to_str().unwrap()));
    assert_eq!(fs::read(&path).unwrap(), oracle::SEED);
    assert_eq!(mode(&path), 0o640);
    fs::rename(&path, temp.path().join("valid")).unwrap();
    fs::set_permissions(temp.path().join("valid"), fs::Permissions::from_mode(0o600)).unwrap();
    symlink(temp.path().join("valid"), &path).unwrap();
    let e = Log::open(temp.path()).unwrap_err();
    assert!(matches!(
        e,
        Error::NotPlain {
            expected: "regular file",
            link: true,
            ..
        }
    ));
    assert_eq!(
        e.to_string(),
        format!(
            "{} is a symbolic link; a regular file is required (a link is refused)",
            path.display()
        )
    );
    fs::remove_file(&path).unwrap();
    fs::write(&path, [0; 31]).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let e = Log::open(temp.path()).unwrap_err();
    assert!(matches!(e, Error::CorruptKey { .. }));
    assert!(e.to_string().contains("corrupt key file"));
    assert_eq!(fs::read(&path).unwrap(), [0; 31]);
    assert_eq!(mode(&path), 0o600);
}
#[test]
fn ac3_identity_from_rfc_seed() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = temp.path().join("identity.key");
    fs::write(&path, oracle::SEED).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    let key = Log::open(temp.path()).unwrap().identity().unwrap();
    assert_eq!(key.as_bytes(), &oracle::PUBLIC);
    assert_eq!(key.to_string(), oracle::zbase32(&oracle::PUBLIC));
}

#[test]
fn s109_ac3_refuse_hard_link_to_valid_seed() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let seed = temp.path().join("seed");
    let path = temp.path().join("identity.key");
    fs::write(&seed, oracle::SEED).unwrap();
    fs::set_permissions(&seed, fs::Permissions::from_mode(0o600)).unwrap();
    fs::hard_link(&seed, &path).unwrap();

    let error = Log::open(temp.path()).unwrap_err().to_string();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert!(error.contains("hard link"), "{error}");
}

#[test]
fn s109_ac5_refuse_other_private_key_mode() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let path = temp.path().join("identity.key");
    fs::write(&path, oracle::SEED).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();

    let error = Log::open(temp.path()).unwrap_err().to_string();
    assert!(error.contains(path.to_str().unwrap()), "{error}");
    assert!(error.contains("0700"), "{error}");
    assert!(error.contains("0600"), "{error}");
}

#[test]
fn s109_ac6_simultaneous_opens_share_the_new_key() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let readers: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    Log::open(temp.path()).unwrap().identity().unwrap()
                })
            })
            .collect();
        let mut identities = readers.into_iter().map(|reader| reader.join().unwrap());
        let identity = identities.next().expect("at least one opener");
        assert!(identities.all(|other| other == identity));
    });
    assert_eq!(mode(&temp.path().join("identity.key")), 0o600);
}

#[test]
fn ac2_directory_refusals() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o750)).unwrap();
    assert!(matches!(
        Log::open(temp.path()),
        Err(Error::Exposed { mode: 0o750, .. })
    ));
    assert!(!temp.path().join("identity.key").exists());
    let link = temp.path().join("link");
    symlink(temp.path(), &link).unwrap();
    assert!(matches!(
        Log::open(&link),
        Err(Error::NotPlain {
            expected: "directory",
            ..
        })
    ));
    assert!(matches!(
        Log::open(std::path::Path::new("")),
        Err(Error::EmptyPath)
    ));
}

#[test]
fn regress_s109_interrupted_creation_still_opens() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let first = Log::open(temp.path()).unwrap().identity().unwrap();
    // A crash between hard_link and remove_file leaves the temporary name.
    fs::hard_link(
        temp.path().join("identity.key"),
        temp.path().join(".identity.key.00000000000000000000000000000000.tmp"),
    )
    .unwrap();
    assert_eq!(Log::open(temp.path()).unwrap().identity().unwrap(), first);
}
