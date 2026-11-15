use super::{json, limits};
use crate::{CallCode, CallError, Caller, EventRecord, Host, Limit, LinkKind, LocalName, Outcome};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Instant,
};
use watane_pkg::{Grant, InterfaceName};

#[derive(Default)]
struct Effects {
    events: Vec<(LocalName, EventRecord, u8)>,
}
struct Chain {
    host: Arc<Host>,
    snapshot: Arc<crate::catalog::Catalog>,
    fakes: Option<crate::Fakes>,
    deadline: Instant,
    effects: Mutex<Effects>,
    publish: bool,
    ready: AtomicBool,
    job: Arc<Job>,
    // Lowest fuel left by an invoke that failed in the current answer of a
    // collect; the collect resets it before each answer (9.4).
    failed_fuel: AtomicU64,
}
#[derive(Clone)]
pub(super) struct Invocation {
    chain: Arc<Chain>,
    local: LocalName,
    depth: u32,
    hop: u8,
}

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
            Err(std::sync::TryLockError::Poisoned(e)) => return Ok(e.into_inner()),
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
    let deadline = deadline(host)?;
    let snapshot = Arc::new(
        host.catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
    );
    let chain = Arc::new(Chain {
        host: host
            .weak
            .upgrade()
            .ok_or_else(|| CallError::internal("host closed"))?,
        snapshot,
        fakes: host
            .fakes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
        deadline,
        effects: Mutex::new(Effects::default()),
        publish,
        ready: AtomicBool::new(false),
        job: Arc::new(Job::new()),
        failed_fuel: AtomicU64::new(0),
    });
    let invocation = Invocation {
        chain: chain.clone(),
        local: local.clone(),
        depth: 0,
        hop: 0,
    };
    let result = invocation.invoke(caller, function, args, host.limits.fuel, subscription);
    let effects = std::mem::take(&mut *chain.effects.lock().unwrap());
    let events = effects
        .events
        .iter()
        .map(|(_, event, _)| event.clone())
        .collect();
    if publish {
        host.events.finish(host, &chain);
    }
    result.map(|(value, _)| Outcome { value, events })
}

impl Invocation {
    pub(super) fn storage(&self) -> Result<(), CallError> {
        let catalog = self.chain.host.catalog.lock().unwrap();
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
        let current = self.chain.host.catalog.lock().unwrap().clone();
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
        let provider = provider.unwrap();
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
            chain: self.chain.clone(),
            local: provider.clone(),
            depth: self.depth.saturating_add(1),
            hop: self.hop,
        };
        nested.invoke(
            &Caller::Mod(self.local.clone()),
            &format!("{}.{function}", linked_interface.interface()),
            args,
            fuel,
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
        let current = self.chain.host.catalog.lock().unwrap().clone();
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
                    chain: self.chain.clone(),
                    local: m.local.clone(),
                    depth: self.depth.saturating_add(1),
                    hop: self.hop,
                };
                self.chain.failed_fuel.store(fuel, Ordering::Release);
                match nested.invoke(
                    &Caller::Mod(self.local.clone()),
                    &format!("{}.{function}", link.interface),
                    args.clone(),
                    fuel,
                    true,
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

    fn invoke(
        &self,
        caller: &Caller,
        name: &str,
        args: Value,
        fuel: u64,
        subscription: bool,
    ) -> Result<(Value, u64), CallError> {
        let chain = &self.chain;
        let host = &chain.host;
        if self.depth > host.limits.depth {
            return Err(CallError::limit(Limit::Depth, "nested call depth exceeded"));
        }
        let m = chain
            .snapshot
            .mods
            .iter()
            .find(|m| m.local == self.local)
            .ok_or_else(|| CallError::new(CallCode::NoMod, format!("no mod {}", self.local)))?;
        if !m.enabled {
            return Err(CallError::new(
                CallCode::Disabled,
                format!("mod {} is disabled", self.local),
            ));
        }
        if let Some(error) = &m.error {
            return Err(CallError::internal(error));
        }
        let compiled = chain
            .snapshot
            .components
            .get(&self.local)
            .ok_or_else(|| CallError::new(CallCode::NoFunction, "mod has no logic part"))?;
        let function = compiled.find(name, subscription)?;
        let params = json::arguments(&function.ty, args)?;
        let lock = host
            .call_locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(m.identity.clone())
            .or_default()
            .clone();
        let _guard = lock_until(&lock, chain.deadline)?;
        {
            let current = host.catalog.lock().unwrap();
            let active = current
                .mods
                .iter()
                .find(|current| {
                    current.local == m.local
                        && current.identity == m.identity
                        && current.package == m.package
                })
                .ok_or_else(|| CallError::new(CallCode::NoMod, "mod was removed or updated"))?;
            if !active.enabled {
                return Err(CallError::new(CallCode::Disabled, "mod is disabled"));
            }
            if let Caller::Mod(other) = caller {
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
        let mut store = super::linker::store(
            &host.engine,
            transaction,
            caller.who(),
            &host.limits,
            chain.deadline,
            self.clone(),
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
        let changed = ctx.state.commit()?;
        let mut effects = chain.effects.lock().unwrap();
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
        let mut pending = self.pending.lock().unwrap();
        let emitter = pending
            .emitters
            .entry(invocation.local.clone())
            .or_default();
        let mut messages = Vec::new();
        chain.job.pending.fetch_add(events.len(), Ordering::Release);
        for event in events {
            let accepted = Arc::new(AtomicBool::new(false));
            messages.push((
                accepted.clone(),
                crate::bus::event_message(&invocation.local, event),
            ));
            emitter.events.push_back(Delivery {
                chain: chain.clone(),
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
                .unwrap()
                .insert(invocation.local.clone());
        }
        if changed || !messages.is_empty() {
            chain.job.publications.fetch_add(1, Ordering::Release);
            emitter.pages.push_back(Publication {
                job: chain.job.clone(),
                messages,
            });
        }
        self.wake.notify_all();
    }

    fn ready(&self, chain: &Chain) {
        let _pending = self.pending.lock().unwrap();
        chain.ready.store(true, Ordering::Release);
        self.wake.notify_all();
    }

    fn finish(&self, host: &Host, chain: &Arc<Chain>) {
        let mut pending = self.pending.lock().unwrap();
        chain.ready.store(true, Ordering::Release);
        chain.job.pending.fetch_sub(1, Ordering::Release);
        self.wake.notify_all();
        loop {
            // A later commit cannot overtake an earlier page batch of this mod.
            // Read-only calls with no effects do not wait for another job.
            for (local, emitter) in &mut pending.emitters {
                while emitter.pages.front().is_some_and(|p| p.job.complete()) {
                    let publication = emitter.pages.pop_front().unwrap();
                    // Coalesce changed at the last commit of this mod in the
                    // original call's completed event chain (9.2 step 6).
                    let changed = !emitter
                        .pages
                        .iter()
                        .any(|p| Arc::ptr_eq(&p.job, &publication.job))
                        && publication.job.changed.lock().unwrap().remove(local);
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
                pending = self.wake.wait(pending).unwrap();
                continue;
            };
            drop(pending);
            process(&delivery);
            pending = self.pending.lock().unwrap();
            pending.emitters.get_mut(&delivery.emitter).unwrap().active = false;
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
    let current = host.catalog.lock().unwrap().clone();
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
                host: host.clone(),
                snapshot: Arc::new(current.clone()),
                fakes: None,
                deadline,
                effects: Mutex::new(Effects::default()),
                publish: true,
                ready: AtomicBool::new(false),
                job: job.clone(),
                failed_fuel: AtomicU64::new(0),
            });
            let invocation = Invocation {
                chain: subscriber.clone(),
                local: m.local.clone(),
                depth: 0,
                hop: delivery.hop + 1,
            };
            let result = invocation.invoke(
                &Caller::Mod(emitter.clone()),
                &format!("{}.{}", link.interface, event.function),
                event.args.clone(),
                host.limits.fuel,
                true,
            );
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
    use std::{path::Path, time::Duration};

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
            .unwrap()
            .insert("local/recs".into(), lock.clone());
        let guard = lock.lock().unwrap();
        let emitter = host.clone();
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
        let reader = host.clone();
        let received = std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || reader.call(&Caller::Page, &recs, "api.received", Value::Null))
            .unwrap()
            .join()
            .unwrap()
            .unwrap();
        assert_eq!(received, serde_json::json!([]));
    }
}
