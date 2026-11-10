mod api;
mod catalog;
mod error;
mod home;
mod pages;
mod server;

pub use catalog::{GrantState, InstalledMod, LinkKind, LinkView};
pub use error::HostError;
pub use home::{LocalName, RunInfo, default_home};
use serde::{Deserialize, Serialize};
pub use server::Served;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
pub use watane_pkg;

#[derive(Debug)]
pub struct Host {
    home: home::Home,
    catalog: Mutex<catalog::Catalog>,
    serving: AtomicBool,
}

impl Host {
    pub fn open(path: &Path) -> Result<Arc<Self>, HostError> {
        let home = home::Home::open(path)?;
        let catalog = catalog::Catalog::open(&home.path)?;
        Ok(Arc::new(Self {
            home,
            catalog: Mutex::new(catalog),
            serving: AtomicBool::new(false),
        }))
    }

    pub fn home(&self) -> &Path {
        &self.home.path
    }
    pub fn is_preview(&self) -> bool {
        true
    }

    pub fn install_folders(
        &self,
        folders: &[PathBuf],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        self.catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .install(self.home(), folders, port)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ready {
    pub watane: u32,
    pub mode: String,
    pub home: PathBuf,
    pub port: u16,
    pub api: String,
    pub mcp: Option<String>,
    pub token: Option<String>,
    pub mods: Vec<ReadyMod>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadyMod {
    pub local: LocalName,
    pub page: Option<String>,
}
