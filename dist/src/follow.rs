//! Following channels (TECHNICAL 5.4): `follows.json`, reading a channel
//! from its web base, and follow, unfollow and refresh.

use crate::{
    Change, ChannelView, Dist, DistError, Follow, Follows, Link, Outcome, Profile, PublicKey,
    RefreshReport, Tombs, Tombstone, http, site, view,
};
use std::collections::{BTreeMap, BTreeSet};

/// The store after a rename whose directory sync failed (4.4): `dirty` until a retry of
/// that sync succeeds, and the work the failed calls left, finished by `settle`.
#[derive(Default)]
pub(crate) struct Unsynced {
    pub(crate) dirty: bool,
    /// Kept marks of failed ingests to take back: the channel and whether it had a record.
    pub(crate) take_back: Vec<(PublicKey, bool)>,
    /// Channels whose unfollow or drop_index still has to prune.
    pub(crate) unfollowed: BTreeSet<PublicKey>,
    pub(crate) dropped: BTreeSet<PublicKey>,
    /// Channels whose left prune `settle` finished: a retry of that unfollow or drop_index
    /// answers `Ok` once, whichever call settled first.
    pub(crate) unfollow_done: BTreeSet<PublicKey>,
    pub(crate) drop_done: BTreeSet<PublicKey>,
}
pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
const MAX_SEEN: usize = 16;
const MAX_TOMBS: usize = 1024;
/// Remembers `base` in `given`: at most 16, the oldest goes first (4.4). Dropping a given
/// base only makes its reads stricter.
pub(crate) fn note_given(list: &mut Vec<String>, base: &str) {
    if base.is_empty() || list.iter().any(|b| b == base) {
        return;
    }
    list.push(base.into());
    if list.len() > MAX_SEEN {
        list.remove(0);
    }
}
/// Remembers `base` in `seen`: at most 16 and none dropped. A 17th is not added and
/// `full` is set, so a link gives no new base from then on (4.4, DD-48).
pub(crate) fn note_seen(list: &mut Vec<String>, full: &mut bool, base: &str) {
    if base.is_empty() || list.iter().any(|b| b == base) {
        return;
    }
    if list.len() >= MAX_SEEN {
        *full = true;
        return;
    }
    list.push(base.into());
}
/// Whether a link at `base` gives it: the participant gave it once, or the record never
/// saw it and its `seen` is not full (4.4, DD-48). A seen base never given may be a hit's
/// link built from the record.
pub(crate) fn link_gives(seen: &[String], seen_full: bool, given: &[String], base: &str) -> bool {
    given.iter().any(|b| b == base) || (!seen_full && !seen.iter().any(|b| b == base))
}
/// Keeps a removed record's `seen` and `given`: at most 1024, the oldest `at` goes first.
/// An evicted channel goes into the `gone` filter (4.4, DD-48).
pub(crate) fn add_tombstone(tombs: &mut Tombs, key: String, tomb: Tombstone) {
    tombs.map.insert(key, tomb);
    while tombs.map.len() > MAX_TOMBS {
        let oldest = tombs
            .map
            .iter()
            .min_by_key(|(_, t)| t.at)
            .map(|(k, _)| k.clone())
            .expect("not empty");
        tombs.map.remove(&oldest);
        tombs.gone.add(&key_bytes(&oldest));
    }
}
/// The 32 bytes of a channel text. A text that is no key (never stored by dist) hashes
/// as its own bytes.
fn key_bytes(text: &str) -> Vec<u8> {
    text.parse::<PublicKey>()
        .map_or_else(|_| text.as_bytes().to_vec(), |k| k.as_bytes().to_vec())
}
const GONE_BYTES: usize = 4096;
const GONE_BITS: usize = GONE_BYTES * 8;
/// A fixed-size filter of the channels whose tombstone was evicted (4.4, DD-48, S105-CHURN
/// "(a) fail closed"). `None`: nothing was ever evicted. A key sets 3 bits; a false
/// positive only makes a channel fail closed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Gone(Option<Vec<u8>>);
impl Gone {
    /// The 3 bits of a key: u16 little-endian of bytes 0..2, 2..4, 4..6 of its BLAKE3.
    fn bits(key: &[u8]) -> [usize; 3] {
        let hash = blake3::hash(key);
        let h = hash.as_bytes();
        [0, 2, 4].map(|i| u16::from_le_bytes([h[i], h[i + 1]]) as usize % GONE_BITS)
    }
    pub(crate) fn add(&mut self, key: &[u8]) {
        let bytes = self.0.get_or_insert_with(|| vec![0; GONE_BYTES]);
        for bit in Self::bits(key) {
            bytes[bit / 8] |= 1 << (bit % 8);
        }
    }
    /// All 3 bits of the key are set.
    pub(crate) fn may_be_gone(&self, key: &PublicKey) -> bool {
        self.0.as_ref().is_some_and(|bytes| {
            Self::bits(key.as_bytes())
                .iter()
                .all(|bit| bytes[bit / 8] & (1 << (bit % 8)) != 0)
        })
    }
    pub(crate) fn to_text(&self) -> Option<String> {
        self.0.as_ref().map(|bytes| {
            bytes
                .iter()
                .flat_map(|b| [b >> 4, b & 15])
                .map(|n| char::from_digit(u32::from(n), 16).expect("a nibble"))
                .collect()
        })
    }
    /// A malformed text, or one of the wrong length, reads as all bits set: fail closed.
    pub(crate) fn from_text(text: Option<&str>) -> Self {
        let Some(text) = text else {
            return Self(None);
        };
        let digits: Option<Vec<u8>> = text
            .chars()
            .map(|c| {
                c.is_ascii_digit()
                    .then_some(c)
                    .or(('a'..='f').contains(&c).then_some(c))
                    .and_then(|c| c.to_digit(16))
                    .map(|n| n as u8)
            })
            .collect();
        match digits {
            Some(d) if d.len() == GONE_BYTES * 2 => {
                Self(Some(d.chunks(2).map(|p| p[0] << 4 | p[1]).collect()))
            }
            _ => Self(Some(vec![0xff; GONE_BYTES])),
        }
    }
}
/// Adds a base a link names to a record: given when the link gives it, appended after the
/// known ones, at most 4 (the oldest learned base after the front one goes). True when the
/// record changed its set of bases, so the caller reads them all (5.9).
pub(crate) fn learn_base(record: &mut Follow, base: &str) -> bool {
    if link_gives(&record.seen, record.seen_full, &record.given, base) {
        note_given(&mut record.given, base);
    }
    let learned = add_base(record, base);
    note_seen(&mut record.seen, &mut record.seen_full, base);
    learned
}
fn add_base(record: &mut Follow, base: &str) -> bool {
    if record.base.is_empty() {
        record.base = base.into();
        return false;
    }
    if record.base == base || record.bases.iter().any(|b| b == base) {
        return false;
    }
    if record.bases.is_empty() {
        record.bases.push(record.base.clone());
    }
    record.bases.push(base.into());
    if record.bases.len() > 4 {
        record.bases.remove(1);
    }
    true
}
/// Whether a read of `base` of this record uses the listed-address rule: the record is
/// listed-only, or the participant never gave this base by a link (5.9, DD-47).
pub(crate) fn listed_read(record: &Follow, base: &str) -> bool {
    (!record.followed && !record.index) || !record.given.iter().any(|b| b == base)
}
/// A listed-only record that turns followed or chosen holds only the base the participant
/// gave (5.9): the listing's bases are dropped.
pub(crate) fn adopt_given_base(record: &mut Follow, base: &str) {
    if !record.followed && !record.index {
        if link_gives(&record.seen, record.seen_full, &record.given, base) {
            note_given(&mut record.given, base);
        }
        note_seen(&mut record.seen, &mut record.seen_full, base);
        record.base = base.into();
        record.bases.clear();
    }
}
/// A link's base the record does not hold: the only kind a link can newly give (5.9).
fn follow_unseen(record: &Follow, base: &str) -> bool {
    record.base != base && !record.bases.iter().any(|b| b == base)
}
#[cfg(test)]
impl Dist {
    /// Tests only: adds a tombstone as `prune` does.
    pub(crate) fn test_add_tombstone(&self, key: &str, tomb: Tombstone) {
        let mut tombs = self.tombs.lock().expect("test tombs");
        add_tombstone(&mut tombs, key.into(), tomb);
    }
    /// Tests only: the channel texts that hold a tombstone.
    pub(crate) fn test_tomb_keys(&self) -> Vec<String> {
        self.tombs
            .lock()
            .expect("test tombs")
            .map
            .keys()
            .cloned()
            .collect()
    }
}
impl Dist {
    /// A link's base the record (or a tombstone) saw but the participant has not given: its
    /// first read is a listed one, also when it turns a listed-only record followed or
    /// chosen, for the link may be built from the record (4.4, 5.9, DD-48).
    pub(crate) fn held_ungiven(&self, key: PublicKey, base: &str) -> Result<bool, DistError> {
        self.settle(true)?;
        // One snapshot: the `follows` lock is held through the tombstone lookup, so a
        // `keep` or a follow cannot move a tombstone into a record between the two. The
        // order is `follows`, then `tombs`, as `save_follows` takes them.
        let records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(r) = records.get(&key) {
            return Ok(!link_gives(&r.seen, r.seen_full, &r.given, base));
        }
        #[cfg(test)]
        if let Some(hook) = self.test_held_gap.lock().expect("test hook").as_ref() {
            hook();
        }
        let tombs = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        Ok(match tombs.map.get(&key.to_string()) {
            Some(t) => !link_gives(&t.seen, t.seen_full, &t.given, base),
            // No record, no tombstone: a channel whose tombstone was evicted fails closed.
            None => tombs.gone.may_be_gone(&key),
        })
    }
    /// A record with no base yet, starting from its channel's tombstone (4.4, DD-48).
    pub(crate) fn new_record(
        &self,
        key: PublicKey,
        followed: bool,
        index: bool,
        kept: bool,
    ) -> Follow {
        let (tomb, gone) = {
            let tombs = self
                .tombs
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let tomb = tombs.map.get(&key.to_string()).cloned();
            let gone = tomb.is_none() && tombs.gone.may_be_gone(&key);
            (tomb, gone)
        };
        let time = now();
        Follow {
            base: String::new(),
            followed,
            index,
            kept,
            bases: Vec::new(),
            given: tomb.as_ref().map(|t| t.given.clone()).unwrap_or_default(),
            seen_full: gone || tomb.as_ref().is_some_and(|t| t.seen_full),
            seen: tomb.map(|t| t.seen).unwrap_or_default(),
            _followed_at: time,
            _last_attempt: if kept { 0 } else { time },
            last_success: None,
        }
    }
    /// Notes a 5.11 provider `base` of `key` as seen before dist reads it: in the channel's
    /// record, or else in its tombstone (created when it has none), saved before the read.
    /// A later link at that base is then a listed first read and gives nothing (4.4, DD-48).
    pub(crate) fn note_provider_base(&self, key: PublicKey, base: &str) -> Result<(), DistError> {
        self.settle(true)?;
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(record) = records.get(&key) {
            let mut next = record.clone();
            note_seen(&mut next.seen, &mut next.seen_full, base);
            if next.seen == record.seen && next.seen_full == record.seen_full {
                return Ok(());
            }
            let mut all = records.clone();
            all.insert(key, next);
            return self.save_staged(&mut records, all, None);
        }
        let mut tombs = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let name = key.to_string();
        let mut tomb = tombs.map.get(&name).cloned().unwrap_or_else(|| Tombstone {
            seen: Vec::new(),
            given: Vec::new(),
            // A channel whose tombstone was evicted fails closed (4.4, DD-48).
            seen_full: tombs.gone.may_be_gone(&key),
            at: now(),
        });
        let before = (tomb.seen.len(), tomb.seen_full);
        note_seen(&mut tomb.seen, &mut tomb.seen_full, base);
        if tombs.map.contains_key(&name) && before == (tomb.seen.len(), tomb.seen_full) {
            return Ok(());
        }
        add_tombstone(&mut tombs, name, tomb);
        let same = records.clone();
        self.save_staged(&mut records, same, Some(tombs))
    }
    /// Saves `next` and makes it the records in `live`; a channel with a record has no
    /// tombstone (4.4).
    pub(crate) fn save_follows(
        &self,
        live: &mut BTreeMap<PublicKey, Follow>,
        next: BTreeMap<PublicKey, Follow>,
    ) -> Result<(), DistError> {
        self.save_staged(live, next, None)
    }
    /// The one write path of `follows.json`. `next` are the records and `staged` the
    /// tombstone map a caller built on a copy. Memory changes exactly when the file is
    /// replaced: a save that fails before the rename leaves records and tombstones as they
    /// were; once the rename installed the file, both become what it holds, and a failed
    /// directory sync after it is still returned and leaves the store unsynced. An unsynced
    /// store first retries that sync and writes nothing while it fails (4.4).
    pub(crate) fn save_staged(
        &self,
        live: &mut BTreeMap<PublicKey, Follow>,
        records: BTreeMap<PublicKey, Follow>,
        staged: Option<Tombs>,
    ) -> Result<(), DistError> {
        #[cfg(test)]
        {
            let mut left = self.test_fail_saves.lock().expect("test switch");
            match left.as_mut() {
                Some(0) => {
                    return Err(DistError::Io {
                        path: self.home.join("dist/follows.json"),
                        source: std::io::Error::other("test: save fails"),
                    });
                }
                Some(n) => *n -= 1,
                None => {}
            }
        }
        self.sync_pending()?;
        let mut tombs = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = staged.unwrap_or_else(|| tombs.clone());
        for key in records.keys() {
            next.map.remove(&key.to_string());
        }
        let file = Follows {
            version: 1,
            channels: records
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            seen: next.map.clone(),
            gone: next.gone.to_text(),
        };
        let (_, synced) = site::replace_equal(
            &self.home.join("dist/follows.json"),
            &serde_json::to_vec(&file).expect("follows JSON"),
            0o600,
        )?;
        *tombs = next;
        *live = records;
        if synced.is_err() {
            self.unsynced_state().dirty = true;
        }
        synced
    }
    fn unsynced_state(&self) -> std::sync::MutexGuard<'_, Unsynced> {
        self.unsynced
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    /// Retries the directory sync an earlier save left failed; `Err` while it fails. A call
    /// runs it again before a request that follows a save of its own (4.4).
    pub(crate) fn sync_pending(&self) -> Result<(), DistError> {
        let mut state = self.unsynced_state();
        if state.dirty {
            site::sync_dir(&self.home.join("dist"))?;
            state.dirty = false;
        }
        Ok(())
    }
    /// True once after `settle` finished the prune a failed drop_index of `channel` left.
    pub(crate) fn take_drop_done(&self, channel: &PublicKey) -> bool {
        self.unsynced_state().drop_done.remove(channel)
    }
    /// Records work a failed call left, for `settle` to finish (4.4).
    pub(crate) fn leave_pending(&self, add: impl FnOnce(&mut Unsynced)) {
        add(&mut self.unsynced_state());
    }
    /// The one gate, called first by each call that reads provenance, makes a request or
    /// changes `follows.json` (4.4): retries a failed directory sync, returning its error
    /// without further work, then finishes the work earlier calls left: the kept marks to
    /// take back and, when `prune` (the caller holds the read lock; a holder method does
    /// not), the prune of an unfollow or drop_index. When that work fails, also when its
    /// save left the store unsynced, it returns the error and leaves the work pending. The
    /// caller holds no `follows` lock and reads its records only after this.
    pub(crate) fn settle(&self, prune: bool) -> Result<(), DistError> {
        self.sync_pending()?;
        let mut done = {
            let mut state = self.unsynced_state();
            let mut work = Unsynced {
                take_back: std::mem::take(&mut state.take_back),
                ..Unsynced::default()
            };
            if prune {
                work.unfollowed = std::mem::take(&mut state.unfollowed);
                work.dropped = std::mem::take(&mut state.dropped);
            }
            work
        };
        // A take-back whose save fails is left pending again, and its error stops the call:
        // a rename with a failed directory sync leaves the store unsynced (F5.2).
        let mut failed = None;
        for (channel, had) in std::mem::take(&mut done.take_back) {
            if let Err(cause) = self.take_back_kept(&channel, had) {
                failed.get_or_insert(cause);
            }
        }
        let pruned = match failed {
            Some(cause) => Err(cause),
            None if !done.unfollowed.is_empty() || !done.dropped.is_empty() => self.prune_listed(),
            None => Ok(()),
        };
        if let Err(cause) = pruned {
            self.leave_pending(|state| {
                state.unfollowed.append(&mut done.unfollowed);
                state.dropped.append(&mut done.dropped);
            });
            return Err(cause);
        }
        let mut state = self.unsynced_state();
        state.unfollow_done.append(&mut done.unfollowed);
        state.drop_done.append(&mut done.dropped);
        Ok(())
    }
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError> {
        if Some(link.channel()) == self.identity() {
            return Err(DistError::OwnChannel);
        }
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let listed = self.held_ungiven(link.channel(), link.base())?;
        let report = self.read_channel_mode(link.channel(), link.base(), listed)?;
        match report.outcome {
            Outcome::Failed { cause } => return Err(cause),
            Outcome::Compromised { seq } => {
                return Err(DistError::Compromised {
                    channel: link.channel(),
                    seq,
                });
            }
            _ => {}
        }
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        let time = now();
        let record = next
            .entry(link.channel())
            .or_insert_with(|| self.new_record(link.channel(), true, false, false));
        adopt_given_base(record, link.base());
        record.followed = true;
        let learned = learn_base(record, link.base());
        record._last_attempt = time;
        if matches!(report.outcome, Outcome::Current { .. }) {
            record.last_success = Some(time);
        }
        self.save_follows(&mut records, next)?;
        drop(records);
        self.after_learning(link.channel(), report, learned)
    }
    /// `open_link` of a link to a known channel: a base the record does not have is read
    /// first, and learned only when it serves the channel. True when the channel was
    /// read again from all its bases.
    pub(crate) fn learn_link_base(&self, link: &Link) -> Result<bool, DistError> {
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.settle(true)?;
        let key = link.channel();
        let unknown = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
            .is_some_and(|r| !r.base.is_empty() && follow_unseen(r, link.base()));
        if !unknown {
            return Ok(false);
        }
        let listed = self.held_ungiven(key, link.base())?;
        let Ok(report) = self.read_channel_mode(key, link.base(), listed) else {
            return Ok(false);
        };
        if !matches!(
            report.outcome,
            Outcome::Current { .. } | Outcome::Behind { .. }
        ) {
            return Ok(false);
        }
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        if let Some(record) = next.get_mut(&key) {
            learn_base(record, link.base());
        }
        self.save_follows(&mut records, next)?;
        drop(records);
        self.read_record(key)?;
        Ok(true)
    }
    /// A learned base reorders the record's bases by the highest head (5.9).
    pub(crate) fn after_learning(
        &self,
        key: PublicKey,
        report: RefreshReport,
        learned: bool,
    ) -> Result<RefreshReport, DistError> {
        if !learned {
            return Ok(report);
        }
        let mut all = self.read_record(key)?;
        all.changes.splice(0..0, report.changes);
        Ok(all)
    }
    pub fn unfollow(&self, channel: &PublicKey) -> Result<(), DistError> {
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        // A retry of an unfollow whose prune was left finishes it here, or finds it
        // finished by an earlier call (4.4).
        self.settle(true)?;
        let finished = self.unsynced_state().unfollow_done.remove(channel);
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        let Some(record) = next.get_mut(channel).filter(|r| r.followed) else {
            if finished {
                return Ok(());
            }
            return Err(DistError::NothingToUnfollow { channel: *channel });
        };
        record.followed = false;
        let saved = self.save_follows(&mut records, next);
        drop(records);
        let result = saved.and_then(|()| self.prune_listed());
        if result.is_err() {
            self.leave_pending(|state| {
                state.unfollowed.insert(*channel);
            });
        }
        result
    }
    pub fn followed(&self) -> Result<Vec<ChannelView>, DistError> {
        let keys: Vec<_> = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .iter()
            .filter(|(_, r)| r.followed)
            .map(|(k, _)| *k)
            .collect();
        keys.iter().map(|key| self.channel(key)).collect()
    }
    pub fn refresh(&self) -> Vec<RefreshReport> {
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        // Finishes left work; while the sync fails, each read below returns its error.
        let _ = self.settle(true);
        let keys: Vec<_> = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .iter()
            .filter(|(_, r)| r.followed || r.index)
            .map(|(k, _)| *k)
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let mut reports = Vec::new();
        for key in keys {
            seen.insert(key);
            reports.push(self.read_record(key).unwrap_or_else(|cause| RefreshReport {
                channel: key,
                outcome: Outcome::Failed { cause },
                changes: Vec::new(),
            }));
        }
        if let Err(error) = self.read_listed(&mut reports, &mut seen) {
            let (channel, cause) = *error;
            reports.push(RefreshReport {
                channel,
                outcome: Outcome::Failed { cause },
                changes: Vec::new(),
            });
        }
        reports
    }
    pub fn refresh_channel(&self, channel: &PublicKey) -> Result<RefreshReport, DistError> {
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        // `read_record` passes the gate before it reads the record (4.4).
        self.read_record(*channel)
    }
    pub(crate) fn read_channel(
        &self,
        key: PublicKey,
        base: &str,
    ) -> Result<RefreshReport, DistError> {
        self.read_channel_mode(key, base, false)
    }
    pub(crate) fn read_channel_mode(
        &self,
        key: PublicKey,
        base: &str,
        listed: bool,
    ) -> Result<RefreshReport, DistError> {
        self.settle(true)?;
        // A channel already stopped by a fork makes no request or new attempt.
        if let watane_log::Status::Compromised(fork) = self.log.status(&key)? {
            return Ok(RefreshReport {
                channel: key,
                outcome: Outcome::Compromised {
                    seq: fork.chain.seq(),
                },
                changes: Vec::new(),
            });
        }
        let result = self.read_channel_entries(key, base, listed);
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if records.contains_key(&key) {
            let mut next = records.clone();
            let record = next.get_mut(&key).expect("followed key");
            record._last_attempt = now();
            if matches!(
                &result,
                Ok(RefreshReport {
                    outcome: Outcome::Current { .. },
                    ..
                })
            ) {
                record.last_success = Some(record._last_attempt);
            }
            self.save_follows(&mut records, next)?;
        }
        result
    }
    pub(crate) fn read_channel_entries(
        &self,
        key: PublicKey,
        base: &str,
        listed: bool,
    ) -> Result<RefreshReport, DistError> {
        // No request from an unsynced store, also after a save in this call (4.4).
        self.sync_pending()?;
        // A listed read checks its first base as it checks a redirect target (5.9).
        if listed {
            self.check_listed_base(base)?;
        }
        let entry_limit = if listed {
            self.options
                .max_listed_entries
                .min(self.options.max_entries_per_read)
        } else {
            self.options.max_entries_per_read
        };
        let head_url = http::file_url(base, "head");
        let head = self.read_entry(key, &head_url, None, listed)?;
        let stored = self.log.head(&key)?.map(|h| h.seq);
        let start = stored.map_or(0, |seq| seq + 1);
        let served = head.seq();
        let mut entries = Vec::new();
        let mut failure = None;
        let mut next = start;
        while next < served && (entries.len() as u64) < entry_limit {
            let url = http::file_url(base, &format!("entries/{next}"));
            match self.read_entry(key, &url, Some(next), listed) {
                Ok(entry) => entries.push(entry),
                Err(cause) => {
                    failure = Some(cause);
                    break;
                }
            }
            next += 1;
        }
        if failure.is_none() {
            if next < served
                || (listed
                    && entries.len() as u64 >= entry_limit
                    && stored.is_none_or(|s| served > s))
            {
                failure = Some(DistError::TooManyEntries {
                    channel: key,
                    stored,
                    served,
                });
            } else if stored.is_none_or(|seq| served > seq) {
                let url = http::file_url(base, &format!("entries/{served}"));
                match self.get(
                    &url,
                    self.options.max_entry_file,
                    self.options.entry_timeout,
                    false,
                    listed,
                ) {
                    // Equality to the decoded head verifies these bytes, author and seq too.
                    Ok(bytes) if bytes == head.bytes() => entries.push(head),
                    Ok(_) => failure = Some(DistError::HeadMismatch { url }),
                    Err(cause) => failure = Some(cause),
                }
            } else {
                entries.push(head);
            }
        }
        let ingest = self.log.ingest(&key, &entries)?;
        let outcome = match ingest.status {
            watane_log::Status::Compromised(fork) => Outcome::Compromised {
                seq: fork.chain.seq(),
            },
            watane_log::Status::Ok => {
                if let Some(expected) = ingest.gap {
                    Outcome::Failed {
                        cause: DistError::WrongSeq {
                            url: head_url,
                            expected,
                            found: served,
                        },
                    }
                } else if let Some(cause) = failure {
                    Outcome::Failed { cause }
                } else if let Some(stored) = stored.filter(|n| *n > served) {
                    Outcome::Behind { served, stored }
                } else {
                    Outcome::Current { head: served }
                }
            }
        };
        let all = self.log.entries(&key, 0, usize::MAX)?;
        let view = view::derive(key, &all);
        let mut changes = Vec::new();
        for entry in all.iter().filter(|e| stored.is_none_or(|s| e.seq() > s)) {
            let seq = entry.seq();
            if let Some(bad) = view.bad.iter().find(|bad| bad.seq == seq) {
                changes.push(Change::Bad(bad.clone()));
            } else if entry.kind() == "profile" {
                if let Ok(profile) = Profile::decode(entry.body()) {
                    changes.push(Change::Name {
                        seq,
                        name: profile.name,
                    });
                }
            } else if let Some(listing) = view.listings.iter().find(|l| l.seq == seq) {
                changes.push(Change::Listed(listing.clone()));
            } else if let Some(listing) = view.listings.iter().find(|l| l.unlisted == Some(seq)) {
                changes.push(Change::Unlisted {
                    seq,
                    listing: listing.clone(),
                });
            } else if let Some(release) = view.releases.iter().find(|r| r.seq == seq) {
                changes.push(Change::Release(release.clone()));
            } else if let Some(release) = view
                .releases
                .iter()
                .find(|r| r.withdrawn.as_ref().is_some_and(|w| w.seq == seq))
            {
                changes.push(Change::Withdrawn {
                    seq,
                    release: release.clone(),
                });
            }
        }
        Ok(RefreshReport {
            channel: key,
            outcome,
            changes,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn s107_legacy_provenance_is_not_authority() {
        let text = r#"{"base":"https://example.invalid/","followed_at":1,"last_attempt":0,"given":["https://example.invalid/"],"seen":["https://example.invalid/"],"seen_full":true}"#;
        let record: Follow = serde_json::from_str(text).unwrap();
        assert!(listed_read(&record, &record.base));
        let saved = serde_json::to_value(record).unwrap();
        assert!(saved.get("seen").is_none());
        assert!(saved.get("seen_full").is_none());
    }

    #[test]
    fn ac5_given_holds_sixteen_oldest_first_out() {
        let mut list = Vec::new();
        for n in 0..20 {
            note_given(&mut list, &format!("http://h{n}/"));
        }
        note_given(&mut list, "http://h19/");
        assert_eq!(list.len(), 16);
        assert_eq!(list[0], "http://h4/");
        assert_eq!(list[15], "http://h19/");
    }
    /// AC2 (DD-48 r5): a full `seen` drops nothing; the 17th base is not noted, `seen_full` is set.
    #[test]
    fn ac2_a_full_seen_keeps_its_sixteen_and_sets_seen_full() {
        let mut list = Vec::new();
        let mut full = false;
        for n in 0..16 {
            note_seen(&mut list, &mut full, &format!("http://h{n}/"));
        }
        assert!(!full, "sixteen fit");
        note_seen(&mut list, &mut full, "http://h0/");
        assert!(!full, "a known base is not a 17th");
        note_seen(&mut list, &mut full, "http://h16/");
        assert!(full);
        assert_eq!(list.len(), 16);
        assert_eq!(list[0], "http://h0/");
        assert_eq!(list[15], "http://h15/");
        assert!(!list.contains(&"http://h16/".to_owned()));
    }
    #[test]
    fn ac2_a_link_at_an_unseen_base_of_a_seen_full_record_gives_nothing() {
        let mut r = record_at("http://a/");
        for n in 0..16 {
            note_seen(&mut r.seen, &mut r.seen_full, &format!("http://h{n}/"));
        }
        r.seen_full = true;
        assert!(!link_gives(&r.seen, r.seen_full, &r.given, "http://new/"));
        learn_base(&mut r, "http://new/");
        assert!(r.given.is_empty(), "the link gave nothing");
        assert!(
            r.bases.contains(&"http://new/".to_owned()),
            "it is still held"
        );
        // Held, not given: its first read is listed.
        assert!(listed_read(&r, "http://new/"));
    }
    #[test]
    fn ac2_a_link_at_a_given_base_of_a_seen_full_record_still_gives_it() {
        let mut r = record_at("http://a/");
        r.given.push("http://g/".into());
        r.seen_full = true;
        assert!(link_gives(&r.seen, r.seen_full, &r.given, "http://g/"));
        learn_base(&mut r, "http://g/");
        assert_eq!(r.given, ["http://g/"]);
        // A not-full record still gives an unseen base.
        r.seen_full = false;
        assert!(link_gives(&r.seen, r.seen_full, &r.given, "http://u/"));
    }
    /// AC3: `seen_full` defaults to false, is not written when false, on records and tombstones.
    #[test]
    fn ac3_seen_full_defaults_false_and_is_skipped_when_false() {
        let text = r#"{"version":1,"channels":{"k":{"base":"","kept":true,"followed_at":1,"last_attempt":0}},"seen":{"k":{"seen":[],"given":[],"at":1}}}"#;
        let file: Follows = serde_json::from_str(text).unwrap();
        assert!(!file.channels["k"].seen_full && !file.seen["k"].seen_full);
        let out = serde_json::to_value(&file).unwrap();
        assert!(out["channels"]["k"].get("seen_full").is_none());
        assert!(out["seen"]["k"].get("seen_full").is_none());
        let mut file = file;
        file.channels.get_mut("k").unwrap().seen_full = true;
        file.seen.get_mut("k").unwrap().seen_full = true;
        let out = serde_json::to_value(&file).unwrap();
        assert_eq!(out["channels"]["k"]["seen_full"], true);
        assert_eq!(out["seen"]["k"]["seen_full"], true);
    }
    fn record_at(base: &str) -> Follow {
        Follow {
            base: base.into(),
            followed: true,
            index: false,
            kept: false,
            bases: Vec::new(),
            given: Vec::new(),
            seen: Vec::new(),
            seen_full: false,
            _followed_at: 0,
            _last_attempt: 0,
            last_success: None,
        }
    }
    #[test]
    fn ac5_a_thousand_and_twenty_four_tombstones_oldest_at_out() {
        let mut tombs = Tombs::default();
        for n in 0..1025u64 {
            let tomb = Tombstone {
                seen: Vec::new(),
                given: Vec::new(),
                seen_full: false,
                at: 1000 - n % 1000 + n / 1000 * 5000,
            };
            add_tombstone(&mut tombs, format!("k{n}"), tomb);
        }
        assert_eq!(tombs.map.len(), 1024);
        let oldest = (0..1025u64)
            .min_by_key(|n| 1000 - n % 1000 + n / 1000 * 5000)
            .unwrap();
        assert!(!tombs.map.contains_key(&format!("k{oldest}")));
    }
    /// AC6: empty `seen` and `given` (record and top level) are not written, and a file
    /// without them reads.
    #[test]
    fn ac6_empty_seen_and_given_are_not_written() {
        let text = r#"{"version":1,"channels":{"k":{"base":"","kept":true,"followed_at":1,"last_attempt":0}}}"#;
        let file: Follows = serde_json::from_str(text).unwrap();
        assert!(file.seen.is_empty() && file.channels["k"].seen.is_empty());
        let out = serde_json::to_value(&file).unwrap();
        assert!(out.get("seen").is_none());
        let r = &out["channels"]["k"];
        assert!(r.get("seen").is_none() && r.get("given").is_none());
    }
    #[test]
    fn ac3_a_given_base_evicted_by_the_cap_is_given_again() {
        let mut r = record_at("http://a/");
        for b in [
            "http://a/",
            "http://b/",
            "http://c/",
            "http://d/",
            "http://e/",
        ] {
            learn_base(&mut r, b);
        }
        assert!(!r.bases.contains(&"http://b/".to_owned()), "b was evicted");
        assert_eq!(r.given.len(), 5);
        learn_base(&mut r, "http://b/");
        assert!(r.bases.contains(&"http://b/".to_owned()));
        assert!(r.given.contains(&"http://b/".to_owned()));
        // A seen base never given gives nothing, held or evicted.
        r.seen.push("http://x/".into());
        learn_base(&mut r, "http://x/");
        assert!(!r.given.contains(&"http://x/".to_owned()));
    }
    /// The `n`-th valid key of a deterministic stream of hashes.
    fn key_of(n: u8) -> PublicKey {
        (0u32..)
            .map(|i| *blake3::hash(&[&[n][..], &i.to_le_bytes()].concat()).as_bytes())
            .find_map(|bytes| PublicKey::from_bytes(&bytes).ok())
            .unwrap()
    }
    /// S105-CHURN (a): a key sets 3 bits, read from its BLAKE3 as three little-endian u16.
    #[test]
    fn churn_a_key_sets_three_bits_of_its_blake3() {
        let key = key_of(1);
        let hash = blake3::hash(key.as_bytes());
        let h = hash.as_bytes();
        let mut gone = Gone::default();
        assert!(!gone.may_be_gone(&key));
        gone.add(key.as_bytes());
        let bytes = gone.0.as_ref().unwrap();
        assert_eq!(bytes.len(), 4096);
        let set: usize = bytes.iter().map(|b| b.count_ones() as usize).sum();
        let mut want: Vec<usize> = [0, 2, 4]
            .iter()
            .map(|i| u16::from_le_bytes([h[*i], h[i + 1]]) as usize % 32768)
            .collect();
        want.sort_unstable();
        want.dedup();
        assert_eq!(set, want.len());
        for bit in want {
            assert!(bytes[bit / 8] & (1 << (bit % 8)) != 0);
        }
        assert!(gone.may_be_gone(&key));
    }
    #[test]
    fn churn_membership_needs_all_three_bits() {
        let mut gone = Gone::default();
        for n in 0..50 {
            gone.add(key_of(n).as_bytes());
        }
        assert!((0..50).all(|n| gone.may_be_gone(&key_of(n))));
        let hits = (100..200).filter(|n| gone.may_be_gone(&key_of(*n))).count();
        assert!(hits < 3, "{hits} false positives of 100");
    }
    #[test]
    fn churn_hex_round_trip_and_default_absent() {
        assert_eq!(Gone::default().to_text(), None);
        assert_eq!(Gone::from_text(None), Gone::default());
        let mut gone = Gone::default();
        gone.add(key_of(2).as_bytes());
        let text = gone.to_text().unwrap();
        assert_eq!(text.len(), 8192);
        assert!(text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        assert_eq!(Gone::from_text(Some(&text)), gone);
    }
    #[test]
    fn churn_a_malformed_filter_reads_as_full() {
        let good = {
            let mut g = Gone::default();
            g.add(key_of(3).as_bytes());
            g.to_text().unwrap()
        };
        let upper = format!("A{}", &good[1..]);
        for bad in ["", "zz", "ab", &good[1..], &upper, &format!("{good}00")] {
            let gone = Gone::from_text(Some(bad));
            assert!(
                (0..20).all(|n| gone.may_be_gone(&key_of(n))),
                "{bad:.8} reads as full"
            );
        }
    }
    /// F3.1 (Codex r3): a present `gone` that is no string (null, a number, an object)
    /// loads as all bits set, as a malformed text does; only an absent one is empty.
    #[test]
    fn churn_a_present_gone_that_is_no_string_reads_as_full() {
        let load = |gone: &str| {
            let text = format!(r#"{{"version":1,"channels":{{}}{gone}}}"#);
            let file: Follows = serde_json::from_str(&text).expect(&text);
            Gone::from_text(file.gone.as_deref())
        };
        assert_eq!(load(""), Gone::default(), "absent is empty");
        for bad in ["null", "5", "true", "{}", "[]", r#"["00"]"#] {
            let gone = load(&format!(r#","gone":{bad}"#));
            assert!(
                (0..20).all(|n| gone.may_be_gone(&key_of(n))),
                "gone: {bad} reads as full"
            );
        }
    }
    #[test]
    fn churn_an_eviction_adds_the_key_and_the_file_keeps_it() {
        let mut tombs = Tombs::default();
        let victim = key_of(9);
        let tomb = |at| Tombstone {
            seen: Vec::new(),
            given: Vec::new(),
            seen_full: false,
            at,
        };
        add_tombstone(&mut tombs, victim.to_string(), tomb(1));
        assert!(tombs.gone.to_text().is_none(), "nothing evicted yet");
        for n in 0..1024u64 {
            add_tombstone(&mut tombs, format!("k{n}"), tomb(10 + n));
        }
        assert!(!tombs.map.contains_key(&victim.to_string()));
        assert!(tombs.gone.may_be_gone(&victim));
        let file = Follows {
            version: 1,
            channels: BTreeMap::new(),
            seen: BTreeMap::new(),
            gone: tombs.gone.to_text(),
        };
        let back: Follows = serde_json::from_slice(&serde_json::to_vec(&file).unwrap()).unwrap();
        assert!(Gone::from_text(back.gone.as_deref()).may_be_gone(&victim));
        let none = Follows { gone: None, ..file };
        assert!(serde_json::to_value(&none).unwrap().get("gone").is_none());
    }
}
