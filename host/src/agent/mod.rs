//! The agent layer (TECHNICAL 19): the host functions of `streams` and `timers` at the
//! call, the effects that a chain applies after its durable commit (19.6) and the
//! application of those effects.

pub(crate) mod streams;
pub(crate) mod timers;

use crate::runtime::call::{Chain, Invocation, Who};
use crate::runtime::json;
use crate::{CallCode, CallError, Caller, Host, LocalName, RequestRecord};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use streams::{Consumer, MAX_OPEN, Producer, STREAM_SINK, StreamError};
use wasmtime::component::Val;

const MAX_ID: usize = 128;
const MAX_LIVE: usize = 256;
const MAX_DELAY_MS: u64 = 2_592_000_000;

/// A mod by local name and identity: an identity alone does not tell two installs apart (19, Terms).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ModRef {
    pub local: LocalName,
    pub identity: String,
}

/// A request or stream effect of a call, applied after the whole chain succeeded and committed (19.6).
#[derive(Clone, Debug)]
pub(crate) enum Effect {
    Open {
        id: String,
        consumer: Consumer,
        producer: ModRef,
        origin: String,
    },
    Push {
        id: String,
        body: Vec<u8>,
        hop: u8,
        by: ModRef,
        origin: String,
    },
    Close {
        id: String,
        outcome: Value,
        by: ModRef,
    },
    Cancel {
        id: String,
    },
    HandOff {
        id: String,
        to: ModRef,
    },
    Resume {
        id: String,
    },
    TimerAfter {
        owner: ModRef,
        id: String,
        due: u64,
        delay_ms: u64,
        origin: String,
    },
    TimerCancel {
        owner: ModRef,
        id: String,
    },
}

pub(crate) fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The requests of a chain, as a harness records them (2.6).
pub(crate) fn records(log: &[Effect]) -> Vec<RequestRecord> {
    log.iter()
        .filter_map(|e| match e {
            Effect::Open { .. } => Some(RequestRecord {
                function: "streams.open".into(),
                args: json!({}),
            }),
            Effect::TimerAfter { id, delay_ms, .. } => Some(RequestRecord {
                function: "timers.after".into(),
                args: json!({"id": id, "delay-ms": delay_ms}),
            }),
            Effect::TimerCancel { id, .. } => Some(RequestRecord {
                function: "timers.cancel".into(),
                args: json!({"id": id}),
            }),
            _ => None,
        })
        .collect()
}

/// The timer writes of a top-level commit, in call order (19.6, 7).
pub(crate) fn timer_ops(log: &[Effect]) -> Vec<crate::state::TimerOp> {
    log.iter()
        .filter_map(|e| match e {
            Effect::TimerAfter {
                owner,
                id,
                due,
                origin,
                ..
            } => Some(crate::state::TimerOp::After {
                key: crate::state::timer_key(&owner.identity, id),
                due: *due,
                origin: origin.clone(),
            }),
            Effect::TimerCancel { owner, id } => Some(crate::state::TimerOp::Cancel {
                key: crate::state::timer_key(&owner.identity, id),
            }),
            _ => None,
        })
        .collect()
}

fn err(e: StreamError) -> Value {
    json!({"err": e.case()})
}
fn ok() -> Value {
    json!({"ok": null})
}

/// What the call knows of a stream: the hub's view, changed by the effects of this chain.
struct View {
    consumer: Consumer,
    producer: Producer,
    origin: String,
    handed_off: bool,
    in_chain: bool,
}

impl Invocation {
    fn me(&self) -> ModRef {
        ModRef {
            local: self.local.clone(),
            identity: self.identity.clone(),
        }
    }
    fn mod_ref(&self, local: &LocalName) -> Option<ModRef> {
        self.chain
            .snapshot
            .mods
            .iter()
            .find(|m| &m.local == local && m.enabled)
            .map(|m| ModRef {
                local: m.local.clone(),
                identity: m.identity.clone(),
            })
    }
    fn exports_sink(&self, local: &LocalName) -> bool {
        self.chain
            .snapshot
            .components
            .get(local)
            .is_some_and(|c| c.host_exports().iter().any(|e| e == STREAM_SINK))
    }
    /// The consumer that a `streams.open` of this invoke makes (19.2, Consumer of a mod stream).
    fn open_consumer(&self, who: &Who) -> Option<Consumer> {
        if !self.link {
            return None;
        }
        match who {
            Who::Caller(Caller::Mod(l)) => self.mod_ref(l).map(Consumer::Mod),
            Who::Caller(Caller::Page) if self.depth == 0 => {
                self.chain.connection.clone().map(|conn| Consumer::Page {
                    conn,
                    local: self.local.clone(),
                })
            }
            _ => None,
        }
    }
    fn view(&self, log: &[Effect], id: &str) -> Option<View> {
        let mut view = log.iter().find_map(|e| match e {
            Effect::Open {
                id: i,
                consumer,
                producer,
                origin,
            } if i == id => Some(View {
                consumer: consumer.clone(),
                producer: Producer::Mod(producer.clone()),
                origin: origin.clone(),
                handed_off: false,
                in_chain: true,
            }),
            _ => None,
        });
        if view.is_none() {
            let info = self.chain.host.hub.info(id)?;
            view = Some(View {
                consumer: info.consumer,
                producer: info.producer,
                origin: info.origin,
                handed_off: info.handed_off,
                in_chain: false,
            });
        }
        let mut view = view?;
        for e in log {
            match e {
                Effect::Close { id: i, .. } | Effect::Cancel { id: i, .. } if i == id => {
                    return None;
                }
                Effect::HandOff { id: i, to, .. } if i == id => {
                    view.consumer = Consumer::Mod(to.clone());
                    view.handed_off = true;
                }
                _ => {}
            }
        }
        Some(view)
    }
    /// Whether this call's instance acts for the consumer (19.2, Acting side).
    fn acts_for(&self, consumer: &Consumer) -> bool {
        match consumer {
            Consumer::Mod(m) => m.local == self.local && m.identity == self.identity,
            Consumer::Page { conn, local } => {
                self.chain.connection.as_deref() == Some(conn) && &self.local == local
            }
            Consumer::Held { .. } => false,
        }
    }
    /// The gate of 19.2 (Commit): held from the checks of a call to the push of its effect, so
    /// that two chains in flight cannot both pass a limit.
    fn gate(&self) -> Gate<'_> {
        let chains = self
            .chain
            .host
            .chains
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let peers = chains
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .filter(|c| !Arc::ptr_eq(c, &self.chain))
            .collect();
        Gate {
            peers,
            _chains: chains,
        }
    }
    fn log(&self) -> std::sync::MutexGuard<'_, Vec<Effect>> {
        self.chain
            .log
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
}

/// The other chains in flight, with the lock that keeps them from changing (19.2, Commit).
struct Gate<'a> {
    peers: Vec<Arc<Chain>>,
    _chains: std::sync::MutexGuard<'a, Vec<std::sync::Weak<Chain>>>,
}
impl Gate<'_> {
    /// Every effect that another chain has queued.
    fn each(&self, mut visit: impl FnMut(&Effect)) {
        for chain in &self.peers {
            let log = chain
                .log
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            log.iter().for_each(&mut visit);
        }
    }
}

/// `streams.push` (19.2): the checks of the call, then the effect.
pub(crate) fn push(inv: &Invocation, id: &str, body: Vec<u8>) -> Result<(), StreamError> {
    if let Some(fakes) = &inv.chain.fakes {
        // A harness answers ok, or what a `Fakes` entry says (19.1).
        let fake = fakes
            .get("watane:host/streams")
            .and_then(|f| f.get("push"))
            .and_then(|v| v.get("err"))
            .and_then(Value::as_str);
        return match fake {
            Some("not-yours") => Err(StreamError::NotYours),
            Some("unknown-stream") => Err(StreamError::UnknownStream),
            Some("full") => Err(StreamError::Full),
            Some("too-large") => Err(StreamError::TooLarge),
            _ => Ok(()),
        };
    }
    let me = inv.me();
    let gate = inv.gate();
    let mut log = inv.log();
    if body.len() > streams::MAX_ITEM {
        return Err(StreamError::TooLarge);
    }
    let view = inv.view(&log, id).ok_or(StreamError::UnknownStream)?;
    if view.producer != Producer::Mod(me.clone()) {
        return Err(StreamError::NotYours);
    }
    let (mut count, mut bytes, mut origin_pending, mut host_pending) = (0, 0, 0, 0);
    let mut queued = |e: &Effect| {
        if let Effect::Push {
            id: i,
            body,
            origin,
            ..
        } = e
        {
            if i == id {
                count += 1;
                bytes += body.len();
            }
            if *origin == view.origin {
                origin_pending += body.len();
            }
            host_pending += body.len();
        }
    };
    gate.each(&mut queued);
    log.iter().for_each(&mut queued);
    inv.chain.host.hub.check_push(
        id,
        body.len(),
        (count, bytes),
        &view.origin,
        origin_pending,
        host_pending,
    )?;
    if view.in_chain && (count + 1 > streams::MAX_ITEMS || bytes + body.len() > streams::MAX_BYTES)
    {
        return Err(StreamError::Full);
    }
    log.push(Effect::Push {
        id: id.to_owned(),
        body,
        hop: inv.hop,
        by: me,
        origin: view.origin,
    });
    Ok(())
}

/// A function of `streams` or `timers` called by the mod of `inv` (19.2, 19.6).
pub(crate) fn call(
    inv: &Invocation,
    who: &Who,
    interface: &str,
    function: &str,
    args: &[Val],
) -> Result<Value, CallError> {
    let name = |i: usize| match args.get(i) {
        Some(Val::String(s)) => Ok(s.clone()),
        _ => Err(CallError::internal(format!(
            "{interface}.{function}: bad argument"
        ))),
    };
    if let Some(fakes) = &inv.chain.fakes {
        return fake(inv, fakes, interface, function, args);
    }
    let me = inv.me();
    match (interface, function) {
        ("streams", "open") => {
            let Some(consumer) = inv.open_consumer(who) else {
                return Ok(err(StreamError::NotYours));
            };
            if let Consumer::Mod(m) = &consumer
                && !inv.exports_sink(&m.local)
            {
                return Ok(err(StreamError::NotYours));
            }
            let gate = inv.gate();
            let mut log = inv.log();
            if let Some(key) = consumer.key() {
                let mut opened = 0;
                let mut queued = |e: &Effect| {
                    if matches!(e, Effect::Open { consumer: c, .. } if c.key().as_deref() == Some(key.as_str()))
                    {
                        opened += 1;
                    }
                };
                gate.each(&mut queued);
                log.iter().for_each(&mut queued);
                if inv.chain.host.hub.open_count(&key) + opened >= MAX_OPEN {
                    return Ok(err(StreamError::Full));
                }
            }
            let id = streams::random_id();
            log.push(Effect::Open {
                id: id.clone(),
                consumer,
                producer: me,
                origin: inv.origin.clone(),
            });
            Ok(json!({"ok": id}))
        }
        ("streams", "push") => {
            let id = name(0)?;
            let Some(Val::List(items)) = args.get(1) else {
                return Err(CallError::internal("streams.push: bad argument"));
            };
            let body: Vec<u8> = items
                .iter()
                .map(|v| if let Val::U8(b) = v { *b } else { 0 })
                .collect();
            Ok(match push(inv, &id, body) {
                Ok(()) => ok(),
                Err(e) => err(e),
            })
        }
        ("streams", "close") => {
            let id = name(0)?;
            let outcome = args
                .get(1)
                .map(json::encode)
                .transpose()?
                .unwrap_or(Value::Null);
            let mut log = inv.log();
            if let Some(view) = inv.view(&log, &id)
                && view.producer == Producer::Mod(me.clone())
            {
                log.push(Effect::Close {
                    id,
                    outcome,
                    by: me,
                });
            }
            Ok(Value::Null)
        }
        ("streams", "cancel") => {
            let id = name(0)?;
            let mut log = inv.log();
            if let Some(view) = inv.view(&log, &id)
                && inv.acts_for(&view.consumer)
            {
                log.push(Effect::Cancel { id });
            }
            Ok(Value::Null)
        }
        ("streams", "resume") => {
            let id = name(0)?;
            let mut log = inv.log();
            let Some(view) = inv.view(&log, &id) else {
                return Ok(err(StreamError::UnknownStream));
            };
            if !inv.acts_for(&view.consumer) {
                return Ok(err(StreamError::NotYours));
            }
            log.push(Effect::Resume { id });
            Ok(ok())
        }
        ("streams", "hand-off") => {
            let id = name(0)?;
            let gate = inv.gate();
            let mut log = inv.log();
            let Some(view) = inv.view(&log, &id) else {
                return Ok(err(StreamError::UnknownStream));
            };
            // Once: another chain in flight may have queued the hand-off already.
            let mut taken = false;
            gate.each(|e| taken |= matches!(e, Effect::HandOff { id: i, .. } if *i == id));
            if taken {
                return Ok(err(StreamError::NotYours));
            }
            let target = match who {
                Who::Caller(Caller::Mod(l)) => {
                    inv.mod_ref(l).filter(|m| inv.exports_sink(&m.local))
                }
                _ => None,
            };
            let (true, Some(to)) = (
                inv.acts_for(&view.consumer) && view.producer == Producer::Host && !view.handed_off,
                target,
            ) else {
                return Ok(err(StreamError::NotYours));
            };
            log.push(Effect::HandOff { id, to });
            Ok(ok())
        }
        ("timers", "after") => {
            let (id, delay) = (
                name(0)?,
                match args.get(1) {
                    Some(Val::U64(d)) => *d,
                    _ => return Err(CallError::internal("timers.after: bad argument")),
                },
            );
            if id.len() > MAX_ID {
                return Ok(json!({"err": "bad-id"}));
            }
            if delay > MAX_DELAY_MS {
                return Ok(json!({"err": "bad-delay"}));
            }
            let gate = inv.gate();
            let mut log = inv.log();
            let mut live: BTreeSet<String> = inv.chain.host.state.timer_ids(&me.identity)?;
            // Other chains' timers count; their cancels do not, as they may roll back.
            gate.each(|e| {
                if let Effect::TimerAfter { owner, id, .. } = e
                    && *owner == me
                {
                    live.insert(id.clone());
                }
            });
            for e in log.iter() {
                match e {
                    Effect::TimerAfter { owner, id, .. } if *owner == me => {
                        live.insert(id.clone());
                    }
                    Effect::TimerCancel { owner, id } if *owner == me => {
                        live.remove(id);
                    }
                    _ => {}
                }
            }
            if !live.contains(&id) && live.len() >= MAX_LIVE {
                return Ok(json!({"err": "too-many"}));
            }
            log.push(Effect::TimerAfter {
                owner: me,
                id,
                due: unix_ms().saturating_add(delay),
                delay_ms: delay,
                origin: inv.origin.clone(),
            });
            Ok(ok())
        }
        ("timers", "cancel") => {
            let id = name(0)?;
            inv.log().push(Effect::TimerCancel { owner: me, id });
            Ok(Value::Null)
        }
        _ => Err(CallError::new(
            CallCode::NoFunction,
            format!("no host function {interface}.{function}"),
        )),
    }
}

/// The fakes of a harness (19.1, Harness): ok, `s1`, `s2`, ..., and what a `Fakes` entry says.
fn fake(
    inv: &Invocation,
    fakes: &crate::Fakes,
    interface: &str,
    function: &str,
    args: &[Val],
) -> Result<Value, CallError> {
    let me = inv.me();
    let mut log = inv.log();
    let mut result = ok();
    match (interface, function) {
        ("streams", "open") => {
            let n = inv
                .chain
                .host
                .fake_serial
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                + 1;
            let id = format!("s{n}");
            log.push(Effect::Open {
                id: id.clone(),
                consumer: Consumer::Held {
                    origin: String::new(),
                },
                producer: me,
                origin: inv.origin.clone(),
            });
            result = json!({"ok": id});
        }
        ("timers", "after") => {
            if let (Some(Val::String(id)), Some(Val::U64(delay))) = (args.first(), args.get(1)) {
                log.push(Effect::TimerAfter {
                    owner: me,
                    id: id.clone(),
                    due: 0,
                    delay_ms: *delay,
                    origin: inv.origin.clone(),
                });
            }
        }
        ("timers", "cancel") => {
            if let Some(Val::String(id)) = args.first() {
                log.push(Effect::TimerCancel {
                    owner: me,
                    id: id.clone(),
                });
            }
            result = Value::Null;
        }
        ("streams", "close" | "cancel") => result = Value::Null,
        _ => {}
    }
    Ok(fakes
        .get(&format!("watane:host/{interface}"))
        .and_then(|f| f.get(function))
        .cloned()
        .unwrap_or(result))
}

impl Host {
    fn mod_installed(&self, identity: &str, local: Option<&LocalName>) -> bool {
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .any(|m| {
                m.identity == identity
                    && m.enabled
                    && m.error.is_none()
                    && local.is_none_or(|l| &m.local == l)
            })
    }
    /// Applies the effects of a succeeded chain, in call order (19.6, At apply).
    pub(crate) fn apply_effects(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                Effect::TimerAfter { owner, id, due, .. } => self
                    .timers
                    .arm(crate::state::timer_key(&owner.identity, &id), due),
                Effect::TimerCancel { .. } => {}
                Effect::Open {
                    ref producer,
                    ref origin,
                    ..
                } => {
                    let valid = self.mod_installed(&producer.identity, Some(&producer.local))
                        && self.mod_installed(origin, None);
                    self.hub.apply(self, effect, valid);
                }
                other => self.hub.apply(self, other, true),
            }
        }
    }
}
