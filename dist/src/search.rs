//! Search (TECHNICAL 5.10): find releases by name or interface across the
//! own, followed and indexed channels, and order the hits.

use crate::{Dist, DistError, Link, PublicKey, Release, ReleaseInfo, Slug, Status, Version, view};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;
use watane_pkg::{Grant, InterfaceName};

#[derive(Clone, Debug, Default)]
pub struct Query {
    pub text: String,
    pub publisher: Option<PublicKey>,
    pub origin: Option<(PublicKey, Slug)>,
    pub interface: Option<InterfaceQuery>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterfaceQuery {
    name: String,
    version: Option<Version>,
}
impl FromStr for InterfaceQuery {
    type Err = DistError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || DistError::BadQuery {
            cause: "expected an interface name with optional @version",
        };
        let qualified = if text.contains('@') {
            text.to_owned()
        } else {
            format!("{text}@0.0.0")
        };
        let interface: InterfaceName = qualified.parse().map_err(|_| invalid())?;
        let (name, _) = qualified.split_once('@').ok_or_else(invalid)?;
        Ok(Self {
            name: name.into(),
            version: text.contains('@').then(|| interface.version()),
        })
    }
}
impl InterfaceQuery {
    fn matches(&self, interface: &InterfaceName) -> bool {
        let text = interface.to_string();
        text.split_once('@')
            .is_some_and(|(name, _)| name == self.name)
            && self
                .version
                .as_ref()
                .is_none_or(|v| *v == interface.version())
    }
}
#[derive(Clone, Debug)]
pub struct Hit {
    pub info: ReleaseInfo,
    pub publisher_name: Option<String>,
    pub link: Option<Link>,
    pub status: Status,
    pub checked_at: Option<u64>,
    pub unchecked_since: Option<u64>,
    pub via: Vec<Via>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Via {
    Own,
    Followed,
    Index {
        index: PublicKey,
        through: Option<PublicKey>,
        note: String,
    },
    Kept,
}

impl Dist {
    /// Searches verified, stored channel entries without making any network request.
    pub fn search(&self, query: &Query) -> Result<Vec<Hit>, DistError> {
        self.search_candidates(query, None)
    }
    pub fn search_stored(
        &self,
        query: &Query,
        channels: &[PublicKey],
    ) -> Result<Vec<Hit>, DistError> {
        self.search_candidates(query, Some(channels))
    }
    fn search_candidates(
        &self,
        query: &Query,
        stored: Option<&[PublicKey]>,
    ) -> Result<Vec<Hit>, DistError> {
        if query.text.len() > 256 {
            return Err(DistError::BadQuery {
                cause: "text exceeds 256 bytes",
            });
        }
        let own = self.identity();
        let records = self
            .follows
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let sets = if stored.is_none() {
            self.read_sets()?
        } else {
            Vec::new()
        };
        // Derive each candidate channel once; ordinary search uses only held channels.
        let mut views = BTreeMap::new();
        let keys: BTreeSet<_> = match stored {
            Some(channels) => channels.iter().copied().collect(),
            None => own.into_iter().chain(records.keys().copied()).collect(),
        };
        for key in keys {
            let mut view = view::derive(key, &self.log.entries(&key, 0, usize::MAX)?);
            view.status = match self.log.status(&key)? {
                watane_log::Status::Ok => Status::Ok,
                watane_log::Status::Compromised(fork) => Status::Compromised {
                    seq: fork.chain.seq(),
                },
            };
            views.insert(key, view);
        }
        let mut candidates: BTreeMap<(PublicKey, Slug), Vec<Via>> = BTreeMap::new();
        for (key, view) in &views {
            if stored.is_some() {
                for r in &view.releases {
                    candidates
                        .entry((*key, r.release.slug.clone()))
                        .or_default();
                }
                continue;
            }
            let via = if Some(*key) == own {
                Some(Via::Own)
            } else if records[key].followed {
                Some(Via::Followed)
            } else {
                None
            };
            if let Some(via) = via {
                for r in &view.releases {
                    add(&mut candidates, *key, &r.release.slug, via.clone());
                }
            }
        }
        for set in sets {
            for (key, through) in set.whole {
                // Own releases already have Via::Own; own listings still count as Index.
                if Some(key) == own {
                    continue;
                }
                if let Some(view) = views.get(&key) {
                    for r in &view.releases {
                        add(
                            &mut candidates,
                            key,
                            &r.release.slug,
                            Via::Index {
                                index: set.index,
                                through,
                                note: String::new(),
                            },
                        );
                    }
                }
            }
            for m in set.mods {
                if Some(m.channel) != own {
                    add(
                        &mut candidates,
                        m.channel,
                        &m.slug,
                        Via::Index {
                            index: set.index,
                            through: m.through,
                            note: m.note,
                        },
                    );
                }
            }
        }
        if stored.is_none() {
            for (key, record) in &records {
                if record.kept
                    && let Some(view) = views.get(key)
                {
                    for r in &view.releases {
                        add(&mut candidates, *key, &r.release.slug, Via::Kept);
                    }
                }
            }
        }
        let words = query.text.to_lowercase();
        let mut hits = Vec::new();
        for ((key, slug), mut via) in candidates {
            if query.publisher.is_some_and(|p| p != key) {
                continue;
            }
            let Some(view) = views.get(&key) else {
                continue;
            };
            let Some(info) = view
                .releases
                .iter()
                .rev()
                .find(|r| r.release.slug == slug && r.withdrawn.is_none())
            else {
                continue;
            };
            if query.origin.as_ref().is_some_and(|(publisher, slug)| {
                info.release.origin.as_ref().is_none_or(|origin| {
                    origin.publisher.as_bytes() != publisher.as_bytes() || origin.slug != *slug
                })
            }) {
                continue;
            }
            let interfaces = interfaces(&info.release);
            if query
                .interface
                .as_ref()
                .is_some_and(|q| !interfaces.iter().any(|i| q.matches(i)))
            {
                continue;
            }
            let mut haystack = vec![
                slug.to_string(),
                info.release.name.clone(),
                info.release.summary.clone(),
                view.name.clone().unwrap_or_default(),
            ];
            haystack.extend(via.iter().filter_map(|v| match v {
                Via::Index { note, .. } => Some(note.clone()),
                _ => None,
            }));
            haystack.extend(interfaces.iter().map(|i| i.to_string()));
            let haystack = haystack.join("\n").to_lowercase();
            if !words.split_whitespace().all(|w| haystack.contains(w)) {
                continue;
            }
            via.sort_by(|a, b| via_order(a).cmp(&via_order(b)));
            let record = records.get(&key).filter(|_| Some(key) != own);
            let checked_at = record.and_then(|r| r.last_success);
            let unchecked_since =
                record.and_then(|r| r.last_success.filter(|s| r._last_attempt > *s));
            let link = record
                .filter(|r| !r.base.is_empty())
                .map(|r| Link::new(key, &r.base, Some(&slug)))
                .transpose()?;
            hits.push(Hit {
                info: info.clone(),
                publisher_name: view.name.clone(),
                link,
                status: view.status,
                checked_at,
                unchecked_since,
                via,
            });
        }
        order_hits(&mut hits);
        Ok(hits)
    }
}
fn order_hits(hits: &mut Vec<Hit>) {
    hits.sort_by_cached_key(|h| {
        (
            std::cmp::Reverse(index_count(&h.via)),
            std::cmp::Reverse(h.via.iter().any(|v| matches!(v, Via::Own | Via::Followed))),
            h.info.release.name.to_lowercase(),
            h.info.reference.to_string(),
        )
    });
    hits.truncate(200);
}
fn add(
    candidates: &mut BTreeMap<(PublicKey, Slug), Vec<Via>>,
    key: PublicKey,
    slug: &Slug,
    via: Via,
) {
    let vias = candidates.entry((key, slug.clone())).or_default();
    if !vias.contains(&via) {
        vias.push(via);
    }
}
fn via_order(via: &Via) -> (u8, Option<PublicKey>, Option<PublicKey>, &str) {
    match via {
        Via::Own => (0, None, None, ""),
        Via::Followed => (1, None, None, ""),
        Via::Kept => (3, None, None, ""),
        Via::Index {
            index,
            through,
            note,
        } => (2, Some(*index), *through, note),
    }
}
fn index_count(via: &[Via]) -> usize {
    via.iter()
        .filter_map(|v| match v {
            Via::Index { index, .. } => Some(index),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .len()
}
fn interfaces(release: &Release) -> Vec<&InterfaceName> {
    release
        .offers
        .iter()
        .chain(&release.events)
        .chain(release.grants.iter().filter_map(|g| match g {
            Grant::Call { interface, .. } | Grant::Subscribe { interface, .. } => Some(interface),
            _ => None,
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntryId, PackageId, ReleaseRef};

    #[test]
    fn ac6_search_cap_after_ordering() {
        let key: PublicKey = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
            .parse()
            .unwrap();
        let mut hits = Vec::new();
        let mut expected = Vec::new();
        for n in (0..202).rev() {
            let slug = format!(
                "mod-{}{}",
                (b'a' + (n / 26) as u8) as char,
                (b'a' + (n % 26) as u8) as char
            );
            let reference: ReleaseRef = format!("{key}/{slug}@0.1.0").parse().unwrap();
            expected.push(reference.to_string());
            hits.push(Hit {
                info: ReleaseInfo {
                    reference,
                    channel: key,
                    entry: EntryId::from_bytes([0; 32]),
                    seq: n,
                    release: Release {
                        slug: slug.parse().unwrap(),
                        version: "0.1.0".parse().unwrap(),
                        name: if n % 2 == 0 {
                            "Alpha".into()
                        } else {
                            "alpha".into()
                        },
                        summary: "Books".into(),
                        package: PackageId::from_bytes([0; 32]),
                        size: 1,
                        source: false,
                        offers: vec![],
                        events: vec![],
                        grants: vec![],
                        origin: None,
                    },
                    withdrawn: None,
                },
                publisher_name: None,
                link: None,
                status: Status::Ok,
                checked_at: None,
                unchecked_since: None,
                via: vec![Via::Own],
            });
        }
        expected.sort();
        expected.truncate(200);
        order_hits(&mut hits);
        assert_eq!(
            hits.iter()
                .map(|h| h.info.reference.to_string())
                .collect::<Vec<_>>(),
            expected
        );
    }
}
