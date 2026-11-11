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
pub(crate) struct Core {
    pub host: Arc<Host>,
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
        served: Served,
        parties: Arc<Parties>,
    ) -> Self {
        Self {
            app,
            host,
            served: Mutex::new(Some(served)),
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
    pub fn tools(&self) -> Result<Vec<String>, CmdError> {
        let mut tools = Vec::new();
        for m in self.host.mods().into_iter().filter(|m| m.enabled) {
            for f in self.host.functions(&m.local)? {
                tools.push(format!(
                    "{}:{}.{}",
                    m.local,
                    f.interface.interface(),
                    f.name
                ));
            }
        }
        Ok(tools)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
