//! `DistHoldings`: net's `Holdings` over a `Dist` (TECHNICAL 4.1).
use crate::{NodeError, convert};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use watane_dist::{Dist, Entry, PublicKey, Reason};
use watane_net::{BlobId, HeadInfo, Hit, Holdings, Ingested, Query, Role, Why};

/// Answers `net`'s questions only from what dist stored and verified. Made before
/// the `Dist` exists; until `set`, it answers as an empty holder.
#[derive(Debug)]
pub struct DistHoldings {
    dist: OnceLock<Arc<Dist>>,
    /// Packages that `need_blob` asked for and that are not stored yet (5.3).
    asked: Mutex<BTreeMap<[u8; 32], u64>>,
}

impl DistHoldings {
    pub fn empty() -> Self {
        Self {
            dist: OnceLock::new(),
            asked: Mutex::new(BTreeMap::new()),
        }
    }

    /// A second call is an error and keeps the first `Dist`.
    pub fn set(&self, dist: Arc<Dist>) -> Result<(), NodeError> {
        self.dist.set(dist).map_err(|_| NodeError::HolderSet)
    }

    fn asked(&self) -> std::sync::MutexGuard<'_, BTreeMap<[u8; 32], u64>> {
        self.asked
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }

    pub(crate) fn ask(&self, id: BlobId, size: u64) {
        self.asked().insert(id.0, size);
    }

    pub(crate) fn asked_done(&self, id: &BlobId) {
        self.asked().remove(&id.0);
    }
}

impl Holdings for DistHoldings {
    fn is_anchor(&self) -> bool {
        self.dist.get().is_some_and(|d| d.identity().is_some())
    }

    fn heads(&self, channels: Option<&[PublicKey]>) -> Vec<(PublicKey, Option<HeadInfo>)> {
        let Some(dist) = self.dist.get() else {
            return Vec::new();
        };
        dist.heads(channels)
            .unwrap_or_default()
            .into_iter()
            .map(|(key, head)| {
                (
                    key,
                    head.map(|h| HeadInfo {
                        seq: h.seq,
                        id: h.id,
                        forked: h.forked,
                    }),
                )
            })
            .collect()
    }

    fn entries(
        &self,
        channel: &PublicKey,
        from: u64,
        limit: usize,
        max_bytes: usize,
    ) -> Vec<Entry> {
        self.dist
            .get()
            .and_then(|d| d.entries(channel, from, limit, max_bytes).ok())
            .unwrap_or_default()
    }

    fn fork(&self, channel: &PublicKey) -> Option<(Entry, Entry)> {
        self.dist.get()?.fork(channel).ok().flatten()
    }

    fn blob(&self, id: &BlobId) -> Option<PathBuf> {
        self.dist.get()?.package_file(&convert::package_id(id))
    }

    fn wanted_blobs(&self, role: Role) -> Vec<(BlobId, u64)> {
        match (self.dist.get(), role) {
            (Some(dist), Role::Full) => dist
                .missing_packages()
                .unwrap_or_default()
                .into_iter()
                .map(|(id, size)| (convert::blob_id(&id), size))
                .collect(),
            (Some(dist), Role::Light) => self
                .asked()
                .iter()
                .filter(|(id, _)| {
                    dist.package_file(&convert::package_id(&BlobId(**id)))
                        .is_none()
                })
                .map(|(id, size)| (BlobId(*id), *size))
                .collect(),
            _ => Vec::new(),
        }
    }

    fn ingest(&self, channel: &PublicKey, entries: Vec<Entry>, why: Why) -> Ingested {
        let Some(dist) = self.dist.get() else {
            return Ingested::Failed("the home is opening".into());
        };
        // Only a replica keeps; on the anchor a `Kept` pull is stored as `Asked` (NO-8).
        let reason = match why {
            Why::Kept if dist.identity().is_none() => Reason::Kept,
            _ => Reason::Asked,
        };
        match dist.ingest(channel, entries, reason) {
            watane_dist::Ingested::Stored(seq) => Ingested::Stored(seq),
            watane_dist::Ingested::NotWanted => Ingested::NotWanted,
            watane_dist::Ingested::Compromised { .. } => Ingested::Compromised,
            watane_dist::Ingested::Failed(e) => Ingested::Failed(e.to_string()),
        }
    }

    fn store_blob(&self, id: &BlobId, file: &Path) -> Result<(), String> {
        let dist = self.dist.get().ok_or("the home is opening")?;
        dist.store_package(&convert::package_id(id), file)
            .map_err(|e| e.to_string())
    }

    fn search(&self, query: &Query) -> Vec<Hit> {
        let Some(dist) = self.dist.get() else {
            return Vec::new();
        };
        let Some((q, interface)) = convert::query(query) else {
            return Vec::new();
        };
        let Ok(hits) = dist.search(&q) else {
            return Vec::new();
        };
        hits.iter()
            .filter(|h| {
                interface
                    .as_ref()
                    .is_none_or(|(mode, name)| convert::has_in_mode(&h.info.release, mode, name))
            })
            .take(usize::try_from(query.limit).unwrap_or(usize::MAX))
            .map(convert::hit)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac8_second_set_is_an_error_and_keeps_the_first() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let first = Arc::new(Dist::open_keyless(a.path(), Default::default()).unwrap());
        let second = Arc::new(Dist::open_keyless(b.path(), Default::default()).unwrap());
        let h = DistHoldings::empty();
        h.set(Arc::clone(&first)).unwrap();
        assert!(matches!(h.set(second), Err(NodeError::HolderSet)));
        assert!(Arc::ptr_eq(h.dist.get().unwrap(), &first));
    }

    #[test]
    fn ac8_light_wants_what_need_blob_asked_and_the_holder_lacks() {
        let a = tempfile::tempdir().unwrap();
        let h = DistHoldings::empty();
        h.set(Arc::new(
            Dist::open_keyless(a.path(), Default::default()).unwrap(),
        ))
        .unwrap();
        assert!(h.wanted_blobs(Role::Light).is_empty());
        h.ask(BlobId([1; 32]), 10);
        h.ask(BlobId([2; 32]), 20);
        assert_eq!(
            h.wanted_blobs(Role::Light),
            vec![(BlobId([1; 32]), 10), (BlobId([2; 32]), 20)]
        );
        h.asked_done(&BlobId([1; 32]));
        assert_eq!(h.wanted_blobs(Role::Light), vec![(BlobId([2; 32]), 20)]);
        assert!(h.wanted_blobs(Role::Full).is_empty());
    }

    #[test]
    fn ac4_before_its_dist_is_set_it_answers_as_an_empty_holder() {
        let h = DistHoldings::empty();
        let key = (1..=255u8)
            .find_map(|n| PublicKey::from_bytes(&[n; 32]).ok())
            .unwrap();
        assert!(!h.is_anchor());
        assert!(h.heads(None).is_empty());
        assert!(h.heads(Some(&[key])).is_empty());
        assert!(h.entries(&key, 0, 10, usize::MAX).is_empty());
        assert!(h.fork(&key).is_none());
        assert!(h.blob(&BlobId([1; 32])).is_none());
        assert!(h.wanted_blobs(Role::Full).is_empty());
        assert_eq!(
            h.ingest(&key, Vec::new(), Why::Kept),
            Ingested::Failed("the home is opening".into())
        );
        assert!(h.store_blob(&BlobId([1; 32]), Path::new("x")).is_err());
        let q = Query {
            text: None,
            publisher: None,
            interface: None,
            origin: None,
            limit: 5,
        };
        assert!(h.search(&q).is_empty());
    }
}
