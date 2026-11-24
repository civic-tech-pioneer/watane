//! Core: every call into host and dist, mapped to the view types the windows
//! show. No rule of installing, running, signing or verifying lives here; a
//! sibling whose API differs is adapted here or in confirm.rs only.

use crate::consent::{GrantLine, Party};
use crate::words::{self, Parties};
use crate::{CmdError, views};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use watane_host::watane_pkg::Grant;
use watane_host::{GrantState, Host, LocalName, Served};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModView {
    pub main: bool,
    pub proposes_main: bool,
    pub links: Vec<ModLink>,
    pub summary: String,
    pub local: String,
    pub name: String,
    pub slug: String,
    pub version: String,
    pub release: Option<String>,
    pub publisher: Option<Party>,
    pub enabled: bool,
    pub page: Option<String>,
    pub package: String,
    pub derived_from: Option<String>,
    pub derived_from_label: Option<String>,
    pub error: Option<String>,
    pub grants: Vec<GrantLine>,
    pub consents: Vec<crate::consent::ConsentLine>,
    pub notice: Option<String>,
    pub offer: Option<String>,
    pub offer_grants: Vec<GrantLine>,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ModLink {
    kind: String,
    pub(crate) provider: String,
    pub(crate) granted: bool,
}
pub(crate) fn mod_links(m: &watane_host::InstalledMod) -> Vec<ModLink> {
    m.links
        .iter()
        .filter_map(|link| {
            let provider = link.provider.as_ref()?.to_string();
            let kind = match link.kind {
                watane_host::LinkKind::Call => "call",
                watane_host::LinkKind::Subscribe => "subscribe",
            };
            let granted =
                m.grants.iter().any(|g| {
                    g.granted
                        && match (&g.grant, &link.kind) {
                            (Grant::Call { interface, .. }, watane_host::LinkKind::Call)
                            | (
                                Grant::Subscribe { interface, .. },
                                watane_host::LinkKind::Subscribe,
                            ) => interface == &link.interface,
                            _ => false,
                        }
                });
            Some(ModLink {
                kind: kind.into(),
                provider,
                granted,
            })
        })
        .collect()
}
#[derive(serde::Serialize)]
pub(crate) struct ToolRow {
    pub entry: String,
    pub tool: String,
}
fn tool_name(entry: &str) -> String {
    entry.replace([':', '.'], "_")
}
fn tool_row(local: &str, interface: &str, function: &str) -> ToolRow {
    let entry = format!("{local}:{interface}.{function}");
    let tool = tool_name(&entry);
    ToolRow { entry, tool }
}
pub(crate) fn toolset_row(name: &str, entries: Vec<String>) -> serde_json::Value {
    let tools: Vec<_> = entries.iter().map(|entry| tool_name(entry)).collect();
    serde_json::json!({"name":name, "entries":entries, "tools":tools})
}
fn map_follow(
    dist: &watane_dist::Dist,
    parties: &Parties,
    result: Result<watane_dist::RefreshReport, watane_dist::DistError>,
) -> Result<watane_dist::RefreshReport, CmdError> {
    result.map_err(|error| {
        if let watane_dist::DistError::WrongChannel {
            url,
            expected,
            found,
        } = &error
        {
            // A cached profile is available even when this channel is not followed.
            let _ = cache_channels(dist, parties, [*expected, *found]);
            CmdError::new(
                "dist",
                words::wrong_channel(url, &expected.to_string(), &found.to_string(), parties),
            )
        } else {
            error.into()
        }
    })
}
fn consumer_pages(mods: &[watane_host::InstalledMod], provider: &LocalName) -> Vec<LocalName> {
    mods.iter()
        .filter(|m| {
            m.page.is_some()
                && m.links
                    .iter()
                    .any(|link| link.provider.as_ref() == Some(provider))
        })
        .map(|m| m.local.clone())
        .collect()
}
pub(crate) struct Core {
    pub node: Arc<watane_node::Node>,
    pub machines: Arc<crate::machines::Machines>,
    pub broker: Arc<crate::consent::Broker>,
    pub main: Arc<crate::main_mod::MainMod>,
    pub host: Arc<Host>,
    pub dist: Arc<watane_dist::Dist>,
    failures: Mutex<std::collections::BTreeMap<String, String>>,
    refresh_times: Mutex<RefreshTimes>,
    updates: Mutex<Vec<UpdateDone>>,
    update_failures: Mutex<std::collections::BTreeMap<String, String>>,
    pub served: Mutex<Option<Served>>,
    app: tauri::AppHandle,
    pub(crate) parties: Arc<Parties>,
    pub(crate) manager_notices: Arc<crate::confirm::ManagerNotices>,
}
#[derive(Clone, serde::Serialize)]
pub(crate) struct UpdateDone {
    local: String,
    name: String,
    from: String,
    to: String,
    at: u64,
}
fn release_offer(
    installed: Option<&watane_dist::ReleaseRef>,
    channel: Option<&watane_dist::ChannelView>,
) -> Option<watane_dist::ReleaseRef> {
    let installed = installed?;
    channel?
        .releases
        .iter()
        .filter(|r| r.reference.slug == installed.slug && r.withdrawn.is_none())
        .max_by(|a, b| a.reference.version.cmp(&b.reference.version))
        .filter(|r| r.reference.version != installed.version)
        .map(|r| r.reference.clone())
}
fn own_releases(own: &watane_dist::ChannelView) -> Vec<serde_json::Value> {
    own.releases.iter().map(|r| serde_json::json!({"release":r.reference.to_string(),"version":r.reference.version.to_string(),"slug":r.reference.slug.to_string(),"withdrawn":r.withdrawn.as_ref().map(|w| &w.reason)})).collect()
}
fn release_notice(
    installed: Option<&watane_dist::ReleaseRef>,
    channel: Option<&watane_dist::ChannelView>,
    failure: Option<&String>,
) -> Option<String> {
    let installed = installed?;
    let channel = channel?;
    if matches!(channel.status, watane_dist::Status::Compromised { .. }) {
        return Some(words::COMPROMISED.into());
    }
    if let Some(withdrawal) = channel
        .releases
        .iter()
        .find(|r| &r.reference == installed)
        .and_then(|r| r.withdrawn.as_ref())
    {
        return Some(words::withdrawn(&withdrawal.reason));
    }
    failure.cloned()
}
fn update_failure(version: &str, error: &CmdError) -> String {
    if error.code == "refused" {
        words::update_declined(version)
    } else {
        words::update_failed(version, &error.message)
    }
}
fn offer_grants(
    offer: Option<&watane_dist::ReleaseRef>,
    channel: Option<&watane_dist::ChannelView>,
    installed: &[GrantState],
    parties: &Parties,
) -> Vec<GrantLine> {
    offer
        .and_then(|offer| channel?.releases.iter().find(|r| &r.reference == offer))
        .map(|r| {
            r.release
                .grants
                .iter()
                .filter(|grant| !installed.iter().any(|g| &g.grant == *grant))
                .map(|grant| words::grant_line(grant, parties, false, true))
                .collect()
        })
        .unwrap_or_default()
}
fn update_offer(
    installed: Option<&watane_dist::ReleaseRef>,
    channel: &watane_dist::ChannelView,
    changes: &[watane_dist::Change],
) -> Option<watane_dist::ReleaseRef> {
    let installed = installed?;
    if matches!(channel.status, watane_dist::Status::Compromised { .. })
        || !changes.iter().any(
            |c| matches!(c, watane_dist::Change::Release(r) if r.reference.slug == installed.slug && r.reference.version > installed.version),
        )
    {
        return None;
    }
    release_offer(Some(installed), Some(channel)).filter(|r| r.version > installed.version)
}
fn find_grant(states: &[GrantState], key: &str) -> Result<Grant, CmdError> {
    states
        .iter()
        .find(|s| s.grant.to_string() == key)
        .map(|s| s.grant.clone())
        .ok_or_else(|| CmdError::new("not_found", "grant not found"))
}
fn origin_label(
    origin: Option<&watane_dist::ReleaseRef>,
    channel: Option<&watane_dist::ChannelView>,
    parties: &Parties,
) -> Option<String> {
    let origin = origin?;
    let release = channel.and_then(|c| c.releases.iter().find(|r| &r.reference == origin));
    let text = match release {
        Some(r) => {
            let party = words::party(&origin.publisher.to_string(), parties);
            let label = words::party_label(&party);
            format!("{} {} by {label}", r.release.name, origin.version)
        }
        None => words::release_label(origin, parties),
    };
    Some(format!("Made from {text} (the publisher says so)"))
}
impl Core {
    pub fn new(
        app: tauri::AppHandle,
        host: Arc<Host>,
        node: Arc<watane_node::Node>,
        parties: Arc<Parties>,
        broker: Arc<crate::consent::Broker>,
        manager_notices: Arc<crate::confirm::ManagerNotices>,
    ) -> Result<Self, CmdError> {
        Ok(Self {
            manager_notices,
            main: Arc::new(crate::main_mod::MainMod::open(&host)?),
            app,
            host,
            served: Mutex::new(None),
            dist: Arc::clone(node.dist()),
            machines: crate::machines::Machines::start(&node, Arc::clone(&broker)),
            node,
            broker,
            failures: Mutex::new(Default::default()),
            refresh_times: Mutex::new(Default::default()),
            parties,
            updates: Mutex::new(Vec::new()),
            update_failures: Mutex::new(Default::default()),
        })
    }
    fn installed_channels(
        dist: &watane_dist::Dist,
        parties: &Parties,
        mods: &[watane_host::InstalledMod],
    ) -> Result<Vec<watane_dist::ChannelView>, CmdError> {
        let keys = mods
            .iter()
            .flat_map(|m| {
                m.publisher
                    .iter()
                    .map(ToString::to_string)
                    .chain(m.origin.iter().map(|r| r.publisher.to_string()))
                    .chain(m.grants.iter().filter_map(|g| match &g.grant {
                        Grant::Call { provider, .. } | Grant::Subscribe { provider, .. } => {
                            provider.publisher.as_ref().map(ToString::to_string)
                        }
                        _ => None,
                    }))
                    .chain(
                        m.binds
                            .iter()
                            .filter_map(|b| b.provider.publisher.as_ref().map(ToString::to_string)),
                    )
                    .chain(
                        m.binds
                            .iter()
                            .filter_map(|b| b.to.split_once('/').map(|(k, _)| k.to_string())),
                    )
            })
            .filter_map(|k| k.parse().ok());
        cache_channels(dist, parties, keys)?;
        Ok(parties.channels.followed(dist)?)
    }
    pub(crate) fn cache_plan_parties(&self, plan: &watane_host::Plan) -> Result<(), CmdError> {
        let keys = plan
            .entries()
            .iter()
            .flat_map(|e| {
                e.publisher
                    .iter()
                    .map(ToString::to_string)
                    .chain(e.origin.iter().map(|r| r.publisher.to_string()))
                    .chain(e.binds.iter().flat_map(|b| {
                        [
                            b.provider.publisher.as_ref().map(ToString::to_string),
                            b.to.split_once('/').map(|(k, _)| k.to_string()),
                        ]
                        .into_iter()
                        .flatten()
                    }))
                    .chain(e.grants.iter().filter_map(|g| match g {
                        Grant::Call { provider, .. } | Grant::Subscribe { provider, .. } => {
                            provider.publisher.as_ref().map(ToString::to_string)
                        }
                        _ => None,
                    }))
            })
            .filter_map(|k| k.parse().ok());
        Ok(cache_channels(&self.dist, &self.parties, keys)?)
    }
    pub fn port(&self) -> u16 {
        self.served
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .as_ref()
            .map_or(0, |s| s.port)
    }
    fn view(
        &self,
        m: watane_host::InstalledMod,
        followed: &[watane_dist::ChannelView],
        names: &std::collections::BTreeMap<String, String>,
    ) -> ModView {
        let channel = m.publisher.as_ref().and_then(|p| {
            followed
                .iter()
                .find(|c| c.channel.to_string() == p.to_string())
        });
        let failures = self
            .update_failures
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let notice = release_notice(m.release.as_ref(), channel, failures.get(m.local.as_ref()))
            .or_else(|| {
                m.publisher.as_ref().and_then(|p| {
                    self.failures
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it")
                        .get(&p.to_string())
                        .map(|cause| words::channel_read(&p.to_string(), cause, &self.parties))
                })
            });
        let offer = release_offer(m.release.as_ref(), channel);
        let mut offer_grants = offer_grants(offer.as_ref(), channel, &m.grants, &self.parties);
        for line in &mut offer_grants {
            if let Some(grant) = offer
                .as_ref()
                .and_then(|offer| channel?.releases.iter().find(|r| &r.reference == offer))
                .and_then(|r| r.release.grants.iter().find(|g| g.to_string() == line.key))
            {
                line.short = words::grant_short_bound(grant, &m.binds, &self.parties, names);
            }
        }
        let offer = offer.map(|r| r.to_string());
        let origin_channel = m
            .origin
            .as_ref()
            .and_then(|r| r.publisher.to_string().parse().ok())
            .and_then(|key| self.parties.channels.channel(&self.dist, &key).ok());
        let derived_from_label =
            origin_label(m.origin.as_ref(), origin_channel.as_ref(), &self.parties);
        let metadata = watane_host::watane_pkg::Mod::read(
            &self.host.home().join("mods").join(m.local.as_ref()),
        )
        .ok();
        let consents = m
            .consents
            .iter()
            .map(|c| words::consent_line(c, &m.name, &self.parties))
            .collect();
        ModView {
            main: self.main.get().as_deref() == Some(m.local.as_ref()),
            proposes_main: metadata.as_ref().is_some_and(|m| m.main()),
            summary: metadata
                .as_ref()
                .map(|m| m.summary().to_owned())
                .unwrap_or_default(),
            links: mod_links(&m),
            derived_from_label,
            notice: self.manager_notices.get(m.local.as_ref(), notice),
            offer,
            offer_grants,
            local: m.local.to_string(),
            name: m.name,
            slug: m.slug.to_string(),
            version: m.version.to_string(),
            release: m.release.map(|r| r.to_string()),
            publisher: m
                .publisher
                .map(|p| words::party(&p.to_string(), &self.parties)),
            enabled: m.enabled,
            page: m.page,
            package: m.package.to_string(),
            derived_from: m.origin.map(|r| r.to_string()),
            error: m.error,
            consents,
            grants: m
                .grants
                .iter()
                .map(|g| {
                    let mut line = words::grant_line_bound(
                        &g.grant,
                        &m.binds,
                        &self.parties,
                        g.granted,
                        false,
                    );
                    line.short = if m.binds.is_empty() {
                        words::grant_short(&g.grant, &self.parties, names)
                    } else {
                        words::grant_short_bound(&g.grant, &m.binds, &self.parties, names)
                    };
                    line
                })
                .collect(),
        }
    }
    pub fn mods(&self) -> Result<Vec<ModView>, CmdError> {
        let mods = self.host.mods();
        let followed = Self::installed_channels(&self.dist, &self.parties, &mods)?;
        let names = mods
            .iter()
            .map(|m| (m.identity.clone(), m.name.clone()))
            .collect();
        Ok(mods
            .into_iter()
            .map(|m| self.view(m, &followed, &names))
            .collect())
    }
    pub fn get(&self, local: &str) -> Result<Option<ModView>, CmdError> {
        let name = LocalName::parse(local)?;
        let mods = self.host.mods();
        let followed = Self::installed_channels(&self.dist, &self.parties, &mods)?;
        let names = mods
            .iter()
            .map(|m| (m.identity.clone(), m.name.clone()))
            .collect();
        Ok(self
            .host
            .get(&name)
            .map(|m| self.view(m, &followed, &names)))
    }
    pub fn enable(&self, local: &str) -> Result<(), CmdError> {
        let name = LocalName::parse(local)?;
        self.manager_notices.enable(&self.host, &name)?;
        self.reload_consumers(&name)
    }
    fn reload_consumers(&self, provider: &LocalName) -> Result<(), CmdError> {
        for local in consumer_pages(&self.host.mods(), provider) {
            if let Some(window) = self.app.get_webview_window(&format!("mod-{local}")) {
                // As for a grant change, a subsequent wait must observe the
                // replacement document's page-load event.
                #[cfg(feature = "driver")]
                self.app
                    .state::<crate::AppState>()
                    .driver
                    .loaded(window.label(), false);
                window.reload()?;
            }
        }
        Ok(())
    }
    pub fn post_close(&self, local: String) {
        let app = self.app.clone();
        if let Err(error) = self.app.run_on_main_thread(move || {
            if let Err(error) = views::close_mod(&app, &local) {
                eprintln!("{}", error.message);
            }
        }) {
            eprintln!("cannot schedule page close: {error}");
        }
    }
    pub fn post_open(self: &Arc<Self>, local: LocalName) {
        let core = Arc::clone(self);
        if let Err(error) = self.app.run_on_main_thread(move || {
            // Recheck at application time: the call's later effects may have
            // disabled or removed the target after OpenPage was posted.
            let Some(m) = crate::confirm::open_page_target(&core.host, &core.broker, &local) else {
                return;
            };
            let party = m
                .publisher
                .map(|p| words::party_label(&words::party(&p.to_string(), &core.parties)))
                .unwrap_or_else(|| "local".into());
            if let Err(error) = views::open_mod(
                &core.app,
                local.as_ref(),
                m.page
                    .as_deref()
                    .expect("open_page_target returns only mods with a page"),
                &format!("{} — {party}", m.name),
            ) {
                eprintln!("{}", error.message);
            }
        }) {
            eprintln!("cannot schedule page open: {error}");
        }
    }
    pub fn disable(&self, local: &str) -> Result<(), CmdError> {
        let name = LocalName::parse(local)?;
        views::close_mod(&self.app, local)?;
        self.host.disable(&name)?;
        self.reload_consumers(&name)
    }
    pub fn remove(&self, local: &str, delete_data: bool) -> Result<(), CmdError> {
        let name = LocalName::parse(local)?;
        views::close_mod(&self.app, local)?;
        self.host.remove(&name, delete_data)?;
        self.main.removed(local)?;
        Ok(())
    }
    pub fn grant(&self, local: &str, key: &str, give: bool) -> Result<(), CmdError> {
        let local = LocalName::parse(local)?;
        let m = self
            .host
            .get(&local)
            .ok_or_else(|| CmdError::new("not_found", "mod not found"))?;
        let grant = find_grant(&m.grants, key)?;
        if give {
            self.host.grant(&local, &grant)?;
        } else {
            self.host.revoke(&local, &grant)?;
        }
        // The stage B host changes authority immediately but emits no page
        // notification for grant changes. Refresh the open page's view of it.
        if let Some(window) = self.app.get_webview_window(&format!("mod-{local}")) {
            // A wait issued just after this command must observe the new
            // page-load event, not the previous document's finished state.
            #[cfg(feature = "driver")]
            self.app
                .state::<crate::AppState>()
                .driver
                .loaded(window.label(), false);
            window.reload()?;
        }
        Ok(())
    }
    /// Takes one consent back (19.2): the host ends what depends on it.
    pub fn consent_withdraw(&self, local: &str, key: &str) -> Result<(), CmdError> {
        let local = LocalName::parse(local)?;
        let consent = watane_host::Consent::parse(key)
            .map_err(|e| CmdError::new("bad_input", crate::cause_chain(&e)))?;
        self.host.withdraw_consent(&local, &consent)?;
        Ok(())
    }
    pub fn tools(&self) -> Result<Vec<ToolRow>, CmdError> {
        let mut tools = Vec::new();
        for m in self.host.mods().into_iter().filter(|m| m.enabled) {
            for f in self.host.functions(&m.local)? {
                tools.push(tool_row(m.local.as_ref(), f.interface.interface(), &f.name));
            }
        }
        Ok(tools)
    }
}
#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshTimes {
    checked_at: Option<u64>,
    #[serde(skip)]
    read_at: std::collections::BTreeMap<String, u64>,
}
impl RefreshTimes {
    fn finish(&mut self, reports: &[watane_dist::RefreshReport], at: u64) {
        for report in reports {
            if matches!(
                report.outcome,
                watane_dist::Outcome::Current { .. } | watane_dist::Outcome::Behind { .. }
            ) {
                self.read_at.insert(report.channel.to_string(), at);
            }
        }
        self.checked_at = Some(at);
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelRow {
    read_at: Option<u64>,
    channel: String,
    fingerprint: String,
    name: Option<String>,
    base: Option<String>,
    status: &'static str,
    cause: Option<String>,
}
fn channel_row(view: &watane_dist::ChannelView, failure: Option<&String>) -> ChannelRow {
    ChannelRow {
        read_at: None,
        channel: view.channel.to_string(),
        fingerprint: view.fingerprint.clone(),
        name: view.name.clone(),
        base: view.base.clone(),
        status: if matches!(view.status, watane_dist::Status::Compromised { .. }) {
            "compromised"
        } else if failure.is_some() {
            "failed"
        } else {
            "ok"
        },
        cause: failure.cloned(),
    }
}
fn keep_outcome(
    failures: &mut std::collections::BTreeMap<String, String>,
    channel: watane_dist::PublicKey,
    outcome: watane_dist::Outcome,
) {
    match outcome {
        watane_dist::Outcome::Failed { cause } => {
            failures.insert(channel.to_string(), CmdError::from(cause).message);
        }
        _ => {
            failures.remove(&channel.to_string());
        }
    }
}
fn resolve_plan<S, P>(
    mut sources: Vec<S>,
    mut plan: impl FnMut(
        &[S],
        &[watane_host::Bind],
    ) -> Result<(P, Vec<Option<watane_dist::ReleaseRef>>), CmdError>,
    mut fetch: impl FnMut(
        &P,
        &watane_dist::ReleaseRef,
    ) -> Result<(Option<S>, Vec<watane_host::Bind>), CmdError>,
) -> Result<(P, Vec<S>), CmdError> {
    let mut added = std::collections::BTreeSet::new();
    let mut binds = Vec::new();
    for _ in 0..32 {
        let (plan, missing) = plan(&sources, &binds)?;
        if missing.is_empty() {
            return Ok((plan, sources));
        }
        let before = sources.len() + binds.len();
        for release in missing {
            let release = release.ok_or_else(|| {
                CmdError::new("missing_providers", "a local provider has no release")
            })?;
            if added.insert(release.clone()) {
                let (source, picked) = fetch(&plan, &release)?;
                sources.extend(source);
                let replan = !picked.is_empty();
                for bind in picked {
                    if !binds.contains(&bind) {
                        binds.push(bind);
                    }
                }
                // The pick binds every current consumer. Remaining missing
                // entries in this plan may now be satisfied by that same pick.
                if replan {
                    break;
                }
            }
        }
        if before == sources.len() + binds.len() {
            return Err(CmdError::new(
                "missing_providers",
                "provider resolution added no source",
            ));
        }
    }
    Err(CmdError::new(
        "missing_providers",
        "provider resolution exceeded 32 rounds",
    ))
}

fn dist_status(
    dist: &watane_dist::Dist,
) -> Result<(serde_json::Value, serde_json::Value), CmdError> {
    if dist.identity().is_none() {
        return Ok((serde_json::Value::Null, serde_json::Value::Null));
    }
    let own = dist.own()?;
    let link = if own.base.is_some() {
        Some(dist.own_link(None)?.to_string())
    } else {
        None
    };
    Ok((
        serde_json::json!({"name":own.name,"fingerprint":dist.fingerprint(),"signed":own.head.is_some(),"ownConfirmed":dist.own_confirmed()?,"releases":own_releases(&own)}),
        serde_json::json!({"dir":dist.site_dir(),"url":own.base,"link":link}),
    ))
}

impl Core {
    pub fn status(&self) -> Result<(serde_json::Value, serde_json::Value), CmdError> {
        dist_status(&self.dist)
    }

    pub fn checked_at(&self) -> Option<u64> {
        self.refresh_times
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .checked_at
    }

    pub fn channels(&self) -> Result<Vec<ChannelRow>, CmdError> {
        let followed = self.parties.channels.followed(&self.dist)?;
        self.parties.remember(&party_views(&self.dist)?);
        if let Some(key) = self.dist.identity() {
            self.parties.set_own(key.to_string());
        }
        let failures = self
            .failures
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        Ok(followed
            .iter()
            .map(|v| {
                let mut row = channel_row(v, failures.get(&v.channel.to_string()));
                row.read_at = self
                    .refresh_times
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .read_at
                    .get(&row.channel)
                    .copied();
                row
            })
            .collect())
    }
    pub fn refresh(&self) -> Result<(Vec<ChannelRow>, Vec<watane_dist::ReleaseRef>), CmdError> {
        let reports = self.node.refresh();
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.refresh_times
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .finish(&reports, at);
        let followed = self.parties.channels.followed(&self.dist)?;
        let mut updates = Vec::new();
        for report in reports {
            if let Some(channel) = followed.iter().find(|c| c.channel == report.channel) {
                for m in self.host.mods().iter().filter(|m| {
                    m.publisher
                        .as_ref()
                        .is_some_and(|p| p.to_string() == report.channel.to_string())
                }) {
                    if let Some(offer) = update_offer(m.release.as_ref(), channel, &report.changes)
                    {
                        updates.push(offer);
                    }
                }
            }
            keep_outcome(
                &mut self
                    .failures
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it"),
                report.channel,
                report.outcome,
            );
        }
        Ok((self.channels()?, updates))
    }
    pub fn updates_done(&self) -> Vec<UpdateDone> {
        self.updates
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone()
    }
    pub fn spawn_updates(self: &Arc<Self>, updates: Vec<watane_dist::ReleaseRef>) {
        for release in updates {
            let core = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                let _ = core
                    .install_resolved(release, None, None, watane_host::Requester::App, None)
                    .await;
            });
        }
    }
    pub fn clear_update_failures(&self, installed: &[watane_host::InstalledMod]) {
        let mut failures = self
            .update_failures
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        for m in installed {
            failures.remove(m.local.as_ref());
        }
    }
    pub async fn install_release(
        self: &Arc<Self>,
        release: watane_dist::ReleaseRef,
        link: Option<watane_dist::Link>,
    ) -> Result<Vec<watane_host::InstalledMod>, CmdError> {
        self.install_resolved(
            release,
            link,
            Some(Arc::clone(&self.app.state::<crate::AppState>().broker)),
            watane_host::Requester::App,
            None,
        )
        .await
    }
    pub async fn install_managed(
        self: &Arc<Self>,
        release: watane_dist::ReleaseRef,
        read_link: Option<watane_dist::Link>,
        requester: watane_host::Requester,
        asked_by: crate::consent::Asker,
    ) -> Result<Vec<watane_host::InstalledMod>, CmdError> {
        self.install_resolved(
            release,
            read_link,
            Some(Arc::clone(&self.broker)),
            requester,
            Some(asked_by),
        )
        .await
    }
    async fn install_resolved(
        self: &Arc<Self>,
        release: watane_dist::ReleaseRef,
        link: Option<watane_dist::Link>,
        pick: Option<Arc<crate::consent::Broker>>,
        requester: watane_host::Requester,
        asked_by: Option<crate::consent::Asker>,
    ) -> Result<Vec<watane_host::InstalledMod>, CmdError> {
        let planner = Arc::clone(self);
        let target = release.clone();
        let background = pick.is_none();
        let result = async {
            let (plan, fetched) = tauri::async_runtime::spawn_blocking(move || {
                let fetched = if let Some(link) = &link
                    && asked_by.is_some()
                {
                    read_managed_release(&planner.node, &target, link)?
                } else {
                    fetch_for_install(&planner.node, &target, link.as_ref())?
                };
                planner.plan_fetched(fetched, pick.as_deref(), asked_by.as_ref())
            })
            .await
            .map_err(|e| CmdError::new("io", e.to_string()))??;
            let automatic = background && !plan.needs_confirmation();
            let before = self.host.mods();
            let installed = self.host.install(plan, requester).await?;
            self.clear_update_failures(&installed);
            if automatic {
                let at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let mut updates = self
                    .updates
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it");
                for m in &installed {
                    if let Some(old) = before
                        .iter()
                        .find(|old| old.local == m.local && old.version != m.version)
                    {
                        updates.insert(
                            0,
                            UpdateDone {
                                local: m.local.to_string(),
                                name: m.name.clone(),
                                from: old.version.to_string(),
                                to: m.version.to_string(),
                                at,
                            },
                        );
                    }
                }
                updates.truncate(50);
            }
            let follower = Arc::clone(self);
            tauri::async_runtime::spawn_blocking(move || follower.follow_fetched(&fetched))
                .await
                .map_err(|e| CmdError::new("io", e.to_string()))?;
            Ok::<_, CmdError>(installed)
        }
        .await;
        match &result {
            Ok(_) => {}
            Err(error) => {
                for m in self.host.mods().iter().filter(|m| {
                    m.release
                        .as_ref()
                        .is_some_and(|r| r.publisher == release.publisher && r.slug == release.slug)
                }) {
                    self.update_failures
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it")
                        .insert(
                            m.local.to_string(),
                            update_failure(&release.version.to_string(), error),
                        );
                }
            }
        }
        result
    }
    pub fn follow(&self, text: &str) -> Result<ChannelRow, CmdError> {
        let link: watane_dist::Link = text.parse()?;
        map_follow(&self.dist, &self.parties, self.node.follow(&link))?;
        self.failures
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .remove(&link.channel().to_string());
        self.channels()?
            .into_iter()
            .find(|c| c.channel == link.channel().to_string())
            .ok_or_else(|| CmdError::new("not_found", "followed channel missing"))
    }
    pub fn plan_link(
        &self,
        text: &str,
    ) -> Result<(watane_host::Plan, Vec<watane_dist::Fetched>), CmdError> {
        let link: watane_dist::Link = text.parse()?;
        if link.slug().is_none() {
            self.follow(text)?;
            return Err(CmdError::new(
                "bad_input",
                "the channel is followed; the link names no mod",
            ));
        }
        let first = self.node.open_link(&link)?;
        self.channels()?;
        self.plan_fetched(
            first,
            Some(&self.app.state::<crate::AppState>().broker),
            None,
        )
    }
    fn plan_fetched(
        &self,
        first: watane_dist::Fetched,
        pick: Option<&crate::consent::Broker>,
        asked_by: Option<&crate::consent::Asker>,
    ) -> Result<(watane_host::Plan, Vec<watane_dist::Fetched>), CmdError> {
        let result = resolve_plan(
            vec![first],
            |fetched, binds| {
                let plan = self.host.plan_with(
                    fetched
                        .iter()
                        .map(|f| watane_host::Source::Release {
                            package: f.package.clone(),
                            entry: f.entry.clone(),
                        })
                        .collect(),
                    binds.to_vec(),
                )?;
                if plan.missing().iter().any(|m| m.release.is_none()) {
                    return Err(
                        watane_host::HostError::MissingProviders(plan.missing().to_vec()).into(),
                    );
                }
                self.cache_plan_parties(&plan)?;
                let missing = plan.missing().iter().map(|m| m.release.clone()).collect();
                Ok((plan, missing))
            },
            |plan, r| {
                let m = plan
                    .missing()
                    .iter()
                    .find(|m| m.release.as_ref() == Some(r))
                    .expect("missing release belongs to plan");
                let consumer = plan
                    .entries()
                    .iter()
                    .find(|e| e.identity == m.needed_by)
                    .expect("missing consumer belongs to plan");
                let key = consumer
                    .publisher
                    .as_ref()
                    .map(|p| p.to_string().parse::<watane_dist::PublicKey>())
                    .transpose()
                    .map_err(|e| CmdError::new("bad_input", e.to_string()))?;
                match self.node.fetch_for(r, key.as_ref()) {
                    Ok(fetched) => Ok((Some(fetched), vec![])),
                    Err(error) => crate::build_on::fetch_or_pick(
                        Err(error.into()),
                        pick.map(|broker| {
                            |cause: &CmdError| {
                                self.pick_provider(plan, m, broker, &cause.message, asked_by)
                            }
                        }),
                    ),
                }
            },
        )?;
        self.channels()?;
        Ok(result)
    }
    fn pick_provider(
        &self,
        plan: &watane_host::Plan,
        missing: &watane_host::MissingProvider,
        broker: &crate::consent::Broker,
        cause: &str,
        asked_by: Option<&crate::consent::Asker>,
    ) -> Result<(Option<watane_dist::Fetched>, Vec<watane_host::Bind>), CmdError> {
        use crate::build_on;
        let provider = &missing.provider;
        let needed = build_on::needed(plan.entries(), provider);
        let interface = needed
            .iter()
            .find_map(|g| match g {
                Grant::Call { interface, .. } | Grant::Subscribe { interface, .. } => {
                    Some(interface.to_string())
                }
                _ => None,
            })
            .and_then(|i| i.split_once('@').map(|(name, _)| name.to_owned()));
        let hits = self
            .node
            .search(&watane_dist::Query {
                interface: interface.map(|i| i.parse()).transpose()?,
                ..Default::default()
            })?
            .into_iter()
            .map(|h| h.hit)
            .collect::<Vec<_>>();
        self.channels()?;
        cache_channels(
            &self.dist,
            &self.parties,
            hits.iter().map(|h| h.info.channel),
        )?;
        let hits = build_on::candidates(&hits, provider, &needed);
        let label = words::provider(provider, &self.parties);
        if hits.is_empty() {
            return Err(build_on::no_candidates(&label, cause));
        }
        let installed = self.host.mods();
        let candidates = hits
            .iter()
            .map(|h| crate::consent::Candidate {
                choice: h.info.reference.to_string(),
                name: h.info.release.name.clone(),
                summary: h.info.release.summary.clone(),
                version: h.info.reference.version.to_string(),
                publisher: words::party(&h.info.channel.to_string(), &self.parties),
                origin: h
                    .info
                    .release
                    .origin
                    .as_ref()
                    .map(|r| words::release_label(r, &self.parties)),
                installed: installed.iter().any(|m| {
                    m.publisher.as_ref() == Some(&h.info.reference.publisher)
                        && m.slug == h.info.reference.slug
                }),
                checked_at: h.checked_at,
            })
            .collect();
        let consumer = plan
            .entries()
            .iter()
            .find(|e| e.identity == missing.needed_by)
            .expect("missing consumer belongs to plan");
        let choice = build_on::picked(broker.ask_blocking(crate::consent::Request::Pick {
            asked_by: asked_by.cloned(),
            consumer: consumer.name.clone(),
            provider: label,
            cause: cause.into(),
            candidates,
        }))?;
        let hit = hits
            .iter()
            .find(|h| h.info.reference.to_string() == choice)
            .ok_or_else(|| CmdError::new("refused", "unknown provider choice"))?;
        let to = format!(
            "{}/{}",
            hit.info.reference.publisher, hit.info.reference.slug
        );
        let fetched = if installed.iter().any(|m| {
            m.publisher.as_ref() == Some(&hit.info.reference.publisher)
                && m.slug == hit.info.reference.slug
        }) {
            None
        } else {
            Some(self.node.fetch(&hit.info.reference)?)
        };
        Ok((fetched, build_on::bindings(plan.entries(), provider, &to)))
    }
    pub fn follow_fetched(&self, fetched: &[watane_dist::Fetched]) {
        for item in fetched {
            if let Err(error) = self.node.installed(item) {
                self.failures
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .insert(item.info.channel.to_string(), CmdError::from(error).message);
            }
        }
        if let Ok(followed) = self.parties.channels.followed(&self.dist) {
            follow_installed_links(
                &self.node,
                fetched,
                &followed,
                &mut self
                    .failures
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it"),
            );
        }
        let _ = self.channels();
    }
}

fn fetch_for_install(
    node: &watane_node::Node,
    release: &watane_dist::ReleaseRef,
    link: Option<&watane_dist::Link>,
) -> Result<watane_dist::Fetched, watane_dist::DistError> {
    if let Some(link) = link {
        node.follow_derived(link)?;
    }
    node.fetch(release)
}

fn follow_installed_links(
    node: &watane_node::Node,
    fetched: &[watane_dist::Fetched],
    followed: &[watane_dist::ChannelView],
    failures: &mut std::collections::BTreeMap<String, String>,
) {
    let links = crate::build_on::follow_links(
        fetched.iter().filter_map(|f| f.link.clone()),
        followed.iter().map(|c| c.channel),
    );
    crate::build_on::follow_all(
        links,
        |link| {
            node.follow_derived(link)
                .map(|_| ())
                .map_err(CmdError::from)
        },
        failures,
    );
}

// Read only after the manager's read consent. Following waits for an install.
fn read_managed_release(
    node: &watane_node::Node,
    release: &watane_dist::ReleaseRef,
    link: &watane_dist::Link,
) -> Result<watane_dist::Fetched, CmdError> {
    let read = node.read_link(link)?;
    if &read.info.reference == release {
        Ok(read)
    } else {
        let mut fetched = node.fetch(release)?;
        fetched.link = read.link;
        Ok(fetched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac1_s75_status_tracks_whether_identity_signed() {
        struct Approve;
        impl watane_dist::Confirm for Approve {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                watane_dist::Decision::Approve
            }
        }
        let home = tempfile::tempdir().unwrap();
        let dist =
            watane_dist::Dist::open(home.path(), Arc::new(Approve), Default::default()).unwrap();
        assert_eq!(dist_status(&dist).unwrap().0["signed"], false);
        dist.set_name("Alice").unwrap();
        assert_eq!(dist_status(&dist).unwrap().0["signed"], true);
    }
    #[test]
    fn ac4_keyless_status_has_null_identity_without_panicking() {
        let home = tempfile::tempdir().unwrap();
        let dist = watane_dist::Dist::open_keyless(home.path(), Default::default()).unwrap();
        let (identity, site) = dist_status(&dist).unwrap();
        let status = serde_json::json!({"identity":identity,"site":site});
        assert!(status["identity"].is_null());
    }
    #[test]
    fn ac7_saved_toolsets_derive_function_and_interface_names_without_mods() {
        let entries = vec![
            "missing:books.list".to_string(),
            "disabled:books".to_string(),
        ];
        let row = toolset_row("reader", entries.clone());
        assert_eq!(
            row,
            serde_json::json!({"name":"reader", "entries":entries, "tools":["missing_books_list", "disabled_books"]})
        );
        assert_eq!(tool_row("missing", "books", "list").tool, row["tools"][0]);
    }
    #[test]
    fn ac1_tool_row_keeps_entry_and_mcp_name() {
        let row = tool_row("reading-list", "books", "list");
        assert_eq!(row.entry, "reading-list:books.list");
        assert_eq!(row.tool, "reading-list_books_list");
    }
    #[test]
    fn ac2_tools_list_row_serializes_entry_and_tool() {
        assert_eq!(
            serde_json::to_value(tool_row("reading-list", "books", "list")).unwrap(),
            serde_json::json!({"entry":"reading-list:books.list", "tool":"reading-list_books_list"})
        );
    }
    #[test]
    fn ac8_unknown_grant_keys_are_not_found() {
        for granted in [false, true] {
            let states = vec![watane_host::GrantState {
                grant: watane_host::watane_pkg::Grant::Storage,
                granted,
            }];
            assert_eq!(
                find_grant(&states, "unknown").unwrap_err().code,
                "not_found"
            );
            assert_eq!(
                find_grant(&states, "storage").unwrap(),
                watane_host::watane_pkg::Grant::Storage
            );
        }
    }
}

#[cfg(test)]
mod stage_c_tests {
    use super::*;
    #[test]
    fn ac4_refresh_times_are_memory_only_and_failures_keep_last_read() {
        let mut times = RefreshTimes::default();
        let alice = stage_d_tests::channel().channel;
        let mut bytes = *alice.as_bytes();
        bytes[31] ^= 0x80;
        let bob = watane_dist::PublicKey::from_bytes(&bytes).unwrap();
        let report = |channel, outcome| watane_dist::RefreshReport {
            channel,
            outcome,
            changes: vec![],
        };
        let failed = || watane_dist::Outcome::Failed {
            cause: watane_dist::DistError::Timeout {
                url: "https://offline.test/head".into(),
            },
        };
        assert_eq!(
            serde_json::to_value(&times).unwrap()["checkedAt"],
            serde_json::Value::Null
        );
        assert_eq!(times.read_at.get(&alice.to_string()), None);
        times.finish(
            &[
                report(alice, watane_dist::Outcome::Current { head: 1 }),
                report(bob, failed()),
            ],
            1700000000123,
        );
        assert_eq!(
            serde_json::to_value(&times).unwrap()["checkedAt"],
            1700000000123u64
        );
        assert_eq!(times.read_at.get(&alice.to_string()), Some(&1700000000123));
        assert_eq!(times.read_at.get(&bob.to_string()), None);
        times.finish(
            &[
                report(alice, failed()),
                report(
                    bob,
                    watane_dist::Outcome::Behind {
                        served: 1,
                        stored: 2,
                    },
                ),
            ],
            1700000000456,
        );
        assert_eq!(times.checked_at, Some(1700000000456));
        assert_eq!(times.read_at.get(&alice.to_string()), Some(&1700000000123));
        assert_eq!(times.read_at.get(&bob.to_string()), Some(&1700000000456));
        times.finish(
            &[report(alice, watane_dist::Outcome::Compromised { seq: 1 })],
            1700000000789,
        );
        assert_eq!(times.read_at.get(&alice.to_string()), Some(&1700000000123));
        assert_eq!(RefreshTimes::default().checked_at, None);
        let view = stage_d_tests::channel();
        let mut row = channel_row(&view, None);
        assert_eq!(
            serde_json::to_value(&row).unwrap()["readAt"],
            serde_json::Value::Null
        );
        row.read_at = times.read_at.get(&alice.to_string()).copied();
        assert_eq!(
            serde_json::to_value(row).unwrap()["readAt"],
            1700000000123u64
        );
    }
    #[test]
    fn ac2_channel_row_prioritizes_compromise_and_keeps_failure() {
        let home = tempfile::tempdir().unwrap();
        struct Refuse;
        impl watane_dist::Confirm for Refuse {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                watane_dist::Decision::Refuse
            }
        }
        let d = watane_dist::Dist::open(home.path(), Arc::new(Refuse), Default::default()).unwrap();
        let mut view = d.own().unwrap();
        assert_eq!(channel_row(&view, None).status, "ok");
        let mut failures = std::collections::BTreeMap::new();
        keep_outcome(
            &mut failures,
            view.channel,
            watane_dist::Outcome::Failed {
                cause: watane_dist::DistError::Timeout {
                    url: "http://site/head".into(),
                },
            },
        );
        let cause = failures.get(&view.channel.to_string()).unwrap().clone();
        let row = channel_row(&view, Some(&cause));
        assert_eq!(row.status, "failed");
        assert_eq!(row.cause.as_deref(), Some(cause.as_str()));
        view.status = watane_dist::Status::Compromised { seq: 2 };
        assert_eq!(channel_row(&view, Some(&cause)).status, "compromised");
        keep_outcome(
            &mut failures,
            view.channel,
            watane_dist::Outcome::Current { head: 3 },
        );
        assert!(failures.is_empty());
    }
    #[test]
    fn ac3_install_link_fetches_missing_and_rejects_stalled_or_32_rounds() {
        let reference: watane_dist::ReleaseRef = format!("{}/reading-list@0.1.0", "y".repeat(52))
            .parse()
            .unwrap();
        let mut fetched = vec![];
        let result = resolve_plan(
            vec![0],
            |sources, _binds| {
                Ok((
                    sources.len(),
                    if sources.len() == 1 {
                        vec![Some(reference.clone())]
                    } else {
                        vec![]
                    },
                ))
            },
            |_, r| {
                fetched.push(r.clone());
                Ok((Some(1), vec![]))
            },
        )
        .unwrap();
        assert_eq!(result.0, 2);
        assert_eq!(fetched, vec![reference.clone()]);
        let e = resolve_plan(
            vec![0],
            |_, _| Ok(((), vec![None])),
            |_, _| Ok((Some(1), vec![])),
        )
        .unwrap_err();
        assert_eq!(e.code, "missing_providers");
        let e = resolve_plan(
            vec![0],
            |_, _| Ok(((), vec![Some(reference.clone())])),
            |_, _| Ok((Some(1), vec![])),
        )
        .unwrap_err();
        assert_eq!(e.code, "missing_providers");
        let mut rounds = 0;
        let e = resolve_plan(
            vec![0],
            |_, _| {
                rounds += 1;
                let mut r = reference.clone();
                r.version = format!("0.0.{rounds}").parse().unwrap();
                Ok(((), vec![Some(r)]))
            },
            |_, _| Ok((Some(1), vec![])),
        )
        .unwrap_err();
        assert_eq!(e.code, "missing_providers");
        assert_eq!(rounds, 32);
        let bind = watane_host::Bind {
            consumer: "local/stats".into(),
            provider: watane_host::watane_pkg::Provider {
                slug: "reading-list".parse().unwrap(),
                publisher: Some(reference.publisher.clone()),
            },
            to: "local/reading-list".into(),
        };
        let result = resolve_plan(
            vec![0],
            |_, binds| {
                Ok((
                    (),
                    if binds.is_empty() {
                        vec![Some(reference.clone())]
                    } else {
                        vec![]
                    },
                ))
            },
            |_, _| Ok((None, vec![bind.clone()])),
        )
        .unwrap();
        assert_eq!(result.1, [0]);
        let mut newer = reference.clone();
        newer.version = "0.2.0".parse().unwrap();
        let mut picks = 0;
        resolve_plan(
            vec![0],
            |_, binds| {
                Ok((
                    (),
                    if binds.is_empty() {
                        vec![Some(reference.clone()), Some(newer.clone())]
                    } else {
                        vec![]
                    },
                ))
            },
            |_, _| {
                picks += 1;
                Ok((None, vec![bind.clone()]))
            },
        )
        .unwrap();
        assert_eq!(
            picks, 1,
            "one pick resolves every consumer, including another pinned version"
        );
    }
    #[test]
    fn ac4_dist_error_keeps_whole_cause_chain() {
        let e = watane_dist::DistError::Io {
            path: "/home/dist/site/head".into(),
            source: std::io::Error::other("disk broke"),
        };
        let converted = CmdError::from(e);
        assert_eq!(converted.code, "dist");
        assert_eq!(converted.message, "/home/dist/site/head: disk broke");
        assert_eq!(converted.message.matches("disk broke").count(), 1);
    }
}

#[cfg(test)]
mod stage_d_tests {
    use super::*;
    use watane_dist::*;
    pub(super) fn reference(version: &str) -> ReleaseRef {
        format!("{}/reading-list@{version}", channel().channel)
            .parse()
            .unwrap()
    }
    pub(super) fn channel() -> ChannelView {
        let mut bytes = [0x66; 32];
        bytes[0] = 0x58;
        let channel = PublicKey::from_bytes(&bytes).unwrap();
        ChannelView {
            channel,
            fingerprint: "yyyyyyyyyy".into(),
            name: None,
            base: None,
            head: None,
            releases: vec![],
            bad: vec![],
            status: Status::Ok,
            followed: true,
            index: false,
            kept: false,
            listed_by: vec![],
            listings: vec![],
            last_attempt: None,
            last_success: None,
        }
    }
    pub(super) fn release(version: &str, withdrawn: Option<&str>) -> ReleaseInfo {
        let reference = reference(version);
        ReleaseInfo {
            channel: channel().channel,
            entry: EntryId::from_bytes([0; 32]),
            seq: 1,
            release: Release {
                slug: reference.slug.clone(),
                version: reference.version.clone(),
                name: "Reading list".into(),
                summary: "Books".into(),
                package: PackageId::from_bytes([0; 32]),
                size: 1,
                source: false,
                offers: vec![],
                events: vec![],
                grants: vec![],
                origin: None,
            },
            reference,
            withdrawn: withdrawn.map(|reason| Withdrawal {
                seq: 2,
                reason: reason.into(),
            }),
        }
    }
    #[test]
    fn ac1_offer_highest_live_including_downgrade() {
        let mut view = channel();
        assert_eq!(release_offer(Some(&reference("0.1.0")), Some(&view)), None);
        view.releases = vec![
            release("0.3.0", Some("breaks lists")),
            release("0.2.0", None),
            release("0.1.0", None),
        ];
        for installed in ["0.1.0", "0.3.0"] {
            assert_eq!(
                release_offer(Some(&reference(installed)), Some(&view)),
                Some(reference("0.2.0"))
            );
        }
        assert_eq!(release_offer(Some(&reference("0.2.0")), Some(&view)), None);
        assert_eq!(release_offer(None, Some(&view)), None);
        assert_eq!(release_offer(Some(&reference("0.1.0")), None), None);
    }
}

#[cfg(test)]
mod stage_d_more_tests {
    use super::stage_d_tests::{channel, reference, release};
    use super::*;
    #[test]
    fn ac2_notice_priority_and_unfollowed_or_local() {
        let mut view = channel();
        let installed = reference("0.3.0");
        let failure = words::update_declined("0.4.0");
        assert_eq!(release_notice(Some(&installed), Some(&view), None), None);
        assert_eq!(
            release_notice(Some(&installed), Some(&view), Some(&failure)).as_deref(),
            Some("You declined the update to 0.4.0.")
        );
        view.releases.push(release("0.3.0", Some("breaks lists")));
        assert_eq!(
            release_notice(Some(&installed), Some(&view), Some(&failure)).as_deref(),
            Some("Withdrawn by the publisher: breaks lists")
        );
        view.status = watane_dist::Status::Compromised { seq: 1 };
        assert_eq!(
            release_notice(Some(&installed), Some(&view), Some(&failure)).as_deref(),
            Some("The publisher's channel is compromised")
        );
        assert_eq!(release_notice(None, Some(&view), Some(&failure)), None);
        assert_eq!(release_notice(Some(&installed), None, Some(&failure)), None);
    }
    #[test]
    fn ac5_update_failure_uses_code_and_keeps_other_causes() {
        assert_eq!(
            update_failure(
                "0.3.0",
                &CmdError::new("refused", "install was not approved")
            ),
            "You declined the update to 0.3.0."
        );
        assert_eq!(
            update_failure("0.3.0", &CmdError::new("io", "refused")),
            "Update to 0.3.0 failed: refused"
        );
    }
    #[test]
    fn ac6_offer_grants_compare_authority_including_withheld_grants() {
        let mut view = channel();
        let mut offered = release("0.3.0", None);
        let network = Grant::Http {
            host: "example.org".parse().unwrap(),
        };
        offered.release.grants = vec![Grant::Storage, network.clone()];
        view.releases.push(offered);
        let installed = vec![GrantState {
            grant: Grant::Storage,
            granted: false,
        }];
        let lines = offer_grants(
            Some(&reference("0.3.0")),
            Some(&view),
            &installed,
            &Parties::new(),
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Connect to example.org");
        assert!(!lines[0].granted);
        assert!(lines[0].new);
        assert_eq!(lines[0].key, network.to_string());
        assert!(offer_grants(None, Some(&view), &installed, &Parties::new()).is_empty());
        assert!(
            offer_grants(Some(&reference("0.3.0")), None, &installed, &Parties::new()).is_empty()
        );
    }
    #[test]
    fn ac6_identity_release_rows_include_withdrawal_reason() {
        let mut view = channel();
        view.releases = vec![
            release("0.2.0", None),
            release("0.3.0", Some("breaks lists")),
        ];
        assert_eq!(
            own_releases(&view),
            vec![
                serde_json::json!({"release":reference("0.2.0").to_string(),"version":"0.2.0","slug":"reading-list","withdrawn":null}),
                serde_json::json!({"release":reference("0.3.0").to_string(),"version":"0.3.0","slug":"reading-list","withdrawn":"breaks lists"}),
            ]
        );
    }
    #[test]
    fn ac7_mod_view_serializes_notice_and_offer_as_nullable_text() {
        for (notice, offer) in [
            (None, None),
            (
                Some("You declined the update to 0.3.0."),
                Some("release text"),
            ),
        ] {
            let row = ModView {
                main: false,
                proposes_main: false,
                summary: String::new(),
                links: vec![],
                local: "reading-list".into(),
                name: "Reading list".into(),
                slug: "reading-list".into(),
                version: "0.1.0".into(),
                release: None,
                publisher: None,
                enabled: true,
                page: None,
                package: "package".into(),
                derived_from: None,
                derived_from_label: None,
                error: None,
                grants: vec![],
                notice: notice.map(str::to_string),
                offer: offer.map(str::to_string),
                offer_grants: vec![],
                consents: vec![],
            };
            let value = serde_json::to_value(row).unwrap();
            assert_eq!(value["notice"], serde_json::json!(notice));
            assert_eq!(value["offer"], serde_json::json!(offer));
            assert_eq!(value["offerGrants"], serde_json::json!([]));
            assert_eq!(value["main"], false);
            assert_eq!(value["proposesMain"], false);
            assert_eq!(value["links"], serde_json::json!([]));
        }
    }
    #[test]
    fn ac3_refresh_updates_only_installed_new_releases_once() {
        let mut view = channel();
        view.releases = vec![release("0.2.0", None), release("0.3.0", None)];
        let changes = vec![
            watane_dist::Change::Release(view.releases[0].clone()),
            watane_dist::Change::Release(view.releases[1].clone()),
        ];
        assert_eq!(
            update_offer(Some(&reference("0.1.0")), &view, &changes),
            Some(reference("0.3.0"))
        );
        assert_eq!(
            update_offer(Some(&reference("0.3.0")), &view, &changes),
            None
        );
        assert_eq!(update_offer(None, &view, &changes), None);
        assert_eq!(update_offer(Some(&reference("0.1.0")), &view, &[]), None);
        let mut other = reference("0.1.0");
        other.slug = "other".parse().unwrap();
        assert_eq!(update_offer(Some(&other), &view, &changes), None);
        assert_eq!(
            update_offer(
                Some(&reference("0.2.0")),
                &view,
                &[watane_dist::Change::Release(release("0.1.0", None))]
            ),
            None
        );
        view.status = watane_dist::Status::Compromised { seq: 1 };
        assert_eq!(
            update_offer(Some(&reference("0.1.0")), &view, &changes),
            None
        );
    }
}

#[cfg(test)]
mod find_tests {
    use super::*;
    use watane_dist::*;
    #[test]
    fn ac2_own_index_rows_and_hits_mark_you() {
        let own = stage_d_tests::channel();
        let parties = Parties::new();
        parties.set_own(own.channel.to_string());
        let hit = Hit {
            info: stage_d_tests::release("0.1.0", None),
            publisher_name: Some("Carol".into()),
            link: None,
            status: Status::Ok,
            checked_at: None,
            unchecked_since: None,
            via: vec![Via::Index {
                index: own.channel,
                through: None,
                note: String::new(),
            }],
        };
        let view = crate::find::hit_view(&hit, &[], &parties);
        assert_eq!(view["publisher"]["you"], true);
        assert_eq!(view["via"][0]["index"]["you"], true);
        assert_eq!(
            index_rows(&own, vec![], &Default::default(), &parties)[0]["you"],
            true
        );
    }
    #[test]
    fn ac5_index_row_failure_names_the_index() {
        let mut view = stage_d_tests::channel();
        view.name = Some("Ivy".into());
        let row = index_row(
            &view,
            false,
            0,
            &[],
            vec![],
            Some(&"offline".into()),
            &Parties::new(),
        );
        assert_eq!(row["cause"], "Could not read Ivy: offline");
    }
    #[test]
    fn ac1_own_index_counts_only_live_list_listings() {
        let mut own = stage_d_tests::channel();
        own.releases = vec![stage_d_tests::release("0.1.0", None)];
        let listing = Listing {
            link: Link::new(own.channel, "https://example.org/", None).unwrap(),
            note: "Index".into(),
        };
        own.listings = [(true, None), (false, Some(4)), (false, None), (false, None)]
            .into_iter()
            .enumerate()
            .map(|(i, (include, unlisted))| ListingInfo {
                entry: EntryId::from_bytes([i as u8; 32]),
                seq: i as u64,
                include,
                listing: listing.clone(),
                unlisted,
            })
            .collect();
        let rows = index_rows(&own, vec![], &Default::default(), &Parties::new());
        assert_eq!(rows[0]["mods"], 2);
    }
    #[test]
    fn ac3_hit_view_status_parties_times_and_installed_identity() {
        let parties = Parties::new();
        let key = stage_d_tests::channel().channel;
        parties.insert(key.to_string(), "Ivy".into());
        let mut hit = Hit {
            info: stage_d_tests::release("0.1.0", None),
            publisher_name: Some("Alice".into()),
            link: None,
            status: Status::Ok,
            checked_at: Some(1700000000123),
            unchecked_since: Some(1700000000456),
            via: vec![Via::Index {
                index: key,
                through: Some(key),
                note: "Books".into(),
            }],
        };
        let host_home = tempfile::tempdir().unwrap();
        let host = Host::open(
            host_home.path(),
            watane_host::Mode::Preview,
            Default::default(),
        )
        .unwrap();
        let row = crate::find::hit_view(&hit, &host.mods(), &parties);
        assert_eq!(row["origin"], serde_json::Value::Null);
        hit.info.release.origin = Some(stage_d_tests::reference("0.1.0"));
        let row = crate::find::hit_view(&hit, &host.mods(), &parties);
        assert_eq!(row["origin"], stage_d_tests::reference("0.1.0").to_string());
        assert_eq!(row["verified"], true);
        assert_eq!(row["installed"], serde_json::Value::Null);
        assert_eq!(row["checkedAt"], 1700000000123u64);
        assert_eq!(row["uncheckedSince"], 1700000000456u64);
        assert_eq!(row["via"][0]["index"]["name"], "Ivy");
        assert_eq!(row["via"][0]["through"]["name"], "Ivy");
        hit.status = Status::Compromised { seq: 1 };
        let row = crate::find::hit_view(&hit, &[], &parties);
        assert_eq!(row["verified"], false);
        assert_eq!(row["status"], "compromised");
    }
    #[test]
    fn ac7_origin_label_names_stored_release_and_marks_publishers_claim() {
        let mut channel = stage_d_tests::channel();
        let origin = stage_d_tests::reference("0.1.0");
        channel.releases.push(stage_d_tests::release("0.1.0", None));
        let parties = Parties::new();
        parties.insert(channel.channel.to_string(), "Alice".into());
        assert_eq!(
            origin_label(Some(&origin), Some(&channel), &parties),
            Some(format!(
                "Made from Reading list 0.1.0 by Alice ({}) (the publisher says so)",
                channel.channel.fingerprint()
            ))
        );
        assert_eq!(
            origin_label(Some(&origin), None, &parties),
            Some(format!(
                "Made from {origin} by Alice ({}) (the publisher says so)",
                channel.channel.fingerprint()
            ))
        );
        assert_eq!(origin_label(None, None, &parties), None);
    }
    #[test]
    fn ac1_build_on_candidates_fit_all_interfaces_in_order_and_cap() {
        use watane_host::watane_pkg::{Grant, Provider};
        let provider = Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some("y".repeat(52).parse().unwrap()),
        };
        let call = Grant::Call {
            provider: provider.clone(),
            interface: "mods:ratings/api@0.1.0".parse().unwrap(),
        };
        let subscribe = Grant::Subscribe {
            provider: provider.clone(),
            interface: "mods:ratings/events@0.1.0".parse().unwrap(),
        };
        let mut good = Hit {
            info: stage_d_tests::release("0.1.0", None),
            publisher_name: None,
            link: None,
            status: Status::Ok,
            checked_at: None,
            unchecked_since: None,
            via: vec![],
        };
        good.info.reference.slug = "ratings".parse().unwrap();
        good.info.release.offers = vec!["mods:ratings/api@0.1.1".parse().unwrap()];
        good.info.release.events = vec!["mods:ratings/events@0.1.0".parse().unwrap()];
        let mut bad = vec![good.clone(); 5];
        bad[0].status = Status::Compromised { seq: 1 };
        bad[1].info.reference.slug = "other".parse().unwrap();
        bad[2].info.reference.publisher = provider.publisher.clone().unwrap();
        bad[3].info.release.offers.clear();
        bad[4].info.release.events.clear();
        let mut hits = bad;
        for n in 0..25 {
            let mut h = good.clone();
            h.info.release.name = n.to_string();
            hits.push(h);
        }
        let kept = crate::build_on::candidates(&hits, &provider, &[call, subscribe]);
        assert_eq!(
            kept.iter()
                .map(|h| h.info.release.name.as_str())
                .collect::<Vec<_>>(),
            (0..20).map(|n| n.to_string()).collect::<Vec<_>>()
        );
    }
    #[test]
    fn ac3_indexes_put_own_channel_first_and_name_included_parties() {
        let mut own = stage_d_tests::channel();
        own.releases = vec![
            stage_d_tests::release("0.1.0", None),
            stage_d_tests::release("0.2.0", None),
        ];
        let mut chosen = own.clone();
        chosen.name = Some("Ivy".into());
        let parties = Parties::new();
        parties.insert(own.channel.to_string(), "Included".into());
        parties.set_own(own.channel.to_string());
        let rows = index_rows(
            &own,
            vec![IndexView {
                view: chosen,
                included: vec![own.channel],
                mods: 7,
                skipped: vec![Skipped {
                    channel: own.channel,
                    cause: "own channel",
                }],
            }],
            &Default::default(),
            &parties,
        );
        assert_eq!(rows[0]["own"], true);
        assert_eq!(rows[0]["you"], true);
        assert_eq!(rows[0]["mods"], 0);
        assert_eq!(rows[1]["own"], false);
        assert_eq!(rows[1]["name"], "Ivy");
        assert_eq!(rows[1]["mods"], 7);
        assert_eq!(rows[1]["included"][0]["name"], "Included");
        assert_eq!(
            rows[1]["skipped"],
            serde_json::json!([{"party":{"name":"Included","fingerprint":own.channel.fingerprint(),"you":true},"cause":"own channel"}])
        );
    }
    #[test]
    fn ac3_index_row_live_listings_and_channel_status() {
        let mut own = stage_d_tests::channel();
        let key = own.channel;
        let listing = Listing {
            link: Link::new(
                key,
                "https://example.org/",
                Some(&"reading-list".parse().unwrap()),
            )
            .unwrap(),
            note: "Books".into(),
        };
        own.listings = vec![
            ListingInfo {
                entry: EntryId::from_bytes([0; 32]),
                seq: 1,
                include: false,
                listing: listing.clone(),
                unlisted: None,
            },
            ListingInfo {
                entry: EntryId::from_bytes([1; 32]),
                seq: 2,
                include: true,
                listing: Listing {
                    link: Link::new(key, "https://example.org/", None).unwrap(),
                    note: "Index".into(),
                },
                unlisted: None,
            },
            ListingInfo {
                entry: EntryId::from_bytes([2; 32]),
                seq: 3,
                include: false,
                listing,
                unlisted: Some(4),
            },
        ];
        let row = index_row(
            &own,
            true,
            3,
            &[],
            vec![],
            Some(&"offline".into()),
            &Parties::new(),
        );
        assert_eq!(row["own"], true);
        assert_eq!(row["mods"], 3);
        assert_eq!(row["status"], "failed");
        assert_eq!(
            row["cause"],
            format!("Could not read {}: offline", key.fingerprint())
        );
        assert_eq!(row["skipped"], serde_json::json!([]));
        assert_eq!(
            row["listings"],
            serde_json::json!([
                {"entry":EntryId::from_bytes([0;32]).to_string(),"kind":"list","target":format!("reading-list by {}",key.fingerprint()),"note":"Books"},
                {"entry":EntryId::from_bytes([1;32]).to_string(),"kind":"include","target":key.fingerprint(),"note":"Index"}
            ])
        );
        own.status = Status::Compromised { seq: 1 };
        assert_eq!(
            index_row(&own, false, 0, &[], vec![], None, &Parties::new())["status"],
            "compromised"
        );
        assert_eq!(
            index_row(&own, false, 0, &[], vec![], None, &Parties::new())["listings"],
            serde_json::json!([])
        );
    }
}

fn skipped_rows(skipped: &[watane_dist::Skipped], parties: &Parties) -> Vec<serde_json::Value> {
    skipped
        .iter()
        .map(|s| {
            serde_json::json!({
                "party": words::party(&s.channel.to_string(), parties),
                "cause": s.cause,
            })
        })
        .collect()
}
fn index_row(
    view: &watane_dist::ChannelView,
    own: bool,
    mods: usize,
    included: &[watane_dist::PublicKey],
    skipped: Vec<serde_json::Value>,
    failure: Option<&String>,
    parties: &Parties,
) -> serde_json::Value {
    let mut row =
        serde_json::to_value(channel_row(view, failure)).expect("a channel row serializes");
    let object = row
        .as_object_mut()
        .expect("a channel row serializes as an object");
    object.insert("you".into(), serde_json::json!(own));
    if let Some(cause) = failure {
        object.insert(
            "cause".into(),
            serde_json::json!(words::index_read(
                &view
                    .name
                    .clone()
                    .unwrap_or_else(|| view.channel.fingerprint()),
                cause
            )),
        );
    }
    object.insert("own".into(), serde_json::json!(own));
    object.insert("mods".into(), serde_json::json!(mods));
    object.insert(
        "included".into(),
        serde_json::json!(
            included
                .iter()
                .map(|key| words::party(&key.to_string(), parties))
                .collect::<Vec<_>>()
        ),
    );
    object.insert("skipped".into(), serde_json::json!(skipped));
    object.insert("listings".into(), serde_json::json!(view.listings.iter().filter(|l| own && l.unlisted.is_none()).map(|l| {
        let link = &l.listing.link;
        serde_json::json!({"entry":l.entry.to_string(), "kind":if l.include {"include"} else {"list"}, "target":link.slug().map(|slug|format!("{slug} by {}", words::party_label(&words::party(&link.channel().to_string(), parties)))).unwrap_or_else(||words::party_label(&words::party(&link.channel().to_string(), parties))), "note":l.listing.note})
    }).collect::<Vec<_>>()));
    row
}
// Dist does not attach the refused release to Refused. Decode it through pkg,
// only on denial, to use the same slug and version validation as publication.
pub(crate) fn publish_denied(bytes: Vec<u8>) -> Result<String, CmdError> {
    use std::os::unix::fs::DirBuilderExt;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|e| CmdError::new("io", e.to_string()))?;
    let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let path = std::env::temp_dir().join(format!("watane-denied-{suffix}"));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let cleanup = Cleanup(path);
    let package = watane_dist::Package::from_bytes(bytes).map_err(watane_dist::DistError::from)?;
    let m = package
        .unpack(&cleanup.0.join("mod"))
        .map_err(watane_dist::DistError::from)?;
    Ok(words::sign_denied(
        "publish",
        &format!("{} {}", m.slug(), m.version()),
    ))
}

pub(crate) fn followed_keys(
    dist: &watane_dist::Dist,
    parties: &Parties,
) -> Result<std::collections::BTreeSet<watane_dist::PublicKey>, CmdError> {
    Ok(parties
        .channels
        .followed(dist)?
        .into_iter()
        .map(|v| v.channel)
        .collect())
}
pub(crate) fn install_result(
    dist: &watane_dist::Dist,
    parties: &Parties,
    before: std::collections::BTreeSet<watane_dist::PublicKey>,
    installed: Vec<watane_host::InstalledMod>,
) -> Result<serde_json::Value, CmdError> {
    let views = parties.channels.followed(dist)?;
    parties.remember(&views);
    let followed: Vec<String> = views
        .iter()
        .filter(|v| !before.contains(&v.channel))
        .map(|v| words::party_label(&words::party(&v.channel.to_string(), parties)))
        .collect();
    Ok(
        serde_json::json!({"installed": installed.into_iter().map(|m| m.local).collect::<Vec<_>>(), "followed": followed}),
    )
}

// Views by channel key while heads and the follow/site stamps hold
// (TECHNICAL 9.2). `listed_by` is not keyed: do not read it from here.
#[derive(Default)]
pub(crate) struct ChannelMemo {
    entries:
        Mutex<std::collections::BTreeMap<(std::path::PathBuf, watane_dist::PublicKey), MemoEntry>>,
    followed: Mutex<std::collections::BTreeMap<std::path::PathBuf, FollowedKeys>>,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}
type Stamp = Option<(std::time::SystemTime, u64, u64)>;
type FollowedKeys = (Stamp, Vec<watane_dist::PublicKey>);
struct MemoEntry {
    head: Option<watane_dist::HeadState>,
    follows: Stamp,
    site: Stamp,
    view: watane_dist::ChannelView,
}
// Dist rewrites both files by rename, so the inode tells apart two writes of
// equal length within one mtime tick.
fn file_stamp(path: &std::path::Path) -> std::io::Result<Stamp> {
    use std::os::unix::fs::MetadataExt;
    match std::fs::metadata(path) {
        Ok(meta) => Ok(Some((meta.modified()?, meta.len(), meta.ino()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
impl ChannelMemo {
    pub(crate) fn channel(
        &self,
        dist: &watane_dist::Dist,
        key: &watane_dist::PublicKey,
    ) -> Result<watane_dist::ChannelView, watane_dist::DistError> {
        let dir = dist
            .site_dir()
            .parent()
            .expect("dist/site has a parent")
            .to_path_buf();
        let stamp = |name| {
            let path = dir.join(name);
            file_stamp(&path).map_err(|source| watane_dist::DistError::Io { path, source })
        };
        let follows = stamp("follows.json")?;
        let site = stamp("site.json")?;
        let head = dist.heads(Some(&[*key]))?[0].1;
        let mut entries = self.entries.lock().expect("channel memo lock poisoned");
        let entry_key = (dir, *key);
        if let Some(entry) = entries.get(&entry_key)
            && entry.head == head
            && entry.follows == follows
            && entry.site == site
        {
            return Ok(entry.view.clone());
        }
        let view = dist.channel(key)?;
        #[cfg(test)]
        self.builds
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        entries.insert(
            entry_key,
            MemoEntry {
                head,
                follows,
                site,
                view: view.clone(),
            },
        );
        Ok(view)
    }
    fn followed(
        &self,
        dist: &watane_dist::Dist,
    ) -> Result<Vec<watane_dist::ChannelView>, watane_dist::DistError> {
        let dir = dist
            .site_dir()
            .parent()
            .expect("dist/site has a parent")
            .to_path_buf();
        let path = dir.join("follows.json");
        let stamp =
            file_stamp(&path).map_err(|source| watane_dist::DistError::Io { path, source })?;
        let mut followed = self.followed.lock().expect("followed memo lock poisoned");
        if let Some((previous, keys)) = followed.get(&dir)
            && *previous == stamp
        {
            return keys.iter().map(|key| self.channel(dist, key)).collect();
        }
        // Dist owns which channels are followed. Refresh this key list only
        // when its records change; otherwise even followed() rebuilds views.
        let heads: std::collections::BTreeMap<_, _> = dist.heads(None)?.into_iter().collect();
        let site_path = dir.join("site.json");
        let site = file_stamp(&site_path).map_err(|source| watane_dist::DistError::Io {
            path: site_path,
            source,
        })?;
        let views = dist.followed()?;
        let keys: Vec<_> = views.iter().map(|view| view.channel).collect();
        let mut entries = self.entries.lock().expect("channel memo lock poisoned");
        for view in &views {
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            entries.insert(
                (dir.clone(), view.channel),
                MemoEntry {
                    head: heads.get(&view.channel).copied().flatten(),
                    follows: stamp,
                    site,
                    view: view.clone(),
                },
            );
        }
        followed.insert(dir, (stamp, keys));
        Ok(views)
    }
    #[cfg(test)]
    fn builds(&self) -> usize {
        self.builds.load(std::sync::atomic::Ordering::Relaxed)
    }
}

pub(crate) fn cache_channels(
    dist: &watane_dist::Dist,
    parties: &Parties,
    keys: impl IntoIterator<Item = watane_dist::PublicKey>,
) -> Result<(), watane_dist::DistError> {
    if let Some(key) = dist.identity() {
        parties.set_own(key.to_string());
    }
    let mut pending: Vec<_> = keys.into_iter().collect();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(key) = pending.pop() {
        if !seen.insert(key) {
            continue;
        }
        let view = parties.channels.channel(dist, &key)?;
        pending.extend(
            view.listings
                .iter()
                .filter(|l| l.unlisted.is_none())
                .map(|l| l.listing.link.channel()),
        );
        for release in &view.releases {
            if let Some(origin) = &release.release.origin
                && let Ok(key) = origin.publisher.to_string().parse()
            {
                pending.push(key);
            }
            for grant in &release.release.grants {
                if let Grant::Call { provider, .. } | Grant::Subscribe { provider, .. } = grant
                    && let Some(key) = provider
                        .publisher
                        .as_ref()
                        .and_then(|p| p.to_string().parse().ok())
                {
                    pending.push(key);
                }
            }
        }
        parties.remember(&[view]);
    }
    Ok(())
}
fn party_views(dist: &watane_dist::Dist) -> Result<Vec<watane_dist::ChannelView>, CmdError> {
    let mut views = dist.followed()?;
    if dist.identity().is_some() {
        views.push(dist.own()?);
    }
    views.extend(dist.indexes()?.into_iter().map(|i| i.view));
    // Read cached profiles of listed publishers and included indexes, including
    // the listings of those includes. This performs no network requests.
    let mut seen: std::collections::BTreeSet<_> = views.iter().map(|v| v.channel).collect();
    let mut at = 0;
    while at < views.len() {
        let keys: Vec<_> = views[at]
            .listings
            .iter()
            .filter(|l| l.unlisted.is_none())
            .map(|l| l.listing.link.channel())
            .collect();
        for key in keys {
            if seen.insert(key) {
                views.push(dist.channel(&key)?);
            }
        }
        at += 1;
    }
    Ok(views)
}
pub(crate) fn choose_index_with(
    dist: &watane_dist::Dist,
    text: &str,
    choose: impl FnOnce(
        &watane_dist::Link,
    ) -> Result<Vec<watane_dist::RefreshReport>, watane_dist::DistError>,
) -> Result<Vec<watane_dist::RefreshReport>, CmdError> {
    let link = crate::commands::parse_link(text)?;
    if Some(link.channel()) == dist.identity() {
        return Err(CmdError::new("bad_input", words::OWN_INDEX));
    }
    choose(&link).map_err(|e| {
        let name = dist
            .channel(&link.channel())
            .ok()
            .and_then(|v| v.name)
            .unwrap_or_else(|| link.channel().fingerprint());
        CmdError::new("dist", words::index_read(&name, &crate::cause_chain(&e)))
    })
}
fn index_rows(
    own: &watane_dist::ChannelView,
    indexes: Vec<watane_dist::IndexView>,
    failures: &std::collections::BTreeMap<String, String>,
    parties: &Parties,
) -> Vec<serde_json::Value> {
    let mut rows = vec![index_row(
        own,
        true,
        own.listings
            .iter()
            .filter(|listing| !listing.include && listing.unlisted.is_none())
            .count(),
        &[],
        vec![],
        None,
        parties,
    )];
    for index in indexes {
        rows.push(index_row(
            &index.view,
            false,
            index.mods,
            &index.included,
            skipped_rows(&index.skipped, parties),
            failures.get(&index.view.channel.to_string()),
            parties,
        ));
    }
    rows
}
fn held_indexes(
    dist: &watane_dist::Dist,
    parties: &Parties,
    failures: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<serde_json::Value>, CmdError> {
    parties.remember(&party_views(dist)?);
    if let Some(key) = dist.identity() {
        parties.set_own(key.to_string());
    }
    if dist.identity().is_none() {
        return Ok(dist
            .indexes()?
            .into_iter()
            .map(|index| {
                index_row(
                    &index.view,
                    false,
                    index.mods,
                    &index.included,
                    skipped_rows(&index.skipped, parties),
                    failures.get(&index.view.channel.to_string()),
                    parties,
                )
            })
            .collect());
    }
    let own = dist.own()?;
    Ok(index_rows(&own, dist.indexes()?, failures, parties))
}
impl Core {
    pub fn indexes(&self) -> Result<Vec<serde_json::Value>, CmdError> {
        held_indexes(
            &self.dist,
            &self.parties,
            &self
                .failures
                .lock()
                .expect("lock poisoned: another thread panicked while holding it"),
        )
    }
    pub fn choose_index(
        &self,
        text: &str,
        derived: bool,
    ) -> Result<Vec<serde_json::Value>, CmdError> {
        for report in choose_index_link(&self.node, text, derived)? {
            keep_outcome(
                &mut self
                    .failures
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it"),
                report.channel,
                report.outcome,
            );
        }
        self.indexes()
    }
    pub fn search(&self, query: &watane_dist::Query) -> Result<Vec<serde_json::Value>, CmdError> {
        self.parties.remember(&party_views(&self.dist)?);
        if let Some(key) = self.dist.identity() {
            self.parties.set_own(key.to_string());
        }
        let installed = self.host.mods();
        let hits = self.node.search(query)?;
        cache_channels(
            &self.dist,
            &self.parties,
            hits.iter().map(|h| h.hit.info.channel),
        )?;
        Ok(hits
            .iter()
            .map(|h| crate::find::node_hit_view(h, &installed, &self.parties))
            .collect())
    }
}

fn choose_index_link(
    node: &watane_node::Node,
    text: &str,
    derived: bool,
) -> Result<Vec<watane_dist::RefreshReport>, CmdError> {
    choose_index_with(node.dist(), text, |link| {
        if derived {
            node.choose_index_derived(link)
        } else {
            node.choose_index(link)
        }
    })
}

#[cfg(test)]
mod find_integration_tests {
    use super::*;
    use watane_dist::*;
    struct Approve;
    impl Confirm for Approve {
        fn confirm(&self, _: &SignRequest) -> Decision {
            Decision::Approve
        }
    }
    struct Site {
        base: String,
        stop: Option<tokio::sync::oneshot::Sender<()>>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Site {
        fn serve(dir: std::path::PathBuf) -> Self {
            let (stop, rx) = tokio::sync::oneshot::channel();
            let (tx, ready) = std::sync::mpsc::channel();
            let thread = std::thread::spawn(move || {
                tokio::runtime::Runtime::new().unwrap().block_on(async {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    tx.send(format!("http://{}/", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(
                        listener,
                        axum::Router::new()
                            .fallback_service(tower_http::services::ServeDir::new(dir)),
                    )
                    .with_graceful_shutdown(async {
                        let _ = rx.await;
                    })
                    .await
                    .unwrap();
                });
            });
            Self {
                base: ready
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap(),
                stop: Some(stop),
                thread: Some(thread),
            }
        }
    }
    impl Drop for Site {
        fn drop(&mut self) {
            let _ = self.stop.take().unwrap().send(());
            self.thread.take().unwrap().join().unwrap();
        }
    }
    fn open(home: &tempfile::TempDir) -> Dist {
        Dist::open(
            home.path(),
            Arc::new(Approve),
            Options {
                listed_local: true,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn package() -> Vec<u8> {
        watane_host::watane_pkg::pack(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list"),
            false,
        )
        .unwrap()
        .into_bytes()
    }

    #[test]
    fn ac1_install_reports_only_newly_followed_labels() {
        let publisher_home = tempfile::tempdir().unwrap();
        let publisher = open(&publisher_home);
        publisher.set_name("Alice").unwrap();
        let site = Site::serve(publisher.site_dir());
        publisher.set_site_url(&site.base).unwrap();
        let published = publisher.publish(package()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let reader = open(&home);
        let parties = Parties::new();
        for expected in [
            vec![format!("Alice ({})", publisher.fingerprint().unwrap())],
            vec![],
        ] {
            let before = followed_keys(&reader, &parties).unwrap();
            let fetched = reader.open_link(published.link.as_ref().unwrap()).unwrap();
            let plan = host
                .plan(vec![watane_host::Source::Release {
                    package: fetched.package,
                    entry: fetched.entry,
                }])
                .unwrap();
            let installed = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(host.install(plan, watane_host::Requester::App))
                .unwrap();
            reader.follow(published.link.as_ref().unwrap()).unwrap();
            let result = install_result(&reader, &parties, before, installed).unwrap();
            assert_eq!(result["installed"], serde_json::json!(["reading-list"]));
            assert_eq!(result["followed"], serde_json::json!(expected));
        }
    }

    #[test]
    fn ac3_mods_channel_snapshot_reuses_followed_views() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        let alice_site = Site::serve(alice.site_dir());
        alice.set_site_url(&alice_site.base).unwrap();
        alice.set_name("Alice").unwrap();
        let ivy_home = tempfile::tempdir().unwrap();
        let ivy = open(&ivy_home);
        let ivy_site = Site::serve(ivy.site_dir());
        ivy.set_site_url(&ivy_site.base).unwrap();
        ivy.set_name("Ivy").unwrap();
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        dist.follow(&alice.own_link(None).unwrap()).unwrap();
        dist.follow(&ivy.own_link(None).unwrap()).unwrap();
        let parties = Parties::new();
        let first = Core::installed_channels(&dist, &parties, &[]).unwrap();
        let before = parties.channels.builds();
        let second = Core::installed_channels(&dist, &parties, &[]).unwrap();
        assert_eq!(parties.channels.builds(), before);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
        let entry = alice.set_name("New Alice").unwrap();
        assert!(matches!(
            dist.ingest(&alice.identity().unwrap(), vec![entry], Reason::Asked),
            Ingested::Stored(_)
        ));
        let changed = Core::installed_channels(&dist, &parties, &[]).unwrap();
        assert_eq!(parties.channels.builds(), before + 1);
        assert_eq!(
            format!("{changed:?}"),
            format!("{:?}", dist.followed().unwrap())
        );
        dist.unfollow(&ivy.identity().unwrap()).unwrap();
        let remaining = Core::installed_channels(&dist, &parties, &[]).unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(parties.channels.builds() > before + 1);
    }

    #[test]
    fn ac3_channel_memo_reuses_and_invalidates_views() {
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        let other_home = tempfile::tempdir().unwrap();
        let other = open(&other_home);
        let key = dist.identity().unwrap();
        let other_key = other.identity().unwrap();
        let parties = Parties::new();
        cache_channels(&dist, &parties, [key, other_key]).unwrap();
        assert_eq!(parties.channels.builds(), 2);
        cache_channels(&dist, &parties, [key, other_key]).unwrap();
        assert_eq!(parties.channels.builds(), 2);
        dist.set_name("Alice").unwrap();
        cache_channels(&dist, &parties, [key, other_key]).unwrap();
        assert_eq!(parties.channels.builds(), 3);
        let site = Site::serve(other.site_dir());
        other.set_site_url(&site.base).unwrap();
        other.set_name("Ivy").unwrap();
        dist.follow(&other.own_link(None).unwrap()).unwrap();
        assert!(
            parties
                .channels
                .channel(&dist, &other_key)
                .unwrap()
                .followed
        );
        let before = parties.channels.builds();
        dist.unfollow(&other_key).unwrap();
        assert!(
            !parties
                .channels
                .channel(&dist, &other_key)
                .unwrap()
                .followed
        );
        assert_eq!(parties.channels.builds(), before + 1);
    }

    #[test]
    fn ac3_s82_read_retains_link_and_exact_release_without_following() {
        let source_home = tempfile::tempdir().unwrap();
        let source = open(&source_home);
        let site = Site::serve(source.site_dir());
        source.set_site_url(&site.base).unwrap();
        let published = source.publish(package()).unwrap();
        let reader_home = tempfile::tempdir().unwrap();
        let reader = watane_node::Node::open_keyless(
            reader_home.path(),
            watane_node::Options {
                dist: Options {
                    listed_local: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let fetched = read_managed_release(
            &reader,
            &published.info.reference,
            published.link.as_ref().unwrap(),
        )
        .unwrap();
        assert_eq!(fetched.info.reference, published.info.reference);
        assert_eq!(fetched.link, published.link);
        assert!(reader.dist().followed().unwrap().is_empty());
        // Planning alone leaves the read channel unfollowed.
        let host_home = tempfile::tempdir().unwrap();
        let host = watane_host::Host::open(
            host_home.path(),
            watane_host::Mode::Preview,
            Default::default(),
        )
        .unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Release {
                package: fetched.package,
                entry: fetched.entry,
            }])
            .unwrap();
        assert!(!plan.entries().is_empty());
        assert!(host.mods().is_empty());
        assert!(reader.dist().followed().unwrap().is_empty());
    }

    #[test]
    fn ac10_keyless_choose_and_indexes_succeed_without_signing() {
        let source_home = tempfile::tempdir().unwrap();
        let source = open(&source_home);
        source.set_name("Ivy").unwrap();
        source.publish(package()).unwrap();
        let site = Site::serve(source.site_dir());
        source.set_site_url(&site.base).unwrap();
        let home = tempfile::tempdir().unwrap();
        let reader = watane_node::Node::open_keyless(
            home.path(),
            watane_node::Options {
                dist: Options {
                    listed_local: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let dist = reader.dist();
        let parties = Parties::new();
        let failures = Default::default();
        assert!(held_indexes(dist, &parties, &failures).unwrap().is_empty());
        let link = source.own_link(None).unwrap().to_string();
        choose_index_with(dist, &link, |link| reader.choose_index(link)).unwrap();
        let rows = held_indexes(dist, &parties, &failures).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["channel"], source.identity().unwrap().to_string());
        assert_eq!(rows[0]["name"], "Ivy");
        assert_eq!(rows[0]["own"], false);
        assert!(!home.path().join("log/identity.key").exists());
        assert!(dist.identity().is_none());
    }
    #[test]
    fn ac3_follow_wrong_channel_uses_names_from_dist_log() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        alice.set_name("Alice").unwrap();
        let alice_site = Site::serve(alice.site_dir());
        alice.set_site_url(&alice_site.base).unwrap();
        let alice_release = alice.publish(package()).unwrap();
        let ivy_home = tempfile::tempdir().unwrap();
        let ivy = open(&ivy_home);
        ivy.set_name("Ivy").unwrap();
        let ivy_site = Site::serve(ivy.site_dir());
        ivy.set_site_url(&ivy_site.base).unwrap();
        let ivy_release = ivy.publish(package()).unwrap();
        let reader_home = tempfile::tempdir().unwrap();
        let reader = open(&reader_home);
        reader
            .read_link(alice_release.link.as_ref().unwrap())
            .unwrap();
        reader
            .read_link(ivy_release.link.as_ref().unwrap())
            .unwrap();
        let wrong = Link::new(alice.identity().unwrap(), &ivy_site.base, None).unwrap();
        let error = map_follow(&reader, &Parties::new(), reader.follow(&wrong)).unwrap_err();
        assert_eq!(error.code, "dist");
        assert!(
            error.message.contains(&format!(
                "serves Ivy ({}), not Alice ({})",
                ivy.fingerprint().unwrap(),
                alice.fingerprint().unwrap()
            )),
            "{}",
            error.message
        );
    }
    #[test]
    fn ac5_failed_index_choose_uses_profile_or_link_fingerprint() {
        let publisher_home = tempfile::tempdir().unwrap();
        let publisher = open(&publisher_home);
        publisher.set_name("Ivy").unwrap();
        let site = Site::serve(publisher.site_dir());
        publisher.set_site_url(&site.base).unwrap();
        let published = publisher.publish(package()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let reader = open(&home);
        reader.read_link(published.link.as_ref().unwrap()).unwrap();
        let unknown_home = tempfile::tempdir().unwrap();
        let unknown = open(&unknown_home);
        let link = publisher.own_link(None).unwrap().to_string();
        drop(site);
        let error =
            choose_index_with(&reader, &link, |link| reader.choose_index(link)).unwrap_err();
        assert!(
            error.message.starts_with("Could not read Ivy: "),
            "{}",
            error.message
        );
        let error =
            choose_index_with(&unknown, &link, |link| unknown.choose_index(link)).unwrap_err();
        assert!(
            error.message.starts_with(&format!(
                "Could not read {}: ",
                publisher.fingerprint().unwrap()
            )),
            "{}",
            error.message
        );
    }
    #[test]
    fn ac1_fetched_unfollowed_channel_and_origin_fill_parties() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        alice.set_name("Alice").unwrap();
        let site = Site::serve(alice.site_dir());
        alice.set_site_url(&site.base).unwrap();
        let published = alice.publish(package()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let reader = open(&home);
        reader.read_link(published.link.as_ref().unwrap()).unwrap();
        assert!(reader.followed().unwrap().is_empty());
        let parties = Parties::new();
        cache_channels(&reader, &parties, [alice.identity().unwrap()]).unwrap();
        assert_eq!(
            words::party(&alice.identity().unwrap().to_string(), &parties)
                .name
                .as_deref(),
            Some("Alice")
        );
        parties.remember(&party_views(&reader).unwrap());
        assert_eq!(
            words::party(&alice.identity().unwrap().to_string(), &parties)
                .name
                .as_deref(),
            Some("Alice")
        );
    }
    #[test]
    fn keyless_reader_caches_channel_names_without_an_own_key() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        alice.set_name("Alice").unwrap();
        let site = Site::serve(alice.site_dir());
        alice.set_site_url(&site.base).unwrap();
        let published = alice.publish(package()).unwrap();
        let home = tempfile::tempdir().unwrap();
        let options = Options {
            listed_local: true,
            ..Default::default()
        };
        let reader = Dist::open_keyless(home.path(), options).unwrap();
        reader.read_link(published.link.as_ref().unwrap()).unwrap();
        let parties = Parties::new();
        cache_channels(&reader, &parties, [alice.identity().unwrap()]).unwrap();
        let party = words::party(&alice.identity().unwrap().to_string(), &parties);
        assert_eq!(party.name.as_deref(), Some("Alice"));
        assert!(!party.you);
    }
    fn assert_no_given(home: &std::path::Path, channel: PublicKey) {
        let file: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.join("dist/follows.json")).unwrap())
                .unwrap();
        let record = &file["channels"][channel.to_string()];
        assert_eq!(record["followed"], true, "{record}");
        assert!(record.get("explicit_given").is_none(), "{record}");
    }

    #[test]
    fn s107_using_a_channel_row_as_an_index_keeps_its_base_derived() {
        let source_home = tempfile::tempdir().unwrap();
        let source = open(&source_home);
        source.set_name("Alice").unwrap();
        let site = Site::serve(source.site_dir());
        source.set_site_url(&site.base).unwrap();
        let link = source.own_link(None).unwrap();
        let reader_home = tempfile::tempdir().unwrap();
        let reader = watane_node::Node::open_keyless(
            reader_home.path(),
            watane_node::Options {
                dist: Options {
                    listed_local: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        reader.follow_derived(&link).unwrap();
        let view = reader.dist().followed().unwrap().remove(0);
        let row = channel_row(&view, None);
        let link = format!("watane:{}@{}", row.channel, row.base.unwrap());
        choose_index_link(&reader, &link, true).unwrap();
        assert!(reader.dist().channel(&view.channel).unwrap().index);
        assert_no_given(reader_home.path(), view.channel);
    }

    #[test]
    fn ac5_install_hit_follows_before_fetch_and_keeps_dist_cause() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        let site = Site::serve(alice.site_dir());
        alice.set_site_url(&site.base).unwrap();
        let published = alice.publish(package()).unwrap();
        let carol_home = tempfile::tempdir().unwrap();
        let host = Host::open(
            carol_home.path(),
            watane_host::Mode::Preview,
            Default::default(),
        )
        .unwrap();
        let node = watane_node::Node::open(
            carol_home.path(),
            Arc::new(Approve),
            watane_node::Options {
                dist: Options {
                    listed_local: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let carol = node.dist();
        let reference = &published.info.reference;
        let expected = node.fetch(reference).unwrap_err().to_string();
        let error = CmdError::from(node.fetch(reference).unwrap_err());
        assert_eq!(error.message, expected);
        assert_eq!(error.code, "dist");
        assert!(carol.followed().unwrap().is_empty());
        let fetched = fetch_for_install(&node, reference, published.link.as_ref()).unwrap();
        assert_no_given(carol_home.path(), fetched.info.channel);
        let post_install_home = tempfile::tempdir().unwrap();
        let post_install = watane_node::Node::open_keyless(
            post_install_home.path(),
            watane_node::Options {
                dist: Options {
                    listed_local: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let mut failures = Default::default();
        follow_installed_links(
            &post_install,
            std::slice::from_ref(&fetched),
            &[],
            &mut failures,
        );
        assert!(failures.is_empty());
        assert_no_given(post_install_home.path(), fetched.info.channel);
        assert_eq!(
            carol.followed().unwrap()[0].channel,
            alice.identity().unwrap()
        );
        let plan = host
            .plan(vec![watane_host::Source::Release {
                package: fetched.package,
                entry: fetched.entry,
            }])
            .unwrap();
        let installed = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(host.install(plan, watane_host::Requester::App))
            .unwrap();
        let hit = carol.search(&Query::default()).unwrap().remove(0);
        assert_eq!(
            crate::find::hit_view(&hit, &installed, &Parties::new())["installed"],
            "reading-list"
        );
        let mut different = hit.clone();
        different.info.reference.slug = "other-mod".parse().unwrap();
        assert!(
            crate::find::hit_view(&different, &installed, &Parties::new())["installed"].is_null()
        );
        different = hit.clone();
        different.info.reference.publisher = "y".repeat(52).parse().unwrap();
        assert!(
            crate::find::hit_view(&different, &installed, &Parties::new())["installed"].is_null()
        );
    }
    #[test]
    fn ac6_parties_include_chosen_indexes_and_listed_publishers() {
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        let alice_site = Site::serve(alice.site_dir());
        alice.set_site_url(&alice_site.base).unwrap();
        alice.set_name("Alice").unwrap();
        alice.publish(package()).unwrap();
        let ivy_home = tempfile::tempdir().unwrap();
        let ivy = open(&ivy_home);
        let ivy_site = Site::serve(ivy.site_dir());
        ivy.set_site_url(&ivy_site.base).unwrap();
        ivy.set_name("Ivy").unwrap();
        ivy.list(
            &alice
                .own_link(Some(&"reading-list".parse().unwrap()))
                .unwrap(),
            "Books",
        )
        .unwrap();
        let carol_home = tempfile::tempdir().unwrap();
        let carol = open(&carol_home);
        choose_index_with(&carol, &ivy.own_link(None).unwrap().to_string(), |link| {
            carol.choose_index(link)
        })
        .unwrap();
        assert!(carol.followed().unwrap().is_empty());
        let parties = Parties::new();
        parties.remember(&party_views(&carol).unwrap());
        assert_eq!(
            words::party(&ivy.identity().unwrap().to_string(), &parties)
                .name
                .as_deref(),
            Some("Ivy")
        );
        assert_eq!(
            words::party(&alice.identity().unwrap().to_string(), &parties)
                .name
                .as_deref(),
            Some("Alice")
        );
        let hit = carol
            .search(&Query {
                text: "reading".into(),
                ..Default::default()
            })
            .unwrap()
            .remove(0);
        assert_eq!(
            crate::find::hit_view(&hit, &[], &parties)["via"][0]["index"]["name"],
            "Ivy"
        );
        let api = crate::find::api_result(&hit);
        assert_eq!(
            api["via"],
            serde_json::json!([{"kind":"index","index":ivy.identity().unwrap().to_string(),"through":null,"note":"Books"}])
        );
        assert!(api["publisher"]["key"].is_string());
    }
    #[test]
    fn regress_s29_own_index_counts_listed_mods() {
        // APP-F-38: each index row shows the number of mods it lists. Carol
        // lists Alice's reading-list in her own index; her own row must count it.
        let alice_home = tempfile::tempdir().unwrap();
        let alice = open(&alice_home);
        let alice_site = Site::serve(alice.site_dir());
        alice.set_site_url(&alice_site.base).unwrap();
        alice.publish(package()).unwrap();
        let carol_home = tempfile::tempdir().unwrap();
        let carol = open(&carol_home);
        carol.follow(&alice.own_link(None).unwrap()).unwrap();
        carol
            .list(
                &alice
                    .own_link(Some(&"reading-list".parse().unwrap()))
                    .unwrap(),
                "Books",
            )
            .unwrap();
        let rows = index_rows(
            &carol.own().unwrap(),
            carol.indexes().unwrap(),
            &Default::default(),
            &Parties::new(),
        );
        assert_eq!(rows[0]["own"], true);
        assert_eq!(rows[0]["you"], true);
        assert_eq!(rows[0]["listings"].as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["mods"], 1, "{}", rows[0]);
    }
}

#[cfg(test)]
mod manager_tests {
    use super::*;
    #[tokio::test]
    async fn ac1_s90_consumer_pages_follow_resolved_bindings() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let plan = host
            .plan(
                ["reading-list", "recommendations"]
                    .into_iter()
                    .map(|n| watane_host::Source::Folder(fixtures.join(n)))
                    .collect(),
            )
            .unwrap();
        host.install(plan, watane_host::Requester::App)
            .await
            .unwrap();
        let provider = LocalName::parse("reading-list").unwrap();
        let recommendations = LocalName::parse("recommendations").unwrap();
        let mut mods = host.mods();
        // An unserved host has no page URLs. Supply the literal URLs a served
        // host exposes; page selection itself needs neither a listener nor GUI.
        for m in &mut mods {
            m.page = Some(format!("http://{}.localhost:1234/", m.local));
        }
        assert_eq!(
            consumer_pages(&mods, &provider),
            vec![recommendations.clone()]
        );
        assert!(consumer_pages(&mods, &recommendations).is_empty());
        for kind in [
            watane_host::LinkKind::Call,
            watane_host::LinkKind::Subscribe,
        ] {
            let mut single = mods.clone();
            single
                .iter_mut()
                .find(|m| m.local == recommendations)
                .unwrap()
                .links
                .retain(|link| std::mem::discriminant(&link.kind) == std::mem::discriminant(&kind));
            assert_eq!(
                consumer_pages(&single, &provider),
                vec![recommendations.clone()]
            );
        }
        // Resolved links, including subscriptions and bindings to a different
        // provider identity, select pages once regardless of grant or enable state.
        let consumer = mods
            .iter_mut()
            .find(|m| m.local == recommendations)
            .unwrap();
        consumer.enabled = false;
        consumer.grants.iter_mut().for_each(|g| g.granted = false);
        consumer.binds.clear();
        consumer
            .links
            .retain(|l| matches!(l.kind, watane_host::LinkKind::Subscribe));
        consumer.links.push(consumer.links[0].clone());
        assert_eq!(
            consumer_pages(&mods, &provider),
            vec![recommendations.clone()]
        );
        let consumer = mods
            .iter_mut()
            .find(|m| m.local == recommendations)
            .unwrap();
        consumer.links.iter_mut().for_each(|l| l.provider = None);
        assert!(consumer_pages(&mods, &provider).is_empty());
        let consumer = mods
            .iter_mut()
            .find(|m| m.local == recommendations)
            .unwrap();
        consumer.links[0].provider = Some(provider.clone());
        consumer.page = None;
        assert!(consumer_pages(&mods, &provider).is_empty());
    }

    #[tokio::test]
    async fn ac7_links_follow_host_bindings_and_withdrawn_grants() {
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Default::default()).unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let plan = host
            .plan(
                ["reading-list", "recommendations"]
                    .into_iter()
                    .map(|n| watane_host::Source::Folder(fixtures.join(n)))
                    .collect(),
            )
            .unwrap();
        host.install(plan, watane_host::Requester::App)
            .await
            .unwrap();
        let local = LocalName::parse("recommendations").unwrap();
        let m = host.get(&local).unwrap();
        let links = mod_links(&m);
        assert_eq!(links.len(), 2);
        assert!(
            links
                .iter()
                .all(|l| l.provider == "reading-list" && l.granted)
        );
        let call = m
            .grants
            .iter()
            .find(|g| matches!(g.grant, Grant::Call { .. }))
            .unwrap();
        host.revoke(&local, &call.grant).unwrap();
        let links = mod_links(&host.get(&local).unwrap());
        assert!(!links.iter().find(|l| l.kind == "call").unwrap().granted);
        assert!(
            links
                .iter()
                .find(|l| l.kind == "subscribe")
                .unwrap()
                .granted
        );
    }
}

#[cfg(test)]
mod machines_tests {
    #[test]
    fn ac10_keyless_indexes_and_names_are_readable() {
        let home = tempfile::tempdir().unwrap();
        let dist = watane_dist::Dist::open_keyless(home.path(), Default::default()).unwrap();
        assert!(super::party_views(&dist).unwrap().is_empty());
    }
}
