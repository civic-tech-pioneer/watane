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
/// Remembers `base` in a `seen` or `given` list: at most 16, the oldest goes first (4.4).
pub(crate) fn note_seen(list: &mut Vec<String>, base: &str) {
    if base.is_empty() || list.iter().any(|b| b == base) {
        return;
    }
    list.push(base.into());
    if list.len() > MAX_SEEN {
        list.remove(0);
    }
}
/// Whether a link at `base` gives it: the record never saw it, or the participant gave it
/// once (4.4, DD-48). A seen base never given may be a hit's link built from the record.
pub(crate) fn link_gives(seen: &[String], given: &[String], base: &str) -> bool {
    !seen.iter().any(|b| b == base) || given.iter().any(|b| b == base)
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
    if link_gives(&record.seen, &record.given, base) {
        note_seen(&mut record.given, base);
    }
    let learned = add_base(record, base);
    note_seen(&mut record.seen, base);
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
        if link_gives(&record.seen, &record.given, base) {
            note_seen(&mut record.given, base);
        }
        note_seen(&mut record.seen, base);
        record.base = base.into();
        record.bases.clear();
    }
}
/// A link's base the record does not hold: the only kind a link can newly give (5.9).
fn follow_unseen(record: &Follow, base: &str) -> bool {
    record.base != base && !record.bases.iter().any(|b| b == base)
}
impl Dist {
    /// A link's base the record (or a tombstone) saw but the participant has not given: its
    /// first read is a listed one, also when it turns a listed-only record followed or
    /// chosen, for the link may be built from the record (4.4, 5.9, DD-48).
    pub(crate) fn held_ungiven(&self, key: PublicKey, base: &str) -> bool {
        if let Some(r) = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
        {
            return !link_gives(&r.seen, &r.given, base);
        }
        self.tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key.to_string())
            .is_some_and(|t| !link_gives(&t.seen, &t.given, base))
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
        let mut tombs = self
            .tombs
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut seen = tombs.clone();
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
    fn ac5_seen_and_given_hold_sixteen_oldest_first_out() {
        let mut list = Vec::new();
        for n in 0..20 {
            note_seen(&mut list, &format!("http://h{n}/"));
        }
        note_seen(&mut list, "http://h19/");
        assert_eq!(list.len(), 16);
        assert_eq!(list[0], "http://h4/");
        assert_eq!(list[15], "http://h19/");
    }
    #[test]
    fn ac5_a_thousand_and_twenty_four_tombstones_oldest_at_out() {
        let mut tombs = BTreeMap::new();
        for n in 0..1025u64 {
            let tomb = Tombstone {
                seen: Vec::new(),
                given: Vec::new(),
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
        let mut r = Follow {
            base: "http://a/".into(),
            followed: true,
            index: false,
            kept: false,
            bases: Vec::new(),
            given: Vec::new(),
            seen: Vec::new(),
            _followed_at: 0,
            _last_attempt: 0,
            last_success: None,
        };
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
