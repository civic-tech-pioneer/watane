pub const HOST_WIT: &str = include_str!("../wit/host.wit");
pub const WATANE_JS: &str = include_str!("watane.js");

mod api;
mod bus;
mod catalog;
mod error;
mod harness;
mod home;
mod pages;
mod runtime;
mod server;
mod state;
mod tools;
pub use runtime::limits::Limits;
pub use tools::{FunctionInfo, Handle, Token, TokenId, ToolsetName};

pub use catalog::{GrantState, InstalledMod, LinkKind, LinkView};
pub use error::{CallCode, CallError, HostError, Limit};
pub use harness::{EventRecord, Fakes, Harness, Outcome};
pub use home::{LocalName, RunInfo, default_home};
use serde::{Deserialize, Serialize};
pub use server::Served;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
pub use watane_pkg;

pub const DEFAULT_PORT: u16 = 47341;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Home,
    Preview,
}

impl Mode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Preview => "preview",
        }
    }
}

#[derive(Debug)]
pub struct Host {
    home: home::Home,
    catalog: Mutex<catalog::Catalog>,
    serving: AtomicBool,
    engine: Arc<wasmtime::Engine>,
    limits: Limits,
    state: state::State,
    call_locks: Mutex<std::collections::BTreeMap<String, Arc<Mutex<()>>>>,
    bus: bus::PageBus,
    tools: Mutex<tools::Tools>,
}

impl Host {
    pub fn open(path: &Path, mode: Mode, limits: Limits) -> Result<Arc<Self>, HostError> {
        let home = home::Home::open(path, mode)?;
        let engine = Arc::new(runtime::engine()?);
        let catalog = catalog::Catalog::open(&home.path, &engine)?;
        let state = state::State::open(&home.path)?;
        let host = Arc::new(Self {
            tools: Mutex::new(tools::Tools::open(&home.path)?),
            bus: bus::PageBus::new(1024),
            state,
            call_locks: Mutex::new(std::collections::BTreeMap::new()),
            engine,
            limits,
            home,
            catalog: Mutex::new(catalog),
            serving: AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&host.engine);
        std::thread::Builder::new()
            .name("watane-epoch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    let Some(engine) = weak.upgrade() else {
                        break;
                    };
                    engine.increment_epoch();
                }
            })?;
        Ok(host)
    }

    /// Blocking. The calling thread needs a 16 MiB stack.
    pub fn call(
        &self,
        caller: &Caller,
        local: &LocalName,
        function: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        self.call_outcome(caller, local, function, args, true)
            .map(|outcome| outcome.value)
    }

    pub(crate) fn call_outcome(
        &self,
        caller: &Caller,
        local: &LocalName,
        function: &str,
        args: serde_json::Value,
        publish: bool,
    ) -> Result<Outcome, CallError> {
        let (compiled, identity) = {
            let catalog = self
                .catalog
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let m = catalog
                .mods
                .iter()
                .find(|m| &m.local == local)
                .ok_or_else(|| CallError::new(CallCode::NoMod, format!("no mod {local}")))?;
            if !m.enabled {
                return Err(CallError::new(
                    CallCode::Disabled,
                    format!("mod {local} is disabled"),
                ));
            }
            if let Some(error) = &m.error {
                return Err(CallError::internal(error));
            }
            let compiled = catalog.components.get(local).cloned().ok_or_else(|| {
                CallError::new(
                    CallCode::NoFunction,
                    format!("mod {local} has no logic part"),
                )
            })?;
            (compiled, m.identity.clone())
        };
        compiled.call(
            self,
            publish.then_some(local),
            &identity,
            caller,
            function,
            args,
        )
    }
    pub async fn call_async(
        self: &Arc<Self>,
        caller: Caller,
        local: LocalName,
        function: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let host = self.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("watane-call".into())
            .stack_size(16 << 20)
            .spawn(move || {
                let result = host.call(&caller, &local, &function, args);
                drop(host);
                let _ = tx.send(result);
            })
            .map_err(CallError::internal)?;
        rx.await.map_err(CallError::internal)?
    }

    pub fn home(&self) -> &Path {
        &self.home.path
    }
    pub fn is_preview(&self) -> bool {
        self.home.mode == Mode::Preview
    }

    pub fn install_folders(
        &self,
        folders: &[PathBuf],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let mut catalog = self
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before: std::collections::BTreeMap<_, _> = catalog
            .mods
            .iter()
            .map(|m| (m.identity.clone(), m.package.clone()))
            .collect();
        let installed =
            catalog.install(self.home(), folders, port, &self.engine, self.home.mode)?;
        for m in &installed {
            if before.get(&m.identity).is_some_and(|id| id != &m.package) {
                self.bus.send(&m.local, bus::RELOAD);
            }
        }
        Ok(installed)
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

#[derive(Clone, Debug)]
pub enum Caller {
    Page,
    Agent(Handle),
}
impl Caller {
    pub fn who(&self) -> String {
        match self {
            Self::Page => "page".into(),
            Self::Agent(handle) => format!("agent:{handle}"),
        }
    }
}
