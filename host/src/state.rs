//! Mod state (TECHNICAL 7): the redb store keyed by identity, with per-call
//! transactions that enforce the storage limit and commit only on success.

use crate::{CallError, HostError, Limit, Limits};
use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, TableError,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SIZES: TableDefinition<&str, u64> = TableDefinition::new("sizes");
/// `<mod id>/<timer id>` -> JSON `{"due":<unix ms>,"origin":"<identity>"}` (TECHNICAL 7).
const TIMERS: TableDefinition<&str, &str> = TableDefinition::new("timers");
/// The `generation` key -> the host's start number (TECHNICAL 7, 19.1).
const HOST: TableDefinition<&str, u64> = TableDefinition::new("host");
/// A scratch workspace id -> the identity of its owner (TECHNICAL 7, 19.3).
const SCRATCH: TableDefinition<&str, &str> = TableDefinition::new("scratch");
/// A Claude Code session id -> the identity of the origin client whose run reported it
/// (TECHNICAL 7, 19.3, Sessions are the origin client's).
const SESSIONS: TableDefinition<&str, &str> = TableDefinition::new("sessions");

/// The writer's counters since open (TECHNICAL 19.9, `Host::test_stats`).
#[derive(Debug, Default)]
#[cfg_attr(
    not(feature = "test-streams"),
    allow(dead_code, reason = "read by Host::test_stats (19.9)")
)]
pub(crate) struct Stats {
    pub commits: AtomicU64,
    pub commit_nanos: AtomicU64,
    /// The commits of the host's own writes (timers, scratch rows, the generation, a removed mod's state), and their time (part of the two above).
    pub host_commits: AtomicU64,
    pub host_commit_nanos: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
}
/// One timer write of a chain's commit (19.6).
#[derive(Clone, Debug)]
pub(crate) enum TimerOp {
    After {
        key: String,
        due: u64,
        origin: String,
    },
    Cancel {
        key: String,
    },
    /// The row of a firing: deleted when it still has this due time.
    DeleteIfDue {
        key: String,
        due: u64,
    },
}
/// The first 32 hex chars of `blake3(identity)`: the id of a mod in the tables.
pub(crate) fn mod_id(identity: &str) -> String {
    blake3::hash(identity.as_bytes()).to_hex()[..32].to_string()
}
pub(crate) fn timer_key(identity: &str, id: &str) -> String {
    format!("{}/{id}", mod_id(identity))
}
fn parse_row(text: &str) -> Option<(u64, String)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    Some((
        v.get("due")?.as_u64()?,
        v.get("origin")?.as_str()?.to_owned(),
    ))
}
#[derive(Debug)]
pub(crate) struct State {
    path: PathBuf,
    db: Mutex<Option<Arc<Database>>>,
    #[cfg(all(test, unix))]
    home_lock_at_drop: std::sync::OnceLock<PathBuf>,
    pub stats: Arc<Stats>,
}
#[cfg(all(test, unix))]
impl Drop for State {
    fn drop(&mut self) {
        if let Some(path) = self.home_lock_at_drop.get() {
            // The drop body runs while the database is still open.
            assert!(self.db.get_mut().expect("state mutex").is_some());
            let lock = std::fs::File::open(path).expect("open host lock");
            assert!(matches!(
                lock.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
        }
    }
}

impl State {
    pub fn open(home: &Path) -> Result<Self, HostError> {
        let path = home.join("state/state.redb");
        let db = if path.exists() {
            Some(Arc::new(
                Database::open(&path).map_err(|e| HostError::Internal(e.to_string()))?,
            ))
        } else {
            None
        };
        Ok(Self {
            path,
            db: Mutex::new(db),
            #[cfg(all(test, unix))]
            home_lock_at_drop: std::sync::OnceLock::new(),
            stats: Arc::default(),
        })
    }
    fn database(&self, create: bool) -> Result<Option<Arc<Database>>, CallError> {
        let mut db = self
            .db
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if db.is_none() && create {
            std::fs::create_dir_all(self.path.parent().expect("database parent"))
                .map_err(CallError::internal)?;
            *db = Some(Arc::new(
                Database::create(&self.path).map_err(CallError::internal)?,
            ));
        }
        Ok(db.clone())
    }
    /// Raises the host's start number by one and returns it: 1 at the first open of a home
    /// (19.1, `host-info`). Durable before it returns.
    pub fn next_generation(&self) -> Result<u64, HostError> {
        let internal = |e: &dyn std::fmt::Display| HostError::Internal(e.to_string());
        let db = self
            .database(true)
            .map_err(|e| HostError::Internal(e.message))?
            .expect("created database");
        let txn = db.begin_write().map_err(|e| internal(&e))?;
        let next = {
            let mut table = txn.open_table(HOST).map_err(|e| internal(&e))?;
            let next = table
                .get("generation")
                .map_err(|e| internal(&e))?
                .map_or(0, |v| v.value())
                + 1;
            table.insert("generation", next).map_err(|e| internal(&e))?;
            next
        };
        self.commit_host(txn).map_err(|e| internal(&e))?;
        Ok(next)
    }
    /// The owner identity of a scratch workspace (19.3, Folders).
    pub fn scratch_owner(&self, id: &str) -> Result<Option<String>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(None);
        };
        let read = db.begin_read().map_err(CallError::internal)?;
        let table = match read.open_table(SCRATCH) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(CallError::internal(e)),
        };
        Ok(table
            .get(id)
            .map_err(CallError::internal)?
            .map(|v| v.value().to_owned()))
    }
    /// Records the owner of a new scratch workspace.
    #[cfg(any(unix, test))] // Scratch records belong to the Unix Claude runner.
    pub fn scratch_add(&self, id: &str, owner: &str) -> Result<(), CallError> {
        let db = self.database(true)?.expect("created database");
        let txn = db.begin_write().map_err(CallError::internal)?;
        txn.open_table(SCRATCH)
            .map_err(CallError::internal)?
            .insert(id, owner)
            .map_err(CallError::internal)?;
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// Deletes the row of one scratch workspace (19.3, Folders, Lifetime).
    #[cfg(any(unix, test))] // Scratch records belong to the Unix Claude runner.
    pub fn scratch_delete(&self, id: &str) -> Result<(), CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(());
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        txn.open_table(SCRATCH)
            .map_err(CallError::internal)?
            .remove(id)
            .map_err(CallError::internal)?;
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// Deletes the rows of the scratch workspaces that `identity` owns and returns their ids
    /// (4.4, remove).
    pub fn scratch_removed(&self, identity: &str) -> Result<Vec<String>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(vec![]);
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        let mut ids = vec![];
        {
            let mut table = txn.open_table(SCRATCH).map_err(CallError::internal)?;
            for row in table.iter().map_err(CallError::internal)? {
                let (key, value) = row.map_err(CallError::internal)?;
                if value.value() == identity {
                    ids.push(key.value().to_owned());
                }
            }
            for id in &ids {
                table.remove(id.as_str()).map_err(CallError::internal)?;
            }
        }
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(ids)
    }
    /// The identity that owns a session id (19.3, Sessions are the origin client's).
    pub fn session_owner(&self, id: &str) -> Result<Option<String>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(None);
        };
        let read = db.begin_read().map_err(CallError::internal)?;
        let table = match read.open_table(SESSIONS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(CallError::internal(e)),
        };
        Ok(table
            .get(id)
            .map_err(CallError::internal)?
            .map(|v| v.value().to_owned()))
    }
    /// Records that `owner`'s run reported session `id`. `Ok(false)` and no write when another
    /// identity holds the id.
    #[cfg_attr(
        windows,
        allow(dead_code, reason = "only the Unix Claude runner reads it")
    )]
    pub fn session_add(&self, id: &str, owner: &str) -> Result<bool, CallError> {
        let db = self.database(true)?.expect("created database");
        let txn = db.begin_write().map_err(CallError::internal)?;
        let free = {
            let mut table = txn.open_table(SESSIONS).map_err(CallError::internal)?;
            let held = table
                .get(id)
                .map_err(CallError::internal)?
                .map(|v| v.value().to_owned());
            match held {
                Some(other) if other != owner => false,
                Some(_) => true,
                None => {
                    table.insert(id, owner).map_err(CallError::internal)?;
                    true
                }
            }
        };
        if free {
            self.commit_host(txn).map_err(CallError::internal)?;
        }
        Ok(free)
    }
    /// Deletes the session rows of `identity` (4.4, remove).
    pub fn sessions_removed(&self, identity: &str) -> Result<(), CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(());
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        {
            let mut table = txn.open_table(SESSIONS).map_err(CallError::internal)?;
            let mut ids = vec![];
            for row in table.iter().map_err(CallError::internal)? {
                let (key, value) = row.map_err(CallError::internal)?;
                if value.value() == identity {
                    ids.push(key.value().to_owned());
                }
            }
            for id in &ids {
                table.remove(id.as_str()).map_err(CallError::internal)?;
            }
        }
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// The ids of the live timers of one mod (19.6).
    pub fn timer_ids(&self, identity: &str) -> Result<BTreeSet<String>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(BTreeSet::new());
        };
        let prefix = format!("{}/", mod_id(identity));
        let read = db.begin_read().map_err(CallError::internal)?;
        let table = match read.open_table(TIMERS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(BTreeSet::new()),
            Err(e) => return Err(CallError::internal(e)),
        };
        let mut ids = BTreeSet::new();
        for row in table
            .range(prefix.as_str()..)
            .map_err(CallError::internal)?
        {
            let (key, _) = row.map_err(CallError::internal)?;
            let Some(id) = key.value().strip_prefix(prefix.as_str()) else {
                break;
            };
            ids.insert(id.to_owned());
        }
        Ok(ids)
    }
    /// Every timer row: key, due, origin.
    pub fn timer_rows(&self) -> Result<Vec<(String, u64, String)>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(vec![]);
        };
        let read = db.begin_read().map_err(CallError::internal)?;
        let table = match read.open_table(TIMERS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(vec![]),
            Err(e) => return Err(CallError::internal(e)),
        };
        let mut rows = vec![];
        for row in table.iter().map_err(CallError::internal)? {
            let (key, value) = row.map_err(CallError::internal)?;
            if let Some((due, origin)) = parse_row(value.value()) {
                rows.push((key.value().to_owned(), due, origin));
            }
        }
        Ok(rows)
    }
    pub fn timer_row(&self, key: &str) -> Result<Option<(u64, String)>, CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(None);
        };
        let read = db.begin_read().map_err(CallError::internal)?;
        let table = match read.open_table(TIMERS) {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(CallError::internal(e)),
        };
        Ok(table
            .get(key)
            .map_err(CallError::internal)?
            .and_then(|v| parse_row(v.value())))
    }
    /// Commits a transaction of the host's own, counted apart as well (19.9).
    fn commit_host(&self, txn: redb::WriteTransaction) -> Result<(), redb::CommitError> {
        let started = Instant::now();
        txn.commit()?;
        self.stats.record_host(started);
        Ok(())
    }
    /// A write transaction that holds only timer operations.
    pub fn timer_write(&self, ops: &[TimerOp]) -> Result<(), CallError> {
        let Some(db) = self.database(true)? else {
            return Ok(());
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        apply_timer_ops(&txn, ops)?;
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// Like `timer_write`, with the operations chosen inside the write transaction, after
    /// `begin_write` (19.6: the check and the delete are one step).
    pub fn timer_write_with(&self, ops: &dyn Fn() -> Vec<TimerOp>) -> Result<(), CallError> {
        let Some(db) = self.database(true)? else {
            return Ok(());
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        apply_timer_ops(&txn, &ops())?;
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// Removal of a mod (4.4, 19.6): deletes its timers; every other timer that carries it as
    /// origin client gets its own mod's identity, `owners` mapping a mod id to its identity
    /// (`""`, which no mod has, when the owner is not installed).
    pub fn timers_removed(
        &self,
        identity: &str,
        owners: &BTreeMap<String, String>,
    ) -> Result<(), CallError> {
        let Some(db) = self.database(false)? else {
            return Ok(());
        };
        let own = format!("{}/", mod_id(identity));
        let txn = db.begin_write().map_err(CallError::internal)?;
        {
            let mut table = txn.open_table(TIMERS).map_err(CallError::internal)?;
            let mut delete = vec![];
            let mut rewrite = vec![];
            for row in table.iter().map_err(CallError::internal)? {
                let (key, value) = row.map_err(CallError::internal)?;
                let key = key.value().to_owned();
                if key.starts_with(&own) {
                    delete.push(key);
                } else if let Some((due, origin)) = parse_row(value.value())
                    && origin == identity
                {
                    let owner = owners.get(&key[..32]).cloned().unwrap_or_default();
                    rewrite.push((key, serde_json::json!({"due": due, "origin": owner})));
                }
            }
            for key in delete {
                table.remove(key.as_str()).map_err(CallError::internal)?;
            }
            for (key, row) in rewrite {
                table
                    .insert(key.as_str(), row.to_string().as_str())
                    .map_err(CallError::internal)?;
            }
        }
        self.commit_host(txn).map_err(CallError::internal)?;
        Ok(())
    }
    /// Holds no lock across `begin_write`: a depth-0 commit takes the catalog lock inside its
    /// write transaction, and a call start reads a timer row under the catalog lock (19.6).
    pub fn delete(&self, identity: &str) -> Result<(), HostError> {
        let Some(db) = self
            .database(false)
            .map_err(|e| HostError::Internal(e.message))?
        else {
            return Ok(());
        };
        let id = blake3::hash(identity.as_bytes()).to_hex()[..32].to_string();
        let txn = db
            .begin_write()
            .map_err(|e| HostError::Internal(e.to_string()))?;
        txn.delete_table(TableDefinition::<&str, &[u8]>::new(&format!("s/{id}")))
            .map_err(|e| HostError::Internal(e.to_string()))?;
        #[expect(
            clippy::semicolon_outside_block,
            reason = "the block ends the table borrow before txn.commit()"
        )]
        {
            let mut sizes = txn
                .open_table(SIZES)
                .map_err(|e| HostError::Internal(e.to_string()))?;
            sizes
                .remove(id.as_str())
                .map_err(|e| HostError::Internal(e.to_string()))?;
        }
        self.commit_host(txn)
            .map_err(|e| HostError::Internal(e.to_string()))?;
        Ok(())
    }
    /// Whether the table `s/<id>` of the identity holds at least one record: data that a remove
    /// kept (7). An empty table is no data.
    pub fn has_records(&self, identity: &str) -> Result<bool, HostError> {
        let Some(db) = self
            .database(false)
            .map_err(|e| HostError::Internal(e.message))?
        else {
            return Ok(false);
        };
        let read = db
            .begin_read()
            .map_err(|e| HostError::Internal(e.to_string()))?;
        let name = format!("s/{}", mod_id(identity));
        match read.open_table(TableDefinition::<&str, &[u8]>::new(&name)) {
            Ok(table) => Ok(!redb::ReadableTableMetadata::is_empty(&table)
                .map_err(|e| HostError::Internal(e.to_string()))?),
            Err(TableError::TableDoesNotExist(_)) => Ok(false),
            Err(e) => Err(HostError::Internal(e.to_string())),
        }
    }
    pub fn transaction(&self, identity: &str, limits: &Limits) -> Result<Transaction, CallError> {
        let db = self.database(true)?.expect("created database");
        Ok(Transaction {
            db,
            stats: Arc::clone(&self.stats),
            id: mod_id(identity),
            read: None,
            overlay: BTreeMap::new(),
            limits: limits.clone(),
            written: 0,
        })
    }
}
impl Stats {
    /// A commit of the host's own, counted apart as well.
    fn record_host(&self, started: Instant) {
        self.host_commits.fetch_add(1, Ordering::Relaxed);
        self.host_commit_nanos.fetch_add(
            u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.record(started);
    }
    fn record(&self, started: Instant) {
        self.commits.fetch_add(1, Ordering::Relaxed);
        self.commit_nanos.fetch_add(
            u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}
fn apply_timer_ops(txn: &redb::WriteTransaction, ops: &[TimerOp]) -> Result<(), CallError> {
    let mut table = txn.open_table(TIMERS).map_err(CallError::internal)?;
    for op in ops {
        match op {
            TimerOp::After { key, due, origin } => {
                let row = serde_json::json!({"due": due, "origin": origin}).to_string();
                table
                    .insert(key.as_str(), row.as_str())
                    .map_err(CallError::internal)?;
            }
            TimerOp::Cancel { key } => {
                table.remove(key.as_str()).map_err(CallError::internal)?;
            }
            TimerOp::DeleteIfDue { key, due } => {
                let current = table
                    .get(key.as_str())
                    .map_err(CallError::internal)?
                    .and_then(|v| parse_row(v.value()));
                if current.is_some_and(|(d, _)| d == *due) {
                    table.remove(key.as_str()).map_err(CallError::internal)?;
                }
            }
        }
    }
    Ok(())
}
pub(crate) struct Transaction {
    db: Arc<Database>,
    stats: Arc<Stats>,
    id: String,
    read: Option<ReadTransaction>,
    overlay: BTreeMap<String, Option<Vec<u8>>>,
    limits: Limits,
    written: usize,
}
impl Transaction {
    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>, CallError> {
        if let Some(value) = self.overlay.get(key) {
            return Ok(value.clone());
        }
        if self.read.is_none() {
            self.read = Some(self.db.begin_read().map_err(CallError::internal)?);
        }
        let name = format!("s/{}", self.id);
        let table = match self
            .read
            .as_ref()
            .expect("read snapshot")
            .open_table(TableDefinition::<&str, &[u8]>::new(&name))
        {
            Ok(t) => t,
            Err(TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(CallError::internal(e)),
        };
        let found = table
            .get(key)
            .map_err(CallError::internal)?
            .map(|v| v.value().to_vec());
        self.stats.bytes_read.fetch_add(
            (key.len() + found.as_ref().map_or(0, Vec::len)) as u64,
            Ordering::Relaxed,
        );
        Ok(found)
    }
    pub fn put(&mut self, key: String, value: Vec<u8>) -> Result<(), CallError> {
        let bytes = key.len().saturating_add(value.len());
        let total = self.written.saturating_add(bytes);
        if key.len() > self.limits.key_bytes
            || value.len() > self.limits.value_bytes
            || total > self.limits.call_write_bytes
        {
            return Err(CallError::limit(
                Limit::StateSize,
                "state put exceeds key, value or call write size limit",
            ));
        }
        self.written = total;
        self.stats
            .bytes_written
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.overlay.insert(key, Some(value));
        Ok(())
    }
    pub fn delete(&mut self, key: String) {
        self.overlay.insert(key, None);
    }
    /// Commits the overlay and the chain's timer operations in one write transaction (7).
    /// `timers` is called inside the transaction, after `begin_write`: redb runs one write
    /// transaction at a time, so what it reads of the catalog is ordered against the rewrite
    /// of `remove` (19.6, Timers). `has_timers` says whether it can return anything.
    pub fn commit(
        self,
        timers: &dyn Fn() -> Vec<TimerOp>,
        has_timers: bool,
    ) -> Result<bool, CallError> {
        if self.overlay.is_empty() && !has_timers {
            return Ok(false);
        }
        let mut changed = false;
        let txn = self.db.begin_write().map_err(CallError::internal)?;
        apply_timer_ops(&txn, &timers())?;
        if !self.overlay.is_empty() {
            let name = format!("s/{}", self.id);
            let mut table = txn
                .open_table(TableDefinition::<&str, &[u8]>::new(&name))
                .map_err(CallError::internal)?;
            let mut sizes = txn.open_table(SIZES).map_err(CallError::internal)?;
            let mut size = sizes
                .get(self.id.as_str())
                .map_err(CallError::internal)?
                .map_or(0, |v| v.value());
            for (key, value) in self.overlay {
                let old = table
                    .get(key.as_str())
                    .map_err(CallError::internal)?
                    .map(|v| v.value().to_vec());
                if old == value {
                    continue;
                }
                changed = true;
                if let Some(old) = old {
                    size -= (key.len() + old.len()) as u64;
                }
                match value {
                    Some(value) => {
                        size = size
                            .checked_add((key.len() + value.len()) as u64)
                            .ok_or_else(|| {
                                CallError::limit(Limit::StateQuota, "state quota exceeded")
                            })?;
                        table
                            .insert(key.as_str(), value.as_slice())
                            .map_err(CallError::internal)?;
                    }
                    None => {
                        table.remove(key.as_str()).map_err(CallError::internal)?;
                    }
                }
            }
            if size > self.limits.quota_bytes {
                return Err(CallError::limit(Limit::StateQuota, "state quota exceeded"));
            }
            sizes
                .insert(self.id.as_str(), size)
                .map_err(CallError::internal)?;
        }
        let started = Instant::now();
        txn.commit().map_err(CallError::internal)?;
        self.stats.record(started);
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::{State, TimerOp};
    use crate::Limits;
    use std::sync::atomic::Ordering::Relaxed;

    #[cfg(unix)]
    #[test]
    fn s114_home_lock_remains_held_until_state_closes() {
        let dir = tempfile::tempdir().expect("temporary preview");
        let host = crate::Host::open(dir.path(), crate::Mode::Preview, Limits::default())
            .expect("open preview");
        let path = dir.path().join("lock");
        host.state
            .home_lock_at_drop
            .set(path.clone())
            .expect("arm drop check");
        drop(host);
        let lock = std::fs::File::open(path).expect("open host lock");
        lock.try_lock().expect("the host released the lock");
    }

    /// The writer's counters tell the host's own commits (timer writes) from the mods'
    /// transactions: both count in `commits`, only the host's in `host_commits` (19.9).
    #[test]
    fn host_commits_count_timer_writes_only() {
        let dir = tempfile::tempdir().unwrap();
        let state = State::open(dir.path()).unwrap();
        let mut txn = state.transaction("m", &Limits::default()).unwrap();
        txn.put("k".into(), b"v".to_vec()).unwrap();
        assert!(txn.commit(&Vec::new, false).unwrap());
        let after = |due| TimerOp::After {
            key: "m/t".into(),
            due,
            origin: "m".into(),
        };
        state.timer_write(&[after(1)]).unwrap();
        state.timer_write_with(&|| vec![after(2)]).unwrap();
        let s = &state.stats;
        assert_eq!(s.commits.load(Relaxed), 3);
        assert_eq!(s.host_commits.load(Relaxed), 2);
        let (all, host) = (
            s.commit_nanos.load(Relaxed),
            s.host_commit_nanos.load(Relaxed),
        );
        assert!(0 < host && host < all, "{host} {all}");
    }

    /// Every commit of the host's own counts as the host's: scratch rows and the generation
    /// as well as timer writes (19.9, F3.5).
    #[test]
    fn host_commits_count_scratch_and_generation_writes() {
        let dir = tempfile::tempdir().unwrap();
        let state = State::open(dir.path()).unwrap();
        let host = || state.stats.host_commits.load(Relaxed);
        assert_eq!(host(), 0);
        state.next_generation().unwrap();
        assert_eq!(host(), 1);
        state.scratch_add("s", "o").unwrap();
        assert_eq!(host(), 2);
        state.scratch_delete("s").unwrap();
        assert_eq!(host(), 3);
        state.scratch_removed("o").unwrap();
        assert_eq!(host(), 4);
        state
            .timers_removed("o", &std::collections::BTreeMap::new())
            .unwrap();
        assert_eq!(host(), 5);
        state.delete("o").unwrap();
        assert_eq!(host(), 6);
        assert_eq!(state.stats.commits.load(Relaxed), 6);
    }
}
