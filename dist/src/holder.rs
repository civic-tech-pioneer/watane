//! The holder for own machines (TECHNICAL 5.12): heads, entries and packages
//! that `node` serves to and stores from other machines. Takes no read lock.

use crate::{
    Dist, DistError, Entry, EntryId, Follow, Package, PackageId, PublicKey, ReleaseInfo, site, view,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeadState {
    pub seq: u64,
    pub id: EntryId,
    pub forked: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Kept,
    Asked,
}
#[derive(Debug)]
pub enum Ingested {
    Stored(u64),
    NotWanted,
    Compromised { seq: u64 },
    Failed(DistError),
}
impl Dist {
    fn held_channels(&self) -> Vec<PublicKey> {
        self.identity()
            .into_iter()
            .chain(
                self.follows
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .keys()
                    .copied(),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn heads(
        &self,
        channels: Option<&[PublicKey]>,
    ) -> Result<Vec<(PublicKey, Option<HeadState>)>, DistError> {
        let held;
        let channels = match channels {
            Some(c) => c,
            None => {
                held = self.held_channels();
                &held
            }
        };
        channels
            .iter()
            .map(|key| {
                let head = self.log.head(key)?;
                let forked = matches!(self.log.status(key)?, watane_log::Status::Compromised(_));
                Ok((
                    *key,
                    head.map(|h| HeadState {
                        seq: h.seq,
                        id: h.id,
                        forked,
                    }),
                ))
            })
            .collect()
    }
    pub fn entries(
        &self,
        channel: &PublicKey,
        from: u64,
        limit: usize,
        max_bytes: usize,
    ) -> Result<Vec<Entry>, DistError> {
        let mut entries = self.log.entries(channel, from, limit)?;
        let mut size = 0usize;
        let mut count = 0;
        for e in &entries {
            if count > 0 && e.bytes().len() > max_bytes.saturating_sub(size) {
                break;
            }
            size = size.saturating_add(e.bytes().len());
            count += 1;
        }
        entries.truncate(count);
        Ok(entries)
    }
    pub fn fork(&self, channel: &PublicKey) -> Result<Option<(Entry, Entry)>, DistError> {
        Ok(match self.log.status(channel)? {
            watane_log::Status::Ok => None,
            watane_log::Status::Compromised(f) => Some((f.chain, f.other)),
        })
    }
    fn live_releases(&self, channels: &[PublicKey]) -> Result<Vec<ReleaseInfo>, DistError> {
        let mut releases = Vec::new();
        for key in channels {
            releases.extend(
                view::derive(*key, &self.log.entries(key, 0, usize::MAX)?)
                    .releases
                    .into_iter()
                    .filter(|r| r.withdrawn.is_none()),
            );
        }
        Ok(releases)
    }
    pub fn package_file(&self, id: &PackageId) -> Option<PathBuf> {
        let path = self.cache_path(id);
        if !path.is_file() {
            return None;
        }
        self.live_releases(&self.log.channels().ok()?)
            .ok()?
            .iter()
            .any(|r| r.release.package == *id)
            .then_some(path)
    }
    fn mark_kept(
        &self,
        channel: &PublicKey,
        records: &mut std::collections::BTreeMap<PublicKey, Follow>,
    ) -> Result<(), DistError> {
        if records.get(channel).is_some_and(|r| r.kept) {
            return Ok(());
        }
        if records.values().filter(|r| r.kept).count() >= self.options.max_kept {
            return Err(DistError::TooManyKept {
                max: self.options.max_kept,
            });
        }
        let mut next = records.clone();
        let record = next
            .entry(*channel)
            .or_insert_with(|| self.new_record(*channel, false, false, true));
        record.kept = true;
        self.save_follows(&next)?;
        *records = next;
        Ok(())
    }
    pub fn keep(&self, channel: &PublicKey) -> Result<(), DistError> {
        if Some(*channel) == self.identity() || self.log.head(channel)?.is_none() {
            return Err(DistError::NotFollowed {
                channel: *channel,
                local_only: false,
            });
        }
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.mark_kept(channel, &mut records)
    }
    pub fn ingest(&self, channel: &PublicKey, entries: Vec<Entry>, reason: Reason) -> Ingested {
        let mut added = None;
        let result = (|| {
            if let watane_log::Status::Compromised(f) = self.log.status(channel)? {
                return Ok(Ingested::Compromised { seq: f.chain.seq() });
            }
            let mut records =
                (reason == Reason::Kept && Some(*channel) != self.identity()).then(|| {
                    self.follows
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it")
                });
            if records.as_ref().is_some_and(|records| {
                !records.get(channel).is_some_and(|r| r.kept)
                    && records.values().filter(|r| r.kept).count() >= self.options.max_kept
            }) {
                return Ok(Ingested::NotWanted);
            }
            // Validate the entire batch's author before adding its kept record, as log does.
            for (index, entry) in entries.iter().enumerate() {
                if entry.author() != *channel {
                    return Err(watane_log::Error::AuthorMismatch {
                        index,
                        expected: *channel,
                        found: entry.author(),
                    }
                    .into());
                }
            }
            if let Some(records) = records.as_mut() {
                let had = records.contains_key(channel);
                if !records.get(channel).is_some_and(|r| r.kept) {
                    self.mark_kept(channel, records)?;
                    added = Some(had);
                }
            }
            drop(records);
            let ingested = self.log.ingest(channel, &entries)?;
            if let watane_log::Status::Compromised(f) = ingested.status {
                return Ok(Ingested::Compromised { seq: f.chain.seq() });
            }
            match self.log.head(channel)? {
                Some(h) => Ok(Ingested::Stored(h.seq)),
                None => Err(DistError::Gap {
                    channel: *channel,
                    seq: ingested.gap.unwrap_or(0),
                }),
            }
        })();
        if let (Some(had), true) = (added, result.is_err()) {
            self.take_back_kept(channel, had);
        }
        result.unwrap_or_else(Ingested::Failed)
    }
    /// Takes back the kept mark a failed `ingest` added, when nothing is stored.
    fn take_back_kept(&self, channel: &PublicKey, had: bool) {
        if self.log.head(channel).ok().flatten().is_some() {
            return;
        }
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        match next.get_mut(channel) {
            // A follow between step 2 and here keeps the record, not the mark.
            Some(r) if had || r.followed => r.kept = false,
            Some(r) if !r.followed => {
                next.remove(channel);
            }
            _ => return,
        }
        if self.save_follows(&next).is_ok() {
            *records = next;
        }
    }
}

impl Dist {
    pub fn missing_packages(&self) -> Result<Vec<(PackageId, u64)>, DistError> {
        let mut channels = self.held_channels();
        if let Some(own) = self.identity() {
            channels.retain(|key| *key != own);
            channels.insert(0, own);
        }
        Ok(self
            .live_releases(&channels)?
            .into_iter()
            .filter(|r| !self.cache_path(&r.release.package).is_file())
            .take(1024)
            .map(|r| (r.release.package, r.release.size))
            .collect())
    }
    pub fn store_package(&self, id: &PackageId, file: &Path) -> Result<(), DistError> {
        use std::io::Read;
        let input = std::fs::File::open(file).map_err(|e| site::io(file, e))?;
        let mut bytes = Vec::new();
        input
            .take(self.options.max_package.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| site::io(file, e))?;
        if bytes.len() as u64 > self.options.max_package {
            return Err(DistError::PackageTooLarge {
                size: bytes.len() as u64,
                max: self.options.max_package,
            });
        }
        let url = file.display().to_string();
        let found = PackageId::of(&bytes);
        if found != *id {
            return Err(DistError::HashMismatch {
                url,
                expected: id.clone(),
                found,
            });
        }
        let info = self
            .live_releases(&self.log.channels()?)?
            .into_iter()
            .find(|r| r.release.package == *id)
            .ok_or_else(|| DistError::NotNamed { id: id.clone() })?;
        let package = Package::from_bytes(bytes).map_err(|cause| DistError::BadPackage {
            url: url.clone(),
            cause,
        })?;
        let (_tmp, m) = self
            .unpacked(&package)
            .map_err(|cause| DistError::BadPackage {
                url: url.clone(),
                cause,
            })?;
        info.release
            .check(&m, &package)
            .map_err(|cause| DistError::PackageMismatch { url, cause })?;
        self.cache_write(&package)?;
        Ok(())
    }
}
