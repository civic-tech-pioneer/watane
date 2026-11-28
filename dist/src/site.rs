//! Export to the site folder (TECHNICAL 4.3), the files a web host serves,
//! with atomic file writes.

use crate::{Dist, DistError, Entry, Exported, Link, Slug, view};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) fn io(path: &Path, source: std::io::Error) -> DistError {
    DistError::Io {
        path: path.into(),
        source,
    }
}
/// `tempfile` wraps a create error with the path it tried (`<os error> at path "<p>"`), and
/// gives no access to the inner error; the caller names the path itself, once (TECHNICAL
/// 3.1), so only the OS text is kept. The OS text never holds ` at path `; the path may.
pub(crate) fn bare_io(error: std::io::Error) -> std::io::Error {
    let text = error.to_string();
    match (error.get_ref(), text.split_once(" at path ")) {
        (Some(_), Some((os, _))) => std::io::Error::new(error.kind(), os),
        _ => error,
    }
}
#[cfg(unix)]
pub(crate) fn public_dir(path: &Path) -> Result<(), DistError> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_dir() => {
            return Err(DistError::ExportConflict {
                path: path.into(),
                cause: "not a plain directory",
            });
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o755)
                .create(path)
                .map_err(|e| io(path, e))?;
        }
        Err(e) => return Err(io(path, e)),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| io(path, e))
}
#[cfg(windows)]
pub(crate) fn public_dir(path: &Path) -> Result<(), DistError> {
    // Windows serves the site as its owner. A writable destination must already
    // be private, or be created with a private ACL.
    crate::private_dir(path).map_err(|error| match error {
        DistError::Io { path, source } if source.kind() == std::io::ErrorKind::PermissionDenied => {
            DistError::ExportConflict {
                path,
                cause: "requires an owner-only Windows ACL",
            }
        }
        error => error,
    })
}
#[cfg(unix)]
pub(crate) fn read_plain(path: &Path) -> Result<Option<Vec<u8>>, DistError> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_file() => Err(DistError::ExportConflict {
            path: path.into(),
            cause: "not a plain file",
        }),
        Ok(_) => fs::read(path).map(Some).map_err(|e| io(path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path, e)),
    }
}
#[cfg(windows)]
pub(crate) fn read_plain(path: &Path) -> Result<Option<Vec<u8>>, DistError> {
    use std::io::Read;
    match retry_sharing(|| watane_log_windows_fs::open_private_file(path, false)) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(|e| io(path, e))?;
            Ok(Some(bytes))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => Err(DistError::ExportConflict {
            path: path.into(),
            cause: "not a plain file",
        }),
        Err(e) => Err(io(path, e)),
    }
}
#[cfg(windows)]
pub(crate) fn retry_sharing<T>(
    mut operation: impl FnMut() -> std::io::Result<T>,
) -> std::io::Result<T> {
    for delay in [5, 10, 20, 40, 80] {
        match operation() {
            Err(error) if error.raw_os_error() == Some(32) => {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            result => return result,
        }
    }
    operation()
}

#[cfg(test)]
thread_local! {
    /// Test seam: the next `n` directory syncs of `follows.json` on this thread fail with EIO.
    pub(crate) static TEST_FAIL_DIR_SYNC: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static TEST_FAIL_TEMP_WRITE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn test_dir_sync_fails() -> bool {
    TEST_FAIL_DIR_SYNC.with(|fail| {
        let left = fail.get();
        fail.set(left.saturating_sub(1));
        left > 0
    })
}
pub(crate) fn write_equal(path: &Path, bytes: &[u8], mode: u32) -> Result<bool, DistError> {
    let (written, synced) = replace_equal(path, bytes, mode)?;
    synced?;
    Ok(written)
}
/// `write_equal` that tells the two failures apart: `Err` when `path` was not replaced
/// (it keeps its old bytes); `Ok((_, Err))` when the rename installed the new bytes but
/// the directory sync failed, so they are in place but may not survive a crash.
pub(crate) fn replace_equal(
    path: &Path,
    bytes: &[u8],
    mode: u32,
) -> Result<(bool, Result<(), DistError>), DistError> {
    if read_plain(path)?.as_deref() == Some(bytes) {
        return Ok((false, Ok(())));
    }
    let parent = path.parent().expect("file parent");
    // Holder writes may run alongside a web fetch. Each writer owns its temporary name.
    #[cfg(unix)]
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| io(parent, bare_io(e)))?;
    #[cfg(windows)]
    let mut tmp = tempfile::Builder::new()
        .make_in(parent, watane_log_windows_fs::create_file)
        .map_err(|e| io(parent, bare_io(e)))?;
    #[cfg(test)]
    if TEST_FAIL_TEMP_WRITE.with(|fail| fail.replace(false)) {
        return Err(io(tmp.path(), std::io::Error::from_raw_os_error(5)));
    }
    // The file itself: `NamedTempFile`'s own `Write` adds its path to the error (3.1).
    tmp.as_file_mut()
        .write_all(bytes)
        .map_err(|e| io(tmp.path(), e))?;
    #[cfg(unix)]
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|e| io(tmp.path(), e))?;
    tmp.as_file().sync_all().map_err(|e| io(tmp.path(), e))?;
    #[cfg(unix)]
    {
        tmp.persist(path).map_err(|e| io(path, e.error))?;
        Ok((true, sync_dir(parent)))
    }
    #[cfg(windows)]
    {
        let _ = mode; // Both private state and the owner-served site use a private DACL.
        let tmp = tmp.into_temp_path();
        for delay in [0, 5, 10, 20, 40, 80] {
            if delay != 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            match watane_log_windows_fs::publish(&tmp, path, true) {
                Ok(()) => break,
                Err(error) if matches!(error.raw_os_error(), Some(5 | 32)) => {
                    // A checked reader pins the name. An identical concurrent
                    // write also satisfies this operation after its durable rename.
                    match read_plain(path) {
                        Ok(Some(current)) if current == bytes => return Ok((false, Ok(()))),
                        Err(DistError::Io { source, .. })
                            if matches!(source.raw_os_error(), Some(5 | 32)) => {}
                        Err(other) => return Err(other),
                        Ok(_) => {}
                    }
                    if delay == 80 {
                        return Err(io(path, error));
                    }
                }
                Err(error) => return Err(io(path, error)),
            }
        }
        #[cfg(test)]
        if test_dir_sync_fails() {
            return Ok((true, Err(io(parent, std::io::Error::from_raw_os_error(5)))));
        }
        Ok((true, Ok(())))
    }
}
/// Syncs directory `dir`, so the renames in it survive a crash.
pub(crate) fn sync_dir(dir: &Path) -> Result<(), DistError> {
    #[cfg(test)]
    if test_dir_sync_fails() {
        return Err(io(dir, std::io::Error::from_raw_os_error(5)));
    }
    #[cfg(unix)]
    return fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| io(dir, e));
    #[cfg(windows)]
    {
        // Only the test seam can leave a pending sync after a write-through rename.
        #[cfg(test)]
        return Ok(());
        #[cfg(not(test))]
        Err(io(
            dir,
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Windows requires a write-through rename to sync dist state",
            ),
        ))
    }
}
impl Dist {
    pub fn set_site_url(&self, url: &str) -> Result<(), DistError> {
        let link = Link::new(self.require_identity()?, url, None)?;
        let mut base = self
            .base
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let bytes = serde_json::to_vec(&serde_json::json!({"version":1,"url":link.base()}))
            .expect("site JSON");
        write_equal(&self.home.join("dist/site.json"), &bytes, 0o600)?;
        *base = Some(link.base().into());
        Ok(())
    }
    pub fn own_link(&self, slug: Option<&Slug>) -> Result<Link, DistError> {
        let key = self.require_identity()?;
        let base = self
            .base
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        Link::new(key, base.as_deref().ok_or(DistError::NoSiteUrl)?, slug)
    }
    pub fn export(&self, dir: &Path) -> Result<Exported, DistError> {
        self.require_identity()?;
        let _guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.export_unlocked(dir)
    }
    pub(crate) fn export_unlocked(&self, dir: &Path) -> Result<Exported, DistError> {
        let own = self.require_identity()?;
        let entries = self.log.entries(&own, 0, usize::MAX)?;
        let head = entries.last().map(Entry::seq);
        let head_path = dir.join("head");
        if let Some(bytes) = read_plain(&head_path)? {
            let entry = Entry::decode(&bytes).map_err(|_| DistError::ExportConflict {
                path: head_path.clone(),
                cause: "head is not an entry",
            })?;
            if entry.author() != own || head.is_none_or(|seq| entry.seq() > seq) {
                return Err(DistError::ExportConflict {
                    path: head_path,
                    cause: "head belongs to another channel or is ahead",
                });
            }
        }
        // Check immutable entry conflicts and package integrity before changing the destination.
        for entry in &entries {
            let path = dir.join("entries").join(entry.seq().to_string());
            if let Some(bytes) = read_plain(&path)? {
                let decoded = Entry::decode(&bytes).map_err(|_| DistError::ExportConflict {
                    path: path.clone(),
                    cause: "invalid entry",
                })?;
                if decoded.author() != own || decoded.seq() != entry.seq() || bytes != entry.bytes()
                {
                    return Err(DistError::ExportConflict {
                        path,
                        cause: "entry differs",
                    });
                }
            }
        }
        let view = view::derive(own, &entries);
        let mut packages = Vec::new();
        for info in view.releases.iter().filter(|r| r.withdrawn.is_none()) {
            packages.push((
                PathBuf::from("packages").join(info.release.package.to_string()),
                self.cache_read(&info.release.package)?,
            ));
        }
        public_dir(dir)?;
        let mut written = Vec::new();
        if !entries.is_empty() {
            public_dir(&dir.join("packages"))?;
            public_dir(&dir.join("entries"))?;
        }
        for (path, bytes) in packages {
            if write_equal(&dir.join(&path), &bytes, 0o644)? {
                written.push(path);
            }
        }
        for entry in &entries {
            let path = PathBuf::from("entries").join(entry.seq().to_string());
            if write_equal(&dir.join(&path), entry.bytes(), 0o644)? {
                written.push(path);
            }
        }
        if let Some(entry) = entries.last()
            && write_equal(&head_path, entry.bytes(), 0o644)?
        {
            written.push("head".into());
        }
        // The managed site mirrors live releases; external exports never remove files.
        if dir == self.site_dir() {
            for info in view.releases.iter().filter(|r| r.withdrawn.is_some()) {
                if view
                    .releases
                    .iter()
                    .any(|r| r.withdrawn.is_none() && r.release.package == info.release.package)
                {
                    continue;
                }
                let path = dir.join("packages").join(info.release.package.to_string());
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(io(&path, e)),
                }
            }
        }
        Ok(Exported {
            dir: dir.into(),
            head,
            written,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests may panic")]
    use super::*;

    /// A temp file that cannot be made names its folder once, even a folder whose own name
    /// holds tempfile's ` at path ` (3.1; s124-2 AC2).
    #[test]
    fn ac2_temp_create_failure_names_the_folder_once() {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("a at path b").join("missing");
        let Err(error) = replace_equal(&parent.join("file"), b"x", 0o600) else {
            panic!("a write into a missing folder succeeded");
        };
        let DistError::Io { path, source } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(*path, parent);
        let mut text = error.to_string();
        let mut cause = std::error::Error::source(&error);
        while let Some(next) = cause {
            text.push_str(": ");
            text.push_str(&next.to_string());
            cause = next.source();
        }
        // a message escapes its path (3.1): on Windows each `\` reads `\\`
        let named = parent.display().to_string().escape_debug().to_string();
        assert_eq!(text.matches(&*named).count(), 1, "{text}");
        assert!(!source.to_string().contains(" at path "), "{text}");
    }
    #[test]
    fn s164_temp_write_failure_cleans_up_and_names_temp_once() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("file");
        TEST_FAIL_TEMP_WRITE.with(|fail| fail.set(true));
        let error = replace_equal(&target, b"new", 0o600).unwrap_err();
        let DistError::Io { path, .. } = &error else {
            panic!("{error:?}");
        };
        assert_eq!(path.parent(), Some(dir.path()));
        assert_ne!(path, &target);
        assert!(!path.exists(), "temporary file survived failed write");
        assert!(!target.exists(), "failed write replaced target");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let named = path.display().to_string().escape_debug().to_string();
        assert_eq!(error.to_string().matches(&*named).count(), 1, "{error}");
    }
}
