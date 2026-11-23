//! Following channels (TECHNICAL 5.4): `follows.json`, reading a channel
//! from its web base, and follow, unfollow and refresh.

use crate::{
    Change, ChannelView, Dist, DistError, Follow, Follows, Link, Outcome, Profile, PublicKey,
    RefreshReport, Tombstone, http, site, view,
};
use std::collections::BTreeMap;

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
pub(crate) fn add_tombstone(tombs: &mut BTreeMap<String, Tombstone>, key: String, tomb: Tombstone) {
    tombs.insert(key, tomb);
    while tombs.len() > MAX_TOMBS {
        let oldest = tombs
            .iter()
            .min_by_key(|(_, t)| t.at)
            .map(|(k, _)| k.clone());
        tombs.remove(&oldest.expect("not empty"));
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
            .keys()
            .cloned()
            .collect()
    }
}
impl Dist {
    /// A link's base the record (or a tombstone) saw but the participant has not given: its
    /// first read is a listed one, also when it turns a listed-only record followed or
    /// chosen, for the link may be built from the record (4.4, 5.9, DD-48).
    pub(crate) fn held_ungiven(&self, key: PublicKey, base: &str) -> bool {
        // One snapshot: the `follows` lock is held through the tombstone lookup, so a
        // `keep` or a follow cannot move a tombstone into a record between the two. The
        // order is `follows`, then `tombs`, as `save_follows` takes them.
        let records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(r) = records.get(&key) {
            return !link_gives(&r.seen, r.seen_full, &r.given, base);
        }
        #[cfg(test)]
        if let Some(hook) = self.test_held_gap.lock().expect("test hook").as_ref() {
            hook();
        }
        self.tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key.to_string())
            .is_some_and(|t| !link_gives(&t.seen, t.seen_full, &t.given, base))
    }
    /// A record with no base yet, starting from its channel's tombstone (4.4, DD-48).
    pub(crate) fn new_record(
        &self,
        key: PublicKey,
        followed: bool,
        index: bool,
        kept: bool,
    ) -> Follow {
        let tomb = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key.to_string())
            .cloned();
        let time = now();
        Follow {
            base: String::new(),
            followed,
            index,
            kept,
            bases: Vec::new(),
            given: tomb.as_ref().map(|t| t.given.clone()).unwrap_or_default(),
            seen_full: tomb.as_ref().is_some_and(|t| t.seen_full),
            seen: tomb.map(|t| t.seen).unwrap_or_default(),
            _followed_at: time,
            _last_attempt: if kept { 0 } else { time },
            last_success: None,
        }
    }
    /// Saves `records`; a channel with a record has no tombstone (4.4).
    pub(crate) fn save_follows(
        &self,
        records: &BTreeMap<PublicKey, Follow>,
    ) -> Result<(), DistError> {
        self.save_staged(records, None)
    }
    /// The one write path of `follows.json`. `staged` is the next tombstone map a caller
    /// built on a copy; the tombstones change in memory only once the file is written, so
    /// a failed save leaves records and tombstones as they were (4.4).
    pub(crate) fn save_staged(
        &self,
        records: &BTreeMap<PublicKey, Follow>,
        staged: Option<BTreeMap<String, Tombstone>>,
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
        let mut tombs = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut seen = staged.unwrap_or_else(|| tombs.clone());
        for key in records.keys() {
            seen.remove(&key.to_string());
        }
        let file = Follows {
            version: 1,
            channels: records
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            seen,
        };
        site::write_equal(
            &self.home.join("dist/follows.json"),
            &serde_json::to_vec(&file).expect("follows JSON"),
            0o600,
        )?;
        *tombs = file.seen;
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
        let listed = self.held_ungiven(link.channel(), link.base());
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
        self.save_follows(&next)?;
        *records = next;
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
        let listed = self.held_ungiven(key, link.base());
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
        self.save_follows(&next)?;
        *records = next;
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
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        let record = next
            .get_mut(channel)
            .filter(|r| r.followed)
            .ok_or(DistError::NothingToUnfollow { channel: *channel })?;
        record.followed = false;
        self.save_follows(&next)?;
        *records = next;
        drop(records);
        self.prune_listed()
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
            self.save_follows(&next)?;
            *records = next;
        }
        result
    }
    pub(crate) fn read_channel_entries(
        &self,
        key: PublicKey,
        base: &str,
        listed: bool,
    ) -> Result<RefreshReport, DistError> {
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
        let mut tombs = BTreeMap::new();
        for n in 0..1025u64 {
            let tomb = Tombstone {
                seen: Vec::new(),
                given: Vec::new(),
                seen_full: false,
                at: 1000 - n % 1000 + n / 1000 * 5000,
            };
            add_tombstone(&mut tombs, format!("k{n}"), tomb);
        }
        assert_eq!(tombs.len(), 1024);
        let oldest = (0..1025u64)
            .min_by_key(|n| 1000 - n % 1000 + n / 1000 * 5000)
            .unwrap();
        assert!(!tombs.contains_key(&format!("k{oldest}")));
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
}
