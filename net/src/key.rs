use crate::error::NetError;
use ed25519_dalek::SigningKey;
use std::{
    fmt,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::Path,
    str::FromStr,
};
use watane_log::{KeyTextError, PublicKey};
use zeroize::Zeroizing;

const ALPHABET: &[u8; 32] = b"ybndrfg8ejkmcpqxot1uwisza345h769";

/// z-base-32 text of bytes (the same alphabet as log's `PublicKey`).
pub(crate) fn zbase32(bytes: &[u8]) -> String {
    (0..(bytes.len() * 8).div_ceil(5))
        .map(|i| {
            let mut n = 0usize;
            for j in 0..5 {
                let bit = i * 5 + j;
                n <<= 1;
                if bit < bytes.len() * 8 {
                    n |= ((bytes[bit / 8] >> (7 - bit % 8)) & 1) as usize;
                }
            }
            ALPHABET[n] as char
        })
        .collect()
}

/// Decode z-base-32 text of exactly `len` bytes. `None` for a char outside
/// the alphabet, a wrong length, or unused trailing bits that are not zero.
pub(crate) fn unzbase32(text: &str, len: usize) -> Option<Vec<u8>> {
    if text.len() != (len * 8).div_ceil(5) {
        return None;
    }
    let mut out = vec![0u8; len];
    for (i, c) in text.bytes().enumerate() {
        let n = ALPHABET.iter().position(|a| *a == c)? as u8;
        for j in 0..5 {
            let bit = i * 5 + j;
            let value = (n >> (4 - j)) & 1;
            if bit < len * 8 {
                out[bit / 8] |= value << (7 - bit % 8);
            } else if value != 0 {
                return None;
            }
        }
    }
    Some(out)
}

/// The Ed25519 key of a node. Text form: 52 characters of z-base-32, as
/// log's `PublicKey`. Short form: the first 8.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKey([u8; 32]);

impl NodeKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn short(&self) -> String {
        self.to_string()[..8].to_owned()
    }
    /// The same key as log's `PublicKey`, when it is a valid one.
    pub fn to_public(&self) -> Result<PublicKey, KeyTextError> {
        PublicKey::from_bytes(&self.0)
    }
}
impl From<PublicKey> for NodeKey {
    fn from(key: PublicKey) -> Self {
        Self(*key.as_bytes())
    }
}
impl fmt::Display for NodeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&zbase32(&self.0))
    }
}
impl fmt::Debug for NodeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeKey({self})")
    }
}
impl FromStr for NodeKey {
    type Err = KeyTextError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(PublicKey::from_str(s)?))
    }
}

fn io_error(path: &Path, e: io::Error) -> NetError {
    NetError::Io {
        path: path.to_owned(),
        cause: e.to_string(),
    }
}
fn corrupt(path: &Path, cause: impl Into<String>) -> NetError {
    NetError::Corrupt {
        path: path.to_owned(),
        cause: cause.into(),
    }
}
fn check(path: &Path, directory: bool) -> Result<(), NetError> {
    let meta = fs::symlink_metadata(path).map_err(|e| io_error(path, e))?;
    let plain = if directory {
        meta.is_dir()
    } else {
        meta.is_file()
    };
    if !plain {
        return Err(corrupt(
            path,
            if directory {
                "not a plain directory"
            } else {
                "not a regular file"
            },
        ));
    }
    let mode = meta.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(corrupt(path, format!("mode {mode:o} lets others in")));
    }
    Ok(())
}
fn read(path: &Path) -> Result<Zeroizing<[u8; 32]>, NetError> {
    check(path, false)?;
    let mut seed = Zeroizing::new([0u8; 32]);
    let mut file = File::open(path).map_err(|e| io_error(path, e))?;
    file.read_exact(&mut seed[..]).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            corrupt(path, "the key file is not 32 bytes")
        } else {
            io_error(path, e)
        }
    })?;
    let mut extra = [0];
    if file.read(&mut extra).map_err(|e| io_error(path, e))? != 0 {
        return Err(corrupt(path, "the key file is not 32 bytes"));
    }
    Ok(seed)
}

/// Open `dir` (create it, 0700) and `dir/node.key` (create it, 0600, the
/// 32-byte seed), with the rules of log's key file (log TECHNICAL 4.7).
pub(crate) fn open(dir: &Path) -> Result<Zeroizing<[u8; 32]>, NetError> {
    if dir.as_os_str().is_empty() {
        return Err(corrupt(dir, "the net directory path is empty"));
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
    let path = dir.join("node.key");
    match fs::symlink_metadata(&path) {
        Ok(_) => return read(&path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(&path, e)),
    }
    let mut seed = Zeroizing::new([0u8; 32]);
    getrandom::fill(&mut *seed).map_err(|e| io_error(&path, io::Error::other(e)))?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| io_error(&path, io::Error::other(e)))?;
    let tmp = dir.join(format!(".node.key.{}.tmp", zbase32(&random)));
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
    fs::remove_file(&tmp).map_err(|e| io_error(&tmp, e))?;
    result?;
    File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|e| io_error(dir, e))?;
    read(&path)
}

/// The public key of a seed.
pub(crate) fn public_of(seed: &[u8; 32]) -> NodeKey {
    NodeKey(SigningKey::from_bytes(seed).verifying_key().to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zbase32_roundtrip_and_unused_bits() {
        let bytes: Vec<u8> = (0..16).collect();
        let text = zbase32(&bytes);
        assert_eq!(text.len(), 26);
        assert_eq!(unzbase32(&text, 16).unwrap(), bytes);
        // the last char carries 3 bits; set a low bit that must be zero
        let mut bad = text.clone();
        bad.pop();
        bad.push('9'); // 31 = 0b11111
        assert_eq!(unzbase32(&bad, 16), None);
    }
}
