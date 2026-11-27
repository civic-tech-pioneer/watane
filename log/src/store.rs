//! The SQLite channel store (TECHNICAL 4.8): append, ingest, reads and fork
//! detection over `channels.db`.

use crate::{Entry, Error, Fork, Ingest, MAX_SEQ, PublicKey, Status, entry};
use ed25519_dalek::SigningKey;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::path::Path;
use std::time::{Duration, Instant};
pub(crate) fn open(dir: &Path, anchor: &crate::identity::Anchor) -> Result<Connection, Error> {
    let path = dir.join("channels.db");
    // Unix creates the file relative to the checked directory handle.
    #[cfg(unix)]
    let create = rustix::fs::openat(
        anchor,
        "channels.db",
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(std::io::Error::from);
    #[cfg(windows)]
    let create = {
        let _ = anchor;
        watane_log_windows_fs::create_file(&path)
    };
    match create {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => return Err(Error::Io { path, source }),
    }
    let mut db = Connection::open(&path)?;
    db.busy_timeout(Duration::from_secs(5))?;
    check_version(&db)?;
    let start = Instant::now();
    loop {
        match db.pragma_update(None, "journal_mode", "WAL") {
            Ok(()) => break,
            Err(rusqlite::Error::SqliteFailure(e, _))
                if matches!(
                    e.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                ) && start.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(e.into()),
        }
    }
    db.pragma_update(None, "synchronous", "FULL")?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if check_version(&tx)? == 0 {
        tx.execute_batch(include_str!("schema.sql"))?;
    }
    tx.commit()?;
    Ok(db)
}
fn check_version(db: &Connection) -> Result<i64, Error> {
    // Read both facts in one SQLite snapshot while another first open may create the schema.
    let (version, tables): (i64, i64) = db.query_row(
        "SELECT user_version, (SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%') FROM pragma_user_version",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if version != 1 && !(version == 0 && tables == 0) {
        return Err(Error::Storage(format!(
            "unsupported schema version {version}"
        )));
    }
    Ok(version)
}
fn corrupt(author: &PublicKey, seq: u64, cause: impl ToString) -> Error {
    Error::CorruptStore {
        author: *author,
        seq,
        cause: cause.to_string(),
    }
}
fn row(author: &PublicKey, r: &rusqlite::Row<'_>) -> Result<Entry, Error> {
    let seq: i64 = r.get(0)?;
    let id: Vec<u8> = r.get(1)?;
    let raw: Vec<u8> = r.get(2)?;
    let e = Entry::decode(&raw).map_err(|cause| corrupt(author, seq as u64, cause))?;
    if seq < 0
        || e.seq() != seq as u64
        || e.author() != *author
        || e.id().as_bytes().as_slice() != id
    {
        return Err(corrupt(
            author,
            seq as u64,
            "columns differ from signed bytes",
        ));
    }
    Ok(e)
}
pub(crate) fn entries(
    db: &Connection,
    author: &PublicKey,
    from: u64,
    limit: usize,
) -> Result<Vec<Entry>, Error> {
    if from > i64::MAX as u64 {
        return Ok(vec![]);
    }
    let mut stmt = db.prepare(
        "SELECT seq,id,raw FROM entries WHERE author=?1 AND seq>=?2 ORDER BY seq LIMIT ?3",
    )?;
    let mut rows = stmt.query(params![
        author.as_bytes(),
        from as i64,
        limit.min(i64::MAX as usize) as i64
    ])?;
    let mut out = vec![];
    while let Some(r) = rows.next()? {
        out.push(row(author, r)?);
    }
    Ok(out)
}
pub(crate) fn head(db: &Connection, author: &PublicKey) -> Result<Option<Entry>, Error> {
    let mut stmt =
        db.prepare("SELECT seq,id,raw FROM entries WHERE author=?1 ORDER BY seq DESC LIMIT 1")?;
    let mut rows = stmt.query([author.as_bytes()])?;
    rows.next()?.map(|r| row(author, r)).transpose()
}
pub(crate) fn channels(db: &Connection) -> Result<Vec<PublicKey>, Error> {
    let mut stmt = db.prepare("SELECT DISTINCT author FROM entries ORDER BY author")?;
    let mut rows = stmt.query([])?;
    let mut authors = vec![];
    while let Some(row) = rows.next()? {
        let bytes: Vec<u8> = row.get(0)?;
        let invalid =
            |cause| Error::Storage(format!("corrupt store: invalid channel author: {cause}"));
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| invalid("expected 32 bytes"))?;
        authors.push(PublicKey::from_bytes(&bytes).map_err(|_| invalid("invalid public key"))?);
    }
    Ok(authors)
}
pub(crate) fn at(db: &Connection, author: &PublicKey, seq: u64) -> Result<Option<Entry>, Error> {
    Ok(entries(db, author, seq, 1)?
        .into_iter()
        .next()
        .filter(|e| e.seq() == seq))
}
pub(crate) fn insert(db: &Connection, e: &Entry) -> Result<(), Error> {
    db.execute(
        "INSERT INTO entries(author,seq,id,raw) VALUES (?1,?2,?3,?4)",
        params![
            e.author().as_bytes(),
            e.seq() as i64,
            e.id().as_bytes(),
            e.bytes()
        ],
    )?;
    Ok(())
}
pub(crate) fn status(db: &Connection, author: &PublicKey) -> Result<Status, Error> {
    // Status must not hide damaged stored entries when there is no fork row.
    let mut stmt = db.prepare("SELECT seq,id,raw FROM entries WHERE author=?1 ORDER BY seq")?;
    let mut rows = stmt.query([author.as_bytes()])?;
    while let Some(r) = rows.next()? {
        row(author, r)?;
    }
    fork(db, author)
}
// The fork row alone, with its read check: the write paths need no scan of the channel.
pub(crate) fn fork(db: &Connection, author: &PublicKey) -> Result<Status, Error> {
    let proof: Option<(Vec<u8>, Vec<u8>)> = db
        .query_row(
            "SELECT chain,other FROM forks WHERE author=?1",
            [author.as_bytes()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((chain, other)) = proof else {
        return Ok(Status::Ok);
    };
    let chain = Entry::decode(&chain).map_err(|e| corrupt(author, 0, e))?;
    let other = Entry::decode(&other).map_err(|e| corrupt(author, chain.seq(), e))?;
    if chain.author() != *author
        || other.author() != *author
        || !((other.seq() == chain.seq() && other.id() != chain.id())
            || (other.seq() == chain.seq() + 1 && other.prev() != Some(chain.id())))
    {
        return Err(corrupt(author, chain.seq(), "invalid fork proof"));
    }
    Ok(Status::Compromised(Box::new(Fork { chain, other })))
}
pub(crate) fn confirmed(db: &Connection, author: &PublicKey) -> Result<bool, Error> {
    let value: Option<Vec<u8>> = db
        .query_row("SELECT value FROM meta WHERE key='unconfirmed'", [], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(value.as_deref() != Some(author.as_bytes().as_slice()))
}

pub(crate) fn append(
    tx: Transaction<'_>,
    key: &SigningKey,
    kind: &str,
    body: &[u8],
) -> Result<Entry, Error> {
    let author =
        PublicKey::from_bytes(key.verifying_key().as_bytes()).expect("generated public key");
    if matches!(fork(&tx, &author)?, Status::Compromised(_)) {
        return Err(Error::Compromised(author));
    }
    if !confirmed(&tx, &author)? {
        return Err(Error::OwnHeadUnconfirmed);
    }
    let head = head(&tx, &author)?;
    let seq = head.as_ref().map_or(0, |e| e.seq() + 1);
    if seq > MAX_SEQ {
        return Err(Error::ChannelFull);
    }
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(MAX_SEQ as u128) as u64;
    let entry = entry::sign_entry(key, seq, head.map(|e| e.id()), time, kind, body);
    insert(&tx, &entry)?;
    tx.commit()?;
    Ok(entry)
}

pub(crate) fn ingest(
    tx: Transaction<'_>,
    author: &PublicKey,
    entries: &[Entry],
) -> Result<Ingest, Error> {
    let status = fork(&tx, author)?;
    if matches!(status, Status::Compromised(_)) {
        return Ok(Ingest {
            stored: vec![],
            gap: None,
            status,
        });
    }
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by_key(|e| (e.seq(), e.id()));
    sorted.dedup_by_key(|e| e.id());
    let mut head = head(&tx, author)?;
    let mut stored = vec![];
    let mut gap = None;
    for e in sorted {
        let existing = at(&tx, author, e.seq())?;
        let conflict = if let Some(s) = existing {
            if s.id() == e.id() {
                continue;
            }
            Some(s)
        } else {
            let next = head.as_ref().map_or(0, |h| h.seq() + 1);
            if e.seq() > next {
                gap.get_or_insert(e.seq());
                continue;
            }
            if e.seq() != next {
                return Err(Error::Storage("non-contiguous channel".into()));
            }
            if e.prev() != head.as_ref().map(Entry::id) {
                head.clone()
            } else {
                None
            }
        };
        if let Some(chain) = conflict {
            tx.execute(
                "INSERT INTO forks(author,chain,other) VALUES (?1,?2,?3)",
                rusqlite::params![author.as_bytes(), chain.bytes(), e.bytes()],
            )?;
            break;
        }
        insert(&tx, e)?;
        stored.push(e.id());
        head = Some(e.clone());
    }
    let status = fork(&tx, author)?;
    tx.commit()?;
    Ok(Ingest {
        stored,
        gap,
        status,
    })
}
