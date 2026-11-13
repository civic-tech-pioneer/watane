//! Open below a caller-supplied root without following any child link.
use std::{ffi::OsString, fs::File, io, path::Path};

pub(crate) fn is_link(error: &io::Error) -> bool {
    #[cfg(unix)]
    {
        matches!(error.raw_os_error(), Some(code) if code == rustix::io::Errno::LOOP.raw_os_error() || code == rustix::io::Errno::MLINK.raw_os_error())
    }
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Dir,
    Link,
    Other,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stat {
    pub kind: Kind,
    pub size: u64,
}

/// An open directory reached by the no-follow walk; entries are listed and
/// stat-ed relative to it, never by a path string.
pub(crate) struct Dir {
    #[cfg(unix)]
    fd: File,
    #[cfg(not(unix))]
    path: std::path::PathBuf,
}

#[cfg(unix)]
fn stat_of(s: &rustix::fs::Stat) -> Stat {
    use rustix::fs::FileType;
    Stat {
        kind: match FileType::from_raw_mode(s.st_mode) {
            FileType::RegularFile => Kind::File,
            FileType::Directory => Kind::Dir,
            FileType::Symlink => Kind::Link,
            _ => Kind::Other,
        },
        size: s.st_size as u64,
    }
}

/// Stat one entry of `dir` without following it.
pub(crate) fn stat(dir: &Dir, name: &str) -> io::Result<Stat> {
    #[cfg(unix)]
    {
        use rustix::fs::{AtFlags, statat};
        Ok(stat_of(&statat(&dir.fd, name, AtFlags::SYMLINK_NOFOLLOW)?))
    }
    #[cfg(not(unix))]
    {
        let m = std::fs::symlink_metadata(dir.path.join(name))?;
        Ok(Stat {
            kind: if m.file_type().is_symlink() {
                Kind::Link
            } else if m.is_dir() {
                Kind::Dir
            } else if m.is_file() {
                Kind::File
            } else {
                Kind::Other
            },
            size: m.len(),
        })
    }
}

/// The entries of `dir`, sorted by name, each stat-ed without following it.
pub(crate) fn entries(dir: &Dir) -> io::Result<Vec<(OsString, Stat)>> {
    let mut list = Vec::new();
    #[cfg(unix)]
    {
        use rustix::fs::{AtFlags, statat};
        use std::os::unix::ffi::OsStringExt;
        for entry in rustix::fs::Dir::read_from(&dir.fd)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == c"." || name == c".." {
                continue;
            }
            let s = statat(&dir.fd, name, AtFlags::SYMLINK_NOFOLLOW)?;
            list.push((OsString::from_vec(name.to_bytes().to_vec()), stat_of(&s)));
        }
    }
    #[cfg(not(unix))]
    for entry in std::fs::read_dir(&dir.path)? {
        let name = entry?.file_name();
        let s = stat(dir, &name.to_string_lossy())?;
        list.push((name, s));
    }
    list.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(list)
}

#[cfg(unix)]
fn descend(
    root: &Path,
    relative: &str,
    last: rustix::fs::OFlags,
) -> Result<File, (String, io::Error)> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, open, openat, statat};
    // The caller's root may itself contain links (e.g. /tmp on macOS).
    let top = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY;
    let mut parent = File::from(
        open(root, top, Mode::empty()).map_err(|e| (String::new(), io::Error::from(e)))?,
    );
    if relative.is_empty() {
        return Ok(parent);
    }
    let parts: Vec<_> = relative.split('/').collect();
    let mut prefix = String::new();
    for (i, part) in parts.iter().enumerate() {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        if part.is_empty() || *part == "." || *part == ".." {
            return Err((
                prefix,
                io::Error::new(io::ErrorKind::InvalidInput, "invalid relative path"),
            ));
        }
        let flags = OFlags::NOFOLLOW | if i + 1 == parts.len() { last } else { top };
        let fd = openat(&parent, *part, flags, Mode::empty()).map_err(|mut e| {
            // DIRECTORY|NOFOLLOW may report ENOTDIR for a link. Inspect only
            // the refused entry, relative to the already-open parent.
            if e == rustix::io::Errno::NOTDIR
                && statat(&parent, *part, AtFlags::SYMLINK_NOFOLLOW)
                    .is_ok_and(|s| FileType::from_raw_mode(s.st_mode) == FileType::Symlink)
            {
                e = rustix::io::Errno::LOOP;
            }
            (prefix.clone(), io::Error::from(e))
        })?;
        parent = File::from(fd);
    }
    Ok(parent)
}

/// Open the directory `relative` below `root` ("" is the root itself, which may
/// be a link); no part below the root is followed.
pub(crate) fn dir(root: &Path, relative: &str) -> Result<Dir, (String, io::Error)> {
    #[cfg(unix)]
    {
        use rustix::fs::OFlags;
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY;
        descend(root, relative, flags).map(|fd| Dir { fd })
    }
    #[cfg(not(unix))]
    {
        let path = root.join(relative);
        if !path.is_dir() {
            return Err((
                relative.into(),
                io::Error::new(io::ErrorKind::NotFound, "not a directory"),
            ));
        }
        Ok(Dir { path })
    }
}

pub(crate) fn file(root: &Path, relative: &str) -> Result<File, (String, io::Error)> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::OFlags;
        descend(
            root,
            relative,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        )?
    };
    #[cfg(not(unix))]
    let file = File::open(root.join(relative)).map_err(|e| (relative.into(), e))?;
    if !file.metadata().map_err(|e| (relative.into(), e))?.is_file() {
        return Err((
            relative.into(),
            io::Error::new(io::ErrorKind::InvalidInput, "expected a regular file"),
        ));
    }
    Ok(file)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn ac3_dir_names_a_link_part() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("real/sub")).unwrap();
        symlink("real", t.path().join("ui")).unwrap();
        let Err((part, e)) = dir(t.path(), "ui/sub") else {
            panic!("expected an error")
        };
        assert_eq!(part, "ui");
        assert!(is_link(&e), "{e}");
    }
    #[test]
    fn ac3_dir_fifo_middle_part_does_not_hang() {
        let t = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(t.path().join("ui"))
                .status()
                .unwrap()
                .success()
        );
        let Err((part, e)) = dir(t.path(), "ui/sub") else {
            panic!("expected an error")
        };
        assert_eq!(part, "ui");
        assert!(!is_link(&e), "{e}");
    }
    #[test]
    fn ac3_dir_root_may_be_a_link() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(t.path().join("real/ui")).unwrap();
        std::fs::write(t.path().join("real/ui/a"), "x").unwrap();
        symlink("real", t.path().join("root")).unwrap();
        let d = dir(&t.path().join("root"), "ui").unwrap();
        let list = entries(&d).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].1.kind, Kind::File);
    }
}
