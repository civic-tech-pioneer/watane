//! The wrappers of TECHNICAL 3: dist's call, with `told` after it and, on a light
//! machine, `need` before it (5.1 to 5.3).
use crate::{Machine, Node, NodeError, OwnHint, convert};
use std::collections::BTreeMap;
use watane_dist::{
    DistError, Entry, EntryId, Fetched, Link, PublicKey, Published, RefreshReport, ReleaseRef,
};
use watane_net::{DeviceList, PairCode};

type Heads = BTreeMap<PublicKey, (u64, EntryId)>;

/// The heads in `after` that are new or higher than in `before`, in key order.
pub(crate) fn raised(before: &Heads, after: &Heads) -> Vec<(PublicKey, u64, EntryId)> {
    after
        .iter()
        .filter(|(key, (seq, _))| before.get(key).is_none_or(|(old, _)| seq > old))
        .map(|(key, (seq, id))| (*key, *seq, *id))
        .collect()
}

impl Node {
    fn heads_now(&self) -> Heads {
        self.dist
            .heads(None)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(key, head)| head.map(|h| (key, (h.seq, h.id))))
            .collect()
    }

    /// A read of a web host: `told` for each head it raised (5.2).
    fn reading<T>(&self, read: impl FnOnce() -> T) -> T {
        let before = self.heads_now();
        let result = read();
        for (key, seq, id) in raised(&before, &self.heads_now()) {
            self.net.told(&key, seq, &id);
        }
        result
    }

    /// An own statement: `told` for the entry it appended (5.1).
    fn signed<T>(
        &self,
        result: Result<T, DistError>,
        entry: impl Fn(&T) -> &Entry,
    ) -> Result<T, DistError> {
        let value = result?;
        if let Some(own) = self.dist.identity() {
            let e = entry(&value);
            self.net.told(&own, e.seq(), &e.id());
        }
        Ok(value)
    }

    fn is_light(&self) -> bool {
        self.machine() == Machine::Light
    }

    fn need_channel(&self, channel: &PublicKey) {
        if self.is_light() {
            let _ = self.net.need(channel);
        }
    }

    /// The package of a stored release, from a full machine when this one lacks it.
    fn need_package(&self, package: &watane_dist::PackageId, size: u64) {
        if self.dist.package_file(package).is_some() {
            return;
        }
        let id = convert::blob_id(package);
        self.holdings.ask(id, size);
        if self.net.need_blob(&id, size).is_ok() && self.dist.package_file(package).is_some() {
            self.holdings.asked_done(&id);
        }
    }

    fn need_release(&self, release: &ReleaseRef) {
        if !self.is_light() {
            return;
        }
        let Ok(channel) = PublicKey::from_bytes(release.publisher.as_bytes()) else {
            return;
        };
        let stored = self
            .dist
            .channel(&channel)
            .ok()
            .and_then(|view| view.releases.into_iter().find(|r| r.reference == *release));
        let stored = stored.or_else(|| {
            let _ = self.net.need(&channel);
            self.dist
                .channel(&channel)
                .ok()?
                .releases
                .into_iter()
                .find(|r| r.reference == *release)
        });
        if let Some(info) = stored {
            self.need_package(&info.release.package, info.release.size);
        }
    }

    fn need_latest(&self, link: &Link) {
        if !self.is_light() {
            return;
        }
        let channel = link.channel();
        let _ = self.net.need(&channel);
        if let Some(slug) = link.slug()
            && let Ok(Some(info)) = self.dist.latest(&channel, slug)
        {
            self.need_package(&info.release.package, info.release.size);
        }
    }

    pub fn set_name(&self, name: &str) -> Result<Entry, DistError> {
        self.signed(self.dist.set_name(name), |e| e)
    }
    pub fn publish(&self, package: Vec<u8>) -> Result<Published, DistError> {
        self.signed(self.dist.publish(package), |p| &p.entry)
    }
    pub fn withdraw(&self, release: &ReleaseRef, reason: &str) -> Result<Entry, DistError> {
        self.signed(self.dist.withdraw(release, reason), |e| e)
    }
    pub fn list(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.signed(self.dist.list(link, note), |e| e)
    }
    pub fn include(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.signed(self.dist.include(link, note), |e| e)
    }
    pub fn unlist(&self, listing: &EntryId) -> Result<Entry, DistError> {
        self.signed(self.dist.unlist(listing), |e| e)
    }
    pub fn confirm_own(&self, own: Option<&Link>) -> Result<(), DistError> {
        self.dist.confirm_own(own)?;
        if let Some(key) = self.dist.identity()
            && let Some((_, Some(head))) = self
                .dist
                .heads(Some(&[key]))
                .ok()
                .and_then(|h| h.into_iter().next())
        {
            self.net.told(&key, head.seq, &head.id);
        }
        Ok(())
    }
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError> {
        self.need_channel(&link.channel());
        self.reading(|| self.dist.follow(link))
    }
    pub fn refresh(&self) -> Vec<RefreshReport> {
        self.reading(|| self.dist.refresh())
    }
    pub fn refresh_channel(&self, channel: &PublicKey) -> Result<RefreshReport, DistError> {
        self.need_channel(channel);
        self.reading(|| self.dist.refresh_channel(channel))
    }
    pub fn choose_index(&self, link: &Link) -> Result<Vec<RefreshReport>, DistError> {
        self.need_channel(&link.channel());
        self.reading(|| self.dist.choose_index(link))
    }
    pub fn fetch(&self, release: &ReleaseRef) -> Result<Fetched, DistError> {
        self.need_release(release);
        self.reading(|| self.dist.fetch(release))
    }
    pub fn fetch_for(
        &self,
        release: &ReleaseRef,
        consumer: Option<&PublicKey>,
    ) -> Result<Fetched, DistError> {
        self.need_release(release);
        self.reading(|| self.dist.fetch_for(release, consumer))
    }
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError> {
        self.need_latest(link);
        self.reading(|| self.dist.open_link(link))
    }
    pub fn read_link(&self, link: &Link) -> Result<Fetched, DistError> {
        self.need_latest(link);
        self.reading(|| self.dist.read_link(link))
    }

    /// After an install of `fetched` (NODE-F-11): a replica keeps a channel that has no link.
    pub fn installed(&self, fetched: &Fetched) -> Result<(), DistError> {
        let replica = matches!(self.machine(), Machine::Full | Machine::Light);
        let channel = fetched.info.channel;
        if replica && fetched.link.is_none() && !self.dist.channel(&channel)?.followed {
            self.dist.keep(&channel)?;
        }
        Ok(())
    }

    /// Joins the participant's machines (5.5).
    pub fn join(&self, code: &PairCode, replace: bool) -> Result<DeviceList, NodeError> {
        if let Some(fingerprint) = self.dist.fingerprint() {
            if self.dist.own()?.head.is_some() {
                return Err(NodeError::IdentityInUse { fingerprint });
            }
            self.dist.drop_identity()?;
        }
        Ok(self.net.join(code, replace)?)
    }

    pub fn own_hints(&self) -> Vec<OwnHint> {
        let Some(own) = self.dist.identity() else {
            return Vec::new();
        };
        self.net
            .own_heads(&own)
            .into_iter()
            .map(|(node, head)| OwnHint { node, head })
            .collect()
    }

    pub fn confirm_own_hint(&self, hint: &OwnHint) -> Result<(), NodeError> {
        let head = hint.head.ok_or(NodeError::EmptyHint)?;
        let own = self.dist.identity().ok_or(DistError::NoIdentity)?;
        let _ = self.net.need(&own);
        if let Ok(view) = self.dist.own() {
            for info in view.releases.iter().filter(|r| r.withdrawn.is_none()) {
                self.need_package(&info.release.package, info.release.size);
            }
        }
        Ok(self.dist.confirm_own_head(head.seq, &head.id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac7_raised_heads_come_in_key_order_and_only_when_higher() {
        let ids: Vec<_> = (0..4u8)
            .map(|n| watane_dist::EntryId::from_bytes([n; 32]))
            .collect();
        let keys: Vec<_> = (1..=255u8)
            .filter_map(|m| PublicKey::from_bytes(&[m; 32]).ok())
            .take(3)
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        let (a, b, c) = (sorted[0], sorted[1], sorted[2]);
        let before: Heads = [(a, (1, ids[0])), (b, (5, ids[1]))].into();
        let after: Heads = [(c, (0, ids[3])), (a, (2, ids[2])), (b, (5, ids[1]))].into();
        assert_eq!(
            raised(&before, &after),
            vec![(a, 2, ids[2]), (c, 0, ids[3])]
        );
        assert!(raised(&after, &after).is_empty());
    }
}
