//! Export to the site folder (TECHNICAL 4.3), the files a web host serves,
//! with atomic file writes.

use crate::{Dist, DistError, Entry, Exported, Link, Slug, view};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) fn io(path: &Path, source: std::io::Error) -> DistError {
    DistError::Io {
        path: path.into(),
        source,
    }
}
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
#[cfg(test)]
thread_local! {
    /// Test seam: the next directory sync after a rename on this thread fails with EIO.
    pub(crate) static TEST_FAIL_DIR_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(crate) fn write_equal(path: &Path, bytes: &[u8], mode: u32) -> Result<bool, DistError> {
    if read_plain(path)?.as_deref() == Some(bytes) {
        return Ok(false);
    }
    let parent = path.parent().expect("file parent");
    // Holder writes may run alongside a web fetch. Each writer owns its temporary name.
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| io(parent, e))?;
    tmp.write_all(bytes).map_err(|e| io(tmp.path(), e))?;
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|e| io(tmp.path(), e))?;
    tmp.as_file().sync_all().map_err(|e| io(tmp.path(), e))?;
    tmp.persist(path).map_err(|e| io(path, e.error))?;
    #[cfg(test)]
    if TEST_FAIL_DIR_SYNC.with(|fail| fail.replace(false)) {
        return Err(io(parent, std::io::Error::from_raw_os_error(5)));
    }
    fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|e| io(parent, e))?;
    Ok(true)
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
