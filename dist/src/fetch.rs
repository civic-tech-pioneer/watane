//! Fetch (TECHNICAL 5.7): get a package from a channel's bases, verify it
//! against its release before use, and keep it in the package cache.

use crate::{
    ChannelView, Dist, DistError, Fetched, Link, Outcome, Package, PackageError, PackageId,
    PublicKey, RefreshReport, ReleaseInfo, ReleaseRef, Status, http, index, site, view,
};
use std::path::PathBuf;
use tempfile::TempDir;

const MAX_PROVIDER_BASES: usize = 4;

impl Dist {
    pub(crate) fn unpacked(
        &self,
        package: &Package,
    ) -> Result<(TempDir, watane_pkg::Mod), PackageError> {
        let path = self.home.join("dist/tmp");
        let tmp = TempDir::new_in(&path).map_err(|source| PackageError::Io { path, source })?;
        let m = package.unpack(&tmp.path().join("mod"))?;
        Ok((tmp, m))
    }
    pub(crate) fn cache_path(&self, id: &PackageId) -> PathBuf {
        self.home.join("dist/packages").join(id.to_string())
    }
    pub(crate) fn cache_read(&self, id: &PackageId) -> Result<Vec<u8>, DistError> {
        let path = self.cache_path(id);
        let bytes = site::read_plain(&path)?.ok_or_else(|| DistError::Corrupt {
            path: path.clone(),
            cause: "missing package".into(),
        })?;
        if PackageId::of(&bytes) != *id {
            return Err(DistError::Corrupt {
                path,
                cause: "package hash mismatch".into(),
            });
        }
        Ok(bytes)
    }
    pub(crate) fn cache_write(&self, package: &Package) -> Result<bool, DistError> {
        let path = self.cache_path(&package.id());
        let existed = path
            .try_exists()
            .map_err(|source| site::io(&path, source))?;
        site::write_equal(&path, package.as_bytes(), 0o600)?;
        Ok(!existed)
    }
}

impl Dist {
    pub fn fetch(&self, reference: &ReleaseRef) -> Result<Fetched, DistError> {
        self.fetch_for(reference, None)
    }
    pub fn fetch_for(
        &self,
        reference: &ReleaseRef,
        consumer: Option<&PublicKey>,
    ) -> Result<Fetched, DistError> {
        let key = PublicKey::from_bytes(reference.publisher.as_bytes()).map_err(|cause| {
            DistError::BadKey {
                key: reference.publisher.clone(),
                cause,
            }
        })?;
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let view = self.channel(&key)?;
        Self::check_channel(&view)?;
        let record = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
            .cloned();
        let stored = view.releases.iter().any(|r| r.reference == *reference);
        let base = if Some(key) == self.identity()
            || record.as_ref().is_some_and(|r| r.kept && r.base.is_empty())
        {
            None
        } else if record.as_ref().is_none_or(|r| !r.followed && !r.index)
            && view
                .releases
                .iter()
                .find(|r| r.reference == *reference)
                .map(|r| {
                    site::read_plain(&self.cache_path(&r.release.package)).map(|bytes| {
                        bytes.is_some_and(|bytes| PackageId::of(&bytes) == r.release.package)
                    })
                })
                .transpose()?
                .unwrap_or(false)
        {
            let (mut bases, _) = self.provider_bases(key, consumer)?;
            if bases.len() < MAX_PROVIDER_BASES
                && let Some(base) = record.as_ref().map(|r| &r.base).filter(|base| {
                    !base.is_empty()
                        && !bases.contains(base)
                        && self.check_listed_base(base).is_ok()
                })
            {
                bases.push(base.clone());
            }
            return self.fetch_from_providers(key, reference, &bases, true);
        } else if record
            .as_ref()
            .is_none_or(|r| !r.followed && !r.index && !stored)
        {
            let (bases, local_only) = self.provider_bases(key, consumer)?;
            if bases.is_empty() {
                return if stored {
                    self.fetched(reference, None)
                } else {
                    Err(DistError::NotFollowed {
                        channel: key,
                        local_only,
                    })
                };
            }
            return self.fetch_from_providers(key, reference, &bases, false);
        } else {
            let record = record.expect("record selected");
            if !record.followed && !record.index {
                self.check_listed_base(&record.base)?;
            }
            (!record.base.is_empty()).then_some(record.base)
        };
        self.fetched(reference, base.as_deref())
    }

    fn fetch_from_providers(
        &self,
        key: PublicKey,
        reference: &ReleaseRef,
        bases: &[String],
        cached: bool,
    ) -> Result<Fetched, DistError> {
        let mut channel_failure = None;
        let mut package_failure = None;
        let mut answered = 0;
        for base in bases {
            match self.read_channel_entries(key, base, true) {
                Ok(RefreshReport {
                    outcome: Outcome::Compromised { seq },
                    ..
                }) => return Err(DistError::Compromised { channel: key, seq }),
                Ok(RefreshReport {
                    outcome: Outcome::Failed { cause },
                    ..
                })
                | Err(cause) => {
                    channel_failure.get_or_insert(cause);
                    continue;
                }
                Ok(_) => answered += 1,
            }
            match self.fetched(reference, Some(base)) {
                Err(DistError::UnknownRelease { .. }) => continue,
                Err(
                    cause @ (DistError::NotFound { .. }
                    | DistError::Timeout { .. }
                    | DistError::Network { .. }),
                ) => {
                    package_failure.get_or_insert(cause);
                }
                result => return result,
            }
        }
        if cached {
            return self.fetched(reference, None);
        }
        if let Some(cause) = package_failure {
            return Err(cause);
        }
        if answered == 0 {
            return Err(channel_failure.expect("every provider base failed"));
        }
        if self
            .channel(&key)?
            .releases
            .iter()
            .any(|r| r.reference.slug == reference.slug)
        {
            Err(DistError::StaleBases {
                release: reference.clone(),
                bases: answered,
                slug_held: true,
            })
        } else {
            Err(DistError::NoRelease {
                channel: key,
                slug: reference.slug.clone(),
            })
        }
    }

    fn check_channel(view: &ChannelView) -> Result<(), DistError> {
        if let Status::Compromised { seq } = view.status {
            return Err(DistError::Compromised {
                channel: view.channel,
                seq,
            });
        }
        Ok(())
    }

    fn check_listed_base(&self, base: &str) -> Result<(), DistError> {
        if !self.options.listed_local && index::local_base(base) {
            return Err(DistError::BadUrl {
                url: base.into(),
                cause: "local address",
            });
        }
        Ok(())
    }

    fn provider_bases(
        &self,
        wanted: PublicKey,
        consumer: Option<&PublicKey>,
    ) -> Result<(Vec<String>, bool), DistError> {
        let records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let mut channels: Vec<_> = self.identity().into_iter().collect();
        channels.extend(consumer.copied());
        for class in 0..3 {
            channels.extend(
                records
                    .iter()
                    .filter(|(_, r)| match class {
                        0 => r.followed,
                        1 => r.index,
                        _ => !r.followed && !r.index,
                    })
                    .map(|(key, _)| *key),
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut bases = Vec::new();
        let mut skipped_local = false;
        for key in channels {
            if !seen.insert(key)
                || matches!(self.log.status(&key)?, watane_log::Status::Compromised(_))
            {
                continue;
            }
            let view = view::derive(key, &self.log.entries(&key, 0, usize::MAX)?);
            for listing in view
                .listings
                .iter()
                .filter(|l| l.unlisted.is_none() && l.listing.link.channel() == wanted)
            {
                let base = listing.listing.link.base();
                if self.check_listed_base(base).is_err() {
                    skipped_local = true;
                } else if !bases.iter().any(|b| b == base) {
                    bases.push(base.to_owned());
                    if bases.len() == MAX_PROVIDER_BASES {
                        return Ok((bases, false));
                    }
                }
            }
        }
        let local_only = bases.is_empty() && skipped_local;
        Ok((bases, local_only))
    }

    fn fetched(&self, reference: &ReleaseRef, base: Option<&str>) -> Result<Fetched, DistError> {
        let key = PublicKey::from_bytes(reference.publisher.as_bytes()).map_err(|cause| {
            DistError::BadKey {
                key: reference.publisher.clone(),
                cause,
            }
        })?;
        let view = self.channel(&key)?;
        Self::check_channel(&view)?;
        let info = view
            .releases
            .into_iter()
            .find(|r| r.reference == *reference)
            .ok_or_else(|| DistError::UnknownRelease {
                release: reference.clone(),
            })?;
        if let Some(withdrawn) = &info.withdrawn {
            return Err(DistError::Withdrawn {
                release: reference.clone(),
                reason: withdrawn.reason.clone(),
            });
        }
        let package = self.fetch_package(&info, base)?;
        let entry = self.log.entries(&key, info.seq, 1)?.remove(0);
        Ok(Fetched {
            link: base
                .map(|b| Link::new(key, b, Some(&reference.slug)))
                .transpose()?,
            package,
            entry,
            info,
            publisher_name: view.name,
            checked_at: view.last_success,
        })
    }
    pub(crate) fn fetch_package(
        &self,
        info: &ReleaseInfo,
        base: Option<&str>,
    ) -> Result<Package, DistError> {
        let id = &info.release.package;
        let path = self.cache_path(id);
        let cached = site::read_plain(&path)?;
        let bytes = match cached {
            Some(bytes) if PackageId::of(&bytes) == *id => Some(bytes),
            Some(_) => {
                std::fs::remove_file(&path).map_err(|e| site::io(&path, e))?;
                None
            }
            None => None,
        };
        let url = base
            .map(|b| http::file_url(b, &format!("packages/{id}")))
            .unwrap_or_else(|| format!("cache:{id}"));
        let bytes = match bytes {
            Some(bytes) => bytes,
            None => {
                if base.is_none() {
                    return Err(DistError::NotFollowed {
                        channel: info.channel,
                        local_only: false,
                    });
                }
                self.get(
                    &url,
                    info.release.size.min(self.options.max_package),
                    self.options.package_timeout,
                    false,
                )?
            }
        };
        let found = PackageId::of(&bytes);
        if found != *id {
            return Err(DistError::HashMismatch {
                url,
                expected: id.clone(),
                found,
            });
        }
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
        Ok(package)
    }
    pub fn read_link(&self, link: &Link) -> Result<Fetched, DistError> {
        let slug = link.slug().ok_or(DistError::BadLink {
            cause: "a release link needs a slug",
        })?;
        let key = link.channel();
        let _guard = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        Self::check_channel(&self.channel(&key)?)?;
        let record = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(&key)
            .cloned();
        let mut base = (Some(key) != self.identity()).then(|| link.base().to_owned());
        if Some(key) != self.identity() {
            if let Some(record) = record {
                // Like open_link, a failed refresh can still use a verified stored release.
                self.read_record(key)?;
                // As open_link: the package comes from the record's base, never the link's.
                if !record.base.is_empty() && !record.followed && !record.index {
                    self.check_listed_base(&record.base)?;
                }
                base = (!record.base.is_empty()).then_some(record.base);
            } else {
                self.check_listed_base(link.base())?;
                match self.read_channel_entries(key, link.base(), true)?.outcome {
                    Outcome::Failed { cause } => return Err(cause),
                    Outcome::Compromised { seq } => {
                        return Err(DistError::Compromised { channel: key, seq });
                    }
                    _ => {}
                }
            }
        }
        let info = self
            .latest(&key, slug)?
            .ok_or_else(|| DistError::NoRelease {
                channel: key,
                slug: slug.clone(),
            })?;
        self.fetched(&info.reference, base.as_deref())
    }
    pub fn open_link(&self, link: &Link) -> Result<Fetched, DistError> {
        let slug = link.slug().ok_or(DistError::BadLink {
            cause: "a release link needs a slug",
        })?;
        if Some(link.channel()) != self.identity() {
            let followed = self
                .follows
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .contains_key(&link.channel());
            if followed {
                if !self.learn_link_base(link)? {
                    self.refresh_channel(&link.channel())?;
                }
            } else {
                self.follow(link)?;
            }
        }
        let info = self
            .latest(&link.channel(), slug)?
            .ok_or_else(|| DistError::NoRelease {
                channel: link.channel(),
                slug: slug.clone(),
            })?;
        self.fetch(&info.reference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Confirm, Decision, Options, PublisherKey, Release, ReleaseError, SignRequest};
    use std::path::Path;
    use std::sync::Arc;
    struct NoSign;
    impl Confirm for NoSign {
        fn confirm(&self, _: &SignRequest) -> Decision {
            panic!("fetch must not sign")
        }
    }
    #[test]
    fn ac1_fetch_checks_release_against_package() {
        let home = tempfile::tempdir().unwrap();
        let dist = Dist::open(home.path(), Arc::new(NoSign), Options::default()).unwrap();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list");
        let package = watane_pkg::pack(&dir, false).unwrap();
        let m = watane_pkg::Mod::read(&dir).unwrap();
        let mut release = Release::new(&m, &package).unwrap();
        release.name = "Does not match the package".into();
        // Preload valid package bytes; even a valid hash cannot replace Release::check.
        std::fs::write(dist.cache_path(&package.id()), package.as_bytes()).unwrap();
        let info = ReleaseInfo {
            reference: ReleaseRef {
                publisher: PublisherKey::from_bytes(*dist.identity().unwrap().as_bytes()),
                slug: release.slug.clone(),
                version: release.version.clone(),
            },
            channel: dist.identity().unwrap(),
            entry: "0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
            seq: 0,
            release,
            withdrawn: None,
        };
        assert!(matches!(
            dist.fetch_package(&info, None),
            Err(DistError::PackageMismatch {
                cause: ReleaseError::Mismatch { field: "name" },
                ..
            })
        ));
    }
}
