//! Windows identity key files (TECHNICAL 4.7): validate, create and replace the key.

use crate::Error;
use ed25519_dalek::SigningKey;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use watane_log_windows_fs::{self as winfs, FileKind};
use zeroize::Zeroizing;

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

fn not_plain(path: &Path, expected: &'static str, kind: FileKind) -> Error {
    if matches!(kind, FileKind::ReparsePoint) {
        return io_error(
            path,
            io::Error::new(
                io::ErrorKind::InvalidData,
                "a Windows reparse point is refused",
            ),
        );
    }
    Error::NotPlain {
        path: path.to_owned(),
        expected,
        link: false,
    }
}

fn check_regular(path: &Path, file: &File) -> Result<(), Error> {
    match winfs::kind(file).map_err(|e| io_error(path, e))? {
        FileKind::Regular { links: 1 } => {}
        FileKind::Regular { links } => {
            return Err(Error::KeyLinks {
                path: path.to_owned(),
                links: u64::from(links),
            });
        }
        kind => return Err(not_plain(path, "regular file", kind)),
    }
    winfs::check_owner_only(file).map_err(|e| io_error(path, e))
}

fn check_directory(path: &Path, directory: &File) -> Result<(), Error> {
    match winfs::kind(directory).map_err(|e| io_error(path, e))? {
        FileKind::Directory => winfs::check_owner_only(directory).map_err(|e| io_error(path, e)),
        kind => Err(not_plain(path, "directory", kind)),
    }
}

fn directory(path: &Path, create: bool) -> Result<File, Error> {
    if path.as_os_str().is_empty() {
        return Err(Error::EmptyPath);
    }
    let directory = match winfs::open_path(path, false) {
        Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
            winfs::create_dir_all(path).map_err(|e| io_error(path, e))?;
            winfs::open_path(path, false)
        }
        result => result,
    }
    .map_err(|e| io_error(path, e))?;
    check_directory(path, &directory)?;
    Ok(directory)
}

fn lock_directory(path: &Path) -> Result<File, Error> {
    let lock = path.join(".identity.lock");
    let file = match winfs::create_file(&lock) {
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => winfs::open_path(&lock, true),
        result => result,
    }
    .map_err(|e| io_error(&lock, e))?;
    check_regular(&lock, &file)?;
    file.lock().map_err(|e| io_error(&lock, e))?;
    Ok(file)
}

/// Removes each leftover `.identity.key.<32 lowercase hex>.tmp` (TECHNICAL 4.7). Under the
/// lock; an entry that cannot be removed is left.
fn sweep(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_str().is_some_and(super::is_stray_tmp) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

pub(crate) fn open_keyless(dir: &Path) -> Result<(), Error> {
    let _directory = directory(dir, true)?;
    let _lock = lock_directory(dir)?;
    sweep(dir);
    let path = dir.join("identity.key");
    match fs::symlink_metadata(&path) {
        Ok(_) => Err(Error::HasIdentity { path }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io_error(&path, e)),
    }
}

fn read(path: &Path) -> Result<Option<SigningKey>, Error> {
    let mut file = match winfs::open_path(path, false) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_error(path, e)),
    };
    check_regular(path, &file)?;
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

pub(crate) fn open(dir: &Path) -> Result<SigningKey, Error> {
    let _directory = directory(dir, true)?;
    let _lock = lock_directory(dir)?;
    sweep(dir);
    let path = dir.join("identity.key");
    if let Some(key) = read(&path)? {
        return Ok(key);
    }
    let mut seed = Zeroizing::new([0; 32]);
    getrandom::fill(&mut *seed).map_err(|e| io_error(&path, io::Error::other(e)))?;
    write_key(dir, &path, &seed)?;
    read(&path)?.ok_or_else(|| io_error(&path, io::ErrorKind::NotFound.into()))
}

/// The publish step is the rename: when `write_key` succeeds, the new key is the file and goes
/// to `slot`.
pub(crate) fn replace(
    dir: &Path,
    key: SigningKey,
    slot: &mut Option<SigningKey>,
) -> Result<(), Error> {
    let _directory = directory(dir, false)?;
    let _lock = lock_directory(dir)?;
    sweep(dir);
    let seed = Zeroizing::new(key.to_bytes());
    write_key(dir, &dir.join("identity.key"), &seed)?;
    *slot = Some(key);
    Ok(())
}

pub(crate) fn remove(dir: &Path, _key: &mut Option<SigningKey>) -> Result<(), Error> {
    Err(io_error(
        &dir.join("identity.key"),
        io::Error::new(
            io::ErrorKind::Unsupported,
            "dropping an identity requires a durable directory flush, which is unavailable on Windows",
        ),
    ))
}

fn write_key(dir: &Path, path: &Path, seed: &[u8; 32]) -> Result<(), Error> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| io_error(dir, io::Error::other(e)))?;
    let tmp = dir.join(format!(
        ".identity.key.{}.tmp",
        data_encoding::HEXLOWER.encode(&random)
    ));
    let mut file = winfs::create_file(&tmp).map_err(|e| io_error(&tmp, e))?;
    let result = (|| {
        file.write_all(seed).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        drop(file);
        winfs::publish(&tmp, path, true).map_err(|e| io_error(path, e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
