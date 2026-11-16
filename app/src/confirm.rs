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
    pub core: OnceLock<Weak<crate::core::Core>>,
    pub notices: Arc<ManagerNotices>,
}

#[derive(Default)]
pub(crate) struct ManagerNotices(std::sync::Mutex<std::collections::BTreeMap<String, String>>);
impl ManagerNotices {
    pub fn get(&self, local: &str, release: Option<String>) -> Option<String> {
        release.or_else(|| self.0.lock().unwrap().get(local).cloned())
    }
    pub fn enable(
        &self,
        host: &watane_host::Host,
        local: &watane_host::LocalName,
    ) -> Result<(), crate::CmdError> {
        host.enable(local)?;
        self.0.lock().unwrap().remove(local.as_ref());
        Ok(())
    }
}
fn manage_outcome(result: Result<(), crate::CmdError>) -> watane_host::ManageOutcome {
    match result {
        Ok(()) => watane_host::ManageOutcome::Approved,
        Err(e) if e.code == "refused" => watane_host::ManageOutcome::Denied,
        Err(e) => watane_host::ManageOutcome::Failed(e.message),
    }
}
fn manage_answer(answer: consent::Answer) -> Result<(), crate::CmdError> {
    match answer {
        consent::Answer::Approve { withheld } if withheld.is_empty() => Ok(()),
        _ => Err(crate::CmdError::new("refused", "request denied")),
    }
}
fn check_manager_link(
    dist: &watane_dist::Dist,
    by: &consent::Asker,
    link: Option<&watane_dist::Link>,
) -> Result<(), crate::CmdError> {
    if let Some(link) = link {
        let key = link.channel();
        let known = dist.identity() == Some(key)
            || dist.followed()?.iter().any(|v| v.channel == key)
            || dist.indexes()?.iter().any(|v| v.view.channel == key);
        if !known {
            return Err(crate::CmdError::new(
                "bad_input",
                format!(
                    "{} named a channel this home does not read: {link}",
                    words::asker_label(by)
                ),
            ));
        }
    }
    Ok(())
}
fn manager_offer(
    offer: Option<String>,
    name: &str,
) -> Result<watane_dist::ReleaseRef, crate::CmdError> {
    offer
        .ok_or_else(|| {
            crate::CmdError::new("not_found", format!("no update is offered for {name}"))
        })?
        .parse()
        .map_err(|e: watane_host::watane_pkg::NameError| {
            crate::CmdError::new("bad_input", e.to_string())
        })
}
fn manager_users(host: &watane_host::Host, target: &watane_host::LocalName) -> Vec<String> {
    host.mods()
        .iter()
        .filter(|m| {
            m.enabled
                && crate::core::mod_links(m)
                    .iter()
                    .any(|l| l.granted && l.provider == target.as_ref())
        })
        .map(|m| m.name.clone())
        .collect()
}
fn reduction_effect(
    reduction: &watane_host::Reduction,
) -> (&watane_host::LocalName, Option<&'static str>, bool) {
    match reduction {
        watane_host::Reduction::Disable(l) => (l, Some("Turned off"), true),
        watane_host::Reduction::Withdraw { local, .. } => (local, Some("Grant withdrawn"), false),
        watane_host::Reduction::ClosePage(l) => (l, None, true),
    }
}
fn asker(m: &watane_host::InstalledMod, parties: &Parties) -> consent::Asker {
    consent::Asker {
        local: m.local.to_string(),
        name: m.name.clone(),
        publisher: m
            .publisher
            .as_ref()
            .map(|p| words::party(&p.to_string(), parties)),
    }
}
fn request_view(
    request: &InstallRequest,
    parties: &Parties,
    host: Option<&watane_host::Host>,
) -> consent::Request {
    consent::Request::Install {
        asked_by: match &request.requester {
            Requester::Mod(by) => host.and_then(|h| h.get(by)).map(|m| asker(&m, parties)),
            _ => None,
        },
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
// Host cancels a manager's request when it loses management authority. Dropping
// its future must also cancel our spawned work and remove any queued consent.
struct ManageTask(tauri::async_runtime::JoinHandle<Result<(), crate::CmdError>>);
impl Drop for ManageTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Confirm for HostConfirm {
    fn manage(
        &self,
        request: watane_host::ManageRequest,
    ) -> Pin<Box<dyn Future<Output = watane_host::ManageOutcome> + Send>> {
        let core = self.core.get().and_then(Weak::upgrade);
        Box::pin(async move {
            let Some(core) = core else {
                return watane_host::ManageOutcome::Failed("app unavailable".into());
            };
            // Host polls this future on its own runtime. All app resources stay on ours.
            let mut task = ManageTask(tauri::async_runtime::spawn(manage(
                core,
                request.by,
                request.ask,
            )));
            match (&mut task.0).await {
                Ok(result) => manage_outcome(result),
                Err(e) => watane_host::ManageOutcome::Failed(e.to_string()),
            }
        })
    }
    fn reduced(&self, by: &watane_host::LocalName, reduction: &watane_host::Reduction) {
        let (local, notice, close) = reduction_effect(reduction);
        if let Some(host) = self.host.get().and_then(Weak::upgrade)
            && let Some(m) = host.get(by)
            && let Some(notice) = notice
        {
            self.notices.0.lock().unwrap().insert(
                local.to_string(),
                format!(
                    "{notice} by {}",
                    words::asker_label(&asker(&m, &self.parties))
                ),
            );
        }
        if close && let Some(core) = self.core.get().and_then(Weak::upgrade) {
            core.post_close(local.to_string());
        }
    }
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        let broker = self.broker.clone();
        let host = self.host.get().and_then(Weak::upgrade);
        let view = request_view(&request, &self.parties, host.as_deref());
        Box::pin(async move { host_answer(broker.ask(view).await, &request) })
    }
}

async fn manage(
    core: Arc<crate::core::Core>,
    by: watane_host::LocalName,
    ask: watane_host::Ask,
) -> Result<(), crate::CmdError> {
    use watane_host::Ask;
    let manager = core
        .host
        .get(&by)
        .ok_or_else(|| crate::CmdError::new("not_found", "asking mod not found"))?;
    let asked_by = asker(&manager, &core.parties);
    let requester = Requester::Mod(by);
    match ask {
        Ask::Install { release, link } => {
            let link = link.map(|l| l.parse::<watane_dist::Link>()).transpose()?;
            check_manager_link(&core.dist, &asked_by, link.as_ref())?;
            // The supplied base is never followed: fetch only through already held bases.
            core.install_managed(release, requester, asked_by).await?;
        }
        Ask::Update(local) => {
            let m = core
                .get(local.as_ref())?
                .ok_or_else(|| crate::CmdError::new("not_found", "mod not found"))?;
            let release = manager_offer(m.offer, &m.name)?;
            core.install_managed(release, requester, asked_by).await?;
        }
        Ask::Enable(ref local) | Ask::Grant { ref local, .. } | Ask::Remove { ref local, .. } => {
            let m = core
                .host
                .get(local)
                .ok_or_else(|| crate::CmdError::new("not_found", "mod not found"))?;
            let (action, grant, users) = match &ask {
                Ask::Enable(_) => (consent::ManageAction::Enable, None, vec![]),
                Ask::Grant { grant, .. } => (
                    consent::ManageAction::Grant,
                    Some(words::grant_line_bound(
                        grant,
                        &m.binds,
                        &core.parties,
                        false,
                        true,
                    )),
                    vec![],
                ),
                Ask::Remove { delete_data, .. } => (
                    if *delete_data {
                        consent::ManageAction::RemoveData
                    } else {
                        consent::ManageAction::Remove
                    },
                    None,
                    manager_users(&core.host, local),
                ),
                _ => unreachable!(),
            };
            let answer = core
                .broker
                .ask(consent::Request::Manage {
                    asked_by,
                    action,
                    target: consent::Target {
                        local: local.to_string(),
                        name: m.name,
                        version: m.version.to_string(),
                        publisher: m
                            .publisher
                            .as_ref()
                            .map(|p| words::party(&p.to_string(), &core.parties)),
                    },
                    grant,
                    users,
                })
                .await;
            manage_answer(answer)?;
            match ask {
                Ask::Enable(_) => core.enable(local.as_ref())?,
                Ask::Grant { grant, .. } => core.grant(local.as_ref(), &grant.to_string(), true)?,
                Ask::Remove { delete_data, .. } => core.remove(local.as_ref(), delete_data)?,
                _ => unreachable!(),
            }
        }
    }
    Ok(())
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
    fn preview() -> (tempfile::TempDir, Arc<watane_host::Host>) {
        let home = tempfile::tempdir().unwrap();
        let host =
            watane_host::Host::open(home.path(), watane_host::Mode::Preview, Default::default())
                .unwrap();
        (home, host)
    }
    async fn fixtures(host: &watane_host::Host) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let plan = host
            .plan(
                ["reading-list", "recommendations"]
                    .into_iter()
                    .map(|n| watane_host::Source::Folder(root.join(n)))
                    .collect(),
            )
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
    }
    #[tokio::test]
    async fn ac3_cancelled_manager_request_leaves_no_consent() {
        let broker = consent::Broker::new();
        let queued = broker.clone();
        let request: consent::Request = serde_json::from_value(serde_json::json!({"kind":"manage","asked_by":{"local":"desk","name":"Desk","publisher":null},"action":"enable","target":{"local":"weather","name":"Weather","version":"1.0.0","publisher":null},"grant":null,"users":[]})).unwrap();
        let task = ManageTask(tauri::async_runtime::spawn(async move {
            manage_answer(queued.ask(request).await)
        }));
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while broker.current().is_none() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(task);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while broker.current().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    #[test]
    fn ac3_manage_outcomes() {
        assert_eq!(
            manage_outcome(manage_answer(consent::Answer::Approve { withheld: vec![] })),
            watane_host::ManageOutcome::Approved
        );
        assert_eq!(
            manage_outcome(manage_answer(consent::Answer::Deny)),
            watane_host::ManageOutcome::Denied
        );
        assert_eq!(
            manage_outcome(Err(crate::CmdError::new("refused", "denied"))),
            watane_host::ManageOutcome::Denied
        );
        assert_eq!(
            manage_outcome(Err(crate::CmdError::new("io", "disk broke"))),
            watane_host::ManageOutcome::Failed("disk broke".into())
        );
    }
    #[test]
    fn ac3_unread_channel_fails_without_connecting() {
        let home = tempfile::tempdir().unwrap();
        let dist = watane_dist::Dist::open_keyless(
            home.path(),
            watane_dist::Options {
                listed_local: true,
                ..Default::default()
            },
        )
        .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let link = watane_dist::Link::new(
            "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
                .parse()
                .unwrap(),
            &format!("http://{}/", listener.local_addr().unwrap()),
            None,
        )
        .unwrap();
        let by = consent::Asker {
            local: "desk".into(),
            name: "Desk".into(),
            publisher: None,
        };
        let error = check_manager_link(&dist, &by, Some(&link)).unwrap_err();
        assert_eq!(
            error.message,
            format!("Desk (local) named a channel this home does not read: {link}")
        );
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[tokio::test]
    async fn ac3_remove_users_and_install_requester_and_missing_offer() {
        let (_home, host) = preview();
        fixtures(&host).await;
        let local = LocalName::parse("reading-list").unwrap();
        assert_eq!(manager_users(&host, &local), ["Recommendations"]);
        let by = LocalName::parse("recommendations").unwrap();
        let view = request_view(
            &InstallRequest {
                entries: vec![],
                requester: Requester::Mod(by.clone()),
            },
            &Parties::new(),
            Some(&host),
        );
        assert!(
            matches!(view, consent::Request::Install { asked_by: Some(consent::Asker { name, .. }), .. } if name == "Recommendations")
        );
        let error = manager_offer(None, "Reading list").unwrap_err();
        assert_eq!(error.message, "no update is offered for Reading list");
        let grants = host.get(&by).unwrap().grants;
        let call = grants
            .iter()
            .find(|g| matches!(g.grant, Grant::Call { .. }))
            .unwrap();
        let subscribe = grants
            .iter()
            .find(|g| matches!(g.grant, Grant::Subscribe { .. }))
            .unwrap();
        host.revoke(&by, &call.grant).unwrap();
        assert_eq!(
            manager_users(&host, &local),
            ["Recommendations"],
            "event receivers still use the mod"
        );
        host.revoke(&by, &subscribe.grant).unwrap();
        assert!(manager_users(&host, &local).is_empty());
        host.grant(&by, &call.grant).unwrap();
        assert_eq!(
            manager_users(&host, &local),
            ["Recommendations"],
            "callers still use the mod"
        );
        host.disable(&by).unwrap();
        assert!(manager_users(&host, &local).is_empty());
    }
    #[tokio::test]
    async fn ac4_reduced_returns_without_an_event_loop_and_notices_clear() {
        let (_home, host) = preview();
        fixtures(&host).await;
        let notices = Arc::new(ManagerNotices::default());
        let confirm = Arc::new(HostConfirm {
            broker: consent::Broker::new(),
            parties: Arc::new(Parties::new()),
            host: OnceLock::from(Arc::downgrade(&host)),
            core: OnceLock::new(),
            notices: notices.clone(),
        });
        let local = LocalName::parse("reading-list").unwrap();
        let by = LocalName::parse("recommendations").unwrap();
        for (reduction, expected, close) in [
            (
                watane_host::Reduction::Disable(local.clone()),
                Some("Turned off by Recommendations (local)"),
                true,
            ),
            (
                watane_host::Reduction::Withdraw {
                    local: local.clone(),
                    grant: Grant::Storage,
                },
                Some("Grant withdrawn by Recommendations (local)"),
                false,
            ),
            (watane_host::Reduction::ClosePage(local.clone()), None, true),
        ] {
            assert_eq!(reduction_effect(&reduction).0, &local);
            assert_eq!(reduction_effect(&reduction).2, close);
            let c = confirm.clone();
            let b = by.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                c.reduced(&b, &reduction);
                tx.send(()).unwrap();
            });
            rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
            if let Some(expected) = expected {
                assert_eq!(notices.get(local.as_ref(), None).as_deref(), Some(expected));
                assert_eq!(
                    notices
                        .get(
                            local.as_ref(),
                            Some("Withdrawn by the publisher: unsafe".into())
                        )
                        .as_deref(),
                    Some("Withdrawn by the publisher: unsafe")
                );
            }
        }
        notices.enable(&host, &local).unwrap();
        assert!(notices.get(local.as_ref(), None).is_none());
        assert!(
            ManagerNotices::default()
                .get(local.as_ref(), None)
                .is_none()
        );
    }
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
                parties.insert(dist.identity().unwrap().to_string(), "Alice".into());
            }
            let consent::Request::Install { mods, .. } =
                request_view(&request, &parties, Some(&host))
            else {
                panic!()
            };
            let label = if known {
                format!("Alice ({})", dist.fingerprint().unwrap())
            } else {
                dist.fingerprint().unwrap()
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
        let consent::Request::Install {
            mods, by_program, ..
        } = request_view(&request, &Parties::new(), None)
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
        let consent::Request::Install {
            mods, by_program, ..
        } = request_view(&request, &Parties::new(), Some(&host))
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
