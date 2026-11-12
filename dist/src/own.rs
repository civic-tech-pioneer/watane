use crate::*;
impl Dist {
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError> {
        body::text(name, 64).map_err(|cause| DistError::BadName { cause })?;
        let guard = self
            .own_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let own = self.identity();
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
            channel: self.identity(),
            fingerprint: self.fingerprint(),
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        body::text(reason, 280).map_err(|cause| DistError::BadReason { cause })?;
        let guard = self
            .own_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        self.channel(&self.identity())
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
        if *key == self.identity() {
            view.base = self
                .base
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
        } else if let Some(record) = self
            .follows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
        {
            view.base = Some(record.base.clone());
            view.followed = true;
            view.last_success = record.last_success;
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
        Ok(self.log.export_identity(passphrase)?)
    }
    pub fn own_confirmed(&self) -> Result<bool, DistError> {
        Ok(self.log.own_head_confirmed()?)
    }
    pub fn restore(&self, backup: &[u8], passphrase: &str) -> Result<PublicKey, DistError> {
        let _guard = self
            .own_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = self.log.restore_identity(backup, passphrase)?;
        let mut base = self
            .base
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _guard = self
            .own_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _read = self
            .read_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(link) = own {
            let key = self.identity();
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
