//! Following channels (TECHNICAL 5.4): `follows.json`, reading a channel
//! from its web base, and follow, unfollow and refresh.

use crate::{
    Change, ChannelView, Dist, DistError, Follow, Follows, Link, Outcome, Profile, PublicKey,
    RefreshReport, http, site, view,
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
const MAX_GIVEN: usize = 16;
/// Remembers `base` in `given`: at most 16, the oldest goes first (4.4). Dropping a given
/// base only makes its reads stricter.
pub(crate) fn note_given(list: &mut Vec<String>, base: &str) {
    if base.is_empty() || list.iter().any(|b| b == base) {
        return;
    }
    list.push(base.into());
    if list.len() > MAX_GIVEN {
        list.remove(0);
    }
}
/// Holds at most four bases, preserving the first. True when the caller should
/// re-read the bases to put the highest head first.
pub(crate) fn add_base(record: &mut Follow, base: &str) -> bool {
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
/// A listed-only record becoming followed or chosen drops its listing's bases.
pub(crate) fn adopt_base(record: &mut Follow, base: &str) {
    if !record.followed && !record.index {
        record.base = base.into();
        record.bases.clear();
    }
}
impl Dist {
    pub(crate) fn new_record(followed: bool, index: bool, kept: bool) -> Follow {
        let time = now();
        Follow {
            base: String::new(),
            followed,
            index,
            kept,
            bases: Vec::new(),
            given: Vec::new(),
            _followed_at: time,
            _last_attempt: if kept { 0 } else { time },
            last_success: None,
        }
    }
    /// The one write path of `follows.json`. Memory changes when the file is
    /// replaced. A save that fails before the rename leaves records as they
    /// were. A directory sync failure after the rename leaves the store unsynced
    /// and is returned to the caller. An unsynced
    /// store first retries that sync and writes nothing while it fails (4.4).
    pub(crate) fn save_follows(
        &self,
        live: &mut BTreeMap<PublicKey, Follow>,
        records: BTreeMap<PublicKey, Follow>,
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
        let file = Follows {
            version: 1,
            channels: records
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        };
        let (_, synced) = site::replace_equal(
            &self.home.join("dist/follows.json"),
            &serde_json::to_vec(&file).expect("follows JSON"),
            0o600,
        )?;
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
    /// Follows a link explicitly supplied by the participant.
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError> {
        self.follow_link(link, true)
    }

    /// Follows a result from discovery or a fetched release without giving its base.
    pub fn follow_derived(&self, link: &Link) -> Result<RefreshReport, DistError> {
        self.follow_link(link, false)
    }

    fn follow_link(&self, link: &Link, given: bool) -> Result<RefreshReport, DistError> {
        if Some(link.channel()) == self.identity() {
            return Err(DistError::OwnChannel);
        }
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.settle(true)?;
        let listed = !given
            && self
                .follows
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .get(&link.channel())
                .is_none_or(|record| listed_read(record, link.base()));
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
            .or_insert_with(|| Self::new_record(true, false, false));
        adopt_base(record, link.base());
        record.followed = true;
        if given {
            note_given(&mut record.given, link.base());
        }
        let learned = add_base(record, link.base());
        record._last_attempt = time;
        if matches!(report.outcome, Outcome::Current { .. }) {
            record.last_success = Some(time);
        }
        self.save_follows(&mut records, next)?;
        drop(records);
        self.after_learning(link.channel(), report, learned)
    }
    /// Gives or learns an explicitly opened base after it serves the channel.
    /// True when the channel was read again from all its bases.
    pub(crate) fn learn_link_base(&self, link: &Link) -> Result<bool, DistError> {
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.settle(true)?;
        let key = link.channel();
        let needs_read = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
            .is_some_and(|r| {
                !r.base.is_empty()
                    && (listed_read(r, link.base())
                        || (r.base != link.base() && !r.bases.iter().any(|b| b == link.base())))
            });
        if !needs_read {
            return Ok(false);
        }
        let Ok(report) = self.read_channel(key, link.base()) else {
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
            if !record.followed && !record.index {
                adopt_base(record, link.base());
                record.followed = true;
            }
            note_given(&mut record.given, link.base());
            add_base(record, link.base());
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
    fn record_at(base: &str) -> Follow {
        Follow {
            base: base.into(),
            followed: true,
            index: false,
            kept: false,
            bases: Vec::new(),
            given: Vec::new(),
            _followed_at: 0,
            _last_attempt: 0,
            last_success: None,
        }
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
            note_given(&mut r.given, b);
            add_base(&mut r, b);
        }
        assert!(!r.bases.contains(&"http://b/".to_owned()), "b was evicted");
        assert_eq!(r.given.len(), 5);
        note_given(&mut r.given, "http://b/");
        add_base(&mut r, "http://b/");
        assert!(r.bases.contains(&"http://b/".to_owned()));
        assert!(r.given.contains(&"http://b/".to_owned()));
    }
}
