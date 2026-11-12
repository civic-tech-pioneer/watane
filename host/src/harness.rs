use crate::{CallError, Caller, Host, HostError, Limits, LocalName};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use watane_pkg::InterfaceName;

/// Interface without version -> function -> JSON result.
pub type Fakes = BTreeMap<String, BTreeMap<String, Value>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub value: Value,
    pub events: Vec<EventRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord {
    pub interface: InterfaceName,
    pub function: String,
    pub args: Value,
}

/// One built mod in a fresh preview home, without a server.
#[derive(Debug)]
pub struct Harness {
    // Fields drop in this order: close the database and lock before removal.
    host: Arc<Host>,
    home: TemporaryHome,
    local: LocalName,
}

impl Harness {
    /// Installs the built folder alone, using fakes for call imports.
    pub fn open(built_mod: &Path, fakes: Fakes) -> Result<Self, HostError> {
        let home = TemporaryHome::new()?;
        let host = Host::open(&home.0, crate::Mode::Preview, Limits::default())?;
        *host.fakes.lock().unwrap() = Some(fakes);
        let plan = host.catalog.lock().unwrap().plan(
            host.home(),
            &[crate::Source::Folder(built_mod.to_path_buf())],
            &host.engine,
            true,
            true,
        )?;
        let installed = host
            .catalog
            .lock()
            .unwrap()
            .commit(host.home(), plan, &[], true)?;
        let local = installed
            .into_iter()
            .next()
            .ok_or_else(|| HostError::Internal("harness install returned no mod".into()))?
            .local;
        Ok(Self { host, home, local })
    }

    /// Calls an export as the page, with a 16 MiB stack on any calling thread.
    pub fn call(&self, function: &str, args: Value) -> Result<Outcome, CallError> {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("watane-harness-call".into())
                .stack_size(16 << 20)
                .spawn_scoped(scope, || {
                    self.host
                        .call_outcome(&Caller::Page, &self.local, function, args, false)
                })
                .map_err(CallError::internal)?
                .join()
                .map_err(|_| CallError::internal("harness call thread panicked"))?
        })
    }

    /// Runs only a subscription export, as its provider slug.
    pub fn deliver(&self, function: &str, args: Value) -> Result<Outcome, CallError> {
        if function.contains(':') || function.contains('/') {
            return Err(CallError::new(
                crate::CallCode::NoFunction,
                format!("no subscription function {function}"),
            ));
        }
        let provider = {
            let catalog = self.host.catalog.lock().unwrap();
            let compiled = catalog.components.get(&self.local).ok_or_else(|| {
                CallError::new(crate::CallCode::NoFunction, "mod has no logic part")
            })?;
            compiled.subscription_provider(function)?
        };
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("watane-harness-deliver".into())
                .stack_size(16 << 20)
                .spawn_scoped(scope, || {
                    crate::runtime::call::run(
                        &self.host,
                        &Caller::Mod(provider),
                        &self.local,
                        function,
                        args,
                        false,
                        true,
                    )
                })
                .map_err(CallError::internal)?
                .join()
                .map_err(|_| CallError::internal("harness delivery thread panicked"))?
        })
    }

    #[doc(hidden)]
    pub fn home(&self) -> &Path {
        &self.home.0
    }
}

#[derive(Debug)]
struct TemporaryHome(PathBuf);

impl TemporaryHome {
    fn new() -> Result<Self, HostError> {
        loop {
            let mut random = [0; 16];
            getrandom::fill(&mut random).map_err(|error| HostError::Internal(error.to_string()))?;
            let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
            let path = std::env::temp_dir().join(format!("watane-harness-{name}"));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for TemporaryHome {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
