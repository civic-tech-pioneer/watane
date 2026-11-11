#[path = "fake.rs"]
mod fake;
use crate::{
    CmdError,
    consent::{GrantLine, Party},
    views,
};
use std::sync::Mutex;
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
    mods: Mutex<Vec<ModView>>,
    app: tauri::AppHandle,
}
impl Core {
    pub fn new(app: tauri::AppHandle) -> Result<Self, CmdError> {
        Ok(Self {
            mods: Mutex::new(fake::read()?),
            app,
        })
    }
    pub fn mods(&self) -> Vec<ModView> {
        self.mods.lock().unwrap().clone()
    }
    pub fn get(&self, local: &str) -> Option<ModView> {
        self.mods
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.local == local)
            .cloned()
    }
    pub fn disable(&self, local: &str) -> Result<(), CmdError> {
        views::close_mod(&self.app, local)?;
        let mut mods = self.mods.lock().unwrap();
        let m = mods
            .iter_mut()
            .find(|m| m.local == local)
            .ok_or_else(|| CmdError::new("not_found", "mod not found"))?;
        m.enabled = false;
        Ok(())
    }
}
