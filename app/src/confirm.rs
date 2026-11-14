use crate::{
    consent,
    words::{self, Parties},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock, Weak},
};
use watane_host::{Answer, Change, Confirm, InstallRequest, Requester};

pub(crate) struct HostConfirm {
    pub broker: Arc<consent::Broker>,
    pub parties: Arc<Parties>,
    pub host: OnceLock<Weak<watane_host::Host>>,
}

fn request_view(
    request: &InstallRequest,
    parties: &Parties,
    host: Option<&watane_host::Host>,
) -> consent::Request {
    consent::Request::Install {
        by_program: matches!(request.requester, Requester::LocalApi),
        mods: request
            .entries
            .iter()
            .map(|e| consent::PlanMod {
                origin: e.origin.as_ref().map(|origin| {
                    let name = host
                        .and_then(|h| {
                            h.mods()
                                .into_iter()
                                .find(|m| m.release.as_ref() == Some(origin))
                        })
                        .map_or_else(|| origin.slug.to_string(), |m| m.name);
                    words::origin_line(&name, origin, parties)
                }),
                binds: e
                    .binds
                    .iter()
                    .filter(|b| {
                        !host
                            .and_then(|h| h.get(&e.local))
                            .is_some_and(|m| m.binds.contains(b))
                    })
                    .map(|b| words::bind_line(b, parties))
                    .collect(),
                local: e.local.to_string(),
                name: e.name.clone(),
                slug: e.slug.to_string(),
                version: e.version.to_string(),
                release: e.release.as_ref().map(ToString::to_string),
                publisher: e
                    .publisher
                    .as_ref()
                    .map(|p| words::party(&p.to_string(), parties)),
                replaces: match &e.change {
                    Change::Update { from_version } => Some(from_version.to_string()),
                    _ => None,
                },
                grants: e
                    .grants
                    .iter()
                    .map(|g| {
                        words::grant_line_bound(
                            g,
                            &e.binds,
                            parties,
                            true,
                            e.new_grants.contains(g),
                        )
                    })
                    .collect(),
                uses: e
                    .providers
                    .iter()
                    .map(|local| {
                        // The host resolves provider identities. Look in the plan first;
                        // an already-installed provider is also named by its grant.
                        let provider =
                            request.entries.iter().find(|p| &p.local == local).map(|p| {
                                watane_host::watane_pkg::Provider {
                                    slug: p.slug.clone(),
                                    publisher: p.publisher.clone(),
                                }
                            });
                        let provider = provider.or_else(|| {
                            host.and_then(|h| h.get(local)).map(|m| {
                                watane_host::watane_pkg::Provider {
                                    slug: m.slug,
                                    publisher: m.publisher,
                                }
                            })
                        });
                        provider
                            .map(|p| {
                                // A uses line names the picked target and the pinned provider.
                                let pinned = e.binds.iter().find(|b| {
                                    b.to == format!(
                                        "{}/{}",
                                        p.publisher
                                            .as_ref()
                                            .map(ToString::to_string)
                                            .unwrap_or_else(|| "local".into()),
                                        p.slug
                                    )
                                });
                                match pinned {
                                    Some(b) => {
                                        words::provider_bound(&b.provider, &e.binds, parties)
                                    }
                                    None => words::provider(&p, parties),
                                }
                            })
                            .unwrap_or_else(|| local.to_string())
                    })
                    .collect(),
            })
            .collect(),
    }
}
fn host_answer(answer: consent::Answer, request: &InstallRequest) -> Answer {
    match answer {
        consent::Answer::Deny | consent::Answer::Pick { .. } => Answer::Deny,
        consent::Answer::Approve { withheld } => {
            let mut withhold = Vec::new();
            for w in withheld {
                let Some((local, grant)) = request
                    .entries
                    .iter()
                    .find(|e| e.local.as_ref() == w.local)
                    .and_then(|e| {
                        e.grants
                            .iter()
                            .find(|g| g.to_string() == w.key)
                            .map(|g| (e.local.clone(), g.clone()))
                    })
                else {
                    return Answer::Deny;
                };
                withhold.push((local, grant));
            }
            Answer::Approve { withhold }
        }
    }
}
impl Confirm for HostConfirm {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        let broker = self.broker.clone();
        let host = self.host.get().and_then(Weak::upgrade);
        let view = request_view(&request, &self.parties, host.as_deref());
        Box::pin(async move { host_answer(broker.ask(view).await, &request) })
    }
}

pub(crate) struct DistConfirm {
    pub broker: Arc<consent::Broker>,
    pub parties: Arc<Parties>,
    pub dist: OnceLock<Weak<watane_dist::Dist>>,
}
impl watane_dist::Confirm for DistConfirm {
    fn confirm(&self, request: &watane_dist::SignRequest) -> watane_dist::Decision {
        let mut profile_error = None;
        if let Some(dist) = self.dist.get().and_then(Weak::upgrade) {
            let mut keys = vec![request.channel];
            match &request.statement {
                watane_dist::Statement::Release { release, .. } => {
                    if let Some(key) = release
                        .origin
                        .as_ref()
                        .and_then(|r| r.publisher.to_string().parse().ok())
                    {
                        keys.push(key);
                    }
                }
                watane_dist::Statement::List(l)
                | watane_dist::Statement::Include(l)
                | watane_dist::Statement::Unlist { listing: l, .. } => keys.push(l.link.channel()),
                _ => {}
            }
            if let Err(error) = crate::core::cache_channels(&dist, &self.parties, keys) {
                profile_error = Some(crate::cause_chain(&error));
            }
        }
        let mut view = sign_view(request, &self.parties);
        if let Some(cause) = profile_error
            && let consent::Request::Sign { details, .. } = &mut view
        {
            details.push(consent::Detail {
                label: "Could not read profile".into(),
                value: cause,
            });
        }
        match self.broker.ask_blocking(view) {
            consent::Answer::Approve { .. } => watane_dist::Decision::Approve,
            consent::Answer::Deny | consent::Answer::Pick { .. } => watane_dist::Decision::Refuse,
        }
    }
}

fn listing_target(link: &watane_dist::Link, parties: &Parties) -> String {
    let key = link.channel().to_string();
    let party = words::party(&key, parties);
    let label = words::party_label(&party);
    match link.slug() {
        Some(slug) => format!("{slug} by {label} — {key}/{slug}"),
        None => format!("{label} — {key}"),
    }
}
fn sign_view(request: &watane_dist::SignRequest, parties: &Parties) -> consent::Request {
    use watane_dist::Statement;
    let (statement, details) = match &request.statement {
        Statement::Profile(p) => (consent::Statement::Profile, vec![("Name", p.name.clone())]),
        Statement::Release { reference, release } => (
            consent::Statement::Release,
            vec![
                (
                    "Release",
                    format!("{} {} — {reference}", reference.slug, reference.version),
                ),
                ("Package", release.package.to_string()),
                ("Size", release.size.to_string()),
                (
                    "Source included",
                    if release.source { "yes" } else { "no" }.into(),
                ),
                ("Summary", release.summary.clone()),
                (
                    "Made from",
                    release
                        .origin
                        .as_ref()
                        .map(|origin| words::origin_line(&origin.slug.to_string(), origin, parties))
                        .unwrap_or_else(|| "none".into()),
                ),
            ],
        ),
        Statement::Withdraw {
            reference,
            withdraw,
            ..
        } => (
            consent::Statement::Withdraw,
            vec![
                (
                    "Release",
                    format!("{} {} — {reference}", reference.slug, reference.version),
                ),
                ("Reason", withdraw.reason.clone()),
            ],
        ),
        Statement::List(listing) | Statement::Include(listing) => (
            if matches!(&request.statement, Statement::List(_)) {
                consent::Statement::List
            } else {
                consent::Statement::Include
            },
            vec![
                (
                    if listing.link.slug().is_some() {
                        "Mod"
                    } else {
                        "Index"
                    },
                    listing_target(&listing.link, parties),
                ),
                ("Location", listing.link.base().into()),
                (
                    "Note",
                    if listing.note.is_empty() {
                        "none".into()
                    } else {
                        listing.note.clone()
                    },
                ),
            ],
        ),
        Statement::Unlist {
            listing, include, ..
        } => (
            consent::Statement::Unlist,
            vec![(
                "Ends",
                format!(
                    "{} {}",
                    if *include { "include" } else { "list" },
                    listing_target(&listing.link, parties)
                ),
            )],
        ),
    };
    let mut signer = words::party(&request.channel.to_string(), parties);
    signer.fingerprint = request.fingerprint.clone();
    consent::Request::Sign {
        statement,
        signer,
        details: details
            .into_iter()
            .map(|(label, value)| consent::Detail {
                label: label.into(),
                value,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watane_host::{
        LocalName, PlanEntry,
        watane_pkg::{Grant, Provider},
    };
    #[test]
    fn ac4_host_origin_new_binds_and_bound_uses() {
        let carol = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        let frank = "y".repeat(52);
        let parties = Parties::new();
        parties.insert(carol.into(), "Carol".into());
        parties.insert(frank.clone(), "Frank".into());
        let provider = Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some(carol.parse().unwrap()),
        };
        let bind = watane_host::Bind {
            consumer: "local/stats".into(),
            provider: provider.clone(),
            to: format!("{frank}/ratings"),
        };
        let origin = format!("{carol}/ratings@1.0.0");
        let grant = Grant::Call {
            interface: "mods:ratings/scores@1.0.0".parse().unwrap(),
            provider,
        };
        let entry = PlanEntry {
            local: LocalName::parse("stats").unwrap(),
            identity: "local/stats".into(),
            slug: "stats".parse().unwrap(),
            name: "Stats".into(),
            summary: String::new(),
            version: "1.0.0".parse().unwrap(),
            publisher: None,
            package: "00".repeat(32).parse().unwrap(),
            release: None,
            origin: Some(origin.parse().unwrap()),
            change: Change::New,
            grants: vec![grant.clone()],
            new_grants: vec![grant],
            providers: vec![LocalName::parse("ratings").unwrap()],
            binds: vec![bind.clone()],
        };
        let provider_entry = PlanEntry {
            local: LocalName::parse("ratings").unwrap(),
            identity: bind.to.clone(),
            slug: "ratings".parse().unwrap(),
            name: "Ratings".into(),
            summary: String::new(),
            version: "1.0.0".parse().unwrap(),
            publisher: Some(frank.parse().unwrap()),
            package: "00".repeat(32).parse().unwrap(),
            release: None,
            origin: None,
            change: Change::New,
            grants: vec![],
            new_grants: vec![],
            providers: vec![],
            binds: vec![],
        };
        let request = InstallRequest {
            entries: vec![provider_entry, entry],
            requester: Requester::App,
        };
        let consent::Request::Install { mods, .. } = request_view(&request, &parties, None) else {
            panic!()
        };
        assert_eq!(
            mods[1].origin.as_deref(),
            Some(format!("ratings 1.0.0 by Carol (47pjoycnsr) — {origin}").as_str())
        );
        let consent::Request::Install { mods: unknown, .. } =
            request_view(&request, &Parties::new(), None)
        else {
            panic!()
        };
        assert_eq!(
            unknown[1].origin.as_deref(),
            Some(format!("ratings 1.0.0 by 47pjoycnsr — {origin}").as_str())
        );
        assert_eq!(mods[1].binds, [words::bind_line(&bind, &parties)]);
        assert_eq!(
            mods[1].uses,
            ["ratings by Frank (yyyyyyyyyy) (in place of ratings by Carol (47pjoycnsr))"]
        );
        assert_eq!(
            mods[1].grants[0].text,
            "Call mods:ratings/scores@1.0.0 of ratings by Frank (yyyyyyyyyy) (in place of ratings by Carol (47pjoycnsr))"
        );
        assert!(mods[0].origin.is_none());
        assert!(mods[0].binds.is_empty());
    }

    #[tokio::test]
    async fn ac8_real_host_omits_existing_binds_and_ac7_names_installed_origin() {
        use watane_host::{Source, watane_pkg};
        let home = tempfile::tempdir().unwrap();
        let host =
            watane_host::Host::open(home.path(), watane_host::Mode::Preview, Default::default())
                .unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let temp = tempfile::tempdir().unwrap();
        fn copy(from: &std::path::Path, to: &std::path::Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap() {
                let e = entry.unwrap();
                if e.file_type().unwrap().is_dir() {
                    copy(&e.path(), &to.join(e.file_name()));
                } else {
                    std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
                }
            }
        }
        copy(&fixtures.join("stats"), temp.path());
        let key = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        std::fs::write(
            temp.path().join("deps.json"),
            format!("{{\"mods:ratings\":\"{key}\"}}"),
        )
        .unwrap();
        let provider = watane_pkg::Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some(key.parse().unwrap()),
        };
        let bind = watane_host::Bind {
            consumer: "local/stats".into(),
            provider,
            to: "local/ratings".into(),
        };
        let plan = host
            .plan_with(
                vec![
                    Source::Folder(temp.path().into()),
                    Source::Folder(fixtures.join("ratings")),
                ],
                vec![bind.clone()],
            )
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        let plan = host.plan(vec![Source::Folder(temp.path().into())]).unwrap();
        assert_eq!(plan.entries()[0].binds, [bind]);
        let request = InstallRequest {
            entries: plan.entries().to_vec(),
            requester: Requester::App,
        };
        let consent::Request::Install { mods, .. } =
            request_view(&request, &Parties::new(), Some(&host))
        else {
            panic!()
        };
        assert!(mods[0].binds.is_empty());
        struct Approve;
        impl watane_dist::Confirm for Approve {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                watane_dist::Decision::Approve
            }
        }
        let publisher_home = tempfile::tempdir().unwrap();
        let dist =
            watane_dist::Dist::open(publisher_home.path(), Arc::new(Approve), Default::default())
                .unwrap();
        let published = dist
            .publish(
                watane_pkg::pack(&fixtures.join("reading-list"), false)
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
            )
            .unwrap();
        let fetched = dist.fetch(&published.info.reference).unwrap();
        let plan = host
            .plan(vec![Source::Release {
                package: fetched.package,
                entry: fetched.entry,
            }])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        let mut request = request;
        request.entries[0].origin = Some(published.info.reference);
        for known in [false, true] {
            let parties = Parties::new();
            if known {
                parties.insert(dist.identity().to_string(), "Alice".into());
            }
            let consent::Request::Install { mods, .. } =
                request_view(&request, &parties, Some(&host))
            else {
                panic!()
            };
            let label = if known {
                format!("Alice ({})", dist.fingerprint())
            } else {
                dist.fingerprint()
            };
            assert_eq!(
                mods[0].origin.as_ref().unwrap(),
                &format!(
                    "Reading list 0.1.0 by {label} — {}",
                    request.entries[0].origin.as_ref().unwrap()
                )
            );
        }
    }
    #[test]
    fn ac4_release_ends_with_made_from_named_or_fingerprint_or_none() {
        let key = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        let origin = format!("{key}/ratings@1.0.0");
        let parties = Parties::new();
        for known in [false, true] {
            if known {
                parties.insert(key.into(), "Carol".into());
            }
            for parent in [None, Some(origin.clone())] {
                let request = watane_dist::SignRequest {
                    channel: key.parse().unwrap(),
                    fingerprint: "signer".into(),
                    statement: watane_dist::Statement::Release {
                        reference: format!("{key}/stats@1.0.0").parse().unwrap(),
                        release: watane_dist::Release {
                            slug: "stats".parse().unwrap(),
                            version: "1.0.0".parse().unwrap(),
                            name: "Stats".into(),
                            summary: "Scores".into(),
                            package: "00".repeat(32).parse().unwrap(),
                            size: 123,
                            source: false,
                            offers: vec![],
                            events: vec![],
                            grants: vec![],
                            origin: parent.as_ref().map(|o| o.parse().unwrap()),
                        },
                    },
                };
                let consent::Request::Sign { details, .. } = sign_view(&request, &parties) else {
                    panic!()
                };
                assert_eq!(details[0].value, format!("stats 1.0.0 — {key}/stats@1.0.0"));
                let expected = match parent {
                    None => "none".into(),
                    Some(o) => format!(
                        "ratings 1.0.0 by {} — {o}",
                        if known {
                            "Carol (47pjoycnsr)"
                        } else {
                            "47pjoycnsr"
                        }
                    ),
                };
                assert_eq!(
                    details.last(),
                    Some(&consent::Detail {
                        label: "Made from".into(),
                        value: expected
                    })
                );
            }
        }
    }

    #[test]
    fn ac2_install_request_and_withheld_answer() {
        let storage = Grant::Storage;
        let call = Grant::Call {
            interface: "mods:reading-list/books@0.1.0".parse().unwrap(),
            provider: Provider {
                slug: "reading-list".parse().unwrap(),
                publisher: None,
            },
        };
        let entry = |local: &str, grants: Vec<Grant>| PlanEntry {
            local: LocalName::parse(local).unwrap(),
            identity: format!("local/{local}"),
            slug: local.parse().unwrap(),
            name: local.into(),
            summary: String::new(),
            version: "0.2.0".parse().unwrap(),
            publisher: None,
            package: "00".repeat(32).parse().unwrap(),
            release: None,
            origin: None,
            change: Change::New,
            grants: grants.clone(),
            new_grants: grants,
            binds: vec![],
            providers: vec![],
        };
        let mut first = entry("reading-list", vec![storage.clone()]);
        first.change = Change::Update {
            from_version: "0.1.0".parse().unwrap(),
        };
        first.new_grants.clear();
        let mut second = entry("recommendations", vec![call.clone()]);
        second.providers.push(first.local.clone());
        let request = InstallRequest {
            entries: vec![first, second],
            requester: Requester::LocalApi,
        };
        let consent::Request::Install { mods, by_program } =
            request_view(&request, &Parties::new(), None)
        else {
            panic!("install expected")
        };
        assert!(by_program);
        assert_eq!(
            mods.iter().map(|m| m.local.as_str()).collect::<Vec<_>>(),
            ["reading-list", "recommendations"]
        );
        assert_eq!(mods[0].replaces.as_deref(), Some("0.1.0"));
        assert!(!mods[0].grants[0].new);
        assert!(mods[1].grants[0].new);
        assert_eq!(mods[1].uses, ["reading-list (local)"]);
        let answer = host_answer(
            consent::Answer::Approve {
                withheld: vec![consent::Withheld {
                    local: "recommendations".into(),
                    key: call.to_string(),
                }],
            },
            &request,
        );
        let Answer::Approve { withhold } = answer else {
            panic!("approve expected")
        };
        assert_eq!(
            withhold,
            vec![(LocalName::parse("recommendations").unwrap(), call)]
        );
        assert!(matches!(
            host_answer(consent::Answer::Deny, &request),
            Answer::Deny
        ));
    }
    #[tokio::test]
    async fn ac2_uses_resolves_an_already_installed_provider() {
        let home = tempfile::tempdir().unwrap();
        let host = watane_host::Host::open(
            home.path(),
            watane_host::Mode::Preview,
            watane_host::Limits::default(),
        )
        .unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let plan = host
            .plan(vec![watane_host::Source::Folder(
                fixtures.join("reading-list"),
            )])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Folder(
                fixtures.join("recommendations"),
            )])
            .unwrap();
        let request = InstallRequest {
            entries: plan.entries().to_vec(),
            requester: Requester::App,
        };
        let consent::Request::Install { mods, by_program } =
            request_view(&request, &Parties::new(), Some(&host))
        else {
            panic!("install expected")
        };
        assert!(!by_program);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].uses, ["reading-list (local)"]);
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    use watane_dist::{EntryId, Listing, SignRequest, Statement};
    #[test]
    fn ac1_index_sign_details_are_complete_and_ordered() {
        let key = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy".to_string();
        let parties = Parties::new();
        let listing = Listing {
            link: format!("watane:{key}@https://example.org/#reading-list")
                .parse()
                .unwrap(),
            note: "".into(),
        };
        let index = Listing {
            link: format!("watane:{key}@https://example.org/")
                .parse()
                .unwrap(),
            note: "Useful".into(),
        };
        for known in [false, true] {
            if known {
                parties.insert(key.clone(), "Alice".into());
            }
            let party = if known {
                "Alice (47pjoycnsr)"
            } else {
                "47pjoycnsr"
            };
            let target = format!("reading-list by {party} — {key}/reading-list");
            let index_target = format!("{party} — {key}");
            for (statement, kind, details) in [
                (
                    Statement::List(listing.clone()),
                    consent::Statement::List,
                    vec![
                        ("Mod", target.clone()),
                        ("Location", "https://example.org/".into()),
                        ("Note", "none".into()),
                    ],
                ),
                (
                    Statement::Include(index.clone()),
                    consent::Statement::Include,
                    vec![
                        ("Index", index_target.clone()),
                        ("Location", "https://example.org/".into()),
                        ("Note", "Useful".into()),
                    ],
                ),
                (
                    Statement::Unlist {
                        entry: EntryId::from_bytes([0; 32]),
                        listing: listing.clone(),
                        include: false,
                    },
                    consent::Statement::Unlist,
                    vec![("Ends", format!("list {target}"))],
                ),
                (
                    Statement::Unlist {
                        entry: EntryId::from_bytes([0; 32]),
                        listing: index.clone(),
                        include: true,
                    },
                    consent::Statement::Unlist,
                    vec![("Ends", format!("include {index_target}"))],
                ),
            ] {
                let request = SignRequest {
                    channel: key.parse().unwrap(),
                    fingerprint: "signer-fingerprint".into(),
                    statement,
                };
                let consent::Request::Sign {
                    statement,
                    signer,
                    details: actual,
                } = sign_view(&request, &parties)
                else {
                    panic!("sign expected")
                };
                assert_eq!(statement, kind);
                assert_eq!(signer.fingerprint, "signer-fingerprint");
                assert_eq!(
                    actual,
                    details
                        .into_iter()
                        .map(|(label, value)| consent::Detail {
                            label: label.into(),
                            value
                        })
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
