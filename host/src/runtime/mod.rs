//! The runtime (TECHNICAL 6): the engine, and compiling a mod with the host's
//! own import check (TECHNICAL 5.3), which runs even when pkg already checked.

pub(crate) mod call;
mod json;
pub(crate) mod limits;
mod linker;
use crate::{CallCode, CallError, HostError};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use wasmtime::component::types::{ComponentFunc, ComponentItem};
use wasmtime::component::{Component, InstancePre};
use wasmtime::{Config, Engine};
use watane_pkg::Mod;

pub(crate) struct Ctx {
    wasi: wasmtime_wasi::WasiCtx,
    table: wasmtime::component::ResourceTable,
    limiter: limits::Limiter,
    state: crate::state::Transaction,
    caller: String,
    stderr: wasmtime_wasi::p2::pipe::MemoryOutputPipe,
    deadline: Instant,
    outbox: Vec<crate::bus::Emission>,
    manages: Vec<crate::manage::Effect>,
    invocation: call::Invocation,
}
#[derive(Clone, Debug)]
struct Function {
    interface: String,
    name: String,
    ty: ComponentFunc,
    docs: Option<String>,
    subscription: bool,
}
pub(crate) struct Compiled {
    pre: InstancePre<Ctx>,
    functions: BTreeMap<String, Function>,
}
impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Compiled")
            .field("functions", &self.functions)
            .finish_non_exhaustive()
    }
}
pub(crate) fn engine() -> Result<Engine, HostError> {
    let mut config = Config::new();
    config
        .consume_fuel(true)
        .epoch_interruption(true)
        .wasm_component_model(true)
        // ResourceLimiter does not cover shared linear memories.
        .wasm_threads(false)
        .max_wasm_stack(1 << 20);
    Engine::new(&config).map_err(|e| HostError::Internal(e.to_string()))
}
pub(crate) fn compile(engine: &Engine, m: &Mod) -> Result<Option<Arc<Compiled>>, HostError> {
    let invalid = |reason: String| HostError::InvalidMod {
        input: m.dir().display().to_string(),
        reason,
    };
    m.check_component().map_err(|e| invalid(e.to_string()))?;
    let Some(path) = m.logic() else {
        return Ok(None);
    };
    let component = Component::from_file(engine, path).map_err(|e| invalid(format!("{e:#}")))?;
    let ty = component.component_type();
    // Independently check the compiled names at the host's security boundary.
    for (name, item) in ty.imports(engine) {
        let base = name
            .parse::<watane_pkg::InterfaceName>()
            .ok()
            .is_some_and(|n| {
                n.namespace() == "wasi"
                    && matches!(
                        n.package(),
                        "cli" | "clocks" | "filesystem" | "io" | "random" | "sockets"
                    )
                    && n.version().major == 0
                    && n.version().minor == 2
            });
        let allowed = base
            || name == "watane:host/caller@0.1.0"
            || (name == "watane:host/state@0.1.0" && m.world().storage)
            || (name == crate::manage::MANAGE && m.world().manage)
            || m.world()
                .events
                .iter()
                .chain(&m.world().calls)
                .any(|n| n.to_string() == name);
        if !allowed || !matches!(item.ty, ComponentItem::ComponentInstance(_)) {
            return Err(invalid(format!("undeclared component import {name}")));
        }
    }
    for (name, _) in ty.exports(engine) {
        if !m
            .world()
            .exports
            .iter()
            .chain(&m.world().subscriptions)
            .any(|n| n.to_string() == name)
        {
            return Err(invalid(format!("undeclared component export {name}")));
        }
    }
    let mut docs = BTreeMap::new();
    let mut resolve = wit_parser::Resolve::default();
    resolve
        .push_dir(m.dir().join("wit"))
        .map_err(|e| invalid(e.to_string()))?;
    // pkg owns world interpretation; this reader extracts only doc comments.
    for (_, interface) in resolve.interfaces.iter() {
        let Some(package) = interface.package else {
            continue;
        };
        let Some(name) = &interface.name else {
            continue;
        };
        let package = &resolve.packages[package].name;
        let full = format!(
            "{}:{}/{}@{}",
            package.namespace,
            package.name,
            name,
            package
                .version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        );
        for (name, function) in &interface.functions {
            docs.insert(
                (full.clone(), name.clone()),
                function
                    .docs
                    .contents
                    .clone()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty()),
            );
        }
    }
    let mut functions = BTreeMap::new();
    for interface in m.world().exports.iter().chain(&m.world().subscriptions) {
        let Some(item) = ty.get_export(engine, &interface.to_string()) else {
            return Err(invalid(format!("missing export {interface}")));
        };
        let ComponentItem::ComponentInstance(instance) = item.ty else {
            return Err(invalid(format!("export {interface} is not an interface")));
        };
        for (name, item) in instance.exports(engine) {
            if let ComponentItem::ComponentFunc(ty) = item.ty
                && ty.params().all(|(_, t)| json::callable(&t))
                && ty.results().all(|t| json::callable(&t))
            {
                functions.insert(
                    if m.world().subscriptions.contains(interface) {
                        format!("{interface}.{name}")
                    } else {
                        format!("{}.{name}", interface.interface())
                    },
                    Function {
                        subscription: m.world().subscriptions.contains(interface),
                        interface: interface.to_string(),
                        name: name.into(),
                        ty,
                        docs: docs
                            .get(&(interface.to_string(), name.into()))
                            .cloned()
                            .flatten(),
                    },
                );
            }
        }
    }
    let pre = linker::linker(engine, &component, m)
        .and_then(|l| l.instantiate_pre(&component))
        .map_err(|e| invalid(format!("{e:#}")))?;
    Ok(Some(Arc::new(Compiled { pre, functions })))
}
impl Compiled {
    fn find(&self, name: &str, subscription: bool) -> Result<&Function, CallError> {
        if !subscription {
            return self
                .functions
                .get(name)
                .filter(|f| !f.subscription)
                .ok_or_else(|| {
                    CallError::new(CallCode::NoFunction, format!("no callable function {name}"))
                });
        }
        let mut matches = self.functions.values().filter(|f| {
            f.subscription == subscription
                && (format!("{}.{}", f.interface, f.name) == name
                    || format!(
                        "{}.{}",
                        f.interface
                            .parse::<watane_pkg::InterfaceName>()
                            .expect("checked interface")
                            .interface(),
                        f.name
                    ) == name)
        });
        let found = matches.next();
        if matches.next().is_some() {
            return Err(CallError::new(
                CallCode::NoFunction,
                format!("ambiguous subscription function {name}"),
            ));
        }
        found.ok_or_else(|| {
            CallError::new(CallCode::NoFunction, format!("no callable function {name}"))
        })
    }
    pub(crate) fn subscription_provider(&self, name: &str) -> Result<crate::LocalName, CallError> {
        let f = self.find(name, true)?;
        let interface: watane_pkg::InterfaceName =
            f.interface.parse().map_err(CallError::internal)?;
        crate::LocalName::parse(interface.package()).map_err(CallError::internal)
    }
    pub(crate) fn functions(&self) -> Vec<crate::FunctionInfo> {
        self.functions
            .values()
            .filter(|f| !f.subscription)
            .map(|f| crate::FunctionInfo {
                interface: f.interface.parse().expect("pkg checked interface"),
                name: f.name.clone(),
                params: f
                    .ty
                    .params()
                    .map(|(n, t)| (n.into(), json::type_text(&t)))
                    .collect(),
                result: f.ty.results().next().map(|t| json::type_text(&t)),
                docs: f.docs.clone(),
            })
            .collect()
    }
    pub(crate) fn schema(&self, function: &str) -> serde_json::Value {
        json::object_schema(self.functions[function].ty.params())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ac6_wall_time_bounds_waiting_for_the_mod_lock() {
        use crate::{CallCode, Caller, Host, Limit, Limits, LocalName};
        use std::sync::{Arc, Mutex, mpsc};
        use std::time::Duration;
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(
            home.path(),
            crate::Mode::Preview,
            Limits {
                time: Duration::from_millis(50),
                ..Limits::default()
            },
        )
        .unwrap();
        host.install_folders(
            &[std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/hog")],
            0,
        )
        .unwrap();
        let lock = Arc::new(Mutex::new(()));
        host.call_locks
            .lock()
            .unwrap()
            .insert("local/hog".into(), Arc::clone(&lock));
        let guard = lock
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || {
                tx.send(host.call(
                    &Caller::Page,
                    &LocalName::parse("hog").unwrap(),
                    "api.ping",
                    serde_json::Value::Null,
                ))
                .unwrap();
            })
            .unwrap();
        let result = rx.recv_timeout(Duration::from_millis(500));
        drop(guard);
        thread.join().unwrap();
        assert_eq!(
            result
                .expect("call must stop waiting at its deadline")
                .unwrap_err()
                .code,
            CallCode::Limit(Limit::Time)
        );
    }
}
