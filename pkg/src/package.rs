use crate::{Mod, ModError, PackageId};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    ops::Range,
    path::{Path, PathBuf},
};

pub const MAX_PACKAGE_BYTES: u64 = 67_108_864;
const MAX_FILES: usize = 4096;
const HEADER: &[u8] = b"WATANEPK\x01";

#[derive(Debug, thiserror::Error)]
pub enum PackageError {
    #[error("the package is {size} bytes; the limit is 67108864")]
    TooLarge { size: u64 },
    #[error("bad package format: {0}")]
    Format(String),
    #[error("path `{path}` is not allowed: {reason}")]
    Path { path: String, reason: &'static str },
    #[error("`{path}` is a symbolic link; a package holds no links")]
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
        if !(1..=MAX_FILES).contains(&count) {
            return Err(PackageError::Format("count must be 1..=4096".into()));
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
    // The fixed package API has no is_empty: the format requires at least one record.
    #[allow(clippy::len_without_is_empty)]
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
                    options.mode(0o644);
                }
                let mut file = options.open(&path).map_err(|e| io(&path, e))?;
                permissions(&path, 0o644)?;
                file.write_all(&self.bytes[range.clone()])
                    .map_err(|e| io(&path, e))?;
            }
            let m = Mod::read(dest)?;
            m.check_component()?;
            Ok(m)
        })();
        if result.is_err() {
            fs::remove_dir_all(dest).map_err(|e| io(dest, e))?;
        }
        result
    }
}

fn create_dir(path: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o755);
    }
    builder.create(path)
}

// Set the final mode explicitly so a restrictive umask cannot change the result.
fn permissions(path: &Path, mode: u32) -> Result<(), PackageError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|e| io(path, e))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

struct Input {
    files: BTreeSet<String>,
    size: u64,
}
impl Input {
    fn walk(&mut self, root: &Path, relative: &str) -> Result<(), PackageError> {
        // Filter before stat: build output and unrelated files are never followed.
        if !directory(relative) && class(relative).is_none() {
            return Ok(());
        }
        let path = root.join(relative);
        let m = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(io(&path, e)),
        };
        if m.file_type().is_symlink() {
            return Err(PackageError::Symlink {
                path: relative.into(),
            });
        }
        grammar(relative)?;
        if m.is_dir() && directory(relative) {
            for entry in fs::read_dir(&path).map_err(|e| io(&path, e))? {
                let entry = entry.map_err(|e| io(&path, e))?;
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                let name = name.to_str().ok_or_else(|| {
                    path_error(
                        &format!("{relative}/{}", name.to_string_lossy()),
                        "path must be ASCII",
                    )
                })?;
                self.walk(root, &format!("{relative}/{name}"))?;
            }
        } else if m.is_file() && class(relative).is_some() {
            if self.files.len() == MAX_FILES {
                return Err(PackageError::Format("count must be 1..=4096".into()));
            }
            self.size = self
                .size
                .saturating_add(6 + relative.len() as u64)
                .saturating_add(m.len());
            check_size(self.size)?;
            self.files.insert(relative.into());
        } else {
            return Err(path_error(
                relative,
                "expected a regular file or a content directory",
            ));
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
        input.walk(dir, path)?;
    }
    if include_source {
        for path in ["logic", "tests"] {
            input.walk(dir, path)?;
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
        fs::File::open(&full_path)
            .map_err(|e| io(&full_path, e))?
            .take(MAX_PACKAGE_BYTES - bytes.len() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io(&full_path, e))?;
        check_size(bytes.len() as u64)?;
        let size = (bytes.len() - size_offset - 4) as u32;
        bytes[size_offset..size_offset + 4].copy_from_slice(&size.to_be_bytes());
    }
    Package::from_bytes(bytes)
}
