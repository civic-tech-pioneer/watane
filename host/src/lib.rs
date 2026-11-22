//! `watane-host`, the security core that installs and runs mods (TECHNICAL 2.1):
//! `Host`, its modes and the crate's re-exports.

pub const HOST_WIT: &str = include_str!("../wit/host.wit");
pub const WATANE_JS: &str = include_str!("watane.js");

mod agent;
pub use agent::accounts::{AccountError, AccountSource, JudgeAccount};
mod api;
mod bus;
mod catalog;
mod confirm;
mod error;
pub use confirm::{
    Answer, Ask, Confirm, InstallRequest, Installed, ManageOutcome, ManageRequest, Reduction,
    RequestId, Requester,
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

#[cfg(feature = "test-streams")]
pub use agent::streams::{Delivery, DeliveryAnswer, StreamError, TestClock};
pub use catalog::{
    Bind, Change, GrantState, InstalledMod, LinkKind, LinkView, MissingProvider, Plan, PlanEntry,
    Source,
};
pub use error::{CallCode, CallError, HostError, Limit, VerifyCheck};
pub use harness::{EventRecord, Fakes, Harness, ManageRecord, Outcome, RequestRecord};
pub use home::{LocalName, RunInfo, default_home};
use serde::{Deserialize, Serialize};
pub use server::Served;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
pub use watane_log;
pub use watane_pkg;

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
    /// The chains in flight, and the gate of 19.2 (Commit): a call checks the limits and queues
    /// its effect, and a chain takes its log to apply it, under this lock.
    chains: Mutex<Vec<std::sync::Weak<runtime::call::Chain>>>,
    /// Held by `remove` from its start to its end: an install of a local name waits for the
    /// removal of that name to finish (4.4).
    removes: Mutex<()>,
    bus: bus::PageBus,
    tools: Mutex<tools::Tools>,
    weak: std::sync::Weak<Host>,
    events: runtime::call::Deliveries,
    fakes: Mutex<Option<Fakes>>,
    requests: Mutex<manage::Requests>,
    /// Harness only: the fake requests answered so far (6.10).
    manage_serial: std::sync::atomic::AtomicUsize,
    /// Harness only: the fake streams opened so far (19.1).
    fake_serial: std::sync::atomic::AtomicUsize,
    /// Streams and page connections (19.2).
    hub: Arc<agent::streams::Hub>,
    /// Timers (19.6).
    timers: Arc<agent::timers::Timers>,
    /// The start number of this open (19.1, `host-info`).
    generation: u64,
    /// The live `claude` runs (19.3).
    sessions: Arc<agent::claude::Sessions>,
    /// A preview only: the program run in place of `claude` (19.9).
    fake_claude: Mutex<Option<PathBuf>>,
}

impl Host {
    pub fn open(path: &Path, mode: Mode, limits: Limits) -> Result<Arc<Self>, HostError> {
        let home = home::Home::open(path, mode)?;
        // Step 11 (3.1 step 3): the groups that a crashed host left, and `runs/`.
        agent::claude::run::clean_runs(&home.path);
        let engine = Arc::new(runtime::engine()?);
        let catalog = catalog::Catalog::open(&home.path, &engine)?;
        let state = state::State::open(&home.path)?;
        let tools = tools::Tools::open(&home.path)?;
        let generation = state.next_generation()?;
        let host = Arc::new_cyclic(|weak| Self {
            weak: std::sync::Weak::<Host>::clone(weak),
            events: runtime::call::Deliveries::default(),
            fakes: Mutex::new(None),
            requests: Mutex::new(manage::Requests::default()),
            manage_serial: std::sync::atomic::AtomicUsize::new(0),
            fake_serial: std::sync::atomic::AtomicUsize::new(0),
            hub: agent::streams::Hub::new(),
            timers: agent::timers::Timers::new(),
            generation,
            sessions: Arc::default(),
            fake_claude: Mutex::new(None),
            tools: Mutex::new(tools),
            bus: bus::PageBus::new(1024),
            state,
            call_locks: Mutex::new(std::collections::BTreeMap::new()),
            chains: Mutex::new(Vec::new()),
            removes: Mutex::new(()),
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
        host.hub.start(&host)?;
        host.timers.start(&host)?;
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
        self.call_connection(caller, local, function, args, None)
            .await
    }
    /// `call_async` of a page call that names its `/events` connection (8.2).
    pub(crate) async fn call_connection(
        self: &Arc<Self>,
        caller: Caller,
        local: LocalName,
        function: String,
        args: serde_json::Value,
        connection: Option<String>,
    ) -> Result<serde_json::Value, CallError> {
        let host = Arc::clone(self);
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::Builder::new()
            .name("watane-call".into())
            .stack_size(16 << 20)
            .spawn(move || {
                let result = runtime::call::run_connection(
                    &host, &caller, &local, &function, args, true, false, connection,
                )
                .map(|outcome| outcome.value);
                drop(host);
                let _ = tx.send(result);
            })
            .map_err(CallError::internal)?;
        rx.await.map_err(CallError::internal)?
    }

    /// A preview only (else `bad_request`): the program that every `claude-code` and
    /// `claude-code-shell` start runs in place of `claude` (19.9).
    pub fn set_fake_claude(&self, program: &Path) -> Result<(), HostError> {
        if !self.is_preview() {
            return Err(HostError::BadRequest(
                "a fake claude can be set in a preview only".into(),
            ));
        }
        *self
            .fake_claude
            .lock()
            .expect("lock poisoned: another thread panicked while holding it") =
            Some(program.to_path_buf());
        Ok(())
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
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .plan_with(
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
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .validate(&plan, self.home())?;
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
                requester: requester.clone(),
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
        let mut report = Installed {
            requester,
            new: vec![],
            updated: vec![],
        };
        for entry in plan.entries() {
            match entry.change {
                Change::New => report.new.push(entry.local.clone()),
                Change::Update { .. } => report.updated.push(entry.local.clone()),
                Change::Unchanged => {}
            }
        }
        let installed = self.commit_plan(
            plan,
            &withhold,
            self.port.load(std::sync::atomic::Ordering::Acquire),
        )?;
        if (!report.new.is_empty() || !report.updated.is_empty())
            && let Mode::Home { app: Some(app) } = &self.home.mode
        {
            app.installed(&report);
        }
        Ok(installed)
    }
    fn commit_plan(
        &self,
        plan: Plan,
        withhold: &[(LocalName, watane_pkg::Grant)],
        port: u16,
    ) -> Result<Vec<InstalledMod>, HostError> {
        let _removes = self
            .removes
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let updates: Vec<_> = plan
            .entries()
            .iter()
            .filter(|e| matches!(e.change, Change::Update { .. }))
            .map(|e| e.local.clone())
            .collect();
        let updated: Vec<String> = updates
            .iter()
            .filter_map(|l| self.get(l).map(|m| m.identity))
            .collect();
        let installed = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .commit(self.home(), plan, withhold, self.is_preview())?;
        for identity in updated {
            // An update of either side ends its streams (19.2, Ends).
            self.hub.end_mod(&identity);
        }
        // A firing that found its mod updated kept its row, and a mod whose error cleared gets
        // its timers back: arm every row again (19.6).
        self.timers.reload(self);
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
        let mut mods = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .clone();
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
        let mut catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
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
        // A disabled mod's timers waited (19.6).
        self.timers.reload(self);
        Ok(())
    }
    pub fn disable(&self, local: &LocalName) -> Result<(), HostError> {
        let identity = self.get(local).map(|m| m.identity);
        self.change_mod(local, |m| {
            m.enabled = false;
            Ok(())
        })?;
        if let Some(identity) = identity {
            self.hub.end_mod(&identity);
        }
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
        // An install waits for the whole removal (4.4).
        let _removes = self
            .removes
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        // Lock the mod before the catalog; a running call may read the catalog.
        let identity = self
            .get(local)
            .ok_or_else(|| HostError::NoSuchMod(local.to_string()))?
            .identity;
        let lock = Arc::clone(
            self.call_locks
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .entry(identity.clone())
                .or_default(),
        );
        let _call = lock
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let mut catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
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
        next.bump_install(local);
        for m in &mut next.mods {
            m.binds.retain(|b| b.to != identity);
        }
        next.relink(self.home(), self.is_preview())?;
        next.save(self.home())?;
        *catalog = next;
        drop(catalog);
        // What protects authority runs first and does not depend on the folder: the streams
        // end, the timers that carry the removed mod are rewritten (4.4, 19.6). A failure is
        // reported at the end; the folder goes last.
        self.hub.end_mod(&identity);
        // Its scratch workspaces go with it (19.3, Folders; 4.4).
        let scratch = self
            .state
            .scratch_removed(&identity)
            .map_err(|e| HostError::Internal(e.message))
            .map(|ids| {
                for id in ids {
                    let _ = std::fs::remove_dir_all(self.home().join("scratch").join(id));
                }
            });
        let owners = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .map(|m| (state::mod_id(&m.identity), m.identity.clone()))
            .collect();
        let rewritten = self
            .state
            .timers_removed(&identity, &owners)
            .map_err(|e| HostError::Internal(e.message));
        let data = if delete_data {
            self.state.delete(&identity)
        } else {
            Ok(())
        };
        self.bus.send(local, bus::CLOSE);
        self.catalog_written();
        let folder = std::fs::remove_dir_all(self.home().join("mods").join(local.as_ref()))
            .map_err(HostError::from);
        folder.and(data).and(rewritten).and(scratch)
    }
}
impl Drop for Host {
    /// The host closes: no `claude` outlives it.
    fn drop(&mut self) {
        self.sessions.stop_all();
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

/// The consumer of a test stream (TECHNICAL 19.9).
#[cfg(feature = "test-streams")]
#[derive(Debug, Clone, PartialEq)]
pub enum TestConsumer {
    Mod(LocalName),
    Held { origin: String },
}
/// The writer's counters since open (TECHNICAL 19.9).
#[cfg(feature = "test-streams")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestStats {
    pub commits: u64,
    pub commit_time: std::time::Duration,
    pub state_bytes_read: u64,
    pub state_bytes_written: u64,
}
/// A host stream opened by `Host::test_stream` (TECHNICAL 19.9).
#[cfg(feature = "test-streams")]
#[derive(Debug)]
pub struct TestStream {
    id: String,
    host: std::sync::Weak<Host>,
}
#[cfg(feature = "test-streams")]
impl Host {
    pub fn test_stream(&self, consumer: TestConsumer) -> Result<TestStream, StreamError> {
        let (consumer, origin) = match consumer {
            TestConsumer::Mod(local) => {
                let catalog = self
                    .catalog
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it");
                let m = catalog
                    .mods
                    .iter()
                    .find(|m| m.local == local && m.enabled && m.error.is_none())
                    .ok_or(StreamError::NotYours)?;
                if !catalog.components.get(&local).is_some_and(|c| {
                    c.host_exports()
                        .iter()
                        .any(|e| e == agent::streams::STREAM_SINK)
                }) {
                    return Err(StreamError::NotYours);
                }
                let m = agent::ModRef {
                    local,
                    identity: m.identity.clone(),
                };
                let origin = m.identity.clone();
                (agent::streams::Consumer::Mod(m), origin)
            }
            TestConsumer::Held { origin } => (
                agent::streams::Consumer::Held {
                    origin: origin.clone(),
                },
                origin,
            ),
        };
        let id = self.hub.test_open(consumer, origin)?;
        Ok(TestStream {
            id,
            host: std::sync::Weak::<Host>::clone(&self.weak),
        })
    }
    pub fn set_test_clock(&self, clock: TestClock) {
        self.hub.set_clock(&clock);
    }
    pub fn test_stats(&self) -> TestStats {
        use std::sync::atomic::Ordering::Relaxed;
        let s = &self.state.stats;
        TestStats {
            commits: s.commits.load(Relaxed),
            commit_time: std::time::Duration::from_nanos(s.commit_nanos.load(Relaxed)),
            state_bytes_read: s.bytes_read.load(Relaxed),
            state_bytes_written: s.bytes_written.load(Relaxed),
        }
    }
}
#[cfg(feature = "test-streams")]
impl TestStream {
    fn hub(&self) -> Option<Arc<Host>> {
        self.host.upgrade()
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn push(&self, body: &[u8]) -> Result<(), StreamError> {
        let host = self.hub().ok_or(StreamError::UnknownStream)?;
        host.hub.test_push(&self.id, body)
    }
    pub fn paused(&self) -> bool {
        self.hub().is_some_and(|h| h.hub.test_paused(&self.id))
    }
    pub fn close(&self, outcome: &str) {
        let value = serde_json::from_str(outcome)
            .unwrap_or_else(|_| serde_json::Value::String(outcome.to_owned()));
        if let Some(host) = self.hub() {
            host.hub.test_close(&self.id, value);
        }
    }
    pub fn ended(&self) -> Option<serde_json::Value> {
        self.hub().and_then(|h| h.hub.test_ended(&self.id))
    }
    pub fn take(&self) -> Vec<(u64, Vec<u8>)> {
        self.hub()
            .map(|h| h.hub.test_take(&self.id))
            .unwrap_or_default()
    }
    pub fn deliveries(&self) -> Vec<Delivery> {
        self.hub()
            .map(|h| h.hub.test_deliveries(&self.id))
            .unwrap_or_default()
    }
}
