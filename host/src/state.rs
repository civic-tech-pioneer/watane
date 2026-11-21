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

/// The writer's counters since open (TECHNICAL 19.9, `Host::test_stats`).
#[derive(Debug, Default)]
#[cfg_attr(
    not(feature = "test-streams"),
    allow(dead_code, reason = "read by Host::test_stats (19.9)")
)]
pub(crate) struct Stats {
    pub commits: AtomicU64,
    pub commit_nanos: AtomicU64,
    pub bytes_read: AtomicU64,
    pub bytes_written: AtomicU64,
}
/// One timer write of a chain's commit (19.6).
#[derive(Clone, Debug)]
pub(crate) enum TimerOp {
    After { key: String, due: u64, origin: String },
    Cancel { key: String },
    /// The row of a firing: deleted when it still has this due time.
    DeleteIfDue { key: String, due: u64 },
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
    Some((v.get("due")?.as_u64()?, v.get("origin")?.as_str()?.to_owned()))
}
#[derive(Debug)]
pub(crate) struct State {
    path: PathBuf,
    db: Mutex<Option<Arc<Database>>>,
    pub stats: Arc<Stats>,
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
        for row in table.range(prefix.as_str()..).map_err(CallError::internal)? {
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
    /// A write transaction that holds only timer operations.
    pub fn timer_write(&self, ops: &[TimerOp]) -> Result<(), CallError> {
        let Some(db) = self.database(true)? else {
            return Ok(());
        };
        let txn = db.begin_write().map_err(CallError::internal)?;
        apply_timer_ops(&txn, ops)?;
        let started = Instant::now();
        txn.commit().map_err(CallError::internal)?;
        self.stats.record(started);
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
        #[expect(
            clippy::semicolon_outside_block,
            reason = "the block ends the table borrow before txn.commit()"
        )]
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
        txn.commit().map_err(CallError::internal)?;
        Ok(())
    }
    pub fn delete(&self, identity: &str) -> Result<(), HostError> {
        let db = self
            .db
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let Some(db) = db.as_ref() else {
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
        txn.commit()
            .map_err(|e| HostError::Internal(e.to_string()))?;
        Ok(())
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
        self.stats.bytes_written.fetch_add(bytes as u64, Ordering::Relaxed);
        self.overlay.insert(key, Some(value));
        Ok(())
    }
    pub fn delete(&mut self, key: String) {
        self.overlay.insert(key, None);
    }
    /// Commits the overlay and the chain's timer operations in one write transaction (7).
    pub fn commit(self, timers: &[TimerOp]) -> Result<bool, CallError> {
        if self.overlay.is_empty() && timers.is_empty() {
            return Ok(false);
        }
        let mut changed = false;
        let txn = self.db.begin_write().map_err(CallError::internal)?;
        apply_timer_ops(&txn, timers)?;
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
