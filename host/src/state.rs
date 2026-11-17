//! Mod state (TECHNICAL 7): the redb store keyed by identity, with per-call
//! transactions that enforce the storage limit and commit only on success.

use crate::{CallError, HostError, Limit, Limits};
use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, TableError,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SIZES: TableDefinition<&str, u64> = TableDefinition::new("sizes");
#[derive(Debug)]
pub(crate) struct State {
    path: PathBuf,
    db: Mutex<Option<Arc<Database>>>,
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
        })
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
        let mut db = self
            .db
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if db.is_none() {
            std::fs::create_dir_all(self.path.parent().expect("database parent"))
                .map_err(CallError::internal)?;
            *db = Some(Arc::new(
                Database::create(&self.path).map_err(CallError::internal)?,
            ));
        }
        Ok(Transaction {
            db: Arc::clone(db.as_ref().expect("opened database")),
            id: blake3::hash(identity.as_bytes()).to_hex()[..32].into(),
            read: None,
            overlay: BTreeMap::new(),
            limits: limits.clone(),
            written: 0,
        })
    }
}
pub(crate) struct Transaction {
    db: Arc<Database>,
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
        Ok(table
            .get(key)
            .map_err(CallError::internal)?
            .map(|v| v.value().to_vec()))
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
        self.overlay.insert(key, Some(value));
        Ok(())
    }
    pub fn delete(&mut self, key: String) {
        self.overlay.insert(key, None);
    }
    pub fn commit(self) -> Result<bool, CallError> {
        if self.overlay.is_empty() {
            return Ok(false);
        }
        let mut changed = false;
        let txn = self.db.begin_write().map_err(CallError::internal)?;
        #[expect(
            clippy::semicolon_outside_block,
            reason = "the block ends the table borrow before txn.commit()"
        )]
        {
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
        txn.commit().map_err(CallError::internal)?;
        Ok(changed)
    }
}
