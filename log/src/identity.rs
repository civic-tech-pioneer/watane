//! The key file and its backup (TECHNICAL 4.9, 5.4): create, read and replace
//! the signing key, and the passphrase-encrypted backup format.

use crate::Error;
use ed25519_dalek::SigningKey;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use zeroize::Zeroizing;
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
fn not_plain(path: &Path, expected: &'static str, link: bool) -> Error {
    Error::NotPlain {
        path: path.to_owned(),
        expected,
        link,
    }
}
fn check_mode(path: &Path, meta: &fs::Metadata) -> Result<(), Error> {
    let mode = meta.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(Error::Exposed {
            path: path.to_owned(),
            mode,
        });
    }
    Ok(())
}
fn check_dir(path: &Path) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_error(path, e))?;
    if !meta.is_dir() {
        return Err(not_plain(path, "directory", meta.file_type().is_symlink()));
    }
    check_mode(path, &meta)
}
/// Opens the key without following a link and without blocking, then checks
/// type and mode on that handle. A missing file is `Ok(None)`.
fn read(path: &Path) -> Result<Option<SigningKey>, Error> {
    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
            return Err(not_plain(path, "regular file", true));
        }
        // A socket cannot be opened (ENXIO): it is not a regular file either.
        Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {
            return Err(not_plain(path, "regular file", false));
        }
        Err(e) => return Err(io_error(path, e)),
    };
    let meta = file.metadata().map_err(|e| io_error(path, e))?;
    if !meta.is_file() {
        return Err(not_plain(path, "regular file", false));
    }
    check_mode(path, &meta)?;
    let mut seed = Zeroizing::new([0; 32]);
    file.read_exact(&mut seed[..]).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            Error::CorruptKey {
                path: path.to_owned(),
            }
        } else {
            io_error(path, e)
        }
    })?;
    let mut extra = [0];
    if file.read(&mut extra).map_err(|e| io_error(path, e))? != 0 {
        return Err(Error::CorruptKey {
            path: path.to_owned(),
        });
    }
    Ok(Some(SigningKey::from_bytes(&seed)))
}
pub(crate) fn prepare_dir(dir: &Path) -> Result<(), Error> {
    if dir.as_os_str().is_empty() {
        return Err(Error::EmptyPath);
    }
    match fs::symlink_metadata(dir) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|e| io_error(dir, e))?,
        Err(e) => return Err(io_error(dir, e)),
        _ => {}
    }
    check_dir(dir)
}
pub(crate) fn open_keyless(dir: &Path) -> Result<(), Error> {
    prepare_dir(dir)?;
    let path = dir.join("identity.key");
    match fs::symlink_metadata(&path) {
        Ok(_) => Err(Error::HasIdentity { path }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(&path, e)),
    }
}
pub(crate) fn open(dir: &Path) -> Result<SigningKey, Error> {
    prepare_dir(dir)?;
    let path = dir.join("identity.key");
    if let Some(key) = read(&path)? {
        return Ok(key);
    }
    let mut seed = Zeroizing::new([0; 32]);
    getrandom::fill(&mut *seed).map_err(|e| io_error(&path, io::Error::other(e)))?;
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| io_error(&path, io::Error::other(e)))?;
    let tmp = dir.join(format!(
        ".identity.key.{}.tmp",
        data_encoding::HEXLOWER.encode(&random)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| io_error(&tmp, e))?;
    let result = (|| {
        file.write_all(&seed[..]).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        match fs::hard_link(&tmp, &path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io_error(&path, e)),
        }
        Ok(())
    })();
    // Only remove the temporary file created by this call.
    fs::remove_file(&tmp).map_err(|e| io_error(&tmp, e))?;
    result?;
    File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| io_error(dir, e))?;
    read(&path)?.ok_or_else(|| io_error(&path, io::ErrorKind::NotFound.into()))
}

pub(crate) fn replace(dir: &Path, key: &SigningKey) -> Result<(), Error> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| io_error(dir, io::Error::other(e)))?;
    let tmp = dir.join(format!(
        ".identity.key.{}.tmp",
        data_encoding::HEXLOWER.encode(&random)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| io_error(&tmp, e))?;
    let result = (|| {
        let seed = Zeroizing::new(key.to_bytes());
        file.write_all(&seed[..]).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        let path = dir.join("identity.key");
        fs::rename(&tmp, &path).map_err(|e| io_error(&path, e))?;
        File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|e| io_error(dir, e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
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
