use crate::{
    CmdError,
    consent::{GrantLine, Party},
    views,
    words::{self, Parties},
};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use watane_host::{GrantState, Host, LocalName, Served, watane_pkg::Grant};
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModView {
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
    pub error: Option<String>,
    pub grants: Vec<GrantLine>,
    pub notice: Option<String>,
    pub offer: Option<String>,
    pub offer_grants: Vec<GrantLine>,
}
#[derive(serde::Serialize)]
pub(crate) struct ToolRow {
    pub entry: String,
    pub tool: String,
}
fn tool_row(local: &str, interface: &str, function: &str) -> ToolRow {
    ToolRow {
        entry: format!("{local}:{interface}.{function}"),
        tool: format!("{local}_{interface}_{function}"),
    }
}
pub(crate) struct Core {
    pub host: Arc<Host>,
    pub dist: Arc<watane_dist::Dist>,
    failures: Mutex<std::collections::BTreeMap<String, String>>,
    update_failures: Mutex<std::collections::BTreeMap<String, String>>,
    pub served: Mutex<Option<Served>>,
    app: tauri::AppHandle,
    parties: Arc<Parties>,
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
impl Core {
    pub fn new(
        app: tauri::AppHandle,
        host: Arc<Host>,
        dist: Arc<watane_dist::Dist>,
        parties: Arc<Parties>,
    ) -> Self {
        Self {
            app,
            host,
            served: Mutex::new(None),
            dist,
            failures: Mutex::new(Default::default()),
            parties,
            update_failures: Mutex::new(Default::default()),
        }
    }
    pub fn port(&self) -> u16 {
        self.served.lock().unwrap().as_ref().map_or(0, |s| s.port)
    }
    fn view(&self, m: watane_host::InstalledMod, followed: &[watane_dist::ChannelView]) -> ModView {
        let channel = m.publisher.as_ref().and_then(|p| {
            followed
                .iter()
                .find(|c| c.channel.to_string() == p.to_string())
        });
        let failures = self.update_failures.lock().unwrap();
        let notice = release_notice(m.release.as_ref(), channel, failures.get(m.local.as_ref()));
        let offer = release_offer(m.release.as_ref(), channel);
        let offer_grants = offer_grants(offer.as_ref(), channel, &m.grants, &self.parties);
        let offer = offer.map(|r| r.to_string());
        ModView {
            notice,
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
            grants: m
                .grants
                .iter()
                .map(|g| words::grant_line(&g.grant, &self.parties, g.granted, false))
                .collect(),
        }
    }
    pub fn mods(&self) -> Result<Vec<ModView>, CmdError> {
        let followed = self.dist.followed()?;
        Ok(self
            .host
            .mods()
            .into_iter()
            .map(|m| self.view(m, &followed))
            .collect())
    }
    pub fn get(&self, local: &str) -> Result<Option<ModView>, CmdError> {
        let name = LocalName::parse(local)?;
        let followed = self.dist.followed()?;
        Ok(self.host.get(&name).map(|m| self.view(m, &followed)))
    }
    pub fn disable(&self, local: &str) -> Result<(), CmdError> {
        let name = LocalName::parse(local)?;
        views::close_mod(&self.app, local)?;
        Ok(self.host.disable(&name)?)
    }
    pub fn remove(&self, local: &str, delete_data: bool) -> Result<(), CmdError> {
        let name = LocalName::parse(local)?;
        views::close_mod(&self.app, local)?;
        Ok(self.host.remove(&name, delete_data)?)
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
            window.reload()?;
        }
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
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChannelRow {
    channel: String,
    fingerprint: String,
    name: Option<String>,
    base: Option<String>,
    status: &'static str,
    cause: Option<String>,
}
fn channel_row(view: &watane_dist::ChannelView, failure: Option<&String>) -> ChannelRow {
    ChannelRow {
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
    mut plan: impl FnMut(&[S]) -> Result<(P, Vec<Option<watane_dist::ReleaseRef>>), CmdError>,
    mut fetch: impl FnMut(&watane_dist::ReleaseRef) -> Result<S, CmdError>,
) -> Result<P, CmdError> {
    let mut added = std::collections::BTreeSet::new();
    for _ in 0..32 {
        let (plan, missing) = plan(&sources)?;
        if missing.is_empty() {
            return Ok(plan);
        }
        let before = sources.len();
        for release in missing {
            let release = release.ok_or_else(|| {
                CmdError::new("missing_providers", "a local provider has no release")
            })?;
            if added.insert(release.clone()) {
                sources.push(fetch(&release)?);
            }
        }
        if before == sources.len() {
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

impl Core {
    pub fn status(&self) -> Result<(serde_json::Value, serde_json::Value), CmdError> {
        let own = self.dist.own()?;
        let link = if own.base.is_some() {
            Some(self.dist.own_link(None)?.to_string())
        } else {
            None
        };
        Ok((
            serde_json::json!({"name":own.name,"fingerprint":self.dist.fingerprint(),"ownConfirmed":self.dist.own_confirmed()?,"releases":own_releases(&own)}),
            serde_json::json!({"dir":self.dist.site_dir(),"url":own.base,"link":link}),
        ))
    }
    pub fn channels(&self) -> Result<Vec<ChannelRow>, CmdError> {
        let followed = self.dist.followed()?;
        self.parties.replace(&party_views(&self.dist)?);
        let failures = self.failures.lock().unwrap();
        Ok(followed
            .iter()
            .map(|v| channel_row(v, failures.get(&v.channel.to_string())))
            .collect())
    }
    pub fn refresh(&self) -> Result<(Vec<ChannelRow>, Vec<watane_dist::ReleaseRef>), CmdError> {
        let reports = self.dist.refresh();
        let followed = self.dist.followed()?;
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
                &mut self.failures.lock().unwrap(),
                report.channel,
                report.outcome,
            );
        }
        Ok((self.channels()?, updates))
    }
    pub fn spawn_updates(self: &Arc<Self>, updates: Vec<watane_dist::ReleaseRef>) {
        for release in updates {
            let core = self.clone();
            tauri::async_runtime::spawn(async move {
                let _ = core.install_release(release, None).await;
            });
        }
    }
    pub fn installed(&self, installed: &[watane_host::InstalledMod]) {
        let mut failures = self.update_failures.lock().unwrap();
        for m in installed {
            failures.remove(m.local.as_ref());
        }
    }
    pub async fn install_release(
        self: &Arc<Self>,
        release: watane_dist::ReleaseRef,
        link: Option<watane_dist::Link>,
    ) -> Result<Vec<watane_host::InstalledMod>, CmdError> {
        let planner = self.clone();
        let target = release.clone();
        let result = async {
            let plan = tauri::async_runtime::spawn_blocking(move || {
                let fetched = fetch_release(&planner.dist, &target, link.as_ref())?;
                planner.plan_fetched(fetched)
            })
            .await
            .map_err(|e| CmdError::new("io", e.to_string()))??;
            Ok::<_, CmdError>(self.host.install(plan, watane_host::Requester::App).await?)
        }
        .await;
        match &result {
            Ok(installed) => self.installed(installed),
            Err(error) => {
                for m in self.host.mods().iter().filter(|m| {
                    m.release
                        .as_ref()
                        .is_some_and(|r| r.publisher == release.publisher && r.slug == release.slug)
                }) {
                    self.update_failures.lock().unwrap().insert(
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
        self.dist.follow(&link)?;
        self.failures
            .lock()
            .unwrap()
            .remove(&link.channel().to_string());
        self.channels()?
            .into_iter()
            .find(|c| c.channel == link.channel().to_string())
            .ok_or_else(|| CmdError::new("not_found", "followed channel missing"))
    }
    pub fn plan_link(&self, text: &str) -> Result<watane_host::Plan, CmdError> {
        let link: watane_dist::Link = text.parse()?;
        if link.slug().is_none() {
            self.follow(text)?;
            return Err(CmdError::new(
                "bad_input",
                "the channel is followed; the link names no mod",
            ));
        }
        let first = self.dist.open_link(&link)?;
        self.channels()?;
        self.plan_fetched(first)
    }
    fn plan_fetched(&self, first: watane_dist::Fetched) -> Result<watane_host::Plan, CmdError> {
        let plan = resolve_plan(
            vec![first],
            |fetched| {
                let plan = self.host.plan(
                    fetched
                        .iter()
                        .map(|f| watane_host::Source::Release {
                            package: f.package.clone(),
                            entry: f.entry.clone(),
                        })
                        .collect(),
                )?;
                if plan.missing().iter().any(|m| m.release.is_none()) {
                    return Err(
                        watane_host::HostError::MissingProviders(plan.missing().to_vec()).into(),
                    );
                }
                let missing = plan.missing().iter().map(|m| m.release.clone()).collect();
                Ok((plan, missing))
            },
            |r| Ok(self.dist.fetch(r)?),
        )?;
        self.channels()?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            |sources| {
                Ok((
                    sources.len(),
                    if sources.len() == 1 {
                        vec![Some(reference.clone())]
                    } else {
                        vec![]
                    },
                ))
            },
            |r| {
                fetched.push(r.clone());
                Ok(1)
            },
        )
        .unwrap();
        assert_eq!(result, 2);
        assert_eq!(fetched, vec![reference.clone()]);
        let e = resolve_plan(vec![0], |_| Ok(((), vec![None])), |_| Ok(1)).unwrap_err();
        assert_eq!(e.code, "missing_providers");
        let e = resolve_plan(
            vec![0],
            |_| Ok(((), vec![Some(reference.clone())])),
            |_| Ok(1),
        )
        .unwrap_err();
        assert_eq!(e.code, "missing_providers");
        let mut rounds = 0;
        let e = resolve_plan(
            vec![0],
            |_| {
                rounds += 1;
                let mut r = reference.clone();
                r.version = format!("0.0.{rounds}").parse().unwrap();
                Ok(((), vec![Some(r)]))
            },
            |_| Ok(1),
        )
        .unwrap_err();
        assert_eq!(e.code, "missing_providers");
        assert_eq!(rounds, 32);
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
                error: None,
                grants: vec![],
                notice: notice.map(str::to_string),
                offer: offer.map(str::to_string),
                offer_grants: vec![],
            };
            let value = serde_json::to_value(row).unwrap();
            assert_eq!(value["notice"], serde_json::json!(notice));
            assert_eq!(value["offer"], serde_json::json!(offer));
            assert_eq!(value["offerGrants"], serde_json::json!([]));
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
        assert_eq!(rows[0]["mods"], 0);
        assert_eq!(rows[1]["own"], false);
        assert_eq!(rows[1]["name"], "Ivy");
        assert_eq!(rows[1]["mods"], 7);
        assert_eq!(rows[1]["included"][0]["name"], "Included");
        assert_eq!(
            rows[1]["skipped"],
            serde_json::json!([{"party":{"name":"Included","fingerprint":own.channel.fingerprint()},"cause":"own channel"}])
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
        assert_eq!(row["cause"], "offline");
        assert_eq!(row["skipped"], serde_json::json!([]));
        assert_eq!(
            row["listings"],
            serde_json::json!([
                {"entry":EntryId::from_bytes([0;32]).to_string(),"kind":"list","target":format!("{key}/reading-list"),"note":"Books"},
                {"entry":EntryId::from_bytes([1;32]).to_string(),"kind":"include","target":key.to_string(),"note":"Index"}
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
    let mut row = serde_json::to_value(channel_row(view, failure)).unwrap();
    let object = row.as_object_mut().unwrap();
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
        serde_json::json!({"entry":l.entry.to_string(), "kind":if l.include {"include"} else {"list"}, "target":link.slug().map(|slug|format!("{}/{slug}",link.channel())).unwrap_or_else(||link.channel().to_string()), "note":l.listing.note})
    }).collect::<Vec<_>>()));
    row
}
fn party_views(dist: &watane_dist::Dist) -> Result<Vec<watane_dist::ChannelView>, CmdError> {
    let mut views = dist.followed()?;
    views.push(dist.own()?);
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
pub(crate) fn choose_index(
    dist: &watane_dist::Dist,
    text: &str,
) -> Result<Vec<watane_dist::RefreshReport>, CmdError> {
    let link = crate::commands::parse_link(text)?;
    if link.channel() == dist.identity() {
        return Err(CmdError::new("bad_input", words::OWN_INDEX));
    }
    Ok(dist.choose_index(&link)?)
}
pub(crate) fn fetch_release(
    dist: &watane_dist::Dist,
    release: &watane_dist::ReleaseRef,
    link: Option<&watane_dist::Link>,
) -> Result<watane_dist::Fetched, CmdError> {
    if let Some(link) = link {
        dist.follow(link)?;
    }
    Ok(dist.fetch(release)?)
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
impl Core {
    pub fn indexes(&self) -> Result<Vec<serde_json::Value>, CmdError> {
        self.parties.replace(&party_views(&self.dist)?);
        let own = self.dist.own()?;
        let failures = self.failures.lock().unwrap();
        Ok(index_rows(
            &own,
            self.dist.indexes()?,
            &failures,
            &self.parties,
        ))
    }
    pub fn choose_index(&self, text: &str) -> Result<Vec<serde_json::Value>, CmdError> {
        for report in choose_index(&self.dist, text)? {
            keep_outcome(
                &mut self.failures.lock().unwrap(),
                report.channel,
                report.outcome,
            );
        }
        self.indexes()
    }
    pub fn search(&self, query: &watane_dist::Query) -> Result<Vec<serde_json::Value>, CmdError> {
        self.parties.replace(&party_views(&self.dist)?);
        let installed = self.host.mods();
        Ok(self
            .dist
            .search(query)?
            .iter()
            .map(|h| crate::find::hit_view(h, &installed, &self.parties))
            .collect())
    }
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
        let carol = open(&carol_home);
        let reference = &published.info.reference;
        let expected = carol.fetch(reference).unwrap_err().to_string();
        let error = fetch_release(&carol, reference, None).unwrap_err();
        assert_eq!(error.message, expected);
        assert_eq!(error.code, "dist");
        assert!(carol.followed().unwrap().is_empty());
        let fetched = fetch_release(&carol, reference, published.link.as_ref()).unwrap();
        assert_eq!(carol.followed().unwrap()[0].channel, alice.identity());
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
        choose_index(&carol, &ivy.own_link(None).unwrap().to_string()).unwrap();
        assert!(carol.followed().unwrap().is_empty());
        let parties = Parties::new();
        parties.replace(&party_views(&carol).unwrap());
        assert_eq!(
            words::party(&ivy.identity().to_string(), &parties)
                .name
                .as_deref(),
            Some("Ivy")
        );
        assert_eq!(
            words::party(&alice.identity().to_string(), &parties)
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
            serde_json::json!([{"kind":"index","index":ivy.identity().to_string(),"through":null,"note":"Books"}])
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
        assert_eq!(rows[0]["listings"].as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["mods"], 1, "{}", rows[0]);
    }
}
