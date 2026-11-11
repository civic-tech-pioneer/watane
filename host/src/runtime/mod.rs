mod json;
pub(crate) mod limits;
mod linker;
use crate::{CallCode, CallError, HostError};
use std::{collections::BTreeMap, sync::Arc, time::Instant};
use wasmtime::{
    Config, Engine,
    component::{
        Component, InstancePre,
        types::{ComponentFunc, ComponentItem},
    },
};
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
}
#[derive(Clone, Debug)]
struct Function {
    interface: String,
    name: String,
    ty: ComponentFunc,
    docs: Option<String>,
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
            || m.world().events.iter().any(|n| n.to_string() == name);
        if !allowed || !matches!(item.ty, ComponentItem::ComponentInstance(_)) {
            return Err(invalid(format!("undeclared component import {name}")));
        }
    }
    for (name, _) in ty.exports(engine) {
        if !m.world().exports.iter().any(|n| n.to_string() == name) {
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
    for interface in &m.world().exports {
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
                    format!("{}.{name}", interface.interface()),
                    Function {
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
    pub(crate) fn functions(&self) -> Vec<crate::FunctionInfo> {
        self.functions
            .values()
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
    pub fn call(
        &self,
        host: &crate::Host,
        page: Option<&crate::LocalName>,
        identity: &str,
        caller: &crate::Caller,
        function: &str,
        args: serde_json::Value,
    ) -> Result<crate::Outcome, CallError> {
        let function = self.functions.get(function).ok_or_else(|| {
            CallError::new(
                CallCode::NoFunction,
                format!("no callable function {function}"),
            )
        })?;
        let params = json::arguments(&function.ty, args)?;
        let deadline = Instant::now()
            .checked_add(host.limits.time)
            .ok_or_else(|| CallError::internal("time limit is too large"))?;
        let lock = host
            .call_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(identity.into())
            .or_default()
            .clone();
        let guard = loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(CallError::limit(
                    crate::Limit::Time,
                    "call exceeded wall time while waiting for its mod",
                ));
            }
            match lock.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep((deadline - now).min(std::time::Duration::from_millis(10)))
                }
            }
        };
        let transaction = host.state.transaction(identity, &host.limits)?;
        let mut store = linker::store(
            &host.engine,
            transaction,
            caller.who(),
            &host.limits,
            deadline,
        )?;
        let stderr = store.data().stderr.clone();
        let result = (|| -> wasmtime::Result<serde_json::Value> {
            let instance = self.pre.instantiate(&mut store)?;
            let (_, interface) = instance
                .get_export(&mut store, None, &function.interface)
                .ok_or_else(|| CallError::internal("compiled interface disappeared"))?;
            let (_, export) = instance
                .get_export(&mut store, Some(&interface), &function.name)
                .ok_or_else(|| CallError::internal("compiled function disappeared"))?;
            let func = instance
                .get_func(&mut store, export)
                .ok_or_else(|| CallError::internal("compiled export is not a function"))?;
            let mut results =
                vec![wasmtime::component::Val::Bool(false); function.ty.results().len()];
            func.call(&mut store, &params, &mut results)?;
            // Wasmtime 48 runs canonical post-return as part of call.
            if Instant::now() >= store.data().deadline {
                return Err(CallError::limit(crate::Limit::Time, "call exceeded wall time").into());
            }
            Ok(match results.first() {
                Some(value) => json::encode(value)?,
                None => serde_json::Value::Null,
            })
        })();
        let value = result.map_err(|e| limits::classify(e, &stderr))?;
        let ctx = store.into_data();
        let events = ctx
            .outbox
            .into_iter()
            .map(|event| {
                Ok(crate::EventRecord {
                    interface: event.interface.parse().map_err(CallError::internal)?,
                    function: event.function,
                    args: event.args,
                })
            })
            .collect::<Result<Vec<_>, CallError>>()?;
        let changed = ctx.state.commit()?;
        if let Some(local) = page {
            let publication = host.bus.publication();
            drop(guard);
            crate::bus::deliver(&publication, local, &events, changed);
        }
        Ok(crate::Outcome { value, events })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ac6_wall_time_bounds_waiting_for_the_mod_lock() {
        use crate::{CallCode, Caller, Host, Limit, Limits, LocalName};
        use std::{
            sync::{Arc, Mutex, mpsc},
            time::Duration,
        };
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
            .insert("local/hog".into(), lock.clone());
        let guard = lock.lock().unwrap();
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
