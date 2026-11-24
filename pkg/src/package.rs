//! The package (TECHNICAL 6): its content, paths and byte format, and pack,
//! decode and unpack. Untrusted bytes are parsed only after their size is checked.

use crate::{Mod, ModError, PackageId};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const MAX_PACKAGE_BYTES: u64 = 67_108_864;
const MAX_FILES: usize = 4096;
const HEADER: &[u8] = b"WATANEPK\x01";

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("the package is {size} bytes; the limit is 67108864")]
    TooLarge { size: u64 },
    #[error("too many files; the limit is 4096")]
    TooManyFiles,
    #[error("bad package format: {0}")]
    Format(String),
    #[error("path `{}` is not allowed: {reason}", path.escape_debug())]
    Path { path: String, reason: &'static str },
    #[error("`{}`: {}", path.escape_debug(), crate::refusal::link(path))]
    Symlink { path: String },
    #[error("{} exists already", path.display())]
    DestExists { path: PathBuf },
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Mod(#[from] ModError),
}

fn io(path: &Path, source: std::io::Error) -> PackageError {
    PackageError::Io {
        path: path.into(),
        source,
    }
}
fn path_error(path: &str, reason: &'static str) -> PackageError {
    PackageError::Path {
        path: path.into(),
        reason,
    }
}
fn check_size(size: u64) -> Result<(), PackageError> {
    if size > MAX_PACKAGE_BYTES {
        Err(PackageError::TooLarge { size })
    } else {
        Ok(())
    }
}
/// Win32 treats these stems as devices even if a filename has an extension.
pub(crate) fn windows_device(segment: &str) -> bool {
    let stem = segment.split_once('.').map_or(segment, |(stem, _)| stem);
    matches!(
        stem.to_ascii_uppercase().as_bytes(),
        b"CON"
            | b"PRN"
            | b"AUX"
            | b"NUL"
            | [b'C', b'O', b'M', b'1'..=b'9']
            | [b'L', b'P', b'T', b'1'..=b'9']
    )
}

fn grammar(path: &str) -> Result<(), PackageError> {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    if path.is_empty() || path.len() > 255 || path.split('/').count() > 16 {
        return Err(path_error(
            path,
            "use 1..=255 bytes and at most 16 segments",
        ));
    }
    for s in path.split('/') {
        if s.is_empty()
            || s.len() > 128
            || !s.bytes().next().is_some_and(first)
            || !s.bytes().all(|b| first(b) || b == b'.')
        {
            return Err(path_error(
                path,
                "each segment must match [A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}",
            ));
        }
        if windows_device(s) {
            return Err(path_error(path, "a Windows device name is reserved"));
        }
    }
    Ok(())
}

// None means outside the allowlist; Some(true) marks source content.
fn class(path: &str) -> Option<bool> {
    let parts: Vec<_> = path.split('/').collect();
    match parts.as_slice() {
        ["mod.toml" | "deps.json" | "logic.wasm"] => Some(false),
        ["ui", _, ..] => Some(false),
        ["wit", file] | ["wit", "deps", file] | ["wit", "deps", _, file]
            if file.ends_with(".wit") =>
        {
            Some(false)
        }
        ["logic", "Cargo.toml" | "Cargo.lock"] | ["logic", "src", _, ..] | ["tests", _, ..] => {
            Some(true)
        }
        _ => None,
    }
}
fn directory(path: &str) -> bool {
    matches!(
        path,
        "ui" | "wit" | "wit/deps" | "logic" | "logic/src" | "tests"
    ) || path.starts_with("ui/")
        || path.starts_with("logic/src/")
        || path.starts_with("tests/")
        || (path.starts_with("wit/deps/") && path.split('/').count() == 3)
}

// Track directories too: a case-folded file must never be an ancestor.
#[derive(Default)]
struct Paths(BTreeMap<String, bool>); // true = file; false = directory
impl Paths {
    fn insert(&mut self, path: &str) -> Result<(), PackageError> {
        grammar(path)?;
        if class(path).is_none() {
            return Err(path_error(path, "not a package content path (section 6.1)"));
        }
        let lower = path.to_ascii_lowercase();
        for (i, _) in lower.match_indices('/') {
            if self.0.get(&lower[..i]) == Some(&true) {
                return Err(path_error(path, "a parent path is already a file"));
            }
            self.0.entry(lower[..i].into()).or_insert(false);
        }
        if self.0.insert(lower, true).is_some() {
            return Err(path_error(
                path,
                "duplicate, ASCII case clash, or file/directory clash",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Package {
    bytes: Vec<u8>,
    files: BTreeMap<String, Range<usize>>,
    id: PackageId,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, size: usize, field: &str) -> Result<&'a [u8], PackageError> {
        if size > self.bytes.len() - self.pos {
            return Err(PackageError::Format(format!("short {field}")));
        }
        let start = self.pos;
        self.pos += size;
        Ok(&self.bytes[start..self.pos])
    }
    fn u16(&mut self, field: &str) -> Result<usize, PackageError> {
        let b = self.take(2, field)?;
        Ok(u16::from_be_bytes([b[0], b[1]]) as usize)
    }
    fn u32(&mut self, field: &str) -> Result<usize, PackageError> {
        let b = self.take(4, field)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
    }
}

impl Package {
    /// Decode bounded canonical bytes without allocating from declared lengths.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Package, PackageError> {
        check_size(bytes.len() as u64)?;
        let mut r = Reader {
            bytes: &bytes,
            pos: 0,
        };
        if r.take(9, "header")? != HEADER {
            return Err(PackageError::Format(
                "expected WATANEPK and format 01".into(),
            ));
        }
        let count = r.u32("count")?;
        if count == 0 {
            return Err(PackageError::Format("count must be at least 1".into()));
        }
        if count > MAX_FILES {
            return Err(PackageError::TooManyFiles);
        }
        let mut files: BTreeMap<String, Range<usize>> = BTreeMap::new();
        let mut paths = Paths::default();
        for _ in 0..count {
            let len = r.u16("path length")?;
            let raw_path = r.take(len, "path")?;
            let path = std::str::from_utf8(raw_path).map_err(|_| {
                path_error(&String::from_utf8_lossy(raw_path), "path must be ASCII")
            })?;
            paths.insert(path)?;
            if files
                .last_key_value()
                .is_some_and(|(previous, _)| previous.as_str() >= path)
            {
                return Err(PackageError::Format(format!(
                    "path `{path}` is not strictly ascending"
                )));
            }
            let size = r.u32(&format!("size for `{path}`"))?;
            let start = r.pos;
            r.take(size, &format!("data for `{path}`"))?;
            files.insert(path.into(), start..r.pos);
        }
        if r.pos != bytes.len() {
            return Err(PackageError::Format("trailing bytes".into()));
        }
        let id = PackageId::of(&bytes);
        Ok(Self { bytes, files, id })
    }
    pub fn id(&self) -> PackageId {
        self.id.clone()
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    #[expect(
        clippy::len_without_is_empty,
        reason = "the fixed package API has no is_empty: the format requires at least one record"
    )]
    pub fn len(&self) -> u64 {
        self.bytes.len() as u64
    }
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(|range| &self.bytes[range.clone()])
    }
    pub fn has_source(&self) -> bool {
        self.paths().any(|path| class(path) == Some(true))
    }

    /// Write to a newly created directory; remove it if validation fails.
    pub fn unpack(&self, dest: &Path) -> Result<Mod, PackageError> {
        match create_dir(dest) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PackageError::DestExists { path: dest.into() });
            }
            Err(e) => return Err(io(dest, e)),
        }
        let result = (|| {
            permissions(dest, 0o755)?;
            for (path, range) in &self.files {
                let mut parent = dest.to_path_buf();
                let parts: Vec<_> = path.split('/').collect();
                for part in &parts[..parts.len() - 1] {
                    parent.push(part);
                    match create_dir(&parent) {
                        Ok(()) => permissions(&parent, 0o755)?,
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                            let m = fs::symlink_metadata(&parent).map_err(|e| io(&parent, e))?;
                            if !m.is_dir() || m.file_type().is_symlink() {
                                return Err(io(&parent, e));
                            }
                        }
                        Err(e) => return Err(io(&parent, e)),
                    }
                }
                let path = dest.join(path);
                let mut options = fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o644)
                };
                let mut file = options.open(&path).map_err(|e| io(&path, e))?;
                permissions(&path, 0o644)?;
                file.write_all(&self.bytes[range.clone()])
                    .map_err(|e| io(&path, e))?;
            }
            let m = Mod::read(dest)?;
            m.check_component()?;
            Ok(m)
        })();
        finish_unpack(dest, result)
    }
}

fn finish_unpack(dest: &Path, result: Result<Mod, PackageError>) -> Result<Mod, PackageError> {
    if result.is_err() {
        // Cleanup is best-effort: its failure must not hide the unpack error.
        let _ = fs::remove_dir_all(dest);
    }
    result
}

fn pack_error(root: &Path, part: String, e: std::io::Error) -> PackageError {
    if crate::open::is_link(&e) {
        PackageError::Symlink { path: part }
    } else if e.kind() == std::io::ErrorKind::IsADirectory {
        path_error(&part, crate::refusal::DIRECTORY)
    } else if e.kind() == std::io::ErrorKind::NotFound {
        path_error(&part, crate::refusal::CHANGED)
    } else if e.kind() == std::io::ErrorKind::InvalidInput || crate::open::is_socket(&e) {
        path_error(&part, crate::refusal::SPECIAL)
    } else if e.kind() == std::io::ErrorKind::NotADirectory {
        path_error(&part, "expected a regular file or a content directory")
    } else {
        io(&root.join(part), e)
    }
}
// A vanished child is named when its stat failed. Other listing errors name the directory.
fn listing(root: &Path, relative: &str, e: crate::open::ListingError) -> PackageError {
    if e.source.kind() == std::io::ErrorKind::NotFound {
        path_error(&e.relative_path(relative), crate::refusal::CHANGED)
    } else {
        io(&root.join(relative), e.source)
    }
}
fn open_pack_file(root: &Path, relative: &str) -> Result<fs::File, PackageError> {
    crate::open::file(root, relative).map_err(|(part, e)| pack_error(root, part, e))
}
fn open_pack_dir(root: &Path, relative: &str) -> Result<crate::open::Dir, PackageError> {
    crate::open::dir(root, relative).map_err(|(part, e)| pack_error(root, part, e))
}
// None when the entry is absent; a link or special part on the way is refused.
fn lstat(root: &Path, relative: &str) -> Result<Option<crate::open::Stat>, PackageError> {
    let (parent, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    let dir = match crate::open::dir(root, parent) {
        Ok(dir) => dir,
        Err((_, e)) if e.kind() == std::io::ErrorKind::NotFound && !crate::open::is_link(&e) => {
            return Ok(None);
        }
        Err((part, e)) => return Err(pack_error(root, part, e)),
    };
    match crate::open::stat(&dir, name) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(&root.join(relative), e)),
    }
}

fn create_dir(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o755).create(path)
    }
    #[cfg(windows)]
    fs::create_dir(path)
}

// Set the final mode explicitly so a restrictive umask cannot change the result.
fn permissions(path: &Path, mode: u32) -> Result<(), PackageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| io(path, e))?
    };
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

// Hidden content is omitted, but every descendant must still be link-free.
fn hidden_links(root: &Path, relative: &str) -> Result<(), PackageError> {
    use crate::open::Kind;
    let mut stack = vec![relative.to_string()];
    while let Some(relative) = stack.pop() {
        let dir = open_pack_dir(root, &relative)?;
        for crate::open::Entry { name, stat, .. } in
            crate::open::entries(&dir).map_err(|e| listing(root, &relative, e))?
        {
            let path = format!("{relative}/{}", name.to_string_lossy());
            match stat.kind {
                Kind::Link => return Err(PackageError::Symlink { path }),
                Kind::Dir => stack.push(path),
                _ => {}
            }
        }
    }
    Ok(())
}

struct Input {
    files: BTreeSet<String>,
    size: u64,
}
impl Input {
    fn walk(
        &mut self,
        root: &Path,
        relative: &str,
        m: crate::open::Stat,
        listed_as_file: bool,
    ) -> Result<(), PackageError> {
        use crate::open::Kind;
        // Filter before descending: build output and unrelated files are never entered.
        let allow_directory = directory(relative);
        let allow_file = class(relative).is_some();
        if !allow_directory && !allow_file {
            return Ok(());
        }
        if m.kind == Kind::Link {
            return Err(PackageError::Symlink {
                path: relative.into(),
            });
        }
        grammar(relative)?;
        if m.kind == Kind::Dir {
            if listed_as_file || !allow_directory {
                return Err(path_error(relative, crate::refusal::DIRECTORY));
            }
            let dir = open_pack_dir(root, relative)?;
            for crate::open::Entry {
                name,
                stat,
                listed_as_file,
            } in crate::open::entries(&dir).map_err(|e| listing(root, relative, e))?
            {
                if name.to_string_lossy().starts_with('.') {
                    let path = format!("{relative}/{}", name.to_string_lossy());
                    match stat.kind {
                        Kind::Link => return Err(PackageError::Symlink { path }),
                        Kind::Dir => hidden_links(root, &path)?,
                        _ => {}
                    }
                    continue;
                }
                let name = name.to_str().ok_or_else(|| {
                    path_error(
                        &format!("{relative}/{}", name.to_string_lossy()),
                        "path must be ASCII",
                    )
                })?;
                self.walk(root, &format!("{relative}/{name}"), stat, listed_as_file)?;
            }
        } else if m.kind == Kind::File && !allow_file {
            // A regular file beside the content, such as wit/deps/README.md, is not
            // package content (section 6.1) and Mod::read ignores it (section 4.7).
        } else if m.kind == Kind::File {
            if self.files.len() == MAX_FILES {
                return Err(PackageError::TooManyFiles);
            }
            self.size = self
                .size
                .saturating_add(6 + relative.len() as u64)
                .saturating_add(m.size);
            check_size(self.size)?;
            self.files.insert(relative.into());
        } else {
            return Err(path_error(relative, crate::refusal::SPECIAL));
        }
        Ok(())
    }
}

/// Pack the allowlisted files of a checked, built mod into canonical bytes.
pub fn pack(dir: &Path, include_source: bool) -> Result<Package, PackageError> {
    let mut input = Input {
        files: BTreeSet::new(),
        size: 13,
    };
    for path in ["mod.toml", "deps.json", "logic.wasm", "wit", "ui"] {
        if let Some(m) = lstat(dir, path)? {
            input.walk(dir, path, m, false)?;
        }
    }
    if include_source {
        for path in ["logic", "tests"] {
            if let Some(m) = lstat(dir, path)? {
                input.walk(dir, path, m, false)?;
            }
        }
    }
    let mut paths = Paths::default();
    for path in &input.files {
        paths.insert(path)?;
    }
    let m = Mod::read(dir)?;
    m.check_component()?;
    let mut bytes = HEADER.to_vec();
    bytes.extend_from_slice(&(input.files.len() as u32).to_be_bytes());
    for path in &input.files {
        bytes.extend_from_slice(&(path.len() as u16).to_be_bytes());
        bytes.extend_from_slice(path.as_bytes());
        let size_offset = bytes.len();
        bytes.extend_from_slice(&[0; 4]);
        check_size(bytes.len() as u64)?;
        let full_path = dir.join(path);
        // Bound the read independently of metadata: a growing file cannot bypass the cap.
        open_pack_file(dir, path)?
            .take(MAX_PACKAGE_BYTES - bytes.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io(&full_path, e))?;
        check_size(bytes.len() as u64)?;
        let size = (bytes.len() - size_offset - 4) as u32;
        bytes[size_offset..size_offset + 4].copy_from_slice(&size.to_be_bytes());
    }
    Package::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac3_s15_error_display() {
        for (path, escaped) in [
            ("ui/a\0b", r"ui/a\0b"),
            ("ui/a\nb", r"ui/a\nb"),
            ("ui/a\x1bb", r"ui/a\u{1b}b"),
            ("ui/a\x7fb", r"ui/a\u{7f}b"),
            ("ui/file.js", "ui/file.js"),
        ] {
            let link = PackageError::Symlink { path: path.into() };
            assert_eq!(
                link.to_string(),
                format!(
                    "`{escaped}`: a symbolic link; replace it with a regular file or directory"
                )
            );
            assert!(matches!(link, PackageError::Symlink { path: raw } if raw == path));
            let error = path_error(path, "test rule");
            assert_eq!(
                error.to_string(),
                format!("path `{escaped}` is not allowed: test rule")
            );
            assert!(matches!(error, PackageError::Path { path: raw, .. } if raw == path));
        }
    }

    #[test]
    fn ac4_s15_unpack_error_survives_failed_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        // remove_dir_all deterministically fails on a regular file, even as root.
        let dest = tmp.path().join("not-a-directory");
        fs::write(&dest, b"keep").unwrap();
        assert!(fs::remove_dir_all(&dest).is_err());
        let original = PackageError::Mod(ModError::Invalid(vec![crate::Violation {
            rule: crate::Rule::ModToml,
            file: "mod.toml".into(),
            message: "invalid manifest".into(),
        }]));
        let error = finish_unpack(&dest, Err(original)).unwrap_err();
        assert!(
            matches!(error, PackageError::Mod(ModError::Invalid(v)) if v.len() == 1
            && v[0].rule == crate::Rule::ModToml && v[0].file == "mod.toml"
            && v[0].message == "invalid manifest")
        );
    }

    #[cfg(unix)]
    #[test]
    fn ac2_s22_every_part_no_follow() {
        use std::os::unix::fs::symlink;
        let tmp = tempfile::tempdir().unwrap();
        let real = tempfile::tempdir().unwrap();
        fs::write(real.path().join("index.html"), "ok").unwrap();
        for (link, file) in [
            ("ui", "ui/index.html"),
            ("wit/deps/x", "wit/deps/x/index.html"),
        ] {
            let path = tmp.path().join(link);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            symlink(real.path(), &path).unwrap();
            assert!(
                matches!(open_pack_file(tmp.path(), file), Err(PackageError::Symlink { path }) if path == link)
            );
        }
        let fifo = tmp.path().join("pipe");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        for file in ["pipe", "pipe/child"] {
            assert!(matches!(
                open_pack_file(tmp.path(), file),
                Err(PackageError::Path { .. })
            ));
        }
        let root = tmp.path().join("root");
        symlink(real.path(), &root).unwrap();
        open_pack_file(&root, "index.html").unwrap();
    }

    #[cfg(unix)]
    fn open_fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("ui")).unwrap();
        fs::write(tmp.path().join("ui/file"), b"content").unwrap();
        open_pack_file(tmp.path(), "ui/file").unwrap();
        tmp
    }

    #[cfg(unix)]
    fn fifo(root: &Path) {
        assert!(
            std::process::Command::new("mkfifo")
                .arg(root.join("ui/pipe"))
                .status()
                .unwrap()
                .success()
        );
    }

    #[cfg(unix)]
    #[test]
    fn ac5_s15_no_follow_link_to_file() {
        let tmp = open_fixture();
        std::os::unix::fs::symlink(tmp.path().join("ui/file"), tmp.path().join("ui/link")).unwrap();
        let error = open_pack_file(tmp.path(), "ui/link").unwrap_err();
        assert!(matches!(error, PackageError::Symlink { path } if path == "ui/link"));
    }

    #[cfg(unix)]
    #[test]
    fn ac5_s15_no_follow_link_to_fifo() {
        let tmp = open_fixture();
        fifo(tmp.path());
        // No writer: following the FIFO with a blocking open would hang.
        std::os::unix::fs::symlink(tmp.path().join("ui/pipe"), tmp.path().join("ui/link")).unwrap();
        let error = open_pack_file(tmp.path(), "ui/link").unwrap_err();
        assert!(matches!(error, PackageError::Symlink { path } if path == "ui/link"));
    }

    #[cfg(unix)]
    #[test]
    fn ac5_s15_no_follow_fifo() {
        let tmp = open_fixture();
        fifo(tmp.path());
        let error = open_pack_file(tmp.path(), "ui/pipe").unwrap_err();
        assert!(matches!(error, PackageError::Path { path, .. } if path == "ui/pipe"));
    }

    #[cfg(unix)]
    fn path_reason(error: PackageError, path: &str) -> &'static str {
        match error {
            PackageError::Path { path: p, reason } if p == path => reason,
            other => panic!("expected a path refusal, got {other:?}"),
        }
    }
    #[cfg(unix)]
    #[test]
    fn ac2_s91_file_gone_before_open_is_changed() {
        let tmp = open_fixture();
        fs::remove_file(tmp.path().join("ui/file")).unwrap();
        let error = open_pack_file(tmp.path(), "ui/file").unwrap_err();
        assert_eq!(path_reason(error, "ui/file"), crate::refusal::CHANGED);
    }
    #[cfg(unix)]
    #[test]
    fn ac3_s91_file_replaced_by_directory_names_a_directory() {
        let tmp = open_fixture();
        fs::remove_file(tmp.path().join("ui/file")).unwrap();
        fs::create_dir(tmp.path().join("ui/file")).unwrap();
        let error = open_pack_file(tmp.path(), "ui/file").unwrap_err();
        assert_eq!(path_reason(error, "ui/file"), crate::refusal::DIRECTORY);
        // A real FIFO opened by pack still names a special file.
        fifo(tmp.path());
        let error = open_pack_file(tmp.path(), "ui/pipe").unwrap_err();
        assert_eq!(path_reason(error, "ui/pipe"), crate::refusal::SPECIAL);
    }
    #[cfg(unix)]
    #[test]
    fn ac4_s91_open_file_tells_directory_from_special() {
        let tmp = open_fixture();
        fifo(tmp.path());
        let Err((_, dir)) = crate::open::file(tmp.path(), "ui") else {
            panic!("a directory is not a file")
        };
        assert_eq!(dir.kind(), std::io::ErrorKind::IsADirectory);
        let Err((_, pipe)) = crate::open::file(tmp.path(), "ui/pipe") else {
            panic!("a FIFO is not a file")
        };
        assert_eq!(pipe.kind(), std::io::ErrorKind::InvalidInput);
    }
    #[test]
    fn ac2_s108_directory_at_a_file_only_path() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("mod.toml")).unwrap();
        let error = pack(tmp.path(), false).unwrap_err();
        assert!(matches!(error, PackageError::Path { path, reason }
            if path == "mod.toml" && reason == "expected a regular file, found a directory"
        ));
    }
    #[cfg(unix)]
    fn pack_with_listing_change(
        relative: &str,
        listing: usize,
        change: impl Fn(&Path) + 'static,
    ) -> PackageError {
        let tmp = open_fixture();
        for (path, bytes) in [
            (
                "mod.toml",
                &include_bytes!("../tests/fixtures/hello/mod.toml")[..],
            ),
            (
                "ui/index.html",
                &include_bytes!("../tests/fixtures/hello/ui/index.html")[..],
            ),
            (
                "wit/world.wit",
                &include_bytes!("../tests/fixtures/hello/wit/world.wit")[..],
            ),
        ] {
            let path = tmp.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        let path = tmp.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "listed file").unwrap();
        let seen = std::cell::Cell::new(0);
        crate::open::BEFORE_ENTRY_STAT.with(|hook| {
            hook.replace(Some(Box::new(move |name| {
                if path.file_name() == Some(name) {
                    seen.set(seen.get() + 1);
                    if seen.get() == listing {
                        change(&path);
                    }
                }
            })));
        });
        let result = pack(tmp.path(), false);
        crate::open::BEFORE_ENTRY_STAT.with(|hook| hook.replace(None));
        result.unwrap_err()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn ac2_s108_listed_file_replaced_by_a_directory() {
        let error = pack_with_listing_change("ui/entry", 1, |path| {
            fs::remove_file(path).unwrap();
            fs::create_dir(path).unwrap();
        });
        assert_eq!(
            path_reason(error, "ui/entry"),
            "expected a regular file, found a directory"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ac3_s108_listing_entry_gone_is_changed() {
        for relative in ["ui/entry", "ui/.hidden/entry"] {
            let error =
                pack_with_listing_change(relative, 1, |path| fs::remove_file(path).unwrap());
            assert_eq!(
                path_reason(error, relative),
                "changed while the mod was read; read it again"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn regress_s108_pack_entry_gone_in_read_listing_named_at_entry() {
        // The first listing belongs to pack. The second belongs to Mod::read.
        let error = pack_with_listing_change("ui/entry", 2, |path| fs::remove_file(path).unwrap());
        let PackageError::Mod(ModError::Invalid(violations)) = error else {
            panic!("expected a mod validation refusal, got {error:?}");
        };
        assert_eq!(
            violations,
            [crate::Violation {
                rule: crate::Rule::Path,
                file: "ui/entry".into(),
                message: "changed while the mod was read; read it again".into(),
            }]
        );
    }

    #[test]
    fn listing_permission_denied_stays_io() {
        let denied = listing(
            Path::new("/m"),
            "ui",
            crate::open::ListingError {
                name: Some("entry".into()),
                source: std::io::ErrorKind::PermissionDenied.into(),
            },
        );
        assert!(
            matches!(denied, PackageError::Io { ref path, .. }
                if path == Path::new("/m/ui")
            ),
            "{denied:?}"
        );
    }
}
