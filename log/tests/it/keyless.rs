#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::oracle;
use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use watane_log::{Entry, Error, Log, PublicKey, Status};

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let path = e.unwrap().path();
            let permissions = fs::metadata(&path).unwrap().permissions();
            #[cfg(unix)]
            let protection = permissions.mode();
            #[cfg(windows)]
            let protection = u32::from(permissions.readonly());
            (path.clone(), (protection, fs::read(path).unwrap()))
        })
        .collect()
}

#[test]
fn ac1_keyless_creates_store_and_reads_oracle_chain() {
    let temp = oracle::directory();
    let dir = temp.path().join("fresh/nested");
    let log = Log::open_keyless(&dir).unwrap();
    #[cfg(unix)] // The Unix directory uses mode 0700.
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(dir.join("channels.db").is_file());
    assert!(!dir.join("identity.key").exists());
    let raw = oracle::chain(oracle::SEED, 3);
    let entries: Vec<_> = raw.iter().map(|r| Entry::decode(r).unwrap()).collect();
    let author = PublicKey::from_bytes(&oracle::PUBLIC).unwrap();
    assert_eq!(log.ingest(&author, &entries).unwrap().stored.len(), 3);
    assert_eq!(log.head(&author).unwrap().unwrap().seq, 2);
    assert_eq!(log.status(&author).unwrap(), Status::Ok);
    drop(log);
    let log = Log::open_keyless(&dir).unwrap();
    assert_eq!(
        log.entries(&author, 0, 10)
            .unwrap()
            .iter()
            .map(|e| e.bytes().to_vec())
            .collect::<Vec<_>>(),
        raw
    );
    assert!(!dir.join("identity.key").exists());
}

#[test]
fn ac2_keyless_refuses_every_form_of_key_unchanged() {
    // The last two fixtures exercise POSIX symlinks.
    let cases = if cfg!(unix) { 5 } else { 3 };
    for form in 0..cases {
        let dir = oracle::directory();
        let path = dir.path().join("identity.key");
        match form {
            0 | 1 => {
                fs::write(&path, if form == 0 { &oracle::SEED[..] } else { b"bad" }).unwrap();
                #[cfg(unix)] // Exercise an exposed Unix mode.
                fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
            }
            2 => fs::create_dir(&path).unwrap(),
            #[cfg(unix)] // The extra cases are POSIX symlinks.
            3 | 4 => {
                let target = dir.path().join("target");
                if form == 3 {
                    fs::write(&target, oracle::SEED).unwrap();
                }
                symlink(target, &path).unwrap();
            }
            _ => unreachable!("fixture case outside the supported range"),
        }
        #[cfg(unix)]
        let before = fs::symlink_metadata(&path).unwrap();
        let bytes = fs::read(&path).ok();
        let link = fs::read_link(&path).ok();
        assert!(
            matches!(Log::open_keyless(dir.path()), Err(Error::HasIdentity { path: p }) if p == path)
        );
        #[cfg(unix)] // Verify the Unix mode remains untouched.
        assert_eq!(
            fs::symlink_metadata(&path).unwrap().permissions().mode(),
            before.permissions().mode()
        );
        assert_eq!(fs::read(&path).ok(), bytes);
        assert_eq!(fs::read_link(&path).ok(), link);
        assert!(!dir.path().join("channels.db").exists());
    }
}

#[test]
fn ac3_keyless_refuses_identity_operations_without_changes() {
    let dir = oracle::directory();
    let log = Log::open_keyless(dir.path()).unwrap();
    let db = rusqlite::Connection::open(dir.path().join("channels.db")).unwrap();
    for unconfirmed in [false, true] {
        if unconfirmed {
            db.execute(
                "INSERT INTO meta VALUES ('unconfirmed', ?1)",
                [oracle::PUBLIC],
            )
            .unwrap();
        }
        let before = snapshot(dir.path());
        assert_eq!(log.identity(), None);
        assert_eq!(format!("{log:?}"), "Log { keyless }");
        assert!(log.own_head_confirmed().unwrap());
        assert!(matches!(log.append("a", b""), Err(Error::NoIdentity)));
        assert!(matches!(log.append("INVALID", b""), Err(Error::NoIdentity)));
        assert!(matches!(log.export_identity(""), Err(Error::NoIdentity)));
        assert!(matches!(
            log.restore_identity(b"", ""),
            Err(Error::NoIdentity)
        ));
        assert!(matches!(
            log.restore_identity(&oracle::hex(oracle::BACKUP), "correct horse"),
            Err(Error::NoIdentity)
        ));
        assert!(matches!(log.confirm_own_head(), Err(Error::NoIdentity)));
        assert!(matches!(log.drop_identity(), Err(Error::NoIdentity)));
        assert!(snapshot(dir.path()) == before, "directory files changed");
    }
}

#[test]
fn ac4_drop_unused_identity_and_preserve_used_identity() {
    for own_entry in [false, true] {
        let dir = oracle::directory();
        let log = Log::open(dir.path()).unwrap();
        let author = log.identity().unwrap();
        let db = rusqlite::Connection::open(dir.path().join("channels.db")).unwrap();
        if own_entry {
            log.append("a", b"").unwrap();
        }
        db.execute(
            "INSERT INTO meta VALUES ('unconfirmed', ?1)",
            [author.as_bytes()],
        )
        .unwrap();
        // Establish this reader's WAL snapshot after the fixture's external write.
        log.head(&author).unwrap();
        let before = snapshot(dir.path());
        if own_entry {
            assert!(matches!(log.drop_identity(), Err(Error::IdentityInUse(a)) if a == author));
            assert_eq!(log.identity(), Some(author));
            assert!(snapshot(dir.path()) == before, "directory files changed");
        } else {
            #[cfg(windows)] // Windows refuses when it cannot durably remove the key.
            {
                let error = log.drop_identity().unwrap_err();
                assert!(
                    matches!(&error, Error::Io { source, .. } if source.kind() == std::io::ErrorKind::Unsupported)
                );
                assert!(error.to_string().contains("durable directory flush"));
                assert_eq!(log.identity(), Some(author));
                assert!(snapshot(dir.path()) == before, "directory files changed");
            }
            #[cfg(unix)] // Unix flushes the directory after removing the identity.
            {
                log.drop_identity().unwrap();
                assert!(!dir.path().join("identity.key").exists());
                assert_eq!(log.identity(), None);
                assert!(matches!(log.append("a", b""), Err(Error::NoIdentity)));
                assert_eq!(
                    db.query_row(
                        "SELECT count(*) FROM meta WHERE key='unconfirmed'",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                    0
                );
                assert_eq!(Log::open_keyless(dir.path()).unwrap().identity(), None);
            }
        }
    }
}

#[test]
fn ac5_channels_are_distinct_sorted_and_own_requires_entry() {
    for keyless in [false, true] {
        let dir = oracle::directory();
        let log = if keyless {
            Log::open_keyless(dir.path())
        } else {
            Log::open(dir.path())
        }
        .unwrap();
        assert!(log.channels().unwrap().is_empty());
        let mut authors = vec![];
        for seed in [[3; 32], [1; 32], [2; 32]] {
            let entries: Vec<_> = oracle::chain(seed, 2)
                .iter()
                .map(|r| Entry::decode(r).unwrap())
                .collect();
            let public = *ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(seed)).pk;
            let author = PublicKey::from_bytes(&public).unwrap();
            log.ingest(&author, &entries).unwrap();
            authors.push(author);
        }
        authors.sort();
        assert_eq!(log.channels().unwrap(), authors);
        if let Some(own) = log.identity() {
            assert!(!authors.contains(&own));
            log.append("a", b"").unwrap();
            authors.push(own);
            authors.sort();
            assert_eq!(log.channels().unwrap(), authors);
        }
    }
}

#[test]
fn ac5_channels_reject_invalid_stored_authors() {
    for author in [vec![0; 31], vec![0; 32]] {
        let dir = oracle::directory();
        let log = Log::open_keyless(dir.path()).unwrap();
        let db = rusqlite::Connection::open(dir.path().join("channels.db")).unwrap();
        db.execute("INSERT INTO entries VALUES (?1, 0, X'00', X'00')", [author])
            .unwrap();
        let error = log.channels().unwrap_err();
        assert!(matches!(error, Error::Storage(_)));
        assert!(
            error
                .to_string()
                .contains("corrupt store: invalid channel author")
        );
    }
}
