use crate::oracle;
use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use watane_log::{Error, Log};
#[test]
fn ac6_backup_roundtrip_and_independent_decryption() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    let raw = l.export_identity("correct horse").unwrap();
    assert_eq!(raw.len(), 108);
    assert_eq!(&raw[..8], b"watanek1");
    let n = |i| u32::from_le_bytes(raw[i..i + 4].try_into().unwrap());
    assert_eq!((n(8), n(12), n(16)), (65536, 3, 4));
    let params = argon2::Params::new(n(8), n(12), n(16), Some(32)).unwrap();
    let mut key = [0; 32];
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(b"correct horse", &raw[20..36], &mut key)
        .unwrap();
    let seed = XChaCha20Poly1305::new((&key).into())
        .decrypt(
            <&chacha20poly1305::XNonce>::try_from(&raw[36..60]).unwrap(),
            Payload {
                msg: &raw[60..],
                aad: &raw[..60],
            },
        )
        .unwrap();
    assert_eq!(seed, std::fs::read(d.path().join("identity.key")).unwrap());
    let dest = oracle::directory();
    let restored = Log::open(dest.path()).unwrap();
    assert_eq!(
        restored.restore_identity(&raw, "correct horse").unwrap(),
        l.identity().unwrap()
    );
    drop(restored);
    assert_eq!(
        Log::open(dest.path()).unwrap().identity().unwrap(),
        l.identity().unwrap()
    );
}
#[test]
fn ac6_golden_nfc_and_unused_identity_replacement() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    assert_eq!(
        l.restore_identity(&oracle::hex(oracle::BACKUP), "correct horse")
            .unwrap()
            .as_bytes(),
        &oracle::PUBLIC
    );
    let backup = l.export_identity("é").unwrap();
    let dest = oracle::directory();
    let r = Log::open(dest.path()).unwrap();
    assert_eq!(
        r.restore_identity(&backup, "e\u{301}").unwrap(),
        l.identity().unwrap()
    );
    assert_eq!(
        r.restore_identity(&backup, "é").unwrap(),
        l.identity().unwrap()
    );
}
#[test]
fn ac6_failures_keep_identity_and_reject_bounds_fast() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    let before = std::fs::read(d.path().join("identity.key")).unwrap();
    assert!(matches!(l.export_identity(""), Err(Error::EmptyPassphrase)));
    let backup = oracle::hex(oracle::BACKUP);
    assert!(matches!(
        l.restore_identity(&backup, ""),
        Err(Error::EmptyPassphrase)
    ));
    assert!(matches!(
        l.restore_identity(&backup, "wrong"),
        Err(Error::WrongPassphrase)
    ));
    let mut changed = backup.clone();
    changed[70] ^= 1;
    assert!(matches!(
        l.restore_identity(&changed, "correct horse"),
        Err(Error::WrongPassphrase)
    ));
    for (field, raw) in [
        ("length", backup[..107].to_vec()),
        ("magic", {
            let mut b = backup.clone();
            b[0] ^= 1;
            b
        }),
        ("memory", {
            let mut b = backup.clone();
            b[8..12].copy_from_slice(&1048577u32.to_le_bytes());
            b
        }),
    ] {
        let start = std::time::Instant::now();
        assert!(
            matches!(l.restore_identity(&raw,"correct horse"),Err(Error::BadBackup{field:f,..}) if f==field)
        );
        assert!(start.elapsed() < std::time::Duration::from_millis(50));
    }
    let mut ascii = backup.clone();
    ascii[0] = b'v';
    let mut high = backup.clone();
    high[0] = 0xff;
    for (raw, shown) in [(ascii, "\"vatanek1\""), (high, "\"\\xffatanek1\"")] {
        let e = l.restore_identity(&raw, "correct horse").unwrap_err();
        assert!(matches!(&e, Error::BadBackup{field:"magic",found,..} if found==shown));
        assert!(!e.to_string().contains('\u{fffd}'));
    }
    assert_eq!(
        std::fs::read(d.path().join("identity.key")).unwrap(),
        before
    );
    l.append("a", b"").unwrap();
    let identity = l.identity().unwrap();
    assert!(
        matches!(l.restore_identity(&backup,"correct horse"),Err(Error::IdentityInUse(a)) if a==identity)
    );
    assert_eq!(l.identity().unwrap(), identity);
    assert_eq!(
        std::fs::read(d.path().join("identity.key")).unwrap(),
        before
    );
}

#[test]
fn ac4_ac5_bad_backup_messages_say_expected_and_found() {
    let d = oracle::directory();
    let l = Log::open(d.path()).unwrap();
    let backup = oracle::hex(oracle::BACKUP);
    let msg = |raw: &[u8]| match l.restore_identity(raw, "correct horse") {
        Err(e @ Error::BadBackup { .. }) => e.to_string(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        msg(&backup[..107]),
        "bad backup: length: expected 108 bytes, found 107 bytes"
    );
    let mut long = backup.clone();
    long.push(0);
    assert!(msg(&long).ends_with("found 109 bytes"));
    let mut b = backup.clone();
    b[0] ^= 1;
    let m = msg(&b);
    assert!(
        m.contains("expected \"watanek1\", found \"vatanek1\""),
        "{m}"
    );
    for (at, v, want) in [
        (
            8,
            1048577u32,
            "memory: expected 8192 to 1048576 KiB and at least 8 x lanes, found 1048577",
        ),
        (12, 11, "passes: expected 1 to 10, found 11"),
        (16, 17, "lanes: expected 1 to 16, found 17"),
    ] {
        let mut b = backup.clone();
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let m = msg(&b);
        assert!(m.contains(want), "{m}");
    }
}
