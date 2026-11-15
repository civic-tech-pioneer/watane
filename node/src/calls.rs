//! The wrappers of TECHNICAL 3. With net off, each is dist's call of the same name.
use crate::Node;
use watane_dist::{
    DistError, Entry, EntryId, Fetched, Link, PublicKey, Published, RefreshReport, ReleaseRef,
};

impl Node {
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError> {
        self.dist.set_name(name)
    }
    pub fn publish(&self, package: Vec<u8>) -> Result<Published, DistError> {
        self.dist.publish(package)
    }
    pub fn withdraw(&self, release: &ReleaseRef, reason: &str) -> Result<Entry, DistError> {
        self.dist.withdraw(release, reason)
    }
    pub fn list(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.dist.list(link, note)
    }
    pub fn include(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.dist.include(link, note)
    }
    pub fn unlist(&self, listing: &EntryId) -> Result<Entry, DistError> {
        self.dist.unlist(listing)
    }
    pub fn confirm_own(&self, own: Option<&Link>) -> Result<(), DistError> {
        self.dist.confirm_own(own)
    }
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError> {
        self.dist.follow(link)
    }
    pub fn refresh(&self) -> Vec<RefreshReport> {
        self.dist.refresh()
    }
    pub fn refresh_channel(&self, channel: &PublicKey) -> Result<RefreshReport, DistError> {
        self.dist.refresh_channel(channel)
    }
    pub fn choose_index(&self, link: &Link) -> Result<Vec<RefreshReport>, DistError> {
        self.dist.choose_index(link)
    }
    pub fn fetch(&self, release: &ReleaseRef) -> Result<Fetched, DistError> {
        self.dist.fetch(release)
    }
    pub fn fetch_for(
        &self,
        release: &ReleaseRef,
        consumer: Option<&PublicKey>,
    ) -> Result<Fetched, DistError> {
        self.dist.fetch_for(release, consumer)
    }
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError> {
        self.dist.open_link(link)
    }
    pub fn read_link(&self, link: &Link) -> Result<Fetched, DistError> {
        self.dist.read_link(link)
    }
}
