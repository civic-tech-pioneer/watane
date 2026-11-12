use crate::*;
use std::collections::BTreeMap;

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
impl Dist {
    pub(crate) fn save_follows(
        &self,
        records: &BTreeMap<PublicKey, Follow>,
    ) -> Result<(), DistError> {
        let file = Follows {
            version: 1,
            channels: records
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        };
        site::write_equal(
            &self.home.join("dist/follows.json"),
            &serde_json::to_vec(&file).expect("follows JSON"),
            0o600,
        )?;
        Ok(())
    }
    pub fn follow(&self, link: &Link) -> Result<RefreshReport, DistError> {
        if link.channel() == self.identity() {
            return Err(DistError::OwnChannel);
        }
        let _guard = self
            .read_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = records.clone();
        let time = now();
        let record = next.entry(link.channel()).or_insert(Follow {
            base: link.base().into(),
            followed: true,
            index: false,
            bases: Vec::new(),
            _followed_at: time,
            _last_attempt: time,
            last_success: None,
        });
        record.followed = true;
        record.base = link.base().into();
        record._last_attempt = time;
        if matches!(report.outcome, Outcome::Current { .. }) {
            record.last_success = Some(time);
        }
        self.save_follows(&next)?;
        *records = next;
        Ok(report)
    }
    pub fn unfollow(&self, channel: &PublicKey) -> Result<(), DistError> {
        let _guard = self
            .read_mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut records = self
            .follows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = records.clone();
        let record = next
            .get_mut(channel)
            .filter(|r| r.followed)
            .ok_or(DistError::NotFollowed { channel: *channel })?;
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
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let keys: Vec<_> = self
            .follows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|(_, r)| r.followed || r.index)
            .map(|(k, r)| (*k, r.base.clone()))
            .collect();
        let mut seen = std::collections::BTreeSet::new();
        let mut reports = Vec::new();
        for (key, base) in keys {
            seen.insert(key);
            reports.push(
                self.read_channel(key, &base)
                    .unwrap_or_else(|cause| RefreshReport {
                        channel: key,
                        outcome: Outcome::Failed { cause },
                        changes: Vec::new(),
                    }),
            );
        }
        if let Err(cause) = self.read_listed(&mut reports, &mut seen) {
            reports.push(RefreshReport {
                channel: self.identity(),
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    fn read_channel_entries(
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
        let head = self.read_entry(key, &head_url, None)?;
        let stored = self.log.head(&key)?.map(|h| h.seq);
        let start = stored.map_or(0, |seq| seq + 1);
        let served = head.seq();
        let mut entries = Vec::new();
        let mut failure = None;
        let mut next = start;
        while next < served && (entries.len() as u64) < entry_limit {
            let url = http::file_url(base, &format!("entries/{next}"));
            match self.read_entry(key, &url, Some(next)) {
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
