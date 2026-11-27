//! Unix identity key files (TECHNICAL 4.7): validate, create and replace the key.

use crate::Error;
use ed25519_dalek::SigningKey;
use rustix::fs::{AtFlags, Mode, OFlags, openat, renameat, statat, unlinkat};
use rustix::io::Errno;
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
fn check_dir(path: &Path, current: u32) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_error(path, e))?;
    check_directory(path, &meta, current)
}

fn check_directory(path: &Path, meta: &fs::Metadata, current: u32) -> Result<(), Error> {
    if !meta.is_dir() {
        return Err(not_plain(path, "directory", meta.file_type().is_symlink()));
    }
    check_mode(path, meta)?;
    let owner = meta.uid();
    if owner != current {
        return Err(Error::DirectoryOwner {
            path: path.to_owned(),
            owner,
            current,
        });
    }
    Ok(())
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
/// The checked directory handle every later file operation is relative to.
pub(crate) type Anchor = File;

const KEY: &str = "identity.key";

fn errno(e: rustix::io::Errno) -> io::Error {
    e.into()
}

/// Opens `name` (the key) relative to the checked directory, without following its
/// final component, and validates the open handle before reading. A missing
/// file is `Ok(None)`.
fn read(dir: &Path, anchor: &Anchor, name: &str) -> Result<Option<SigningKey>, Error> {
    let path = dir.join(name);
    #[cfg(target_os = "linux")]
    let flags = OFlags::NOFOLLOW | OFlags::PATH | OFlags::CLOEXEC;
    #[cfg(not(target_os = "linux"))]
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::NOCTTY;

    let file = match openat(anchor, name, flags, Mode::empty()) {
        Ok(fd) => File::from(fd),
        Err(Errno::NOENT) => return Ok(None),
        #[cfg(not(target_os = "linux"))]
        Err(Errno::LOOP) => return Err(not_plain(&path, "regular file", true)),
        // macOS can reject a socket at open. On Linux, O_PATH lets fstat classify it.
        #[cfg(not(target_os = "linux"))]
        Err(Errno::NXIO | Errno::OPNOTSUPP) => {
            return Err(not_plain(&path, "regular file", false));
        }
        Err(e) => return Err(io_error(&path, errno(e))),
    };
    let path = path.as_path();
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
    check_dir(dir, effective_uid())
}
/// Opens the directory once, checks the handle and locks it. The path is not
/// used again by any later operation.
fn lock_dir(dir: &Path) -> Result<Anchor, Error> {
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)
        .map_err(|e| io_error(dir, e))?;
    lock_anchor(dir, &directory)?;
    Ok(directory)
}
/// Checks the held handle again and takes the exclusive lock on it.
fn lock_anchor(dir: &Path, directory: &Anchor) -> Result<(), Error> {
    let meta = directory.metadata().map_err(|e| io_error(dir, e))?;
    check_directory(dir, &meta, effective_uid())?;
    // Serialize the lookup and first publication with other opens and restores.
    directory.lock().map_err(|e| io_error(dir, e))
}
fn unlock(dir: &Path, anchor: &Anchor) -> Result<(), Error> {
    anchor.unlock().map_err(|e| io_error(dir, e))
}
pub(crate) fn open_keyless(dir: &Path) -> Result<Anchor, Error> {
    prepare_dir(dir)?;
    let anchor = lock_dir(dir)?;
    let path = dir.join(KEY);
    match statat(&anchor, KEY, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => return Err(Error::HasIdentity { path }),
        Err(Errno::NOENT) => {}
        Err(e) => return Err(io_error(&path, errno(e))),
    }
    unlock(dir, &anchor)?;
    Ok(anchor)
}
pub(crate) fn open(dir: &Path) -> Result<(SigningKey, Anchor), Error> {
    prepare_dir(dir)?;
    let anchor = lock_dir(dir)?;
    let key = match read(dir, &anchor, KEY)? {
        Some(key) => key,
        None => {
            let mut seed = Zeroizing::new([0; 32]);
            let path = dir.join(KEY);
            getrandom::fill(&mut *seed).map_err(|e| io_error(&path, io::Error::other(e)))?;
            write_key(dir, &anchor, &seed)?;
            read(dir, &anchor, KEY)?
                .ok_or_else(|| io_error(&path, io::ErrorKind::NotFound.into()))?
        }
    };
    unlock(dir, &anchor)?;
    Ok((key, anchor))
}

pub(crate) fn replace(dir: &Path, anchor: &Anchor, key: &SigningKey) -> Result<(), Error> {
    lock_anchor(dir, anchor)?;
    let seed = Zeroizing::new(key.to_bytes());
    let result = write_key(dir, anchor, &seed);
    unlock(dir, anchor)?;
    result
}

pub(crate) fn remove(
    dir: &Path,
    anchor: &Anchor,
    key: &mut Option<SigningKey>,
) -> Result<(), Error> {
    lock_anchor(dir, anchor)?;
    let result = remove_key(dir, anchor, key);
    unlock(dir, anchor)?;
    result
}
fn remove_key(dir: &Path, anchor: &Anchor, key: &mut Option<SigningKey>) -> Result<(), Error> {
    let path = dir.join(KEY);
    unlinkat(anchor, KEY, AtFlags::empty()).map_err(|e| io_error(&path, errno(e)))?;
    // SigningKey zeroizes on drop. Stop signing even if the following sync fails.
    *key = None;
    anchor.sync_all().map_err(|e| io_error(dir, e))
}

/// Writes the seed to a temporary name in the held directory and publishes it
/// as `identity.key`, all relative to that handle.
fn write_key(dir: &Path, anchor: &Anchor, seed: &[u8; 32]) -> Result<(), Error> {
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|e| io_error(dir, io::Error::other(e)))?;
    let name = format!(
        ".identity.key.{}.tmp",
        data_encoding::HEXLOWER.encode(&random)
    );
    let tmp = dir.join(&name);
    let fd = openat(
        anchor,
        name.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|e| io_error(&tmp, errno(e)))?;
    let mut file = File::from(fd);
    let result = (|| {
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| io_error(&tmp, e))?;
        file.write_all(seed).map_err(|e| io_error(&tmp, e))?;
        file.sync_all().map_err(|e| io_error(&tmp, e))?;
        renameat(anchor, name.as_str(), anchor, KEY).map_err(|e| io_error(&dir.join(KEY), errno(e)))
    })();
    if let Err(error) = result {
        let _ = unlinkat(anchor, name.as_str(), AtFlags::empty());
        return Err(error);
    }
    anchor.sync_all().map_err(|e| io_error(dir, e))
}
#[cfg(test)]
mod tests {
    use super::{check_dir, check_key_owner, effective_uid, open, read, remove_key, write_key};
    use crate::Error;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Path;

    #[test]
    fn s112_private_directory_with_foreign_owner_is_refused() {
        let dir = tempfile::tempdir().expect("create log directory");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("set private directory mode");
        let owner = fs::metadata(dir.path()).expect("stat directory").uid();
        let error = check_dir(dir.path(), owner ^ 1).expect_err("refuse the foreign owner");
        assert!(matches!(
            error,
            Error::DirectoryOwner { path, owner: found, current }
                if path == dir.path() && found == owner && current == owner ^ 1
        ));
    }

    #[test]
    fn s112_root_refuses_to_open_a_private_directory_owned_by_another_user() {
        if effective_uid() != 0 {
            return; // The check above runs without chown privileges.
        }
        let parent = tempfile::tempdir().expect("create temporary parent");
        let dir = parent.path().join("log");
        fs::create_dir(&dir).expect("create log directory");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .expect("set private directory mode");
        std::os::unix::fs::chown(&dir, Some(65534), None)
            .expect("change temporary directory owner");

        for result in [crate::Log::open(&dir), crate::Log::open_keyless(&dir)] {
            assert!(
                matches!(
                    &result,
                    Err(Error::DirectoryOwner { path, owner: 65534, current: 0 }) if path == &dir
                ),
                "{result:?}"
            );
        }
        assert_eq!(fs::read_dir(&dir).expect("list log directory").count(), 0);
    }

    /// Review S112 F1.2: the owner check on `lock_dir`'s handle is the only one for
    /// `replace` and `remove`.
    #[test]
    fn s112_root_refuses_to_replace_or_remove_in_a_directory_now_owned_by_another_user() {
        if effective_uid() != 0 {
            return; // Changing the owner needs root.
        }
        let parent = tempfile::tempdir().expect("create temporary parent");
        let dir = parent.path().join("log");
        let (old, anchor) = open(&dir).expect("create identity");
        std::os::unix::fs::chown(&dir, Some(65534), None).expect("change directory owner");
        let new = ed25519_dalek::SigningKey::from_bytes(&[9; 32]);
        let refused = |result: Result<(), Error>| {
            matches!(
                result,
                Err(Error::DirectoryOwner {
                    owner: 65534,
                    current: 0,
                    ..
                })
            )
        };
        assert!(refused(super::replace(&dir, &anchor, &new)));
        let mut key = Some(new);
        assert!(refused(super::remove(&dir, &anchor, &mut key)));
        assert!(key.is_some());
        std::os::unix::fs::chown(&dir, Some(0), None).expect("restore directory owner");
        assert_eq!(
            open(&dir).expect("reopen identity").0.to_bytes(),
            old.to_bytes()
        );
    }

    #[test]
    fn regress_s109_creation_publishes_a_single_name() {
        let dir = tempfile::tempdir().expect("create log directory");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700))
            .expect("set private directory mode");
        let directory = super::lock_dir(dir.path()).expect("lock the log directory");
        let path = dir.path().join("identity.key");
        let seed = [7; 32];

        write_key(dir.path(), &directory, &seed).expect("publish the seed");
        drop(directory);

        let meta = fs::metadata(&path).expect("stat published key");
        assert_eq!(meta.nlink(), 1);
        assert_eq!(
            fs::read_dir(dir.path())
                .expect("list log directory")
                .count(),
            1
        );
        assert_eq!(
            open(dir.path()).expect("reopen published key").0.to_bytes(),
            seed
        );
    }

    #[test]
    fn s109_ac4_device_is_refused() {
        // Use the system device so this runs without permission to create one.
        let path = Path::new("/dev/null");
        let anchor = fs::File::open("/dev").expect("open /dev");
        let result = read(Path::new("/dev"), &anchor, "null");
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

    /// S158 AC1: after the directory is checked, a swapped path redirects nothing.
    #[test]
    fn ac1_swapped_directory_path_does_not_redirect_key_operations() {
        let parent = tempfile::tempdir().expect("create parent");
        let dir = parent.path().join("log");
        let other = parent.path().join("other");
        let moved = parent.path().join("moved");
        for d in [&dir, &other] {
            fs::create_dir(d).expect("create directory");
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("make private");
        }
        let anchor = super::lock_dir(&dir).expect("lock the log directory");
        fs::rename(&dir, &moved).expect("move the checked directory away");
        std::os::unix::fs::symlink(&other, &dir).expect("put a link in its place");

        write_key(&dir, &anchor, &[1; 32]).expect("write through the handle");
        assert_eq!(fs::read(moved.join("identity.key")).expect("key"), [1; 32]);
        write_key(&dir, &anchor, &[2; 32]).expect("replace through the handle");
        assert_eq!(fs::read(moved.join("identity.key")).expect("key"), [2; 32]);
        assert_eq!(
            read(&dir, &anchor, "identity.key")
                .expect("read through the handle")
                .expect("key exists")
                .to_bytes(),
            [2; 32]
        );
        let mut key = Some(ed25519_dalek::SigningKey::from_bytes(&[2; 32]));
        remove_key(&dir, &anchor, &mut key).expect("remove through the handle");
        assert!(key.is_none());
        assert_eq!(fs::read_dir(&moved).expect("list").count(), 0);
        assert_eq!(fs::read_dir(&other).expect("list other").count(), 0);
    }

    /// S158 AC3: a failed publication removes the temporary file through the handle.
    #[test]
    fn ac3_failed_publication_removes_the_temporary_file() {
        let dir = tempfile::tempdir().expect("create directory");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).expect("make private");
        // A directory named `identity.key` makes `renameat` fail.
        fs::create_dir(dir.path().join("identity.key")).expect("block the key name");
        let anchor = super::lock_dir(dir.path()).expect("lock the directory");
        assert!(matches!(
            write_key(dir.path(), &anchor, &[3; 32]),
            Err(Error::Io { path, .. }) if path == dir.path().join("identity.key")
        ));
        let names: Vec<_> = fs::read_dir(dir.path())
            .expect("list")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert_eq!(names, ["identity.key"]);
    }

    /// S158 AC4: the store file is created inside the checked directory, not by path.
    #[test]
    fn ac4_store_file_is_created_through_the_handle() {
        let parent = tempfile::tempdir().expect("create parent");
        let dir = parent.path().join("log");
        let other = parent.path().join("other");
        let moved = parent.path().join("moved");
        for d in [&dir, &other] {
            fs::create_dir(d).expect("create directory");
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("make private");
        }
        let anchor = super::lock_dir(&dir).expect("lock the log directory");
        fs::rename(&dir, &moved).expect("move the checked directory away");
        std::os::unix::fs::symlink(&other, &dir).expect("put a link in its place");
        // SQLite itself opens by path (TECHNICAL 4.8); only the creation is pinned here.
        let _ = crate::store::open(&dir, &anchor);
        let meta = fs::symlink_metadata(moved.join("channels.db")).expect("created in place");
        assert!(meta.is_file());
        assert_eq!(meta.mode() & 0o7777, 0o600);
    }

    /// S158-fix1 AC2: a swapped parent path makes the store open fail; no database is written.
    #[test]
    fn ac2_swapped_parent_fails_the_store_open_and_writes_nothing() {
        let parent = tempfile::tempdir().expect("create parent");
        let dir = parent.path().join("log");
        let other = parent.path().join("other");
        let moved = parent.path().join("moved");
        for d in [&dir, &other] {
            fs::create_dir(d).expect("create directory");
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("make private");
        }
        let foreign = b"not a database, owned by the other folder";
        fs::write(other.join("channels.db"), foreign).expect("plant a database");
        let anchor = super::lock_dir(&dir).expect("lock the log directory");
        fs::rename(&dir, &moved).expect("move the checked directory away");
        std::os::unix::fs::symlink(&other, &dir).expect("put a link in its place");
        let result = crate::store::open(&dir, &anchor);
        // SQLITE_OPEN_NOFOLLOW already refuses a path with a link in it.
        result.expect_err("a link in the path is refused");
        assert_eq!(fs::read(other.join("channels.db")).expect("read"), foreign);
        assert_eq!(
            fs::metadata(moved.join("channels.db")).expect("stat").len(),
            0
        );
        let names: Vec<_> = fs::read_dir(&other).expect("list").collect();
        assert_eq!(
            names.len(),
            1,
            "no -wal or -shm next to the foreign database"
        );
    }

    /// S158-fix1 AC2: a swapped parent that is a real directory fails the inode check.
    #[test]
    fn ac2_swapped_parent_directory_fails_the_inode_check() {
        let parent = tempfile::tempdir().expect("create parent");
        let dir = parent.path().join("log");
        let other = parent.path().join("other");
        let moved = parent.path().join("moved");
        for d in [&dir, &other] {
            fs::create_dir(d).expect("create directory");
            fs::set_permissions(d, fs::Permissions::from_mode(0o700)).expect("make private");
        }
        let foreign = b"not a database, owned by the other folder";
        fs::write(other.join("channels.db"), foreign).expect("plant a database");
        let anchor = super::lock_dir(&dir).expect("lock the log directory");
        fs::rename(&dir, &moved).expect("move the checked directory away");
        fs::rename(&other, &dir).expect("put the other directory in its place");
        let result = crate::store::open(&dir, &anchor);
        assert!(matches!(result, Err(Error::Io { path, .. }) if path == dir.join("channels.db")));
        assert_eq!(fs::read(dir.join("channels.db")).expect("read"), foreign);
        assert_eq!(
            fs::metadata(moved.join("channels.db")).expect("stat").len(),
            0
        );
        assert_eq!(fs::read_dir(&dir).expect("list").count(), 1);
    }
}
