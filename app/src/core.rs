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
    pub served: Mutex<Option<Served>>,
    app: tauri::AppHandle,
    parties: Arc<Parties>,
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
        }
    }
    pub fn port(&self) -> u16 {
        self.served.lock().unwrap().as_ref().map_or(0, |s| s.port)
    }
    fn view(&self, m: watane_host::InstalledMod) -> ModView {
        ModView {
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
    pub fn mods(&self) -> Vec<ModView> {
        self.host.mods().into_iter().map(|m| self.view(m)).collect()
    }
    pub fn get(&self, local: &str) -> Result<Option<ModView>, CmdError> {
        Ok(self
            .host
            .get(&LocalName::parse(local)?)
            .map(|m| self.view(m)))
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
            serde_json::json!({"name":own.name,"fingerprint":self.dist.fingerprint(),"ownConfirmed":self.dist.own_confirmed()?}),
            serde_json::json!({"dir":self.dist.site_dir(),"url":own.base,"link":link}),
        ))
    }
    pub fn channels(&self) -> Result<Vec<ChannelRow>, CmdError> {
        let followed = self.dist.followed()?;
        let mut parties = followed.clone();
        parties.push(self.dist.own()?);
        self.parties.replace(&parties);
        let failures = self.failures.lock().unwrap();
        Ok(followed
            .iter()
            .map(|v| channel_row(v, failures.get(&v.channel.to_string())))
            .collect())
    }
    pub fn refresh(&self) -> Result<Vec<ChannelRow>, CmdError> {
        for report in self.dist.refresh() {
            let mut failures = self.failures.lock().unwrap();
            keep_outcome(&mut failures, report.channel, report.outcome);
        }
        self.channels()
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
