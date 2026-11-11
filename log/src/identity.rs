use crate::Error;
use ed25519_dalek::SigningKey;
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::Path,
};
use zeroize::Zeroizing;
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
fn check(path: &Path, directory: bool) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_error(path, e))?;
    if !(if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    }) {
        return Err(Error::NotPlain {
            path: path.to_owned(),
        });
    }
    let mode = meta.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(Error::Exposed {
            path: path.to_owned(),
            mode,
        });
    }
    Ok(())
}
fn read(path: &Path) -> Result<SigningKey, Error> {
    check(path, false)?;
    let mut seed = Zeroizing::new([0; 32]);
    let mut file = File::open(path).map_err(|e| io_error(path, e))?;
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
    Ok(SigningKey::from_bytes(&seed))
}
pub(crate) fn open(dir: &Path) -> Result<SigningKey, Error> {
    if dir.as_os_str().is_empty() {
        return Err(io_error(
            dir,
            io::Error::new(io::ErrorKind::InvalidInput, "empty path"),
        ));
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
    check(dir, true)?;
    let path = dir.join("identity.key");
    match fs::symlink_metadata(&path) {
        Ok(_) => return read(&path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(&path, e)),
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
    read(&path)
}
