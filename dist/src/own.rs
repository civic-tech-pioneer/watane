use crate::{
    ChannelView, Decision, Dist, DistError, Entry, EntryId, Exported, Link, Listing, Outcome,
    Package, Profile, PublicKey, Published, PublisherKey, Release, ReleaseError, ReleaseInfo,
    ReleaseRef, SignRequest, Slug, Statement, Status, Withdraw, body, http, site, view,
};
impl Dist {
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError> {
        self.require_identity()?;
        body::text(name, 64).map_err(|cause| DistError::BadName { cause })?;
        let guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.sign_and_append(
            guard,
            "profile",
            Profile { name: name.into() }.encode(),
            None,
        )
        .map(|(entry, _)| entry)
    }
    fn sign_and_append(
        &self,
        _guard: std::sync::MutexGuard<'_, ()>,
        kind: &str,
        body: Vec<u8>,
        package: Option<&Package>,
    ) -> Result<(Entry, Exported), DistError> {
        let own = self.require_identity()?;
        if let watane_log::Status::Compromised(fork) = self.log.status(&own)? {
            return Err(DistError::Compromised {
                channel: own,
                seq: fork.chain.seq(),
            });
        }
        if !self.log.own_head_confirmed()? {
            return Err(DistError::OwnHeadUnconfirmed);
        }
        let statement = match kind {
            "profile" => Statement::Profile(
                Profile::decode(&body).map_err(|cause| DistError::BadName { cause })?,
            ),
            "withdraw" => {
                let withdraw =
                    Withdraw::decode(&body).map_err(|cause| DistError::BadReason { cause })?;
                let info = self
                    .own()?
                    .releases
                    .into_iter()
                    .find(|r| r.entry == withdraw.release)
                    .expect("withdraw checks own release under guard");
                Statement::Withdraw {
                    withdraw,
                    reference: info.reference,
                    release: info.release,
                }
            }
            "list" | "include" => {
                let listing =
                    Listing::decode(&body).map_err(|cause| DistError::BadNote { cause })?;
                if kind == "include" {
                    Statement::Include(listing)
                } else {
                    Statement::List(listing)
                }
            }
            "unlist" => {
                let entry = body::decode_unlist(&body).expect("unlist body encoded under guard");
                let info = self
                    .own()?
                    .listings
                    .into_iter()
                    .find(|l| l.entry == entry && l.unlisted.is_none())
                    .expect("unlist checks live listing under guard");
                Statement::Unlist {
                    entry,
                    listing: info.listing,
                    include: info.include,
                }
            }
            "release" => {
                let release = Release::decode(&body)?;
                Statement::Release {
                    reference: ReleaseRef {
                        publisher: PublisherKey::from_bytes(*own.as_bytes()),
                        slug: release.slug.clone(),
                        version: release.version.clone(),
                    },
                    release,
                }
            }
            _ => unreachable!("only known statement kinds are signed"),
        };
        let request = SignRequest {
            channel: own,
            fingerprint: own.fingerprint(),
            statement,
        };
        if self.confirm.confirm(&request) == Decision::Refuse {
            return Err(DistError::Refused);
        }
        let created = match package {
            Some(p) => self.cache_write(p)?,
            None => false,
        };
        let entry = match self.log.append(kind, &body) {
            Ok(entry) => entry,
            Err(error) => {
                if created && let Some(p) = package {
                    let _ = std::fs::remove_file(self.cache_path(&p.id()));
                }
                return Err(error.into());
            }
        };
        let site =
            self.export_unlocked(&self.site_dir())
                .map_err(|cause| DistError::SiteNotWritten {
                    entry: entry.id(),
                    cause: Box::new(cause),
                })?;
        Ok((entry, site))
    }
    pub fn publish(&self, bytes: Vec<u8>) -> Result<Published, DistError> {
        self.require_identity()?;
        if bytes.len() as u64 > self.options.max_package {
            return Err(DistError::PackageTooLarge {
                size: bytes.len() as u64,
                max: self.options.max_package,
            });
        }
        let package = Package::from_bytes(bytes)?;
        let (_tmp, m) = self.unpacked(&package)?;
        let release = Release::new(&m, &package).map_err(|error| match error {
            ReleaseError::LocalProvider { slug } => {
                DistError::UnreleasedProvider { provider: slug }
            }
            other => DistError::Release(other),
        })?;
        let guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(info) = self
            .own()?
            .releases
            .into_iter()
            .find(|r| r.release.slug == release.slug && r.release.version == release.version)
        {
            return Err(DistError::VersionExists {
                release: info.reference,
                seq: info.seq,
            });
        }
        let (entry, site) =
            self.sign_and_append(guard, "release", release.encode(), Some(&package))?;
        let info = self
            .own()?
            .releases
            .into_iter()
            .find(|r| r.entry == entry.id())
            .expect("appended release");
        let link = match self.own_link(Some(&release.slug)) {
            Ok(link) => Some(link),
            Err(DistError::NoSiteUrl) => None,
            Err(e) => return Err(e),
        };
        Ok(Published {
            entry,
            info,
            link,
            site,
        })
    }
    pub fn withdraw(&self, release: &ReleaseRef, reason: &str) -> Result<Entry, DistError> {
        self.require_identity()?;
        body::text(reason, 280).map_err(|cause| DistError::BadReason { cause })?;
        let guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let info = self
            .own()?
            .releases
            .into_iter()
            .find(|r| r.reference == *release)
            .ok_or_else(|| DistError::NotOwnRelease {
                release: release.clone(),
            })?;
        if let Some(withdrawal) = info.withdrawn {
            return Err(DistError::AlreadyWithdrawn {
                release: release.clone(),
                seq: withdrawal.seq,
            });
        }
        self.sign_and_append(
            guard,
            "withdraw",
            Withdraw {
                release: info.entry,
                reason: reason.into(),
            }
            .encode(),
            None,
        )
        .map(|(entry, _)| entry)
    }
    pub fn own(&self) -> Result<ChannelView, DistError> {
        self.channel(&self.require_identity()?)
    }
    pub fn channel(&self, key: &PublicKey) -> Result<ChannelView, DistError> {
        let entries = self.log.entries(key, 0, usize::MAX)?;
        let mut view = view::derive(*key, &entries);
        view.status = match self.log.status(key)? {
            watane_log::Status::Ok => Status::Ok,
            watane_log::Status::Compromised(fork) => Status::Compromised {
                seq: fork.chain.seq(),
            },
        };
        if Some(*key) == self.identity() {
            view.base = self
                .base
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .clone();
        } else if let Some(record) = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(key)
        {
            view.base = (!record.base.is_empty()).then(|| record.base.clone());
            view.kept = record.kept;
            view.followed = record.followed;
            view.index = record.index;
            view.last_attempt = Some(record._last_attempt);
            view.last_success = record.last_success;
        }
        if Some(*key) != self.identity() {
            view.listed_by = self
                .read_sets()?
                .iter()
                .filter(|s| {
                    s.whole.iter().any(|(k, _)| k == key)
                        || s.mods.iter().any(|m| m.channel == *key)
                })
                .map(|s| s.index)
                .collect();
            view.listed_by.sort();
        }
        Ok(view)
    }
    pub fn latest(
        &self,
        channel: &PublicKey,
        slug: &Slug,
    ) -> Result<Option<ReleaseInfo>, DistError> {
        Ok(self
            .channel(channel)?
            .releases
            .into_iter()
            .rev()
            .find(|r| r.reference.slug == *slug && r.withdrawn.is_none()))
    }
}

impl Dist {
    pub fn backup(&self, passphrase: &str) -> Result<Vec<u8>, DistError> {
        self.require_identity()?;
        Ok(self.log.export_identity(passphrase)?)
    }
    pub fn own_confirmed(&self) -> Result<bool, DistError> {
        self.require_identity()?;
        Ok(self.log.own_head_confirmed()?)
    }
    pub fn restore(&self, backup: &[u8], passphrase: &str) -> Result<PublicKey, DistError> {
        self.require_identity()?;
        let _guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let key = self.log.restore_identity(backup, passphrase)?;
        let mut base = self
            .base
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let path = self.home.join("dist/site.json");
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(site::io(&path, e)),
        }
        *base = None;
        Ok(key)
    }
    pub fn confirm_own(&self, own: Option<&Link>) -> Result<(), DistError> {
        self.require_identity()?;
        let _guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let _read = self
            .read_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(link) = own {
            let key = self.require_identity()?;
            if link.channel() != key {
                return Err(DistError::NotOwnLink {
                    found: link.channel(),
                });
            }
            match self.read_channel(key, link.base())?.outcome {
                Outcome::Current { .. } => {}
                Outcome::Failed { cause } => return Err(cause),
                Outcome::Compromised { seq } => {
                    return Err(DistError::Compromised { channel: key, seq });
                }
                Outcome::Behind { served, stored } => {
                    return Err(DistError::WrongSeq {
                        url: http::file_url(link.base(), "head"),
                        expected: stored,
                        found: served,
                    });
                }
            }
            // Restored homes have the entries but still need the verified packages to export.
            for info in self
                .own()?
                .releases
                .iter()
                .filter(|r| r.withdrawn.is_none())
            {
                self.fetch_package(info, Some(link.base()))?;
            }
            self.log.confirm_own_head()?;
            self.set_site_url(link.base())?;
            self.export_unlocked(&self.site_dir())?;
        } else {
            self.log.confirm_own_head()?;
        }
        Ok(())
    }
}

impl Dist {
    pub fn list(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.add_listing(link, note, false)
    }
    pub fn include(&self, link: &Link, note: &str) -> Result<Entry, DistError> {
        self.add_listing(link, note, true)
    }
    fn add_listing(&self, link: &Link, note: &str, include: bool) -> Result<Entry, DistError> {
        self.require_identity()?;
        body::note_text(note).map_err(|cause| DistError::BadNote { cause })?;
        if include == link.slug().is_some() {
            return Err(DistError::BadLink {
                cause: if include {
                    "include needs a channel link"
                } else {
                    "list needs a mod link"
                },
            });
        }
        let key = link.channel();
        if Some(key) == self.identity() {
            return Err(DistError::OwnListing);
        }
        let guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let Some(old) = self.own()?.listings.iter().find(|l| {
            l.unlisted.is_none()
                && l.include == include
                && l.listing.link.channel() == key
                && l.listing.link.slug() == link.slug()
        }) {
            return Err(DistError::AlreadyListed { seq: old.seq });
        }
        {
            let _read = self
                .read_mutex
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            if self.log.head(&key)?.is_none() {
                match self.read_channel(key, link.base())?.outcome {
                    Outcome::Current { .. } | Outcome::Behind { .. } => {}
                    Outcome::Failed { cause } => return Err(cause),
                    Outcome::Compromised { seq } => {
                        return Err(DistError::Compromised { channel: key, seq });
                    }
                }
            }
        }
        let channel = self.channel(&key)?;
        if let Status::Compromised { seq } = channel.status {
            return Err(DistError::Compromised { channel: key, seq });
        }
        if let Some(slug) = link.slug()
            && !channel
                .releases
                .iter()
                .any(|r| r.reference.slug == *slug && r.withdrawn.is_none())
        {
            return Err(DistError::NoRelease {
                channel: key,
                slug: slug.clone(),
            });
        }
        self.sign_and_append(
            guard,
            if include { "include" } else { "list" },
            Listing {
                link: link.clone(),
                note: note.into(),
            }
            .encode(),
            None,
        )
        .map(|(entry, _)| entry)
    }
    pub fn unlist(&self, listing: &EntryId) -> Result<Entry, DistError> {
        self.require_identity()?;
        let guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if !self
            .own()?
            .listings
            .iter()
            .any(|l| l.entry == *listing && l.unlisted.is_none())
        {
            return Err(DistError::NotListed { entry: *listing });
        }
        self.sign_and_append(guard, "unlist", body::encode_unlist(listing), None)
            .map(|(entry, _)| entry)
    }
}

impl Dist {
    pub fn drop_identity(&self) -> Result<(), DistError> {
        let _guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.require_identity()?;
        self.log.drop_identity()?;
        let path = self.home.join("dist/site.json");
        match std::fs::remove_file(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(site::io(&path, e)),
        }
        *self
            .base
            .lock()
            .expect("lock poisoned: another thread panicked while holding it") = None;
        for item in std::fs::read_dir(self.site_dir()).map_err(|e| site::io(&self.site_dir(), e))? {
            let path = item.map_err(|e| site::io(&self.site_dir(), e))?.path();
            let result = if path
                .symlink_metadata()
                .map_err(|e| site::io(&path, e))?
                .is_dir()
            {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            result.map_err(|e| site::io(&path, e))?;
        }
        Ok(())
    }
    pub fn confirm_own_head(&self, seq: u64, id: &EntryId) -> Result<(), DistError> {
        let _guard = self
            .own_mutex
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let key = self.require_identity()?;
        if self.log.own_head_confirmed()? {
            return Ok(());
        }
        if let watane_log::Status::Compromised(f) = self.log.status(&key)? {
            return Err(DistError::Compromised {
                channel: key,
                seq: f.chain.seq(),
            });
        }
        if !self
            .log
            .head(&key)?
            .is_some_and(|h| h.seq == seq && h.id == *id)
        {
            return Err(DistError::HeadNotStored { seq });
        }
        self.log.confirm_own_head()?;
        self.export_unlocked(&self.site_dir())
            .map_err(|cause| DistError::SiteNotWritten {
                entry: *id,
                cause: Box::new(cause),
            })?;
        Ok(())
    }
}
