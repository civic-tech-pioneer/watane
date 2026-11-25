//! HostConfirm and DistConfirm (TECHNICAL 9.2): adapters that turn host's and
//! dist's confirmations into requests on the consent Broker.

use crate::consent;
use crate::words::{self, Parties};
use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, OnceLock, Weak};
use watane_host::{Answer, Change, Confirm, InstallRequest, Requester};

pub(crate) struct HostConfirm {
    pub broker: Arc<consent::Broker>,
    pub parties: Arc<Parties>,
    pub host: OnceLock<Weak<watane_host::Host>>,
    pub core: OnceLock<Weak<crate::core::Core>>,
    pub main: OnceLock<Arc<crate::main_mod::MainMod>>,
    pub notices: Arc<ManagerNotices>,
    pub accounts: Arc<crate::accounts::Accounts>,
}

#[derive(Default)]
pub(crate) struct ManagerNotices(std::sync::Mutex<std::collections::BTreeMap<String, String>>);
impl ManagerNotices {
    pub fn get(&self, local: &str, release: Option<String>) -> Option<String> {
        release.or_else(|| {
            self.0
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .get(local)
                .cloned()
        })
    }
    pub fn enable(
        &self,
        host: &watane_host::Host,
        local: &watane_host::LocalName,
    ) -> Result<(), crate::CmdError> {
        host.enable(local)?;
        self.0
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .remove(local.as_ref());
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
fn manager_read_request(
    dist: &watane_dist::Dist,
    by: &consent::Asker,
    link: Option<&watane_dist::Link>,
    release: &str,
) -> Result<Option<consent::Request>, crate::CmdError> {
    if let Some(link) = link {
        let key = link.channel();
        let known = dist.identity() == Some(key)
            || dist.followed()?.iter().any(|v| v.channel == key)
            || dist.indexes()?.iter().any(|v| v.view.channel == key);
        if !known {
            return Ok(Some(consent::Request::Read {
                asked_by: by.clone(),
                link: link.to_string(),
                release: release.into(),
            }));
        }
    }
    Ok(None)
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
        watane_host::Reduction::OpenPage(l) => (l, None, false),
    }
}
pub(crate) fn open_page_target(
    host: &watane_host::Host,
    broker: &consent::Broker,
    local: &watane_host::LocalName,
) -> Option<watane_host::InstalledMod> {
    if broker.current().is_some() {
        return None;
    }
    host.get(local).filter(|m| m.enabled && m.page.is_some())
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
/// A provider as section 4 writes it, from the `via` of a consent or a request.
fn provider_text(
    host: &watane_host::Host,
    via: &watane_host::LocalName,
    parties: &Parties,
) -> String {
    host.get(via).map_or_else(
        || via.to_string(),
        |m| {
            words::provider(
                &watane_host::watane_pkg::Provider {
                    slug: m.slug,
                    publisher: m.publisher,
                },
                parties,
            )
        },
    )
}
/// The consent view of a host's question (19.2); `None` when its origin client is gone.
fn agent_view(
    host: &watane_host::Host,
    request: &watane_host::ConsentRequest,
    parties: &Parties,
) -> Option<consent::Request> {
    let origin = asker(&host.get(&request.origin)?, parties);
    let what = match &request.consent {
        watane_host::Consent::Workspace { folder, .. } => consent::AgentWhat::Workspace {
            folder: folder.clone(),
            profile: request.profile.clone(),
            provider: provider_text(host, &request.via, parties),
        },
        watane_host::Consent::Account { .. } => consent::AgentWhat::Account,
        watane_host::Consent::Toolset { toolset, .. } => consent::AgentWhat::Toolset {
            name: toolset.clone(),
            functions: request.functions.clone(),
        },
    };
    Some(consent::Request::Agent { what, origin })
}
/// The provider a plan entry's call reaches, from the plan first and else the installed mod
/// (the host resolves identities), and whether it holds a Claude Code grant (19.2, Plan line).
fn resolve_provider(
    request: &InstallRequest,
    host: Option<&watane_host::Host>,
    local: &watane_host::LocalName,
) -> Option<(watane_host::watane_pkg::Provider, bool)> {
    use watane_host::watane_pkg::{Grant, Provider};
    let claude = |g: &Grant| matches!(g, Grant::ClaudeCode | Grant::ClaudeCodeShell);
    if let Some(p) = request.entries.iter().find(|p| &p.local == local) {
        return Some((
            Provider {
                slug: p.slug.clone(),
                publisher: p.publisher.clone(),
            },
            p.grants.iter().any(claude),
        ));
    }
    host.and_then(|h| h.get(local)).map(|m| {
        let held = m.grants.iter().any(|g| g.granted && claude(&g.grant));
        (
            Provider {
                slug: m.slug,
                publisher: m.publisher,
            },
            held,
        )
    })
}
// The host calls Confirm::install inside the pending install future.
tokio::task_local! {
    pub(crate) static INSTALL_FOLLOWS: Vec<String>;
}

fn request_view(
    request: &InstallRequest,
    parties: &Parties,
    host: Option<&watane_host::Host>,
) -> consent::Request {
    let mut mods: Vec<_> = request
        .entries
        .iter()
        .map(|e| consent::PlanMod {
            becomes_main: false,
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
                    words::consent_grant_line(g, &e.binds, parties, true, e.new_grants.contains(g))
                })
                .collect(),
            uses: e
                .providers
                .iter()
                .map(|local| {
                    resolve_provider(request, host, local)
                        .map(|(p, _)| {
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
                                Some(b) => words::provider_bound(&b.provider, &e.binds, parties),
                                None => words::provider(&p, parties),
                            }
                        })
                        .unwrap_or_else(|| local.to_string())
                })
                .collect(),
            agents_via: e
                .providers
                .iter()
                .filter_map(|local| resolve_provider(request, host, local))
                .filter(|(_, claude)| *claude)
                .map(|(p, _)| words::provider(&p, parties))
                .collect(),
        })
        .collect();

    let mut call_words = BTreeMap::new();
    for grant in mods.iter().flat_map(|m| &m.grants) {
        if !grant.short.is_empty() {
            *call_words.entry(grant.short.clone()).or_insert(0) += 1;
        }
    }
    for grant in mods.iter_mut().flat_map(|m| &mut m.grants) {
        if call_words.get(&grant.short).is_some_and(|&count| count > 1) {
            grant.short.clear();
        }
    }
    consent::Request::Install {
        asked_by: match &request.requester {
            Requester::Mod(by) => host.and_then(|h| h.get(by)).map(|m| asker(&m, parties)),
            _ => None,
        },
        by_program: matches!(request.requester, Requester::LocalApi),
        following: INSTALL_FOLLOWS.try_with(Clone::clone).unwrap_or_default(),
        mods,
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
struct ManageTask<T = Result<(), crate::CmdError>>(tauri::async_runtime::JoinHandle<T>);
impl<T> Drop for ManageTask<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Confirm for HostConfirm {
    fn accounts(&self) -> Option<Arc<dyn watane_host::AccountSource>> {
        Some(Arc::new(crate::accounts::Source(Arc::clone(
            &self.accounts,
        ))))
    }
    fn installed(&self, report: &watane_host::Installed) {
        let Some(main) = self.main.get() else { return };
        if main.get().is_some() {
            return;
        }
        let Some(host) = self.host.get().and_then(Weak::upgrade) else {
            return;
        };
        // No host catalog calls or event-loop work on the host's install thread.
        for local in &report.new {
            if crate::main_mod::proposes(&host, local.as_ref()) {
                if let Err(error) = main.adopt(local.as_ref()) {
                    eprintln!("{}", error.message);
                }
                break;
            }
        }
    }

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
    fn consent(
        &self,
        request: watane_host::ConsentRequest,
    ) -> Pin<Box<dyn Future<Output = watane_host::ConsentAnswer> + Send>> {
        let broker = Arc::clone(&self.broker);
        let view = self
            .host
            .get()
            .and_then(Weak::upgrade)
            .and_then(|host| agent_view(&host, &request, &self.parties));
        Box::pin(async move {
            // No view means the origin is gone: nobody to name, nothing to keep.
            let Some(view) = view else {
                return watane_host::ConsentAnswer::Refused;
            };
            // Host polls this future on its own runtime. The broker's queue and timer run on
            // ours; dropping this future aborts the task, which leaves the queue.
            let mut task = ManageTask(tauri::async_runtime::spawn(async move {
                broker.ask(view).await
            }));
            match (&mut task.0).await {
                Ok(consent::Answer::Approve { withheld }) if withheld.is_empty() => {
                    watane_host::ConsentAnswer::Granted
                }
                _ => watane_host::ConsentAnswer::Refused,
            }
        })
    }
    fn reduced(&self, by: &watane_host::LocalName, reduction: &watane_host::Reduction) {
        let (local, notice, close) = reduction_effect(reduction);
        if let Some(host) = self.host.get().and_then(Weak::upgrade)
            && let Some(m) = host.get(by)
            && let Some(notice) = notice
        {
            self.notices
                .0
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .insert(
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
        if matches!(reduction, watane_host::Reduction::OpenPage(_))
            && let Some(core) = self.core.get().and_then(Weak::upgrade)
        {
            core.post_open(local.clone());
        }
    }
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        let broker = Arc::clone(&self.broker);
        let host = self.host.get().and_then(Weak::upgrade);
        let mut view = request_view(&request, &self.parties, host.as_deref());
        let core = self.core.get().and_then(Weak::upgrade);
        Box::pin(async move {
            if let Some(core) = core {
                match core.main.proposed(&core.host, &request.entries) {
                    Ok(proposed) => {
                        if let consent::Request::Install { mods, .. } = &mut view {
                            for m in mods {
                                m.becomes_main = proposed.as_deref() == Some(&m.local);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", e.message);
                        return Answer::Deny;
                    }
                }
            }
            host_answer(broker.ask(view).await, &request)
        })
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
            let read =
                manager_read_request(&core.dist, &asked_by, link.as_ref(), &release.to_string())?;
            let read_link = if let Some(read) = read {
                manage_answer(core.broker.ask(read).await)?;
                link
            } else {
                None
            };
            core.install_managed(release, read_link, requester, asked_by)
                .await?;
        }
        Ask::Update(local) => {
            let m = core
                .get(local.as_ref())?
                .ok_or_else(|| crate::CmdError::new("not_found", "mod not found"))?;
            let release = manager_offer(m.offer, &m.name)?;
            core.install_managed(release, None, requester, asked_by)
                .await?;
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
                    Some(words::consent_grant_line(
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
        let mut first_signature = false;
        if let Some(dist) = self.dist.get().and_then(Weak::upgrade) {
            first_signature =
                dist.identity().is_some() && dist.own().is_ok_and(|own| own.head.is_none());
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
        if first_signature && let consent::Request::Sign { details, .. } = &mut view {
            details.push(consent::Detail {
                label: String::new(),
                value: "After this, this home cannot join your other machines.".into(),
            });
        }
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
    use watane_host::watane_pkg::{Grant, Provider};
    use watane_host::{LocalName, PlanEntry};
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
    async fn ac1_installed_adopts_only_first_new_proposal_for_every_requester() {
        for requester in [
            Requester::App,
            Requester::LocalApi,
            Requester::Mod(LocalName::parse("manager").unwrap()),
        ] {
            let (_home, host) = preview();
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
            let other = tempfile::tempdir().unwrap();
            std::fs::create_dir(other.path().join("wit")).unwrap();
            std::fs::create_dir(other.path().join("ui")).unwrap();
            std::fs::write(
                other.path().join("mod.toml"),
                "name = \"Other desk\"\nsummary = \"Desk\"\nmain = true\n",
            )
            .unwrap();
            std::fs::write(
                other.path().join("wit/world.wit"),
                "package mods:zdesk@0.1.0; world zdesk {}",
            )
            .unwrap();
            std::fs::write(other.path().join("ui/index.html"), "<h1>Other desk</h1>").unwrap();
            let plan = host
                .plan(vec![
                    watane_host::Source::Folder(root.join("desk")),
                    watane_host::Source::Folder(root.join("reading-list")),
                    watane_host::Source::Folder(other.path().into()),
                ])
                .unwrap();
            host.install(plan, Requester::App).await.unwrap();
            let main = Arc::new(crate::main_mod::MainMod::open(&host).unwrap());
            let confirm = HostConfirm {
                broker: consent::Broker::new(),
                parties: Arc::new(Parties::new()),
                host: OnceLock::from(Arc::downgrade(&host)),
                core: OnceLock::new(),
                main: OnceLock::from(Arc::clone(&main)),
                notices: Arc::new(ManagerNotices::default()),
                accounts: Arc::new(crate::accounts::Accounts::new(_home.path())),
            };
            let report = watane_host::Installed {
                requester,
                new: vec![
                    LocalName::parse("reading-list").unwrap(),
                    LocalName::parse("zdesk").unwrap(),
                    LocalName::parse("desk").unwrap(),
                ],
                updated: vec![],
            };
            confirm.installed(&report);
            assert_eq!(main.get().as_deref(), Some("zdesk"));
            assert_eq!(
                std::fs::read_to_string(host.home().join("app/main")).unwrap(),
                "zdesk\n"
            );
            main.set(&host, Some("reading-list")).unwrap();
            confirm.installed(&report);
            assert_eq!(main.get().as_deref(), Some("reading-list"));
            main.set(&host, None).unwrap();
            let update = host
                .plan(vec![watane_host::Source::Folder(root.join("desk"))])
                .unwrap();
            assert_eq!(main.proposed(&host, update.entries()).unwrap(), None);
            confirm.installed(&watane_host::Installed {
                requester: report.requester,
                new: vec![],
                updated: report.new,
            });
            assert_eq!(main.get(), None);
            assert!(!host.home().join("app/main").exists());
            assert!(!crate::main_mod::proposes(&host, "absent"));
            std::fs::write(host.home().join("mods/desk/mod.toml"), "invalid").unwrap();
            assert!(!crate::main_mod::proposes(&host, "desk"));
        }
    }
    #[tokio::test]
    async fn ac3_cancelled_manager_request_leaves_no_consent() {
        let broker = consent::Broker::new();
        let queued = Arc::clone(&broker);
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
    #[tokio::test]
    async fn ac3_host_consent_answers_the_held_future_and_leaves_the_queue_when_dropped() {
        let (_home, host) = preview();
        fixtures(&host).await;
        let confirm = HostConfirm {
            broker: consent::Broker::new(),
            parties: Arc::new(Parties::new()),
            host: OnceLock::from(Arc::downgrade(&host)),
            core: OnceLock::new(),
            main: OnceLock::new(),
            notices: Arc::new(ManagerNotices::default()),
            accounts: Arc::new(crate::accounts::Accounts::new(_home.path())),
        };
        let request = |origin: &str| watane_host::ConsentRequest {
            origin: LocalName::parse(origin).unwrap(),
            via: LocalName::parse("reading-list").unwrap(),
            consent: watane_host::Consent::Workspace {
                via: "local/reading-list".into(),
                folder: "/w".into(),
            },
            profile: Some("coder".into()),
            functions: vec![],
        };
        let wait_head = |broker: Arc<consent::Broker>, open: bool| async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while broker.current().is_some() != open {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        };
        for (approve, expected) in [
            (true, watane_host::ConsentAnswer::Granted),
            (false, watane_host::ConsentAnswer::Refused),
        ] {
            let future = tokio::spawn(confirm.consent(request("recommendations")));
            wait_head(Arc::clone(&confirm.broker), true).await;
            let (_, view) = confirm.broker.current().unwrap();
            let consent::Request::Agent { what, origin } = view else {
                panic!("not an agent request");
            };
            assert_eq!(
                words::agent_consent(&what, &origin).1,
                "Let Recommendations (local) have agents work in /w (coder, through reading-list (local))?"
            );
            confirm.broker.answer_head(approve);
            assert_eq!(future.await.unwrap(), expected);
        }
        // A closed view or a quit denies every request.
        let future = tokio::spawn(confirm.consent(request("recommendations")));
        wait_head(Arc::clone(&confirm.broker), true).await;
        confirm.broker.deny_all();
        assert_eq!(future.await.unwrap(), watane_host::ConsentAnswer::Refused);
        // The host drops the future: the request leaves the queue.
        let future = confirm.consent(request("recommendations"));
        let task = tokio::spawn(future);
        wait_head(Arc::clone(&confirm.broker), true).await;
        task.abort();
        wait_head(Arc::clone(&confirm.broker), false).await;
        // An origin that is gone is refused without a view.
        assert_eq!(
            confirm.consent(request("gone")).await,
            watane_host::ConsentAnswer::Refused
        );
        assert!(confirm.broker.current().is_none());
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
    fn ac3_s82_unread_channel_requests_read_without_connecting() {
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
        let release = format!("{}/counter@0.1.0", link.channel());
        assert_eq!(
            manager_read_request(&dist, &by, Some(&link), &release).unwrap(),
            Some(consent::Request::Read {
                asked_by: by.clone(),
                link: link.to_string(),
                release
            })
        );
        assert!(
            manager_read_request(&dist, &by, None, "release")
                .unwrap()
                .is_none()
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
            main: OnceLock::new(),
            notices: Arc::clone(&notices),
            accounts: Arc::new(crate::accounts::Accounts::new(_home.path())),
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
            (watane_host::Reduction::OpenPage(local.clone()), None, false),
        ] {
            assert_eq!(reduction_effect(&reduction).0, &local);
            assert_eq!(reduction_effect(&reduction).2, close);
            let previous_notice = notices.get(local.as_ref(), None);
            let c = Arc::clone(&confirm);
            let b = by.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                c.reduced(&b, &reduction);
                tx.send(()).unwrap();
            });
            rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
            if expected.is_none() {
                assert_eq!(notices.get(local.as_ref(), None), previous_notice);
            }
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

    #[tokio::test]
    async fn ac1_open_page_rechecks_consent_and_current_mod_state() {
        let (_home, host) = preview();
        fixtures(&host).await;
        let _served = host.serve(0, axum::Router::new()).await.unwrap();
        let broker = consent::Broker::new();
        let local = LocalName::parse("reading-list").unwrap();
        assert!(open_page_target(&host, &broker, &local).is_some());
        host.disable(&local).unwrap();
        assert!(open_page_target(&host, &broker, &local).is_none());
        host.enable(&local).unwrap();
        let mut request = Box::pin(broker.ask(consent::Request::Install {
            asked_by: None,
            by_program: false,
            following: vec![],
            mods: vec![],
        }));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(
                request.as_mut().poll(cx).is_pending()
            ))
            .await
        );
        assert!(broker.current().is_some());
        assert!(open_page_target(&host, &broker, &local).is_none());
        drop(request);
        assert!(open_page_target(&host, &broker, &local).is_some());
        host.remove(&local, false).unwrap();
        assert!(open_page_target(&host, &broker, &local).is_none());

        let plan = host
            .plan(vec![watane_host::Source::Folder(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../host/tests/fixtures/mods/echo"),
            )])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        assert!(open_page_target(&host, &broker, &LocalName::parse("echo").unwrap()).is_none());
    }
    #[test]
    fn regress_s113_repeated_call_words_show_exact_grants() {
        let entry = |local: &str, interface: &str| PlanEntry {
            local: LocalName::parse(local).unwrap(),
            identity: format!("local/{local}"),
            slug: local.parse().unwrap(),
            name: local.into(),
            summary: String::new(),
            version: "1.0.0".parse().unwrap(),
            publisher: None,
            package: "00".repeat(32).parse().unwrap(),
            release: None,
            origin: None,
            change: Change::New,
            grants: vec![Grant::Call {
                interface: interface.parse().unwrap(),
                provider: Provider {
                    slug: "ratings".parse().unwrap(),
                    publisher: None,
                },
            }],
            new_grants: vec![],
            providers: vec![],
            binds: vec![],
            kept_state: false,
        };
        let request = InstallRequest {
            entries: vec![
                entry("first", "mods:a/books@0.1.0"),
                entry("second", "mods:b/books@0.1.0"),
                entry("third", "mods:a/scores@0.1.0"),
            ],
            requester: Requester::App,
        };
        let consent::Request::Install { mods, .. } = request_view(&request, &Parties::new(), None)
        else {
            panic!("expected install consent");
        };
        let first = &mods[0].grants[0];
        let second = &mods[1].grants[0];
        assert!(first.short.is_empty());
        assert!(second.short.is_empty());
        assert_ne!(first.text, second.text);
        assert_eq!(mods[2].grants[0].short, "Call scores of ratings (local)");
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
            kept_state: false,
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
            kept_state: false,
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
        // 19.2, Plan line: only a provider holding a Claude Code grant is named.
        assert!(mods[1].agents_via.is_empty());
        let mut request = request;
        request.entries[0].grants = vec![Grant::ClaudeCodeShell];
        let consent::Request::Install { mods, .. } = request_view(&request, &parties, None) else {
            panic!()
        };
        assert_eq!(mods[1].agents_via, ["ratings by Frank (yyyyyyyyyy)"]);
        assert!(mods[0].agents_via.is_empty());
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
    fn ac5_s81_first_publish_warns_join_is_lost() {
        struct Approve;
        impl watane_dist::Confirm for Approve {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                watane_dist::Decision::Approve
            }
        }
        let package = |fixture: &str| {
            watane_host::watane_pkg::pack(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(fixture),
                false,
            )
            .unwrap()
            .into_bytes()
        };
        for signed in [false, true] {
            let home = tempfile::tempdir().unwrap();
            if signed {
                watane_dist::Dist::open(home.path(), Arc::new(Approve), Default::default())
                    .unwrap()
                    .publish(package("reading-list"))
                    .unwrap();
            }
            let broker = consent::Broker::new();
            let confirm = Arc::new(DistConfirm {
                broker: Arc::clone(&broker),
                parties: Arc::new(Parties::new()),
                dist: OnceLock::new(),
            });
            let dist = Arc::new(
                watane_dist::Dist::open(
                    home.path(),
                    Arc::<DistConfirm>::clone(&confirm),
                    Default::default(),
                )
                .unwrap(),
            );
            confirm.dist.set(Arc::downgrade(&dist)).unwrap();
            let signing = Arc::clone(&dist);
            let hostile = package("hostile");
            let pending = std::thread::spawn(move || signing.publish(hostile));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            let request = loop {
                if let Some((_, current)) = broker.current() {
                    break current;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            };
            broker.deny_all();
            pending.join().unwrap().unwrap_err();
            let consent::Request::Sign { details, .. } = request else {
                panic!()
            };
            let found = details
                .iter()
                .any(|d| d.value == "After this, this home cannot join your other machines.");
            assert_eq!(found, !signed, "signed home: {signed}");
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
            kept_state: false,
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
        // Its granted grants hold no Claude Code grant: no plan line (19.2).
        assert!(mods[0].agents_via.is_empty());
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
