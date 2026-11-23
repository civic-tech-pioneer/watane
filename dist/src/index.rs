//! Indexes and the read set (TECHNICAL 5.9): choosing and dropping index
//! channels, and walking listings into the channels a search may read.

use crate::{
    ChannelView, Dist, DistError, Follow, Link, ListingInfo, Outcome, PublicKey, RefreshReport,
    Slug, follow, view,
};
use std::collections::{BTreeMap, BTreeSet};

// Keep the channel attached to failures until refresh builds its report.
type ChannelError = Box<(PublicKey, DistError)>;

#[derive(Clone, Debug)]
pub struct IndexView {
    pub view: ChannelView,
    pub included: Vec<PublicKey>,
    pub mods: usize,
    pub skipped: Vec<Skipped>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub channel: PublicKey,
    pub cause: &'static str,
}
pub(crate) struct ReadSet {
    pub index: PublicKey,
    pub whole: Vec<(PublicKey, Option<PublicKey>)>,
    pub mods: Vec<ListedMod>,
    pub skipped: Vec<Skipped>,
}
pub(crate) struct ListedMod {
    pub channel: PublicKey,
    pub slug: Slug,
    pub through: Option<PublicKey>,
    pub note: String,
}
impl Dist {
    fn stored_view(&self, key: PublicKey) -> Result<ChannelView, DistError> {
        Ok(view::derive(key, &self.log.entries(&key, 0, usize::MAX)?))
    }
    pub(crate) fn read_sets(&self) -> Result<Vec<ReadSet>, DistError> {
        Ok(self.walk()?.sets)
    }
    fn walk(&self) -> Result<Walk, DistError> {
        self.walk_channels().map_err(|error| error.1)
    }
    fn walk_channels(&self) -> Result<Walk, ChannelError> {
        let mut walk = Walk::default();
        let records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let mut admitted = BTreeSet::new();
        for key in self.index_keys() {
            let mut set = ReadSet {
                index: key,
                whole: vec![(key, None)],
                mods: Vec::new(),
                skipped: Vec::new(),
            };
            if matches!(
                self.log
                    .status(&key)
                    .map_err(|cause| Box::new((key, cause.into())))?,
                watane_log::Status::Compromised(_)
            ) {
                walk.sets.push(set);
                continue;
            }
            let listings = self
                .stored_view(key)
                .map_err(|cause| Box::new((key, cause)))?
                .listings;
            let mut per_index = BTreeSet::new();
            let mut skipped = Vec::new();
            for l in listings
                .iter()
                .filter(|l| l.unlisted.is_none() && !l.include)
            {
                if self.admit(l, &records, &mut admitted, &mut per_index, &mut skipped) {
                    walk.add(l, None, &mut set, false);
                }
            }
            for l in listings
                .iter()
                .filter(|l| l.unlisted.is_none() && l.include)
            {
                if self.admit(l, &records, &mut admitted, &mut per_index, &mut skipped) {
                    walk.add(l, None, &mut set, false);
                }
            }
            for (included, _) in set.whole.clone().into_iter().skip(1) {
                if matches!(
                    self.log
                        .status(&included)
                        .map_err(|cause| Box::new((included, cause.into())))?,
                    watane_log::Status::Compromised(_)
                ) {
                    continue;
                }
                for l in self
                    .stored_view(included)
                    .map_err(|cause| Box::new((included, cause)))?
                    .listings
                    .iter()
                    .filter(|l| l.unlisted.is_none() && !l.include)
                {
                    if self.admit(l, &records, &mut admitted, &mut per_index, &mut skipped) {
                        walk.add(l, Some(included), &mut set, true);
                    }
                }
            }
            set.skipped = skipped
                .into_iter()
                .filter(|s| {
                    !set.whole.iter().any(|(key, _)| *key == s.channel)
                        && !set.mods.iter().any(|m| m.channel == s.channel)
                })
                .collect();
            walk.sets.push(set);
        }
        Ok(walk)
    }
    fn admit(
        &self,
        l: &ListingInfo,
        records: &BTreeMap<PublicKey, Follow>,
        all: &mut BTreeSet<PublicKey>,
        per_index: &mut BTreeSet<PublicKey>,
        skipped: &mut Vec<Skipped>,
    ) -> bool {
        let key = l.listing.link.channel();
        let mut skip = |cause| {
            if !skipped.iter().any(|s| s.channel == key) {
                skipped.push(Skipped {
                    channel: key,
                    cause,
                });
            }
            false
        };
        if Some(key) == self.identity() {
            return skip("own channel");
        }
        if records.get(&key).is_some_and(|r| r.followed || r.index) {
            return true;
        }
        if self.check_listed_base(l.listing.link.base()).is_err() {
            return skip("local address");
        }
        if (!all.contains(&key) && all.len() >= self.options.max_listed)
            || (!per_index.contains(&key) && per_index.len() >= self.options.max_listed_per_index)
        {
            return skip("over the listing limit");
        }
        all.insert(key);
        per_index.insert(key);
        true
    }
    fn index_keys(&self) -> Vec<PublicKey> {
        let mut keys: Vec<_> = self.identity().into_iter().collect();
        keys.extend(
            self.follows
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .iter()
                .filter(|(_, r)| r.index)
                .map(|(k, _)| *k),
        );
        keys
    }
    pub fn indexes(&self) -> Result<Vec<IndexView>, DistError> {
        self.read_sets()?
            .into_iter()
            .filter(|s| Some(s.index) != self.identity())
            .map(|s| {
                let mut mods = BTreeSet::new();
                for (key, _) in &s.whole {
                    for r in self
                        .stored_view(*key)?
                        .releases
                        .into_iter()
                        .filter(|r| r.withdrawn.is_none())
                    {
                        mods.insert((*key, r.release.slug));
                    }
                }
                for m in &s.mods {
                    if self
                        .follows
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it")
                        .contains_key(&m.channel)
                        && self
                            .stored_view(m.channel)?
                            .releases
                            .iter()
                            .any(|r| r.release.slug == m.slug && r.withdrawn.is_none())
                    {
                        mods.insert((m.channel, m.slug.clone()));
                    }
                }
                Ok(IndexView {
                    view: self.channel(&s.index)?,
                    included: s.whole.iter().filter_map(|(_, via)| *via).collect(),
                    mods: mods.len(),
                    skipped: s.skipped,
                })
            })
            .collect()
    }
    pub fn drop_index(&self, channel: &PublicKey) -> Result<(), DistError> {
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
            .filter(|r| r.index && Some(*channel) != self.identity())
            .ok_or(DistError::NotIndex { channel: *channel })?;
        record.index = false;
        self.save_follows(&next)?;
        *records = next;
        drop(records);
        self.prune_listed()
    }
    pub fn choose_index(&self, link: &Link) -> Result<Vec<RefreshReport>, DistError> {
        if link.slug().is_some() {
            return Err(DistError::BadLink {
                cause: "index needs a channel link",
            });
        }
        if Some(link.channel()) == self.identity() {
            return Err(DistError::OwnChannel);
        }
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let report = self.read_channel(link.channel(), link.base())?;
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
        let time = follow::now();
        let r = next.entry(link.channel()).or_insert(Follow {
            base: link.base().into(),
            followed: false,
            index: true,
            kept: false,
            bases: Vec::new(),
            given: Vec::new(),
            _followed_at: time,
            _last_attempt: time,
            last_success: None,
        });
        follow::adopt_given_base(r, link.base());
        r.index = true;
        let learned = follow::learn_base(r, link.base());
        r._last_attempt = time;
        if matches!(report.outcome, Outcome::Current { .. }) {
            r.last_success = Some(time);
        }
        self.save_follows(&next)?;
        *records = next;
        drop(records);
        let report = self.after_learning(link.channel(), report, learned)?;
        let mut reports = vec![report];
        let mut seen = BTreeSet::from([link.channel()]);
        self.read_listed(&mut reports, &mut seen)
            .map_err(|error| error.1)?;
        Ok(reports)
    }
    pub(crate) fn read_listed(
        &self,
        reports: &mut Vec<RefreshReport>,
        seen: &mut BTreeSet<PublicKey>,
    ) -> Result<(), ChannelError> {
        let explicit: BTreeSet<_> = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .iter()
            .filter(|(_, r)| r.followed || r.index)
            .map(|(key, _)| *key)
            .collect();
        let mut read = BTreeSet::new();
        let mut per_index: BTreeMap<PublicKey, BTreeSet<PublicKey>> = BTreeMap::new();
        for indirect in [false, true] {
            let walk = self.walk_channels()?;
            let keys = if indirect {
                &walk.indirect
            } else {
                &walk.direct
            };
            for key in keys {
                if !explicit.contains(key) && !seen.contains(key) {
                    let owners: Vec<_> = walk
                        .sets
                        .iter()
                        .filter(|s| {
                            s.whole.iter().any(|(k, _)| k == key)
                                || s.mods.iter().any(|m| m.channel == *key)
                        })
                        .map(|s| s.index)
                        .collect();
                    if read.len() >= self.options.max_listed
                        || owners.iter().all(|i| {
                            per_index
                                .get(i)
                                .is_some_and(|keys| keys.len() >= self.options.max_listed_per_index)
                        })
                    {
                        continue;
                    }
                    read.insert(*key);
                    for owner in owners {
                        per_index.entry(owner).or_default().insert(*key);
                    }
                }
                self.sync_listed(*key, &walk.bases[key])
                    .map_err(|cause| Box::new((*key, cause)))?;
                if seen.insert(*key) {
                    reports.push(
                        self.read_record(*key)
                            .map_err(|cause| Box::new((*key, cause)))?,
                    );
                }
            }
        }
        self.prune_listed_channels()?;
        Ok(())
    }
    fn sync_listed(&self, key: PublicKey, bases: &[String]) -> Result<(), DistError> {
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        let record = next.entry(key).or_insert(Follow {
            base: bases[0].clone(),
            followed: false,
            index: false,
            kept: false,
            bases: Vec::new(),
            given: Vec::new(),
            _followed_at: follow::now(),
            _last_attempt: 0,
            last_success: None,
        });
        if !record.followed && !record.index {
            let mut ordered: Vec<_> = record
                .bases
                .iter()
                .filter(|b| bases.contains(b))
                .cloned()
                .collect();
            ordered.extend(bases.iter().filter(|b| !record.bases.contains(b)).cloned());
            record.bases = ordered;
            record.base = record.bases[0].clone();
        }
        self.save_follows(&next)?;
        *records = next;
        Ok(())
    }
    pub(crate) fn read_record(&self, key: PublicKey) -> Result<RefreshReport, DistError> {
        let record = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
            .cloned()
            .ok_or(DistError::NotFollowed {
                channel: key,
                local_only: false,
            })?;
        if record.kept && record.base.is_empty() && record.bases.is_empty() {
            return Ok(RefreshReport {
                channel: key,
                outcome: Outcome::Current {
                    head: self.log.head(&key)?.map_or(0, |h| h.seq),
                },
                changes: Vec::new(),
            });
        }
        let bases = if record.bases.is_empty() {
            vec![record.base.clone()]
        } else {
            record.bases.clone()
        };
        let mut best = None;
        let mut changes = Vec::new();
        let mut highest: Option<(u64, String)> = None;
        let mut any_current = false;
        for base in bases {
            let listed = follow::listed_read(&record, &base);
            if listed && self.check_listed_base(&base).is_err() {
                continue;
            }
            let mut report = self
                .read_channel_mode(key, &base, listed)
                .unwrap_or_else(|cause| RefreshReport {
                    channel: key,
                    outcome: Outcome::Failed { cause },
                    changes: Vec::new(),
                });
            changes.append(&mut report.changes);
            let current = matches!(report.outcome, Outcome::Current { .. });
            any_current |= current;
            let stop = matches!(report.outcome, Outcome::Compromised { .. });
            let served = match report.outcome {
                Outcome::Current { head } => Some(head),
                Outcome::Behind { served, .. } => Some(served),
                _ => None,
            };
            if let Some(head) = served
                && highest.as_ref().is_none_or(|(old, _)| head > *old)
            {
                highest = Some((head, base.clone()));
            }
            let rank = |r: &RefreshReport| match r.outcome {
                Outcome::Compromised { .. } => (4, 0),
                Outcome::Current { head } => (3, head),
                Outcome::Behind { served, .. } => (2, served),
                Outcome::Failed { .. } => (1, 0),
            };
            if best.as_ref().is_none_or(|old| rank(&report) > rank(old)) {
                best = Some(report);
            }
            if stop {
                break;
            }
        }
        if highest.is_some() || any_current {
            let mut records = self
                .follows
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let mut next = records.clone();
            let r = next.get_mut(&key).expect("listed record");
            if let Some((_, base)) = highest.filter(|_| !r.bases.is_empty()) {
                r.bases.retain(|b| b != &base);
                r.bases.insert(0, base.clone());
                r.base = base;
            }
            if any_current {
                r.last_success = Some(r._last_attempt);
            }
            self.save_follows(&next)?;
            *records = next;
        }
        let mut report = best.unwrap_or(RefreshReport {
            channel: key,
            outcome: Outcome::Failed {
                cause: DistError::BadLink {
                    cause: "listed channel has no permitted base",
                },
            },
            changes: Vec::new(),
        });
        report.changes = changes;
        Ok(report)
    }
    pub(crate) fn prune_listed(&self) -> Result<(), DistError> {
        self.prune_listed_channels().map_err(|error| error.1)
    }
    fn prune_listed_channels(&self) -> Result<(), ChannelError> {
        let walk = self.walk_channels()?;
        let stored: BTreeSet<_> = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .keys()
            .copied()
            .collect();
        for (key, bases) in &walk.bases {
            if stored.contains(key) {
                self.sync_listed(*key, bases)
                    .map_err(|cause| Box::new((*key, cause)))?;
            }
        }
        let mut records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut next = records.clone();
        let removed = next
            .iter()
            .find(|(k, r)| !r.followed && !r.index && !r.kept && !walk.bases.contains_key(k))
            .map(|(k, _)| *k);
        let channel = removed
            .or_else(|| walk.bases.keys().next().copied())
            .or_else(|| next.keys().next().copied())
            .or_else(|| self.identity());
        let Some(channel) = channel else {
            return Ok(());
        };
        next.retain(|k, r| r.followed || r.index || r.kept || walk.bases.contains_key(k));
        for (key, record) in &mut next {
            if record.kept && !record.followed && !record.index && !walk.bases.contains_key(key) {
                record.base.clear();
                record.bases.clear();
            }
        }
        self.save_follows(&next)
            .map_err(|cause| Box::new((channel, cause)))?;
        *records = next;
        Ok(())
    }
}
#[derive(Default)]
struct Walk {
    sets: Vec<ReadSet>,
    bases: BTreeMap<PublicKey, Vec<String>>,
    direct: BTreeSet<PublicKey>,
    indirect: BTreeSet<PublicKey>,
}
impl Walk {
    fn add(
        &mut self,
        l: &ListingInfo,
        through: Option<PublicKey>,
        set: &mut ReadSet,
        indirect: bool,
    ) {
        let key = l.listing.link.channel();
        let bases = self.bases.entry(key).or_default();
        let base = l.listing.link.base().to_owned();
        if !bases.contains(&base) && bases.len() < 4 {
            bases.push(base);
        }
        if indirect {
            self.indirect.insert(key);
        } else {
            self.direct.insert(key);
        }
        if l.include {
            set.whole.push((key, Some(key)));
        } else {
            set.mods.push(ListedMod {
                channel: key,
                slug: l
                    .listing
                    .link
                    .slug()
                    .expect("a mod listing links to a slug")
                    .clone(),
                through,
                note: l.listing.note.clone(),
            });
        }
    }
}

pub(crate) fn local_base(base: &str) -> bool {
    fn local_v4(ip: std::net::Ipv4Addr) -> bool {
        ip.is_loopback() || ip.is_unspecified() || ip.is_private() || ip.is_link_local()
    }
    match url::Url::parse(base)
        .ok()
        .and_then(|u| u.host().map(|h| h.to_owned()))
    {
        Some(url::Host::Domain(name)) => {
            // `localhost.` is the same name: trailing dots do not make a host non-local.
            let name = name.trim_end_matches('.');
            name == "localhost" || name.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => local_v4(ip),
        Some(url::Host::Ipv6(ip)) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || (ip.segments()[0] & 0xfe00 == 0xfc00)
                || (ip.segments()[0] & 0xffc0 == 0xfe80)
                || ip.to_ipv4_mapped().is_some_and(local_v4)
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac8_local_literal_hosts() {
        for host in [
            "localhost",
            "a.localhost",
            "127.0.0.1",
            "0.0.0.0",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "[::1]",
            "[fc00::1]",
            "[fe80::1]",
            "169.254.1.1",
            "localhost.",
            "a.localhost.",
            "localhost..",
            "127.0.0.1.",
        ] {
            assert!(local_base(&format!("http://{host}/")), "{host}");
        }
        for host in ["example.org", "example.org.", "8.8.8.8", "172.32.0.1"] {
            assert!(!local_base(&format!("http://{host}/")), "{host}");
        }
    }
}
