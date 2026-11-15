pub const HOST_WIT: &str = include_str!("../wit/host.wit");
pub const WATANE_JS: &str = include_str!("watane.js");

mod api;
mod bus;
mod catalog;
mod confirm;
mod error;
pub use confirm::{
    Answer, Ask, Confirm, InstallRequest, ManageOutcome, ManageRequest, Reduction, RequestId,
    Requester,
};
mod harness;
mod home;
mod manage;
mod pages;
mod runtime;
mod server;
mod state;
mod tools;
pub use runtime::limits::Limits;
pub use tools::{FunctionInfo, Handle, Token, TokenId, TokenInfo, ToolsetName};

pub use catalog::{
    Bind, Change, GrantState, InstalledMod, LinkKind, LinkView, MissingProvider, Plan, PlanEntry,
    Source,
};
pub use error::{CallCode, CallError, HostError, Limit, VerifyCheck};
pub use harness::{EventRecord, Fakes, Harness, ManageRecord, Outcome};
pub use home::{LocalName, RunInfo, default_home};
use serde::{Deserialize, Serialize};
pub use server::Served;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::AtomicBool},
};
pub use {watane_log, watane_pkg};

pub const DEFAULT_PORT: u16 = 47341;

#[derive(Clone)]
pub enum Mode {
    Home { app: Option<Arc<dyn Confirm>> },
    Preview,
}
impl std::fmt::Debug for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Home { app } => f.debug_struct("Home").field("app", &app.is_some()).finish(),
            Self::Preview => f.write_str("Preview"),
        }
    }
}
impl Mode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Home { .. } => "home",
            Self::Preview => "preview",
        }
    }
    pub(crate) fn has_app(&self) -> bool {
        matches!(self, Self::Home { app: Some(_) })
    }
}

#[derive(Debug)]
pub struct Host {
    home: home::Home,
    catalog: Mutex<catalog::Catalog>,
    serving: AtomicBool,
    port: std::sync::atomic::AtomicU16,
    pub(crate) installing: tokio::sync::Mutex<()>,
    engine: Arc<wasmtime::Engine>,
    limits: Limits,
    state: state::State,
    call_locks: Mutex<std::collections::BTreeMap<String, Arc<Mutex<()>>>>,
    bus: bus::PageBus,
    tools: Mutex<tools::Tools>,
    weak: std::sync::Weak<Host>,
    events: runtime::call::Deliveries,
    fakes: Mutex<Option<Fakes>>,
    requests: Mutex<manage::Requests>,
    /// Harness only: the fake requests answered so far (6.10).
    manage_serial: std::sync::atomic::AtomicUsize,
}

impl Host {
    pub fn open(path: &Path, mode: Mode, limits: Limits) -> Result<Arc<Self>, HostError> {
        let home = home::Home::open(path, mode)?;
        let engine = Arc::new(runtime::engine()?);
        let catalog = catalog::Catalog::open(&home.path, &engine)?;
        let state = state::State::open(&home.path)?;
        let tools = tools::Tools::open(&home.path)?;
        let host = Arc::new_cyclic(|weak| Self {
            weak: weak.clone(),
            events: runtime::call::Deliveries::default(),
            fakes: Mutex::new(None),
            requests: Mutex::new(manage::Requests::default()),
            manage_serial: std::sync::atomic::AtomicUsize::new(0),
            tools: Mutex::new(tools),
            bus: bus::PageBus::new(1024),
            state,
            call_locks: Mutex::new(std::collections::BTreeMap::new()),
            engine,
            limits,
            home,
            catalog: Mutex::new(catalog),
            serving: AtomicBool::new(false),
            port: std::sync::atomic::AtomicU16::new(0),
            installing: tokio::sync::Mutex::new(()),
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
        runtime::call::run(self, caller, local, function, args, publish, false)
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
        matches!(self.home.mode, Mode::Preview)
    }

    pub fn install_folders(
        &self,
        folders: &[PathBuf],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let plan = self.plan(folders.iter().cloned().map(Source::Folder).collect())?;
        if plan.needs_confirmation() {
            return Err(HostError::Refused(
                "use install with the app to confirm the install".into(),
            ));
        }
        self.commit_plan(plan, &[], port)
    }
    pub fn plan(&self, sources: Vec<Source>) -> Result<Plan, HostError> {
        self.plan_with(sources, vec![])
    }
    pub fn plan_with(&self, sources: Vec<Source>, binds: Vec<Bind>) -> Result<Plan, HostError> {
        self.catalog.lock().unwrap().plan_with(
            self.home(),
            &sources,
            &self.engine,
            self.is_preview(),
            false,
            &binds,
        )
    }
    pub async fn install(
        &self,
        plan: Plan,
        requester: Requester,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let _install = self.installing.lock().await;
        self.install_locked(plan, requester).await
    }
    /// The caller holds `installing` (section 11: `/api/install` plans under it).
    pub(crate) async fn install_locked(
        &self,
        plan: Plan,
        requester: Requester,
    ) -> Result<Vec<InstalledMod>, HostError> {
        self.catalog.lock().unwrap().validate(&plan, self.home())?;
        // Step 10 (6.10): a manager's install is always asked.
        let manager = match &requester {
            Requester::Mod(by) => {
                if !self.is_manager(by) {
                    return Err(HostError::Refused(format!("{by} holds no grant to manage")));
                }
                Some(by.clone())
            }
            _ => None,
        };
        let asks = plan.needs_confirmation()
            || manager.is_some()
                && !self.is_preview()
                && plan
                    .entries()
                    .iter()
                    .any(|e| !matches!(e.change, Change::Unchanged));
        let withhold = if asks {
            let Mode::Home { app: Some(app) } = &self.home.mode else {
                return Err(HostError::Refused(
                    "no app is attached to confirm the install".into(),
                ));
            };
            let request = InstallRequest {
                entries: plan
                    .entries()
                    .iter()
                    .filter(|e| !matches!(e.change, Change::Unchanged))
                    .cloned()
                    .collect(),
                requester,
            };
            match tokio::time::timeout(std::time::Duration::from_secs(600), app.install(request))
                .await
            {
                Ok(Answer::Approve { withhold }) => withhold,
                _ => return Err(HostError::Refused("install was not approved".into())),
            }
        } else {
            vec![]
        };
        if let Some(by) = &manager
            && !self.is_manager(by)
        {
            return Err(HostError::Refused(format!("{by} lost its grant to manage")));
        }
        self.commit_plan(
            plan,
            &withhold,
            self.port.load(std::sync::atomic::Ordering::Acquire),
        )
    }
    fn commit_plan(
        &self,
        plan: Plan,
        withhold: &[(LocalName, watane_pkg::Grant)],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let updates: Vec<_> = plan
            .entries()
            .iter()
            .filter(|e| matches!(e.change, Change::Update { .. }))
            .map(|e| e.local.clone())
            .collect();
        let installed =
            self.catalog
                .lock()
                .unwrap()
                .commit(self.home(), plan, withhold, self.is_preview())?;
        for local in updates {
            self.bus.send(&local, bus::RELOAD);
        }
        self.catalog_written();
        Ok(installed
            .into_iter()
            .map(|m| self.mod_view(m, port))
            .collect())
    }
    fn mod_view(&self, mut m: InstalledMod, port: u16) -> InstalledMod {
        m.page = m
            .page
            .and_then(|_| (port != 0).then(|| format!("http://{}.localhost:{port}/", m.local)));
        m
    }
    pub fn mods(&self) -> Vec<InstalledMod> {
        let mut mods = self.catalog.lock().unwrap().mods.clone();
        mods.sort_by(|a, b| a.local.cmp(&b.local));
        let port = self.port.load(std::sync::atomic::Ordering::Acquire);
        mods.into_iter().map(|m| self.mod_view(m, port)).collect()
    }
    pub fn get(&self, local: &LocalName) -> Option<InstalledMod> {
        self.mods().into_iter().find(|m| &m.local == local)
    }
    fn change_mod(
        &self,
        local: &LocalName,
        change: impl FnOnce(&mut InstalledMod) -> Result<(), HostError>,
    ) -> Result<(), HostError> {
        let mut catalog = self.catalog.lock().unwrap();
        let mut next = catalog.clone();
        let m = next
            .mods
            .iter_mut()
            .find(|m| &m.local == local)
            .ok_or_else(|| HostError::NoSuchMod(local.to_string()))?;
        change(m)?;
        next.save(self.home())?;
        *catalog = next;
        Ok(())
    }
    pub fn enable(&self, local: &LocalName) -> Result<(), HostError> {
        self.change_mod(local, |m| {
            m.enabled = true;
            Ok(())
        })?;
        self.bus.send(local, bus::RELOAD);
        self.catalog_written();
        Ok(())
    }
    pub fn disable(&self, local: &LocalName) -> Result<(), HostError> {
        self.change_mod(local, |m| {
            m.enabled = false;
            Ok(())
        })?;
        self.bus.send(local, bus::CLOSE);
        self.catalog_written();
        Ok(())
    }
    fn set_grant(
        &self,
        local: &LocalName,
        grant: &watane_pkg::Grant,
        granted: bool,
    ) -> Result<(), HostError> {
        self.change_mod(local, |m| {
            let state = m
                .grants
                .iter_mut()
                .find(|g| &g.grant == grant)
                .ok_or_else(|| HostError::NotDeclared {
                    local: local.clone(),
                    grant: grant.clone(),
                })?;
            state.granted = granted;
            Ok(())
        })?;
        self.catalog_written();
        Ok(())
    }
    pub fn grant(&self, local: &LocalName, grant: &watane_pkg::Grant) -> Result<(), HostError> {
        self.set_grant(local, grant, true)
    }
    pub fn revoke(&self, local: &LocalName, grant: &watane_pkg::Grant) -> Result<(), HostError> {
        self.set_grant(local, grant, false)
    }
    pub fn remove(&self, local: &LocalName, delete_data: bool) -> Result<(), HostError> {
        // Lock the mod before the catalog; a running call may read the catalog.
        let identity = self
            .get(local)
            .ok_or_else(|| HostError::NoSuchMod(local.to_string()))?
            .identity;
        let lock = self
            .call_locks
            .lock()
            .unwrap()
            .entry(identity.clone())
            .or_default()
            .clone();
        let _call = lock.lock().unwrap();
        let mut catalog = self.catalog.lock().unwrap();
        if !catalog
            .mods
            .iter()
            .any(|m| &m.local == local && m.identity == identity)
        {
            return Err(HostError::NoSuchMod(local.to_string()));
        }
        let mut next = catalog.clone();
        next.mods.retain(|m| &m.local != local);
        next.components.remove(local);
        for m in &mut next.mods {
            m.binds.retain(|b| b.to != identity);
        }
        next.relink(self.home(), self.is_preview())?;
        next.save(self.home())?;
        *catalog = next;
        drop(catalog);
        std::fs::remove_dir_all(self.home().join("mods").join(local.as_ref()))?;
        if delete_data {
            self.state.delete(&identity)?;
        }
        self.bus.send(local, bus::CLOSE);
        self.catalog_written();
        Ok(())
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
    Mod(LocalName),
}
impl Caller {
    pub fn who(&self) -> String {
        match self {
            Self::Page => "page".into(),
            Self::Agent(handle) => format!("agent:{handle}"),
            Self::Mod(local) => format!("mod:{local}"),
        }
    }
}
