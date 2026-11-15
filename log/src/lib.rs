mod entry;
mod identity;
mod key;
mod store;
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

fn not_plain_text(path: &std::path::Path, expected: &str, link: bool) -> String {
    if link {
        format!(
            "{} is a symbolic link; a {expected} is required (a link is refused)",
            path.display()
        )
    } else {
        format!("{} is not a {expected}", path.display())
    }
}

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
    #[error("{}", not_plain_text(path, expected, *link))]
    NotPlain {
        path: PathBuf,
        expected: &'static str,
        link: bool,
    },
    #[error("the log directory path is empty")]
    EmptyPath,
    #[error("corrupt key file {path}: it must hold exactly 32 bytes")]
    CorruptKey { path: PathBuf },
    #[error("channel store: {0}")]
    Storage(String),
    #[error("corrupt store: entry {seq} of {author}: {cause}")]
    CorruptStore {
        author: PublicKey,
        seq: u64,
        cause: String,
    },
    #[error("invalid kind {0:?}: 1 to 32 bytes of [a-z0-9-], starting with a letter")]
    Kind(String),
    #[error("the body has {0} bytes; the limit is 65536")]
    BodyTooLarge(usize),
    #[error("the own channel is full")]
    ChannelFull,
    #[error("the channel of {0} is compromised; no entry can be added")]
    Compromised(PublicKey),
    #[error(
        "the own channel was restored; read its newest entries and confirm the head before you append"
    )]
    OwnHeadUnconfirmed,
    #[error("entry {index} has author {found}, not {expected}")]
    AuthorMismatch {
        index: usize,
        expected: PublicKey,
        found: PublicKey,
    },
    #[error("the passphrase is empty")]
    EmptyPassphrase,
    #[error("bad backup: {field}: expected {expected}, found {found}")]
    BadBackup {
        field: &'static str,
        expected: String,
        found: String,
    },
    #[error("wrong passphrase or damaged backup")]
    WrongPassphrase,
    #[error("the current identity {0} has signed entries; it cannot be replaced")]
    IdentityInUse(PublicKey),
    #[error("this home holds no identity; it can read and store channels but never sign")]
    NoIdentity,
    #[error("{path} holds an identity; a home with no identity has no key file")]
    HasIdentity { path: PathBuf },
}
pub struct Log {
    inner: std::sync::Mutex<Inner>,
}
impl Log {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let key = identity::open(dir)?;
        let db = store::open(dir)?;
        Ok(Self {
            inner: std::sync::Mutex::new(Inner {
                key: Some(key),
                db,
                dir: dir.to_owned(),
            }),
        })
    }
    pub fn open_keyless(dir: &Path) -> Result<Self, Error> {
        identity::open_keyless(dir)?;
        let db = store::open(dir)?;
        Ok(Self {
            inner: std::sync::Mutex::new(Inner {
                key: None,
                db,
                dir: dir.to_owned(),
            }),
        })
    }
    pub fn identity(&self) -> Option<PublicKey> {
        self.lock().identity()
    }
}
impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.identity() {
            Some(key) => write!(f, "Log {{ identity: {key} }}"),
            None => f.write_str("Log { keyless }"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    pub seq: u64,
    pub id: EntryId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingest {
    pub stored: Vec<EntryId>,
    pub gap: Option<u64>,
    pub status: Status,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    Compromised(Box<Fork>),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fork {
    pub chain: Entry,
    pub other: Entry,
}
struct Inner {
    key: Option<SigningKey>,
    db: rusqlite::Connection,
    dir: PathBuf,
}
impl Inner {
    fn identity(&self) -> Option<PublicKey> {
        self.key.as_ref().map(|key| {
            PublicKey::from_bytes(key.verifying_key().as_bytes()).expect("generated public key")
        })
    }
}
impl Log {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn append(&self, kind: &str, body: &[u8]) -> Result<Entry, Error> {
        let mut inner = self.lock();
        let Inner { key, db, .. } = &mut *inner;
        let key = key.as_ref().ok_or(Error::NoIdentity)?;
        if !entry::valid_kind(kind) {
            return Err(Error::Kind(kind.to_owned()));
        }
        if body.len() > MAX_BODY {
            return Err(Error::BodyTooLarge(body.len()));
        }
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        store::append(tx, key, kind, body)
    }
    pub fn ingest(&self, author: &PublicKey, entries: &[Entry]) -> Result<Ingest, Error> {
        for (index, e) in entries.iter().enumerate() {
            if e.author() != *author {
                return Err(Error::AuthorMismatch {
                    index,
                    expected: *author,
                    found: e.author(),
                });
            }
        }
        let mut inner = self.lock();
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        store::ingest(tx, author, entries)
    }
    pub fn entries(
        &self,
        author: &PublicKey,
        from_seq: u64,
        limit: usize,
    ) -> Result<Vec<Entry>, Error> {
        store::entries(&self.lock().db, author, from_seq, limit)
    }
    pub fn head(&self, author: &PublicKey) -> Result<Option<Head>, Error> {
        Ok(store::head(&self.lock().db, author)?.map(|e| Head {
            seq: e.seq(),
            id: e.id(),
        }))
    }
    pub fn status(&self, author: &PublicKey) -> Result<Status, Error> {
        store::status(&self.lock().db, author)
    }
    pub fn own_head_confirmed(&self) -> Result<bool, Error> {
        let inner = self.lock();
        match inner.identity() {
            Some(author) => store::confirmed(&inner.db, &author),
            None => Ok(true),
        }
    }
    pub fn confirm_own_head(&self) -> Result<(), Error> {
        let inner = self.lock();
        inner.key.as_ref().ok_or(Error::NoIdentity)?;
        inner
            .db
            .execute("DELETE FROM meta WHERE key='unconfirmed'", [])?;
        Ok(())
    }
    pub fn export_identity(&self, passphrase: &str) -> Result<Vec<u8>, Error> {
        let seed = zeroize::Zeroizing::new(
            self.lock()
                .key
                .as_ref()
                .ok_or(Error::NoIdentity)?
                .to_bytes(),
        );
        identity::export(&seed, passphrase)
    }
    pub fn restore_identity(&self, backup: &[u8], passphrase: &str) -> Result<PublicKey, Error> {
        self.lock().key.as_ref().ok_or(Error::NoIdentity)?;
        let key = identity::decrypt(backup, passphrase)?;
        let public =
            PublicKey::from_bytes(key.verifying_key().as_bytes()).expect("generated public key");
        let mut inner = self.lock();
        let current = inner.identity().ok_or(Error::NoIdentity)?;
        if public == current {
            return Ok(public);
        }
        let tx = inner
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if store::head(&tx, &current)?.is_some() {
            return Err(Error::IdentityInUse(current));
        }
        tx.execute("INSERT INTO meta(key,value) VALUES ('unconfirmed',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[public.as_bytes()])?;
        tx.commit()?;
        identity::replace(&inner.dir, &key)?;
        inner.key = Some(key);
        Ok(public)
    }
    pub fn drop_identity(&self) -> Result<(), Error> {
        let mut inner = self.lock();
        let author = inner.identity().ok_or(Error::NoIdentity)?;
        if store::head(&inner.db, &author)?.is_some() {
            return Err(Error::IdentityInUse(author));
        }
        let Inner { key, db, dir } = &mut *inner;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if store::head(&tx, &author)?.is_some() {
            return Err(Error::IdentityInUse(author));
        }
        tx.execute(
            "DELETE FROM meta WHERE key='unconfirmed' AND value=?1",
            [author.as_bytes()],
        )?;
        let path = dir.join("identity.key");
        std::fs::remove_file(&path).map_err(|source| Error::Io { path, source })?;
        // SigningKey zeroizes on drop. Stop signing even if the following sync fails.
        *key = None;
        std::fs::File::open(&*dir)
            .and_then(|f| f.sync_all())
            .map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
        tx.commit()?;
        Ok(())
    }
    pub fn channels(&self) -> Result<Vec<PublicKey>, Error> {
        store::channels(&self.lock().db)
    }
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Self::Storage(e.to_string())
    }
}
