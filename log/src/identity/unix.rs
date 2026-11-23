//! Unix identity key files (TECHNICAL 4.7): validate, create and replace the key.

use crate::Error;
use ed25519_dalek::SigningKey;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
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
fn key_metadata(path: &Path, file: &File) -> Result<fs::Metadata, Error> {
    let meta = file.metadata().map_err(|e| io_error(path, e))?;
    if !meta.is_file() {
        return Err(not_plain(
            path,
            "regular file",
            meta.file_type().is_symlink(),
        ));
    }
    let links = meta.nlink();
    if links != 1 {
        return Err(Error::KeyLinks {
            path: path.to_owned(),
            links,
        });
    }
    check_mode(path, &meta)?;
    let mode = meta.mode() & 0o7777;
    if mode != 0o600 {
        return Err(Error::KeyMode {
            path: path.to_owned(),
            mode,
        });
    }
    Ok(meta)
}
fn effective_uid() -> u32 {
    rustix::process::geteuid().as_raw()
}
fn check_key_owner(path: &Path, meta: &fs::Metadata, current: u32) -> Result<(), Error> {
    let owner = meta.uid();
    if owner != current {
        return Err(Error::KeyOwner {
            path: path.to_owned(),
            owner,
            current,
        });
    }
    Ok(())
}
/// Resolves the key path once without following its final component and
/// validates the open handle before reading. A missing file is `Ok(None)`.
fn read(path: &Path) -> Result<Option<SigningKey>, Error> {
    #[cfg(target_os = "linux")]
    let flags = libc::O_NOFOLLOW | libc::O_PATH;
    #[cfg(not(target_os = "linux"))]
    let flags = libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_NOCTTY;

    let file = match OpenOptions::new().read(true).custom_flags(flags).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        #[cfg(not(target_os = "linux"))]
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
            return Err(not_plain(path, "regular file", true));
        }
        // macOS can reject a socket at open. On Linux, O_PATH lets fstat classify it.
        #[cfg(not(target_os = "linux"))]
        Err(e) if matches!(e.raw_os_error(), Some(libc::ENXIO | libc::EOPNOTSUPP)) => {
            return Err(not_plain(path, "regular file", false));
        }
        Err(e) => return Err(io_error(path, e)),
    };
    let meta = key_metadata(path, &file)?;
    let current = effective_uid();
    check_key_owner(path, &meta, current)?;

    #[cfg(target_os = "linux")]
    let mut file = {
        // O_PATH inspects the inode without triggering a device open. Follow
        // the kernel's descriptor link to read that inode, never the key path.
        let reader = File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .map_err(|e| io_error(path, e))?;
        let meta = key_metadata(path, &reader)?;
        check_key_owner(path, &meta, current)?;
        reader
    };
    #[cfg(not(target_os = "linux"))]
    let mut file = file;

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

fn prepare_dir(dir: &Path) -> Result<(), Error> {
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
fn publish_key(tmp: &Path, path: &Path) -> Result<(), Error> {
    fs::rename(tmp, path).map_err(|e| io_error(path, e))
}
fn lock_dir(dir: &Path) -> Result<File, Error> {
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)
        .map_err(|e| io_error(dir, e))?;
    // Serialize the lookup and first publication with other opens and restores.
    directory.lock().map_err(|e| io_error(dir, e))?;
    Ok(directory)
}
pub(crate) fn open(dir: &Path) -> Result<SigningKey, Error> {
    prepare_dir(dir)?;
    let directory = lock_dir(dir)?;
    let path = dir.join("identity.key");
    if let Some(key) = read(&path)? {
        return Ok(key);
    }
    let mut seed = Zeroizing::new([0; 32]);
    getrandom::fill(&mut *seed).map_err(|e| io_error(&path, io::Error::other(e)))?;
    write_key(dir, &directory, &path, &seed)?;
    read(&path)?.ok_or_else(|| io_error(&path, io::ErrorKind::NotFound.into()))
}

pub(crate) fn replace(dir: &Path, key: &SigningKey) -> Result<(), Error> {
    let directory = lock_dir(dir)?;
    let seed = Zeroizing::new(key.to_bytes());
    write_key(dir, &directory, &dir.join("identity.key"), &seed)
}

pub(crate) fn remove(dir: &Path, key: &mut Option<SigningKey>) -> Result<(), Error> {
    let path = dir.join("identity.key");
    fs::remove_file(&path).map_err(|e| io_error(&path, e))?;
    // SigningKey zeroizes on drop. Stop signing even if the following sync fails.
    *key = None;
    File::open(dir)
        .and_then(|file| file.sync_all())
        .map_err(|e| io_error(dir, e))
}

fn write_key(dir: &Path, directory: &File, path: &Path, seed: &[u8; 32]) -> Result<(), Error> {
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
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| io_error(&tmp, e))?;
        file.write_all(seed).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        publish_key(&tmp, path)
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    directory.sync_all().map_err(|e| io_error(dir, e))
}
#[cfg(test)]
mod tests {
    use super::{check_key_owner, effective_uid, open, publish_key, read};
    use crate::Error;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    #[test]
    fn regress_s109_creation_publishes_a_single_name() {
        let dir = tempfile::tempdir().expect("create log directory");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("set private directory mode");
        let tmp = dir.path().join(".identity.key.test.tmp");
        let path = dir.path().join("identity.key");
        let seed = [7; 32];
        fs::write(&tmp, seed).expect("write seed");
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).expect("set key mode");

        publish_key(&tmp, &path).expect("publish the seed");

        // Assert the state an interruption immediately after publication leaves.
        let meta = fs::metadata(&path).expect("stat published key");
        assert_eq!(meta.nlink(), 1);
        assert!(!tmp.exists());
        assert_eq!(
            open(dir.path()).expect("reopen published key").to_bytes(),
            seed
        );
    }

    #[test]
    fn s109_ac4_device_is_refused() {
        // Use the system device so this runs without permission to create one.
        let path = Path::new("/dev/null");
        let result = read(path);
        assert!(
            matches!(
                &result,
                Err(Error::NotPlain { path: found, expected: "regular file", link: false })
                    if found == path
            ),
            "{result:?}"
        );
    }

    #[test]
    fn s109_ac5_foreign_owner_is_refused() {
        let file = tempfile::tempfile().expect("create key file");
        let meta = file.metadata().expect("stat key handle");
        let path = Path::new("identity.key");
        let current = meta.uid() ^ 1;
        let result = check_key_owner(path, &meta, current);
        assert!(
            matches!(
                &result,
                Err(Error::KeyOwner { path: found, owner, current: expected })
                    if found == path && *owner == meta.uid() && *expected == current
            ),
            "{result:?}"
        );
    }

    #[test]
    fn regress_s109_effective_uid_matches_file_owner() {
        let file = tempfile::tempfile().expect("create a file owned by this process");
        let owner = file.metadata().expect("stat the file handle").uid();
        let current = effective_uid();
        assert_eq!(current, owner);
    }
}
