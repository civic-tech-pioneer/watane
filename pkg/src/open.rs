//! Open below a caller-supplied root without following any child link.
use std::{fs::File, io, path::Path};

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

pub(crate) fn file(root: &Path, relative: &str) -> Result<File, (String, io::Error)> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{AtFlags, FileType, Mode, OFlags, openat, statat};
        // The caller's root may itself contain links (e.g. /tmp on macOS).
        let mut parent = File::open(root).map_err(|e| (String::new(), e))?;
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
            let flags = OFlags::RDONLY
                | OFlags::CLOEXEC
                | OFlags::NOFOLLOW
                | if i + 1 == parts.len() {
                    OFlags::NONBLOCK
                } else {
                    OFlags::DIRECTORY
                };
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
        parent
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
