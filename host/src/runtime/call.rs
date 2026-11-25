//! The call algorithm (TECHNICAL 6.6) and event delivery (TECHNICAL 9.2): runs
//! a top-level call and its chain, then delivers the events it emitted.

use super::{json, limits};
use crate::agent::Effect;
use crate::agent::tools::{AgentFailure, AgentRun, Denied};
use crate::{CallCode, CallError, Caller, EventRecord, Host, Limit, LinkKind, LocalName, Outcome};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Instant;
use watane_pkg::{Grant, InterfaceName};

#[derive(Default)]
struct Effects {
    events: Vec<(LocalName, EventRecord, u8)>,
    manages: Vec<crate::manage::Effect>,
}
pub(crate) struct Chain {
    pub(crate) host: Arc<Host>,
    pub(crate) snapshot: Arc<crate::catalog::Catalog>,
    pub(crate) fakes: Option<crate::Fakes>,
    deadline: Instant,
    effects: Mutex<Effects>,
    publish: bool,
    ready: AtomicBool,
    job: Arc<Job>,
    // Lowest fuel left by an invoke that failed in the current answer of a
    // collect; the collect resets it before each answer (9.4).
    failed_fuel: AtomicU64,
    /// The requests and stream effects of the chain in call order (19.6). An invoke that fails
    /// cuts the log back to where it was when the invoke began.
    pub(crate) log: Mutex<Vec<Effect>>,
    /// The live `/events` connection that a page call named (8.2).
    pub(crate) connection: Option<String>,
    /// The top-level call is an `items` call: a `defer` answer rolls it back (19.2).
    defers: bool,
    deferred: AtomicBool,
    /// The top-level call is the firing of this timer row (19.6).
    timer: Option<(String, u64)>,
    /// The top-level call failed because its own mod is disabled, removed or updated.
    unavailable: AtomicBool,
    /// The top-level call is an agent's tool: its authority is decided at the run (19.5), and the
    /// chain runs on borrowed authority (19.5, Borrowed authority).
    pub(crate) agent: Option<AgentRun>,
    /// Why the host did not start that tool.
    denied: Mutex<Option<Denied>>,
}
#[derive(Clone)]
pub(crate) struct Invocation {
    pub(crate) chain: Arc<Chain>,
    pub(crate) local: LocalName,
    pub(crate) identity: String,
    pub(crate) depth: u32,
    pub(crate) hop: u8,
    /// The call came through a call link, a top-level `Host::call` of a mod, or a page (19.2).
    pub(crate) link: bool,
    /// The origin client of the call (19, Terms): an identity.
    pub(crate) origin: String,
    /// The install generation of the origin client (19, Terms): an identity alone does not tell
    /// two installs apart. `None` only for a firing, which takes the current one at its call.
    pub(crate) origin_gen: Option<u64>,
    /// The consumer's `Call` link interface that this call came through, if it came through one
    /// (19.2, Ends: the grant a mod stream depends on).
    pub(crate) via: Option<InterfaceName>,
}

/// The caller of a call: a `Caller`, or the host itself (19.1).
#[derive(Clone, Debug)]
pub(crate) enum Who {
    Caller(Caller),
    Host,
}
impl Who {
    pub(crate) fn who(&self) -> String {
        match self {
            Self::Caller(c) => c.who(),
            Self::Host => "host".into(),
        }
    }
}
/// The arguments of a call: JSON, or WIT values that the host built (items, 19.2).
pub(crate) enum Args {
    Json(Value),
    Vals(Vec<wasmtime::component::Val>),
}
/// A top-level call.
pub(crate) struct Start {
    pub who: Who,
    pub local: LocalName,
    pub function: String,
    pub args: Args,
    pub publish: bool,
    pub subscription: bool,
    /// A host-only export (`stream-sink`, `stream-producer`, `timer-sink`).
    pub internal: bool,
    pub origin: Option<String>,
    /// The install generation that the origin client had when the chain's first effect named it
    /// (a stream's): `None` takes the generation of the snapshot.
    pub origin_gen: Option<u64>,
    pub connection: Option<String>,
    pub hop: u8,
    pub defers: bool,
    pub timer: Option<(String, u64)>,
    /// An agent's tool (19.5, Authority at the run).
    pub agent: Option<AgentRun>,
}
impl Start {
    /// A call of the host's own: an item call, a notification or a firing.
    pub fn internal(
        who: Who,
        local: LocalName,
        function: String,
        args: Args,
        origin: String,
    ) -> Self {
        Self {
            who,
            local,
            function,
            args,
            publish: true,
            subscription: false,
            internal: true,
            origin: Some(origin),
            origin_gen: None,
            connection: None,
            hop: 0,
            defers: false,
            timer: None,
            agent: None,
        }
    }
}
pub(crate) struct Done {
    pub result: Result<Value, CallError>,
    pub deferred: bool,
    /// The call's own mod was disabled, removed or updated (a firing keeps its row, 19.6).
    pub unavailable: bool,
    pub outcome: Option<Outcome>,
    /// The host did not start an agent's tool (19.5).
    pub denied: Option<Denied>,
}
const DEFERRED: &str = "the consumer deferred the batch";
const IGNORED: &str = "the timer was cancelled or replaced while its firing waited";

pub(super) fn lock_until<T>(
    lock: &Mutex<T>,
    deadline: Instant,
) -> Result<MutexGuard<'_, T>, CallError> {
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(CallError::limit(
                Limit::Time,
                "call exceeded wall time while waiting for its mod",
            ));
        }
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                panic!("mod lock poisoned: another thread panicked while holding it")
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep((deadline - now).min(std::time::Duration::from_millis(1)))
            }
        }
    }
}
fn deadline(host: &Host) -> Result<Instant, CallError> {
    Instant::now()
        .checked_add(host.limits.time)
        .ok_or_else(|| CallError::internal("time limit is too large"))
}

pub(crate) fn run(
    host: &Host,
    caller: &Caller,
    local: &LocalName,
    function: &str,
    args: Value,
    publish: bool,
    subscription: bool,
) -> Result<Outcome, CallError> {
    run_connection(
        host,
        caller,
        local,
        function,
        args,
        publish,
        subscription,
        None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "one entry point for every top-level call"
)]
pub(crate) fn run_connection(
    host: &Host,
    caller: &Caller,
    local: &LocalName,
    function: &str,
    args: Value,
    publish: bool,
    subscription: bool,
    connection: Option<String>,
) -> Result<Outcome, CallError> {
    let done = run_start(
        host,
        Start {
            who: Who::Caller(caller.clone()),
            local: local.clone(),
            function: function.to_owned(),
            args: Args::Json(args),
            publish,
            subscription,
            internal: false,
            origin: None,
            origin_gen: None,
            connection,
            hop: 0,
            defers: false,
            timer: None,
            agent: None,
        },
    );
    done.result.map(|value| {
        let mut outcome = done.outcome.expect("a succeeded call has an outcome");
        outcome.value = value;
        outcome
    })
}

/// An agent's tool as a top-level call (19.5): the handle's origin client is its origin client,
/// and the host decides just before it starts whether it may run (Authority at the run).
pub(crate) fn run_agent(
    host: &Host,
    run: &AgentRun,
    handle: crate::Handle,
    local: &LocalName,
    function: &str,
    args: Value,
) -> Result<Value, AgentFailure> {
    let done = run_start(
        host,
        Start {
            who: Who::Caller(Caller::Agent(handle)),
            local: local.clone(),
            function: function.to_owned(),
            args: Args::Json(args),
            publish: true,
            subscription: false,
            internal: false,
            origin: None,
            origin_gen: None,
            connection: None,
            hop: 0,
            defers: false,
            timer: None,
            agent: Some(run.clone()),
        },
    );
    match (done.denied, done.result) {
        (Some(d), _) => Err(AgentFailure::Denied(d)),
        (None, Ok(v)) => Ok(v),
        // The tool's mod was removed, disabled or updated before the call started: the tool is
        // not in the handle's tools now (19.5, Authority at the run).
        (None, Err(_)) if done.unavailable => Err(AgentFailure::Denied(Denied::UnknownTool)),
        (None, Err(e)) => Err(AgentFailure::Call(e)),
    }
}

fn identity_of(snapshot: &crate::catalog::Catalog, local: &LocalName) -> String {
    snapshot
        .mods
        .iter()
        .find(|m| &m.local == local)
        .map_or_else(|| format!("local/{local}"), |m| m.identity.clone())
}

/// Puts a chain in flight: its queued effects count for the limits of every call (19.2, Commit).
fn register(host: &Host, chain: &Arc<Chain>) {
    let mut chains = host
        .chains
        .lock()
        .expect("lock poisoned: another thread panicked while holding it");
    chains.retain(|c| c.strong_count() > 0);
    chains.push(Arc::downgrade(chain));
}
/// Takes the chain's queued effects, under the gate of 19.2 (Commit), and applies them when the
/// chain succeeded; else they are given back (a harness) or dropped. Either way the chain's
/// reservation ends.
fn take_and_apply(host: &Host, chain: &Chain, apply: bool) -> Vec<Effect> {
    let log = {
        let _gate = host
            .chains
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let log = std::mem::take(
            &mut *chain
                .log
                .lock()
                .expect("lock poisoned: another thread panicked while holding it"),
        );
        if apply {
            host.apply_effects(log);
            Vec::new()
        } else {
            log
        }
    };
    // Its pending bytes are no longer held: a producer refused for them may fit now.
    host.chain_ended();
    log
}

/// Ends a chain of `test_chain` as a failed call does: its effects are dropped.
#[cfg(test)]
pub(crate) fn test_rollback(host: &Host, chain: &Chain) {
    take_and_apply(host, chain, false);
}

/// A chain in flight that holds `effects` and commits nothing, for the tests of the budgets.
#[cfg(test)]
pub(crate) fn test_chain(host: &Host, effects: Vec<Effect>) -> Arc<Chain> {
    let chain = Arc::new(Chain {
        host: host.weak.upgrade().expect("host open"),
        snapshot: Arc::new(
            host.catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .clone(),
        ),
        fakes: None,
        deadline: Instant::now() + std::time::Duration::from_secs(60),
        effects: Mutex::new(Effects::default()),
        publish: false,
        ready: AtomicBool::new(false),
        job: Arc::new(Job::new()),
        failed_fuel: AtomicU64::new(0),
        log: Mutex::new(effects),
        connection: None,
        defers: false,
        deferred: AtomicBool::new(false),
        timer: None,
        unavailable: AtomicBool::new(false),
        agent: None,
        denied: Mutex::new(None),
    });
    register(host, &chain);
    chain
}

pub(crate) fn run_start(host: &Host, start: Start) -> Done {
    let failed = |error: CallError| Done {
        result: Err(error),
        deferred: false,
        unavailable: false,
        outcome: None,
        denied: None,
    };
    let deadline = match deadline(host) {
        Ok(d) => d,
        Err(e) => return failed(e),
    };
    let snapshot = Arc::new(
        host.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone(),
    );
    let Some(strong) = host.weak.upgrade() else {
        return failed(CallError::internal("host closed"));
    };
    let Start {
        who,
        local,
        function,
        args,
        publish,
        subscription,
        internal,
        origin,
        origin_gen,
        connection,
        hop,
        defers,
        timer,
        agent,
    } = start;
    let origin = origin.unwrap_or_else(|| match &who {
        Who::Caller(Caller::Mod(m)) if !subscription => identity_of(&snapshot, m),
        _ => identity_of(&snapshot, &local),
    });
    // A firing takes the generation of its row's origin at its call, after the lock.
    let origin_gen = origin_gen.or_else(|| {
        if timer.is_some() {
            return None;
        }
        snapshot
            .mods
            .iter()
            .find(|m| m.identity == origin)
            .and_then(|m| snapshot.install_of(&m.local))
    });
    let connection = connection
        .filter(|c| matches!(who, Who::Caller(Caller::Page)) && host.hub.conn_live(c, &local));
    let link =
        !subscription && !internal && matches!(who, Who::Caller(Caller::Mod(_) | Caller::Page));
    let chain = Arc::new(Chain {
        host: strong,
        snapshot: Arc::clone(&snapshot),
        fakes: host
            .fakes
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone(),
        deadline,
        effects: Mutex::new(Effects::default()),
        publish,
        ready: AtomicBool::new(false),
        job: Arc::new(Job::new()),
        failed_fuel: AtomicU64::new(0),
        log: Mutex::new(Vec::new()),
        connection,
        defers,
        deferred: AtomicBool::new(false),
        timer,
        unavailable: AtomicBool::new(false),
        agent,
        denied: Mutex::new(None),
    });
    register(host, &chain);
    let invocation = Invocation {
        chain: Arc::clone(&chain),
        identity: identity_of(&snapshot, &local),
        local,
        depth: 0,
        hop,
        link,
        origin,
        origin_gen,
        via: None,
    };
    let result = invocation.invoke(
        &who,
        &function,
        args,
        host.limits.fuel,
        subscription,
        internal,
    );
    let effects = std::mem::take(
        &mut *chain
            .effects
            .lock()
            .expect("lock poisoned: another thread panicked while holding it"),
    );
    let log = take_and_apply(host, &chain, chain.fakes.is_none() && result.is_ok());
    let deferred = chain.deferred.load(Ordering::Acquire);
    let events = effects
        .events
        .iter()
        .map(|(_, event, _)| event.clone())
        .collect();
    let manages = effects.manages;
    let mut requests = Vec::new();
    let records = if chain.fakes.is_some() {
        if result.is_ok() {
            let manage_requests = manages
                .iter()
                .filter(|e| matches!(e.action, crate::manage::Action::Fake { request: true }))
                .count();
            host.manage_serial
                .fetch_add(manage_requests, Ordering::AcqRel);
            requests = crate::agent::records(&log);
        }
        manages.iter().map(|e| e.record.clone()).collect()
    } else {
        host.apply_manage(manages);
        Vec::new()
    };
    if publish {
        host.events.finish(host, &chain);
    }
    #[cfg(feature = "test-streams")]
    if matches!(who, Who::Caller(Caller::Agent(_))) {
        host.agent_gate.finish();
    }
    match result {
        Ok((value, _)) => Done {
            outcome: Some(Outcome {
                value: Value::Null,
                events,
                manages: records,
                requests,
            }),
            result: Ok(value),
            deferred: false,
            unavailable: false,
            denied: None,
        },
        Err(error) => Done {
            result: Err(error),
            deferred,
            unavailable: chain.unavailable.load(Ordering::Acquire),
            outcome: None,
            denied: *chain
                .denied
                .lock()
                .expect("lock poisoned: another thread panicked while holding it"),
        },
    }
}

impl Invocation {
    pub(super) fn storage(&self) -> Result<(), CallError> {
        let catalog = self
            .chain
            .host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if catalog.mods.iter().any(|m| {
            m.local == self.local
                && m.grants
                    .iter()
                    .any(|g| g.granted && matches!(g.grant, Grant::Storage))
        }) {
            Ok(())
        } else {
            Err(CallError::new(
                CallCode::NotGranted,
                format!("{} has no grant for watane:host/state", self.local),
            ))
        }
    }
    /// The mod of this invocation holds the grant, else `not_granted` (the stub of W9).
    pub(super) fn granted(&self, grant: &Grant, interface: &str) -> Result<(), CallError> {
        let catalog = self
            .chain
            .host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if catalog.mods.iter().any(|m| {
            m.local == self.local && m.grants.iter().any(|g| g.granted && &g.grant == grant)
        }) {
            Ok(())
        } else {
            Err(CallError::new(
                CallCode::NotGranted,
                format!("{} has no grant for watane:host/{interface}", self.local),
            ))
        }
    }
    /// A function of `watane:host/manage` called by this invocation's mod (6.10).
    pub(super) fn manage(
        &self,
        queued: &mut Vec<crate::manage::Effect>,
        function: &str,
        args: Value,
    ) -> Result<Value, CallError> {
        let serial = self.chain.host.manage_serial.load(Ordering::Acquire);
        let fake = self.chain.fakes.as_ref().map(|fakes| {
            (
                fakes.get("watane:host/manage").cloned().unwrap_or_default(),
                serial,
            )
        });
        let chain = self
            .chain
            .effects
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .manages
            .clone();
        self.chain.host.manage_call(
            &self.local,
            fake.as_ref().map(|(fakes, serial)| (fakes, *serial)),
            queued,
            &chain,
            function,
            args,
        )
    }
    pub(super) fn nested(
        &self,
        interface: &InterfaceName,
        function: &str,
        args: Value,
        fuel: u64,
    ) -> Result<(Value, u64), CallError> {
        let fake_key = format!(
            "{}:{}/{}",
            interface.namespace(),
            interface.package(),
            interface.interface()
        );
        if let Some(fakes) = &self.chain.fakes {
            let value = fakes
                .get(&fake_key)
                .and_then(|functions| functions.get(function))
                .cloned()
                .ok_or_else(|| {
                    CallError::new(CallCode::Trap, format!("no fake for {fake_key}.{function}"))
                })?;
            return Ok((value, fuel));
        }
        let current = self
            .chain
            .host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let consumer = current
            .mods
            .iter()
            .find(|m| m.local == self.local)
            .ok_or_else(|| CallError::new(CallCode::NotGranted, "consumer was removed"))?;
        let granted = consumer.grants.iter().any(|g| {
            g.granted && matches!(&g.grant, Grant::Call { interface: i, .. } if i == interface)
        });
        let link = consumer
            .links
            .iter()
            .find(|l| matches!(l.kind, LinkKind::Call) && &l.interface == interface);
        let provider = link.and_then(|l| l.provider.as_ref());
        if !granted || provider.is_none() {
            return Err(CallError::new(
                CallCode::NotGranted,
                format!(
                    "{} may not call {interface}: not granted or no provider is linked",
                    self.local
                ),
            ));
        }
        let provider = provider.expect("provider.is_none() returned above");
        let target = current
            .mods
            .iter()
            .find(|m| &m.local == provider)
            .filter(|m| m.enabled && m.error.is_none());
        if target.is_none() {
            return Err(CallError::new(
                CallCode::NotGranted,
                format!("provider {provider} is disabled or unavailable"),
            ));
        }
        let linked_interface = link
            .and_then(|l| l.provider_interface.as_ref())
            .ok_or_else(|| {
                CallError::new(CallCode::NotGranted, "no provider interface is linked")
            })?;
        let nested = Self {
            chain: Arc::clone(&self.chain),
            local: provider.clone(),
            identity: target.map(|m| m.identity.clone()).unwrap_or_default(),
            depth: self.depth.saturating_add(1),
            hop: self.hop,
            link: true,
            origin: self.origin.clone(),
            origin_gen: self.origin_gen,
            via: Some(interface.clone()),
        };
        nested.invoke(
            &Who::Caller(Caller::Mod(self.local.clone())),
            &format!("{}.{function}", linked_interface.interface()),
            Args::Json(args),
            fuel,
            false,
            false,
        )
    }

    /// A collect of the collector's own outlet (9.4): the joined answers.
    pub(super) fn collect(
        &self,
        interface: &InterfaceName,
        function: &str,
        args: Value,
        mut fuel: u64,
    ) -> Result<(Value, u64), CallError> {
        if let Some(fakes) = &self.chain.fakes {
            let key = format!(
                "{}:{}/{}",
                interface.namespace(),
                interface.package(),
                interface.interface()
            );
            let value = fakes
                .get(&key)
                .and_then(|functions| functions.get(function))
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new()));
            return Ok((value, fuel));
        }
        let current = self
            .chain
            .host
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        let mut subscribers: Vec<_> = current
            .mods
            .iter()
            .filter(|m| m.enabled && m.error.is_none())
            .collect();
        subscribers.sort_by(|a, b| a.local.cmp(&b.local));
        let mut answers = Vec::new();
        for m in subscribers {
            for link in m.links.iter().filter(|l| {
                matches!(l.kind, LinkKind::Subscribe)
                    && l.provider.as_ref() == Some(&self.local)
                    && l.provider_interface.as_ref() == Some(interface)
            }) {
                if !m.grants.iter().any(|g| g.granted && matches!(&g.grant, Grant::Subscribe { interface, .. } if interface == &link.interface)) {
                    continue;
                }
                let nested = Self {
                    chain: Arc::clone(&self.chain),
                    local: m.local.clone(),
                    identity: m.identity.clone(),
                    depth: self.depth.saturating_add(1),
                    hop: self.hop,
                    link: false,
                    origin: m.identity.clone(),
                    origin_gen: self.chain.snapshot.install_of(&m.local),
                    via: None,
                };
                self.chain.failed_fuel.store(fuel, Ordering::Release);
                match nested.invoke(
                    &Who::Caller(Caller::Mod(self.local.clone())),
                    &format!("{}.{function}", link.interface),
                    Args::Json(args.clone()),
                    fuel,
                    true,
                    false,
                ) {
                    Ok((Value::Array(list), left)) => {
                        fuel = left;
                        answers.extend(list);
                    }
                    Ok(_) => eprintln!(
                        "watane: collect {interface}.{function} of {} from {} failed: not a list",
                        self.local, m.local
                    ),
                    Err(error)
                        if matches!(error.code, CallCode::Limit(Limit::Time | Limit::Fuel)) =>
                    {
                        return Err(error);
                    }
                    Err(error) => {
                        // What the failed answer burned stays spent.
                        fuel = self.chain.failed_fuel.load(Ordering::Acquire).min(fuel);
                        eprintln!(
                            "watane: collect {interface}.{function} of {} from {} failed: {:?}: {}",
                            self.local, m.local, error.code, error.message
                        );
                    }
                }
            }
        }
        Ok((Value::Array(answers), fuel))
    }

    /// Runs one invoke; a failure cuts the chain's log back to where it began (19.6).
    pub(crate) fn invoke(
        &self,
        caller: &Who,
        name: &str,
        args: Args,
        fuel: u64,
        subscription: bool,
        internal: bool,
    ) -> Result<(Value, u64), CallError> {
        let mark = self
            .chain
            .log
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .len();
        let result = self.invoke_inner(caller, name, args, fuel, subscription, internal);
        if result.is_err() {
            self.chain
                .log
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .truncate(mark);
        }
        result
    }
    fn invoke_inner(
        &self,
        caller: &Who,
        name: &str,
        args: Args,
        fuel: u64,
        subscription: bool,
        internal: bool,
    ) -> Result<(Value, u64), CallError> {
        let chain = &self.chain;
        let host = &chain.host;
        if self.depth > host.limits.depth {
            return Err(CallError::limit(Limit::Depth, "nested call depth exceeded"));
        }
        let unavailable = |error: CallError| {
            if self.depth == 0 {
                chain.unavailable.store(true, Ordering::Release);
            }
            error
        };
        let m = chain
            .snapshot
            .mods
            .iter()
            .find(|m| m.local == self.local)
            .ok_or_else(|| {
                unavailable(CallError::new(
                    CallCode::NoMod,
                    format!("no mod {}", self.local),
                ))
            })?;
        if !m.enabled {
            return Err(unavailable(CallError::new(
                CallCode::Disabled,
                format!("mod {} is disabled", self.local),
            )));
        }
        if let Some(error) = &m.error {
            return Err(CallError::internal(error));
        }
        let compiled = chain
            .snapshot
            .components
            .get(&self.local)
            .ok_or_else(|| CallError::new(CallCode::NoFunction, "mod has no logic part"))?;
        let function = if internal {
            compiled.find_host(name)?
        } else {
            compiled.find(name, subscription)?
        };
        let params = match args {
            Args::Json(args) => json::arguments(&function.ty, args)?,
            Args::Vals(values) => values,
        };
        #[cfg(feature = "test-streams")]
        if self.depth == 0 && matches!(caller, Who::Caller(Caller::Agent(_))) {
            host.agent_gate.pass();
        }
        let lock = Arc::clone(
            host.call_locks
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .entry(m.identity.clone())
                .or_default(),
        );
        let _guard = lock_until(&lock, chain.deadline)?;
        let (origin, origin_gen);
        {
            let current = host
                .catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let active = current
                .mods
                .iter()
                .find(|current| {
                    current.local == m.local
                        && current.identity == m.identity
                        && current.package == m.package
                })
                .ok_or_else(|| {
                    unavailable(CallError::new(
                        CallCode::NoMod,
                        "mod was removed or updated",
                    ))
                })?;
            if !active.enabled {
                return Err(unavailable(CallError::new(
                    CallCode::Disabled,
                    "mod is disabled",
                )));
            }
            // The origin client must still be there when the call starts, after the lock
            // (19, Terms): else the call carries the called mod's own identity. A firing also
            // reads its row again: one cancelled or replaced meanwhile is ignored (19.6).
            let (mut wanted, mut wanted_gen) = (self.origin.clone(), self.origin_gen);
            if self.depth == 0
                && let Some((key, due)) = &chain.timer
            {
                match host.state.timer_row(key) {
                    Ok(Some((row_due, origin))) if row_due == *due => {
                        (wanted, wanted_gen) = (origin, None);
                    }
                    _ => {
                        chain.deferred.store(true, Ordering::Release);
                        return Err(CallError::internal(IGNORED));
                    }
                }
            }
            if let (0, Some(run)) = (self.depth, &chain.agent) {
                // An agent's tool runs as the handle's origin client, or not at all: never as
                // the tool's mod (19, Terms; 19.5, Authority at the run).
                (origin, origin_gen) =
                    host.agent_authority(&current, run, &self.local, name)
                        .map_err(|denied| {
                            *chain.denied.lock().expect(
                                "lock poisoned: another thread panicked while holding it",
                            ) = Some(denied);
                            CallError::new(CallCode::NotGranted, "the agent's tool may not run now")
                        })?;
            } else {
                // The identity and the install generation (a reinstall is another install).
                let still = current.mods.iter().find(|o| {
                    o.identity == wanted
                        && o.enabled
                        && o.error.is_none()
                        && wanted_gen.is_none_or(|g| current.install_of(&o.local) == Some(g))
                });
                let who = still.unwrap_or(active);
                origin = who.identity.clone();
                origin_gen = current.install_of(&who.local);
            }
            if let Who::Caller(Caller::Mod(other)) = caller {
                // Selection can precede a wait for this mod's lock. Check
                // authority again before invoking the selected export.
                let authorized = if subscription && (chain.publish || self.depth > 0) {
                    current.mods.iter().any(|p| &p.local == other && p.enabled)
                        && active.links.iter().any(|link| {
                            matches!(link.kind, LinkKind::Subscribe)
                                && link.provider.as_ref() == Some(other)
                                && link.interface.to_string() == function.interface
                                && active.grants.iter().any(|g| g.granted && matches!(&g.grant, Grant::Subscribe { interface, .. } if interface == &link.interface))
                        })
                } else if self.depth > 0 {
                    current.mods.iter().filter(|m| &m.local == other && m.enabled).any(|consumer| {
                        consumer.links.iter().any(|link| {
                            matches!(link.kind, LinkKind::Call)
                                && link.provider.as_ref() == Some(&self.local)
                                && link.provider_interface.as_ref().is_some_and(|i| i.to_string() == function.interface)
                                && consumer.grants.iter().any(|g| g.granted && matches!(&g.grant, Grant::Call { interface, .. } if interface == &link.interface))
                        })
                    })
                } else {
                    true
                };
                if !authorized {
                    return Err(CallError::new(
                        CallCode::NotGranted,
                        "grant or provider was withdrawn while waiting for the mod",
                    ));
                }
            }
        }

        let transaction = host.state.transaction(&m.identity, &host.limits)?;
        let identity = match caller {
            Who::Caller(Caller::Page) => "page".to_owned(),
            Who::Caller(Caller::Agent(handle)) => format!("agent:{handle}"),
            Who::Caller(Caller::Mod(l)) => identity_of(&chain.snapshot, l),
            Who::Host => "host".to_owned(),
        };
        let mut store = super::linker::store(
            &host.engine,
            transaction,
            (caller.clone(), identity),
            &host.limits,
            chain.deadline,
            Self {
                origin,
                origin_gen,
                ..self.clone()
            },
        )?;
        store.set_fuel(fuel).map_err(CallError::internal)?;
        let stderr = store.data().stderr.clone();
        let result = (|| -> wasmtime::Result<Value> {
            let instance = compiled.pre.instantiate(&mut store)?;
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
            if Instant::now() >= chain.deadline {
                return Err(CallError::limit(Limit::Time, "call exceeded wall time").into());
            }
            Ok(match results.first() {
                Some(value) => json::encode(value)?,
                None => Value::Null,
            })
        })();
        if result.is_err() {
            // The minimum keeps the burn of a failed nested call of this mod.
            chain
                .failed_fuel
                .fetch_min(store.get_fuel().unwrap_or(0), Ordering::AcqRel);
        }
        let value = result.map_err(|e| limits::classify(e, &stderr))?;
        if self.depth == 0 && chain.defers && value == Value::String("defer".into()) {
            // A deferred call is rolled back like a failed one, but it is no failure (19.2).
            chain.deferred.store(true, Ordering::Release);
            return Err(CallError::internal(DEFERRED));
        }
        let fuel = store.get_fuel().map_err(CallError::internal)?;
        let ctx = store.into_data();
        let events = ctx
            .outbox
            .into_iter()
            .map(|event| {
                Ok(EventRecord {
                    interface: event.interface.parse().map_err(CallError::internal)?,
                    function: event.function,
                    args: event.args,
                })
            })
            .collect::<Result<Vec<_>, CallError>>()?;
        let manages = ctx.manages;
        // The rows are built inside the write transaction (`State::commit`): the catalog is
        // read for a moment there. Removal changes the catalog before it opens the timer
        // rewrite transaction (19.6, Timers).
        let has_timers = self.depth == 0
            && chain.fakes.is_none()
            && (chain.timer.is_some()
                || chain
                    .log
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .iter()
                    .any(|e| matches!(e, Effect::TimerAfter { .. } | Effect::TimerCancel { .. })));
        let timers = || {
            if !has_timers {
                return Vec::new();
            }
            let log = chain
                .log
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let mut ops: Vec<_> = chain
                .timer
                .iter()
                .map(|(key, due)| crate::state::TimerOp::DeleteIfDue {
                    key: key.clone(),
                    due: *due,
                })
                .collect();
            let catalog = host
                .catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            ops.extend(crate::agent::timer_ops(&log, |origin, generation| {
                catalog.mods.iter().any(|m| {
                    m.identity == origin
                        && m.enabled
                        && m.error.is_none()
                        && generation.is_some_and(|g| catalog.install_of(&m.local) == Some(g))
                })
            }));
            ops
        };
        let changed = ctx.state.commit(&timers, has_timers)?;
        let mut effects = chain
            .effects
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        effects.manages.extend(manages);
        if chain.publish {
            // Enqueue before releasing the mod lock; no invocation waits here.
            host.events.enqueue(self, &events, changed);
        }
        effects.events.extend(
            events
                .into_iter()
                .map(|e| (self.local.clone(), e, self.hop)),
        );
        Ok((value, fuel))
    }
}

// Queue coordination never covers a Wasm call or a wait for a mod lock.
#[derive(Default)]
pub(crate) struct Deliveries {
    pending: Mutex<Pending>,
    wake: Condvar,
}
impl std::fmt::Debug for Deliveries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deliveries").finish_non_exhaustive()
    }
}
#[derive(Default)]
struct Pending {
    emitters: BTreeMap<LocalName, Emitter>,
}
#[derive(Default)]
struct Emitter {
    events: VecDeque<Delivery>,
    active: bool,
    pages: VecDeque<Publication>,
}
struct Job {
    pending: AtomicUsize,
    publications: AtomicUsize,
    deliveries: AtomicUsize,
    changed: Mutex<BTreeSet<LocalName>>,
}
impl Job {
    fn new() -> Self {
        Self {
            // The root invoke holds one count until it releases its mod locks.
            pending: AtomicUsize::new(1),
            publications: AtomicUsize::new(0),
            deliveries: AtomicUsize::new(0),
            changed: Mutex::new(BTreeSet::new()),
        }
    }
    fn complete(&self) -> bool {
        self.pending.load(Ordering::Acquire) == 0
    }
    fn published(&self) -> bool {
        self.complete() && self.publications.load(Ordering::Acquire) == 0
    }
}
struct Delivery {
    chain: Arc<Chain>,
    emitter: LocalName,
    event: EventRecord,
    hop: u8,
    accepted: Arc<AtomicBool>,
}
struct Publication {
    job: Arc<Job>,
    // Emitted event messages are included only if they survive the limits.
    messages: Vec<(Arc<AtomicBool>, String)>,
}
impl Deliveries {
    fn enqueue(&self, invocation: &Invocation, events: &[EventRecord], changed: bool) {
        let chain = &invocation.chain;
        let mut pending = self
            .pending
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let emitter = pending
            .emitters
            .entry(invocation.local.clone())
            .or_default();
        let mut messages = Vec::new();
        chain.job.pending.fetch_add(events.len(), Ordering::Release);
        for event in events {
            let accepted = Arc::new(AtomicBool::new(false));
            messages.push((
                Arc::clone(&accepted),
                crate::bus::event_message(&invocation.local, event),
            ));
            emitter.events.push_back(Delivery {
                chain: Arc::clone(chain),
                emitter: invocation.local.clone(),
                event: event.clone(),
                hop: invocation.hop,
                accepted,
            });
        }
        if changed {
            chain
                .job
                .changed
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .insert(invocation.local.clone());
        }
        if changed || !messages.is_empty() {
            chain.job.publications.fetch_add(1, Ordering::Release);
            emitter.pages.push_back(Publication {
                job: Arc::clone(&chain.job),
                messages,
            });
        }
        self.wake.notify_all();
    }

    fn ready(&self, chain: &Chain) {
        let _pending = self
            .pending
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        chain.ready.store(true, Ordering::Release);
        self.wake.notify_all();
    }

    fn finish(&self, host: &Host, chain: &Arc<Chain>) {
        let mut pending = self
            .pending
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        chain.ready.store(true, Ordering::Release);
        chain.job.pending.fetch_sub(1, Ordering::Release);
        self.wake.notify_all();
        loop {
            // A later commit cannot overtake an earlier page batch of this mod.
            // Read-only calls with no effects do not wait for another job.
            for (local, emitter) in &mut pending.emitters {
                while emitter.pages.front().is_some_and(|p| p.job.complete()) {
                    let publication = emitter
                        .pages
                        .pop_front()
                        .expect("the loop checked the front page");
                    // Coalesce changed at the last commit of this mod in the
                    // original call's completed event chain (9.2 step 6).
                    let changed = !emitter
                        .pages
                        .iter()
                        .any(|p| Arc::ptr_eq(&p.job, &publication.job))
                        && publication
                            .job
                            .changed
                            .lock()
                            .expect("lock poisoned: another thread panicked while holding it")
                            .remove(local);
                    let sender = host.bus.publication();
                    for (accepted, message) in publication.messages {
                        if accepted.load(Ordering::Acquire) {
                            let _ = sender.send((local.clone(), message.into()));
                        }
                    }
                    if changed {
                        let _ = sender.send((local.clone(), crate::bus::CHANGED.into()));
                    }
                    publication.job.publications.fetch_sub(1, Ordering::Release);
                }
            }
            self.wake.notify_all();
            if chain.job.published() {
                return;
            }
            let next = pending.emitters.values_mut().find_map(|emitter| {
                if emitter.active
                    || !emitter.events.front().is_some_and(|e| {
                        Arc::ptr_eq(&e.chain.job, &chain.job)
                            && e.chain.ready.load(Ordering::Acquire)
                    })
                {
                    return None;
                }
                emitter.active = true;
                emitter.events.pop_front()
            });
            let Some(delivery) = next else {
                pending = self
                    .wake
                    .wait(pending)
                    .expect("lock poisoned: another thread panicked while holding it");
                continue;
            };
            drop(pending);
            process(&delivery);
            pending = self
                .pending
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            pending
                .emitters
                .get_mut(&delivery.emitter)
                .expect("a delivery names a registered emitter")
                .active = false;
            delivery.chain.job.pending.fetch_sub(1, Ordering::Release);
            self.wake.notify_all();
        }
    }
}

fn process(delivery: &Delivery) {
    let chain = &delivery.chain;
    let host = &chain.host;
    let job = &chain.job;
    let emitter = &delivery.emitter;
    let event = &delivery.event;
    if delivery.hop >= host.limits.hops {
        eprintln!("watane: event hop limit reached");
        return;
    }
    if job.deliveries.load(Ordering::Acquire) >= host.limits.deliveries {
        eprintln!("watane: event delivery limit reached");
        return;
    }
    delivery.accepted.store(true, Ordering::Release);
    let current = host
        .catalog
        .lock()
        .expect("lock poisoned: another thread panicked while holding it")
        .clone();
    let mut subscribers: Vec<_> = current
        .mods
        .iter()
        .filter(|m| m.enabled && m.error.is_none())
        .collect();
    subscribers.sort_by(|a, b| a.local.cmp(&b.local));
    for m in subscribers {
        for link in m.links.iter().filter(|l| {
            matches!(l.kind, LinkKind::Subscribe)
                && l.provider.as_ref() == Some(emitter)
                && l.provider_interface.as_ref() == Some(&event.interface)
        }) {
            if !m.grants.iter().any(|g| g.granted && matches!(&g.grant, Grant::Subscribe { interface,.. } if interface == &link.interface)) { continue; }
            // Only the original caller drains this job's events.
            let count = job.deliveries.load(Ordering::Relaxed);
            if count >= host.limits.deliveries {
                return;
            }
            job.deliveries.store(count + 1, Ordering::Relaxed);
            let Ok(deadline) = deadline(host) else {
                continue;
            };
            let subscriber = Arc::new(Chain {
                host: Arc::clone(host),
                snapshot: Arc::new(current.clone()),
                fakes: None,
                deadline,
                effects: Mutex::new(Effects::default()),
                publish: true,
                ready: AtomicBool::new(false),
                job: Arc::clone(job),
                failed_fuel: AtomicU64::new(0),
                log: Mutex::new(Vec::new()),
                connection: None,
                defers: false,
                deferred: AtomicBool::new(false),
                timer: None,
                unavailable: AtomicBool::new(false),
                agent: None,
                denied: Mutex::new(None),
            });
            register(host, &subscriber);
            let invocation = Invocation {
                chain: Arc::clone(&subscriber),
                local: m.local.clone(),
                identity: m.identity.clone(),
                depth: 0,
                hop: delivery.hop + 1,
                link: false,
                origin: m.identity.clone(),
                origin_gen: current.install_of(&m.local),
                via: None,
            };
            let result = invocation.invoke(
                &Who::Caller(Caller::Mod(emitter.clone())),
                &format!("{}.{}", link.interface, event.function),
                Args::Json(event.args.clone()),
                host.limits.fuel,
                true,
                false,
            );
            take_and_apply(host, &subscriber, result.is_ok());
            // The effects of a delivery apply after its invoke (6.10).
            let manages = std::mem::take(
                &mut subscriber
                    .effects
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .manages,
            );
            host.apply_manage(manages);
            // Nested calls have also released their locks before these events run.
            host.events.ready(&subscriber);
            if let Err(error) = result {
                eprintln!(
                    "watane: event {}.{} from {emitter} to {} failed: {:?}: {}",
                    event.interface, event.function, m.local, error.code, error.message
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, Mode};
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn ac7_s12a_subscription_rechecks_grant_after_waiting_for_its_lock() {
        let dir = tempfile::tempdir().unwrap();
        // Preview avoids consent here: this unit test isolates runtime delivery.
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let fixture = |name| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/mods")
                .join(name)
        };
        host.install_folders(&[fixture("shelf"), fixture("recs")], 0)
            .unwrap();
        let recs = LocalName::parse("recs").unwrap();
        let grant = host
            .get(&recs)
            .unwrap()
            .grants
            .into_iter()
            .find(|g| matches!(g.grant, Grant::Subscribe { .. }))
            .unwrap()
            .grant;
        let lock = Arc::new(Mutex::new(()));
        host.call_locks
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .insert("local/recs".into(), Arc::clone(&lock));
        let guard = lock
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let emitter = Arc::clone(&host);
        let thread = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || {
                emitter.call(
                    &Caller::Page,
                    &LocalName::parse("shelf").unwrap(),
                    "items.add",
                    serde_json::json!({"title":"X"}),
                )
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        // The map and this test own two references. The third is the
        // subscriber invocation waiting in lock_until, after grant selection.
        while Arc::strong_count(&lock) < 3 {
            assert!(Instant::now() < deadline, "delivery did not start");
            std::thread::sleep(Duration::from_millis(1));
        }
        host.revoke(&recs, &grant).unwrap();
        drop(guard);
        assert_eq!(thread.join().unwrap().unwrap(), Value::Null);
        let reader = Arc::clone(&host);
        let received = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || reader.call(&Caller::Page, &recs, "api.received", Value::Null))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(received, serde_json::json!([]));
    }

    /// 19.6 (Timers): `Done.unavailable` says the call's own mod was disabled while it waited,
    /// not a nested call's: a firing keeps its row for the first only.
    #[test]
    fn unavailable_is_the_own_mods_not_a_nested_calls() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods");
        let names = ["latch", "relay", "advo", "advo-front"];
        host.install_folders(&names.map(|n| fixtures.join(n)), 0)
            .unwrap();
        // Each case: the mod whose lock is held while the call waits, then disabled.
        for (waits, own) in [("relay", false), ("advo-front", true)] {
            let lock = Arc::new(Mutex::new(()));
            host.call_locks
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .insert(format!("local/{waits}"), Arc::clone(&lock));
            let guard = lock
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            let caller = Arc::clone(&host);
            let thread = std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn(move || {
                    let mut start = Start::internal(
                        Who::Caller(Caller::Page),
                        LocalName::parse("advo-front").unwrap(),
                        "api.cancel-then-sleep".into(),
                        Args::Json(serde_json::json!({"s": "x", "ms": 0})),
                        "local/advo-front".into(),
                    );
                    start.internal = false;
                    run_start(&caller, start)
                })
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            while Arc::strong_count(&lock) < 3 {
                assert!(
                    Instant::now() < deadline,
                    "the call did not wait for {waits}"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
            let local = LocalName::parse(waits).unwrap();
            host.disable(&local).unwrap();
            drop(guard);
            let done = thread.join().unwrap();
            let code = done.result.as_ref().map_err(|e| e.code).err();
            assert_eq!(code, Some(CallCode::Disabled), "{waits}");
            assert_eq!(done.unavailable, own, "{waits}");
            host.enable(&local).unwrap();
        }
    }
}
