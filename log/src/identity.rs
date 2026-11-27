//! The key file and its backup (TECHNICAL 4.7, 4.9, 5.4).

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::{Anchor, open, open_keyless, remove, replace};
#[cfg(windows)]
mod windows;
/// Windows holds its directory handle inside each call; nothing is passed on.
#[cfg(windows)]
pub(crate) struct Anchor;
#[cfg(windows)]
pub(crate) fn open(dir: &std::path::Path) -> Result<(SigningKey, Anchor), Error> {
    windows::open(dir).map(|key| (key, Anchor))
}
#[cfg(windows)]
pub(crate) fn open_keyless(dir: &std::path::Path) -> Result<Anchor, Error> {
    windows::open_keyless(dir).map(|()| Anchor)
}
#[cfg(windows)]
pub(crate) fn replace(
    dir: &std::path::Path,
    _: &Anchor,
    key: SigningKey,
    slot: &mut Option<SigningKey>,
) -> Result<(), Error> {
    windows::replace(dir, key, slot)
}
/// Whether `name` is exactly `.identity.key.<32 lowercase hex>.tmp`: the leftover of a crashed
/// key write, which the next open removes (TECHNICAL 4.7).
fn is_stray_tmp(name: &str) -> bool {
    name.strip_prefix(".identity.key.")
        .and_then(|rest| rest.strip_suffix(".tmp"))
        .is_some_and(|hex| {
            hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}
#[cfg(windows)]
pub(crate) fn remove(
    dir: &std::path::Path,
    _: &Anchor,
    key: &mut Option<SigningKey>,
) -> Result<(), Error> {
    windows::remove(dir, key)
}

use crate::Error;
use ed25519_dalek::SigningKey;
use zeroize::Zeroizing;

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use unicode_normalization::UnicodeNormalization;
fn password(passphrase: &str) -> Result<Zeroizing<String>, Error> {
    let p = Zeroizing::new(passphrase.nfc().collect::<String>());
    if p.is_empty() {
        return Err(Error::EmptyPassphrase);
    }
    Ok(p)
}
fn bad(field: &'static str, expected: String, found: String) -> Error {
    Error::BadBackup {
        field,
        expected,
        found,
    }
}
fn bad_memory(memory: u32) -> Error {
    bad(
        "memory",
        "8192 to 1048576 KiB and at least 8 x lanes".into(),
        format!("{memory} KiB"),
    )
}
fn derive(header: &[u8], password: &str) -> Result<Zeroizing<[u8; 32]>, Error> {
    let n = |i| u32::from_le_bytes(header[i..i + 4].try_into().expect("checked header"));
    let params = argon2::Params::new(n(8), n(12), n(16), Some(32)).map_err(|_| bad_memory(n(8)))?;
    let mut key = Zeroizing::new([0; 32]);
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(password.as_bytes(), &header[20..36], &mut *key)
        .map_err(|e| Error::Storage(e.to_string()))?;
    Ok(key)
}
pub(crate) fn export(seed: &[u8; 32], passphrase: &str) -> Result<Vec<u8>, Error> {
    let password = password(passphrase)?;
    let mut header = b"watanek1".to_vec();
    for n in [65536u32, 3, 4] {
        header.extend(n.to_le_bytes());
    }
    header.resize(60, 0);
    getrandom::fill(&mut header[20..60]).map_err(|e| Error::Storage(e.to_string()))?;
    let key = derive(&header, &password)?;
    let cipher = XChaCha20Poly1305::new((&*key).into());
    let encrypted = cipher
        .encrypt(
            <&chacha20poly1305::XNonce>::try_from(&header[36..60]).expect("the slice is 24 bytes"),
            Payload {
                msg: seed,
                aad: &header,
            },
        )
        .map_err(|_| Error::Storage("backup encryption failed".into()))?;
    header.extend(encrypted);
    Ok(header)
}
pub(crate) fn decrypt(backup: &[u8], passphrase: &str) -> Result<SigningKey, Error> {
    if backup.len() != crate::BACKUP_LEN {
        return Err(bad(
            "length",
            format!("{} bytes", crate::BACKUP_LEN),
            format!("{} bytes", backup.len()),
        ));
    }
    if &backup[..8] != b"watanek1" {
        return Err(bad(
            "magic",
            "\"watanek1\"".into(),
            format!("\"{}\"", backup[..8].escape_ascii()),
        ));
    }
    let n = |i| u32::from_le_bytes(backup[i..i + 4].try_into().expect("checked backup length"));
    let (memory, passes, lanes) = (n(8), n(12), n(16));
    if !(8192..=1048576).contains(&memory) || u64::from(memory) < 8 * u64::from(lanes) {
        return Err(bad_memory(memory));
    }
    if !(1..=10).contains(&passes) {
        return Err(bad("passes", "1 to 10".into(), passes.to_string()));
    }
    if !(1..=16).contains(&lanes) {
        return Err(bad("lanes", "1 to 16".into(), lanes.to_string()));
    }
    let password = password(passphrase)?;
    let key = derive(backup, &password)?;
    let plain = Zeroizing::new(
        XChaCha20Poly1305::new((&*key).into())
            .decrypt(
                <&chacha20poly1305::XNonce>::try_from(&backup[36..60])
                    .expect("the slice is 24 bytes"),
                Payload {
                    msg: &backup[60..],
                    aad: &backup[..60],
                },
            )
            .map_err(|_| Error::WrongPassphrase)?,
    );
    let seed =
        Zeroizing::new(<[u8; 32]>::try_from(plain.as_slice()).map_err(|_| Error::WrongPassphrase)?);
    Ok(SigningKey::from_bytes(&seed))
}
