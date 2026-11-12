use crate::*;
impl Dist {
    pub fn set_name(&self, name: &str) -> Result<Entry, DistError> {
        body::text(name, 64).map_err(|cause| DistError::BadName { cause })?;
        let guard = self
            .own_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.sign_and_append(guard, "profile", Profile { name: name.into() }.encode())
    }
    fn sign_and_append(
        &self,
        _guard: std::sync::MutexGuard<'_, ()>,
        kind: &str,
        body: Vec<u8>,
    ) -> Result<Entry, DistError> {
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
            _ => unreachable!("only profile and withdraw are signed in S1/S2"),
        };
        let request = SignRequest {
            channel: self.identity(),
            fingerprint: self.fingerprint(),
            statement,
        };
        if self.confirm.confirm(&request) == Decision::Refuse {
            return Err(DistError::Refused);
        }
        let entry = self.log.append(kind, &body)?;
        self.after_append(&entry);
        Ok(entry)
    }
    // S21a-2 fills this hook with site export after a successful append.
    fn after_append(&self, _entry: &Entry) {}
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
        )
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
            view.base = self.base.clone();
        } else if let Some(record) = self.follows.get(key) {
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
