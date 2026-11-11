mod entry;
mod identity;
mod key;
use ed25519_dalek::SigningKey;
pub use entry::{DecodeError, Entry, EntryId, IdTextError};
pub use key::{KeyTextError, PublicKey};
use std::{
    fmt,
    path::{Path, PathBuf},
};
pub const MAX_KIND: usize = 32;
pub const MAX_BODY: usize = 65_536;
pub const MAX_ENTRY: usize = 65_728;
pub const MAX_SEQ: u64 = (1 << 53) - 1;
pub const BACKUP_LEN: usize = 108;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "exposed: {path} has mode {mode:04o}; group and others must have no access (chmod 700 for the directory, 600 for the key)"
    )]
    Exposed { path: PathBuf, mode: u32 },
    #[error("{path} is not a regular file or directory (a symbolic link is refused)")]
    NotPlain { path: PathBuf },
    #[error("corrupt key file {path}: it must hold exactly 32 bytes")]
    CorruptKey { path: PathBuf },
}
pub struct Log {
    key: SigningKey,
}
impl Log {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        Ok(Self {
            key: identity::open(dir)?,
        })
    }
    pub fn identity(&self) -> PublicKey {
        PublicKey::from_bytes(self.key.verifying_key().as_bytes()).expect("generated public key")
    }
}
impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Log {{ identity: {} }}", self.identity())
    }
}
