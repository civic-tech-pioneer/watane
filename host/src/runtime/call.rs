use super::{json, limits};
use crate::{CallCode, CallError, Caller, EventRecord, Host, Limit, LinkKind, LocalName, Outcome};
use serde_json::Value;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};
use watane_pkg::{Grant, InterfaceName};

#[derive(Default)]
struct Effects {
    events: Vec<(LocalName, EventRecord, u8)>,
    changed: BTreeSet<LocalName>,
}
struct Chain {
    host: Arc<Host>,
    snapshot: Arc<crate::catalog::Catalog>,
    fakes: Option<crate::Fakes>,
    deadline: Instant,
    effects: Mutex<Effects>,
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
    // Deliveries finish before a later top-level call can publish a commit.
    let _execution = lock_until(&host.execution, deadline)?;
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
        deliver(&chain, effects);
    }
    result.map(|(value, _)| Outcome { value, events })
}

impl Invocation {
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
        let consumer = self
            .chain
            .snapshot
            .mods
            .iter()
            .find(|m| m.local == self.local)
            .expect("invoked mod exists");
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
        let target = self
            .chain
            .snapshot
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
        if changed || !events.is_empty() {
            effects.changed.insert(self.local.clone());
        }
        effects.events.extend(
            events
                .into_iter()
                .map(|e| (self.local.clone(), e, self.hop)),
        );
        Ok((value, fuel))
    }
}

fn deliver(chain: &Arc<Chain>, effects: Effects) {
    let host = &chain.host;
    let mut queue: VecDeque<_> = effects.events.into();
    let mut changed = effects.changed;
    let mut deliveries = 0;
    while let Some((emitter, event, hop)) = queue.pop_front() {
        if hop >= host.limits.hops {
            eprintln!("watane: event hop limit reached");
            continue;
        }
        if deliveries >= host.limits.deliveries {
            eprintln!("watane: event delivery limit reached");
            break;
        }
        let message = crate::bus::event_message(&emitter, &event);
        host.bus.send(&emitter, message.clone());
        let mut subscribers: Vec<_> = chain
            .snapshot
            .mods
            .iter()
            .filter(|m| m.enabled && m.error.is_none())
            .collect();
        subscribers.sort_by(|a, b| a.local.cmp(&b.local));
        for m in subscribers {
            for link in m.links.iter().filter(|l| {
                matches!(l.kind, LinkKind::Subscribe)
                    && l.provider.as_ref() == Some(&emitter)
                    && l.provider_interface.as_ref() == Some(&event.interface)
            }) {
                if !m.grants.iter().any(|g| g.granted && matches!(&g.grant, Grant::Subscribe { interface,.. } if interface == &link.interface)) { continue; }
                if deliveries >= host.limits.deliveries {
                    break;
                }
                deliveries += 1;
                let Ok(deadline) = deadline(host) else {
                    continue;
                };
                let delivery = Arc::new(Chain {
                    host: host.clone(),
                    snapshot: chain.snapshot.clone(),
                    fakes: None,
                    deadline,
                    effects: Mutex::new(Effects::default()),
                });
                let invocation = Invocation {
                    chain: delivery.clone(),
                    local: m.local.clone(),
                    depth: 0,
                    hop: hop + 1,
                };
                let result = invocation.invoke(
                    &Caller::Mod(emitter.clone()),
                    &format!("{}.{}", link.interface, event.function),
                    event.args.clone(),
                    host.limits.fuel,
                    true,
                );
                let effects = std::mem::take(&mut *delivery.effects.lock().unwrap());
                queue.extend(effects.events);
                changed.extend(effects.changed);
                match result {
                    Ok(_) => {
                        host.bus.send(&m.local, message.clone());
                        changed.insert(m.local.clone());
                    }
                    Err(error) => eprintln!(
                        "watane: event {}.{} from {emitter} to {} failed: {:?}: {}",
                        event.interface, event.function, m.local, error.code, error.message
                    ),
                }
            }
        }
    }
    for local in changed {
        host.bus.send(&local, crate::bus::CHANGED);
    }
}
