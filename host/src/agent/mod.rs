//! The agent layer (TECHNICAL 19): the host functions of `streams` and `timers` at the
//! call, the effects that a chain applies after its durable commit (19.6) and the
//! application of those effects.

pub(crate) mod accounts;
pub(crate) mod claude;
pub(crate) mod consent;
pub(crate) mod streams;
pub(crate) mod timers;
pub(crate) mod tools;

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
        origin_gen: Option<u64>,
        /// The consumer's `Call` link interface that `open` ran through (19.2, Ends).
        via: Option<watane_pkg::InterfaceName>,
        /// The install generations of the producer and of a mod consumer at the call: at apply
        /// a side updated, removed or reinstalled since ends the stream `gone` (19.2, Ends).
        producer_gen: Option<u64>,
        consumer_gen: Option<u64>,
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
        /// The consumer the call acted for: applied only while it still is the stream's, and
        /// the stream has changed consumer as often as it had at the call.
        by: Consumer,
        handoffs: u32,
    },
    HandOff {
        id: String,
        to: ModRef,
        /// The install generation of `to` at the call (19.6, At apply).
        to_gen: Option<u64>,
    },
    Resume {
        id: String,
        by: Consumer,
        handoffs: u32,
    },
    TimerAfter {
        owner: ModRef,
        id: String,
        due: u64,
        delay_ms: u64,
        origin: String,
        origin_gen: Option<u64>,
    },
    TimerCancel {
        owner: ModRef,
        id: String,
    },
    /// `claude-code.start` or `claude-code-shell.start` (19.3).
    ClaudeStart {
        id: String,
        consumer: ModRef,
        consumer_gen: Option<u64>,
        origin: String,
        origin_gen: Option<u64>,
        launch: Arc<claude::launch::Launch>,
        shell: bool,
    },
    /// `claude-session.say`, `interrupt` or `compact` (19.3).
    ClaudeWrite {
        id: String,
        what: claude::Write,
        by: ModRef,
    },
    /// `agent-tools.open` (19.5).
    AgentOpen {
        id: String,
        /// The caller name of the handle, `h<30 hex>`.
        name: String,
        owner: ModRef,
        owner_gen: Option<u64>,
        origin: String,
        origin_gen: Option<u64>,
        toolsets: Vec<String>,
    },
    /// `agent-tools.call` (19.5).
    AgentCall {
        handle: String,
        key: String,
        /// The id of the job's stream.
        id: String,
        local: LocalName,
        /// `<interface>.<function>` of the tool.
        function: String,
        args: String,
        owner: ModRef,
        owner_gen: Option<u64>,
        origin: String,
        origin_gen: Option<u64>,
    },
    /// A harness only: a request that a fake answered (19.1, Harness).
    Fake {
        function: &'static str,
        args: Value,
    },
}
impl Effect {
    /// The key of the consumer's 32-streams limit, for an effect that opens a stream.
    fn consumer_key(&self) -> Option<String> {
        match self {
            Self::Open { consumer, .. } => consumer.key(),
            Self::ClaudeStart { consumer, .. } => Consumer::Mod(consumer.clone()).key(),
            _ => None,
        }
    }
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
            Effect::Fake { function, args } => Some(RequestRecord {
                function: (*function).into(),
                args: args.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The timer writes of a top-level commit, in call order (19.6, 7).
/// An origin client that is no longer installed and enabled becomes the timer's own mod (19, Terms).
pub(crate) fn timer_ops(
    log: &[Effect],
    installed: impl Fn(&str, Option<u64>) -> bool,
) -> Vec<crate::state::TimerOp> {
    log.iter()
        .filter_map(|e| match e {
            Effect::TimerAfter {
                owner,
                id,
                due,
                origin,
                origin_gen,
                ..
            } => Some(crate::state::TimerOp::After {
                key: crate::state::timer_key(&owner.identity, id),
                due: *due,
                origin: if installed(origin, *origin_gen) {
                    origin.clone()
                } else {
                    owner.identity.clone()
                },
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
    handoffs: u32,
    in_chain: bool,
}

impl Invocation {
    fn me(&self) -> ModRef {
        ModRef {
            local: self.local.clone(),
            identity: self.identity.clone(),
        }
    }
    /// The install generation of a mod as this chain's snapshot has it: the install that runs.
    fn package_of(&self, local: &LocalName) -> Option<u64> {
        self.chain.snapshot.install_of(local)
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
        self.view_of(log, id, false)
    }
    /// The view for a consumer's `resume` and `cancel`, which also reach a stream that its
    /// producer ended while items wait for the consumer (19.2, Deferred batch).
    fn consumer_view(&self, log: &[Effect], id: &str) -> Option<View> {
        self.view_of(log, id, true)
    }
    fn view_of(&self, log: &[Effect], id: &str, closing: bool) -> Option<View> {
        let mut view = log.iter().find_map(|e| match e {
            Effect::Open {
                id: i,
                consumer,
                producer,
                origin,
                ..
            } if i == id => Some(View {
                consumer: consumer.clone(),
                producer: Producer::Mod(producer.clone()),
                origin: origin.clone(),
                handed_off: false,
                handoffs: 0,
                in_chain: true,
            }),
            // A stream that `claude-code.start` opened in this chain (19.2, Commit).
            Effect::ClaudeStart {
                id: i,
                consumer,
                origin,
                ..
            } if i == id => Some(View {
                consumer: Consumer::Mod(consumer.clone()),
                producer: Producer::Host,
                origin: origin.clone(),
                handed_off: false,
                handoffs: 0,
                in_chain: true,
            }),
            _ => None,
        });
        if view.is_none() {
            let info = if closing {
                self.chain.host.hub.info_closing(id)?
            } else {
                self.chain.host.hub.info(id)?
            };
            view = Some(View {
                consumer: info.consumer,
                producer: info.producer,
                origin: info.origin,
                handed_off: info.handed_off,
                handoffs: info.handoffs,
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
                    view.handoffs += 1;
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
    inv.chain
        .host
        .hub
        .check_push(id, body.len(), (count, bytes), origin_pending, host_pending)?;
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
        ("claude-code", "start") => claude::start(inv, claude::launch::Kind::Code, args),
        ("claude-code-shell", "start") => claude::start(inv, claude::launch::Kind::Shell, args),
        ("claude-session", _) => claude::session_call(inv, function, args),
        ("agent-tools", _) => tools::call(inv, function, args),
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
                    if e.consumer_key().as_deref() == Some(key.as_str()) {
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
            let consumer_ref = consumer.clone();
            log.push(Effect::Open {
                id: id.clone(),
                consumer,
                producer: me,
                origin: inv.origin.clone(),
                origin_gen: inv.origin_gen,
                via: inv.via.clone(),
                producer_gen: inv.package_of(&inv.local),
                consumer_gen: match &consumer_ref {
                    Consumer::Mod(m) => inv.package_of(&m.local),
                    _ => None,
                },
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
            if let Some(view) = inv.consumer_view(&log, &id)
                && inv.acts_for(&view.consumer)
            {
                log.push(Effect::Cancel {
                    id,
                    by: view.consumer,
                    handoffs: view.handoffs,
                });
            }
            Ok(Value::Null)
        }
        ("streams", "resume") => {
            let id = name(0)?;
            let mut log = inv.log();
            let Some(view) = inv.consumer_view(&log, &id) else {
                return Ok(err(StreamError::UnknownStream));
            };
            if !inv.acts_for(&view.consumer) {
                return Ok(err(StreamError::NotYours));
            }
            log.push(Effect::Resume {
                id,
                by: view.consumer,
                handoffs: view.handoffs,
            });
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
            let to_gen = inv.package_of(&to.local);
            log.push(Effect::HandOff { id, to, to_gen });
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
                origin_gen: inv.origin_gen,
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
                origin_gen: None,
                via: None,
                producer_gen: None,
                consumer_gen: None,
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
                    origin_gen: None,
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
        ("agent-tools", "open") => {
            let n = inv
                .chain
                .host
                .fake_serial
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                + 1;
            result = json!({"ok": format!("{n:032x}")});
            log.push(Effect::Fake {
                function: "agent-tools.open",
                args: json!({"toolsets": args.first().map(json::encode).transpose()?}),
            });
        }
        ("agent-tools", "describe") => result = json!({"ok": []}),
        ("agent-tools", "call") => {
            let n = inv
                .chain
                .host
                .fake_serial
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                + 1;
            result = json!({"ok": format!("s{n}")});
            log.push(Effect::Fake {
                function: "agent-tools.call",
                args: json!({
                    "handle": args.first().map(json::encode).transpose()?,
                    "key": args.get(1).map(json::encode).transpose()?,
                    "tool": args.get(2).map(json::encode).transpose()?,
                    "args": args.get(3).map(json::encode).transpose()?,
                }),
            });
        }
        ("agent-tools", "close") => result = Value::Null,
        ("claude-code" | "claude-code-shell", "start") => {
            let n = inv
                .chain
                .host
                .fake_serial
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
                + 1;
            result = json!({"ok": format!("s{n}")});
            log.push(Effect::Fake {
                function: if interface == "claude-code" {
                    "claude-code.start"
                } else {
                    "claude-code-shell.start"
                },
                args: json!({"launch": args.first().map(json::encode).transpose()?}),
            });
        }
        ("claude-session", "say" | "answer" | "interrupt" | "compact") => {
            let function_name = match function {
                "say" => "claude-session.say",
                "answer" => "claude-session.answer",
                "interrupt" => "claude-session.interrupt",
                _ => "claude-session.compact",
            };
            let names: &[&str] = if function == "answer" {
                &["s", "ask", "d"]
            } else {
                &["s", "content"]
            };
            let mut named = serde_json::Map::new();
            for (name, value) in names.iter().zip(args) {
                named.insert((*name).to_owned(), json::encode(value)?);
            }
            log.push(Effect::Fake {
                function: function_name,
                args: Value::Object(named),
            });
        }
        _ => {}
    }
    Ok(fakes
        .get(&format!("watane:host/{interface}"))
        .and_then(|f| f.get(function))
        .cloned()
        .unwrap_or(result))
}

/// Whether an enabled, loaded mod has this identity, and the install generation `pkg`
/// (19.6, At apply); with `local`, under that local name.
fn live_mod(
    catalog: &crate::catalog::Catalog,
    local: Option<&LocalName>,
    identity: &str,
    pkg: Option<u64>,
) -> bool {
    catalog.mods.iter().any(|m| {
        m.identity == identity
            && m.enabled
            && m.error.is_none()
            && local.is_none_or(|l| &m.local == l)
            && pkg.is_some_and(|g| catalog.install_of(&m.local) == Some(g))
    })
}

/// Whether the consumer still holds the enabled mod, the `Call` link `via` to this provider and
/// the grant of it (19.2, Ends; Q46).
fn via_holds(
    catalog: &crate::catalog::Catalog,
    consumer: &ModRef,
    producer: &LocalName,
    via: &watane_pkg::InterfaceName,
) -> bool {
    catalog.mods.iter().any(|m| {
        m.local == consumer.local
            && m.identity == consumer.identity
            && m.enabled
            && m.links.iter().any(|l| {
                matches!(l.kind, crate::LinkKind::Call)
                    && l.interface == *via
                    && l.provider.as_ref() == Some(producer)
            }) && m.grants.iter().any(|g| {
            g.granted
                && matches!(&g.grant, watane_pkg::Grant::Call { interface, .. } if interface == via)
        })
    })
}

impl Host {
    /// Ends the mod streams whose consumer lost the `Call` grant, the link or the provider that
    /// `open` ran through (19.2, Ends). Called after every catalog write.
    pub(crate) fn end_lost_streams(&self) {
        let vias = self.hub.vias();
        if vias.is_empty() {
            return;
        }
        let lost: Vec<String> = {
            let catalog = self
                .catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            vias.into_iter()
                .filter(|(_, consumer, producer, via)| {
                    !via_holds(&catalog, consumer, producer, via)
                })
                .map(|(id, ..)| id)
                .collect()
        };
        self.hub.end_gone(&lost);
    }
    /// Runs `f` with the bytes that the chains in flight have queued for `origin` and in all,
    /// holding their gate (19.2, Commit): host admission counts them as a mod's push does.
    pub(crate) fn with_pending<R>(&self, origin: &str, f: impl FnOnce((usize, usize)) -> R) -> R {
        let chains = self
            .chains
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let pending = pending_in_flight(&chains);
        f((pending.0.get(origin).copied().unwrap_or(0), pending.1))
    }
    /// The pending bytes of the chains in flight, by origin and in all, without waiting for their
    /// gate: `None` when the gate or a chain's log is busy. The hub lock is held by the caller,
    /// which is after the gate in the lock order, so it may only try (19.2, Commit).
    pub(crate) fn try_pending(&self) -> Option<Pending> {
        let chains = self.chains.try_lock().ok()?;
        let logs: Option<Vec<_>> = chains
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .map(|c| c.log.try_lock().ok().map(|log| pending_of(&log)))
            .collect();
        Some(sum_pending(logs?))
    }
    /// A chain ended, committed or rolled back: its pending bytes are no longer held, so the
    /// producers refused for them are scanned again, with the gate held (19.2, In flight).
    pub(crate) fn chain_ended(&self) {
        let chains = self
            .chains
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        self.hub.scan_ready_with(pending_in_flight(&chains));
    }
    /// Applies the effects of a succeeded chain, in call order (19.6, At apply).
    pub(crate) fn apply_effects(&self, effects: Vec<Effect>) {
        // A `claude` process starts after the whole chain is applied, so that a `cancel` of the
        // same chain ends the stream before any process starts (19.3, At apply).
        let mut runs = Vec::new();
        for effect in effects {
            match effect {
                Effect::TimerAfter { owner, id, due, .. } => self
                    .timers
                    .arm(crate::state::timer_key(&owner.identity, &id), due),
                Effect::TimerCancel { .. } => {}
                Effect::Open {
                    ref producer,
                    ref consumer,
                    ref origin,
                    origin_gen,
                    ref via,
                    producer_gen,
                    consumer_gen,
                    ..
                } => {
                    // The checks of the call are made again where the effect lands, under the
                    // catalog lock that a catalog change takes too (19.6, At apply).
                    let catalog = self
                        .catalog
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it");
                    let live = |local: Option<&LocalName>, identity: &str, pkg: Option<u64>| {
                        live_mod(&catalog, local, identity, pkg)
                    };
                    let valid = live(Some(&producer.local), &producer.identity, producer_gen)
                        && live(None, origin, origin_gen)
                        && match consumer {
                            Consumer::Mod(c) => {
                                live(Some(&c.local), &c.identity, consumer_gen)
                                    && via.as_ref().is_none_or(|via| {
                                        via_holds(&catalog, c, &producer.local, via)
                                    })
                            }
                            _ => true,
                        };
                    self.hub.apply(self, effect, valid);
                }
                Effect::HandOff { ref to, to_gen, .. } => {
                    // The new consumer must still be the install that was called (19.6, At apply).
                    let catalog = self
                        .catalog
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it");
                    let valid = catalog.mods.iter().any(|m| {
                        m.local == to.local
                            && m.identity == to.identity
                            && m.enabled
                            && m.error.is_none()
                            && to_gen.is_some_and(|g| catalog.install_of(&m.local) == Some(g))
                    });
                    self.hub.apply(self, effect, valid);
                }
                Effect::ClaudeStart {
                    id,
                    consumer,
                    consumer_gen,
                    origin,
                    origin_gen,
                    launch,
                    shell,
                } => {
                    // The stream opens under the catalog lock, so that a `remove` of the
                    // origin either comes first (not valid) or ends the stream (19.6, At apply).
                    let catalog = self
                        .catalog
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it");
                    let valid = {
                        let grant = if shell {
                            watane_pkg::Grant::ClaudeCodeShell
                        } else {
                            watane_pkg::Grant::ClaudeCode
                        };
                        live_mod(
                            &catalog,
                            Some(&consumer.local),
                            &consumer.identity,
                            consumer_gen,
                        ) && live_mod(&catalog, None, &origin, origin_gen)
                            && catalog.mods.iter().any(|m| {
                                m.local == consumer.local
                                    && m.grants.iter().any(|g| g.granted && g.grant == grant)
                            })
                    };
                    runs.extend(
                        self.claude_start(id, consumer, origin, origin_gen, launch, shell, valid),
                    );
                    drop(catalog);
                }
                Effect::ClaudeWrite { id, what, by } => self.claude_write(&id, what, &by),
                Effect::AgentOpen {
                    id,
                    name,
                    owner,
                    owner_gen,
                    origin,
                    origin_gen,
                    toolsets,
                } => {
                    // The owner and the origin client must still be the installs of the call.
                    let valid = {
                        let catalog = self
                            .catalog
                            .lock()
                            .expect("lock poisoned: another thread panicked while holding it");
                        live_mod(&catalog, Some(&owner.local), &owner.identity, owner_gen)
                            && live_mod(&catalog, None, &origin, origin_gen)
                    };
                    self.agent_open(
                        id, name, owner, owner_gen, origin, origin_gen, toolsets, valid,
                    );
                }
                Effect::AgentCall {
                    handle,
                    key,
                    id,
                    local,
                    function,
                    args,
                    owner,
                    owner_gen,
                    origin,
                    origin_gen,
                } => {
                    let valid = {
                        let catalog = self
                            .catalog
                            .lock()
                            .expect("lock poisoned: another thread panicked while holding it");
                        live_mod(&catalog, Some(&owner.local), &owner.identity, owner_gen)
                            && live_mod(&catalog, None, &origin, origin_gen)
                    };
                    self.agent_call(
                        handle,
                        key,
                        id,
                        (local, function),
                        args,
                        owner,
                        (origin, origin_gen),
                        valid,
                    );
                }
                Effect::Fake { .. } => {}
                other => self.hub.apply(self, other, true),
            }
        }
        for job in runs {
            self.claude_run(job);
        }
    }
}

/// The bytes that chains in flight have queued, by origin and in all (19.2, Commit).
/// The third part is each stream's own pending items and bytes, as a push counts them.
pub(crate) type Pending = (
    std::collections::HashMap<String, usize>,
    usize,
    std::collections::HashMap<String, (usize, usize)>,
);

fn pending_of(log: &[Effect]) -> Pending {
    let (mut by_origin, mut all, mut by_stream) = (
        std::collections::HashMap::new(),
        0,
        std::collections::HashMap::new(),
    );
    for e in log {
        if let Effect::Push {
            body, origin, id, ..
        } = e
        {
            *by_origin.entry(origin.clone()).or_insert(0) += body.len();
            all += body.len();
            let c: &mut (usize, usize) = by_stream.entry(id.clone()).or_insert((0, 0));
            c.0 += 1;
            c.1 += body.len();
        }
    }
    (by_origin, all, by_stream)
}
/// The pending bytes of the chains in flight, under their gate.
fn pending_in_flight(chains: &[std::sync::Weak<Chain>]) -> Pending {
    sum_pending(
        chains
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .map(|c| {
                pending_of(
                    &c.log
                        .lock()
                        .expect("lock poisoned: another thread panicked while holding it"),
                )
            })
            .collect(),
    )
}
fn sum_pending(logs: Vec<Pending>) -> Pending {
    let (mut by_origin, mut all, mut by_stream) = (
        std::collections::HashMap::new(),
        0,
        std::collections::HashMap::new(),
    );
    for (o, a, st) in logs {
        for (k, v) in o {
            *by_origin.entry(k).or_insert(0) += v;
        }
        for (k, v) in st {
            let c: &mut (usize, usize) = by_stream.entry(k).or_insert((0, 0));
            c.0 += v.0;
            c.1 += v.1;
        }
        all += a;
    }
    (by_origin, all, by_stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, Mode};
    use std::path::Path;
    use std::time::{Duration, Instant};

    fn call(host: &Arc<Host>, target: &str, function: &str, args: Value) -> Value {
        let host = Arc::clone(host);
        let target = LocalName::parse(target).unwrap();
        let function = function.to_owned();
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || host.call(&Caller::Page, &target, &function, args).unwrap())
            .unwrap()
            .join()
            .unwrap()
    }
    fn ended(host: &Arc<Host>, target: &str) -> Value {
        call(host, target, "api.ended", Value::Null)
    }
    /// A host with `advo-front` as the consumer of a stream of `advo`, opened through its link.
    fn with_stream() -> (tempfile::TempDir, Arc<Host>, String) {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let folders: Vec<_> = ["latch", "relay", "advo", "advo-front"]
            .iter()
            .map(|n| {
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/mods")
                    .join(n)
            })
            .collect();
        host.install_folders(&folders, 0).unwrap();
        let s = call(&host, "advo-front", "api.open", Value::Null)["ok"]
            .as_str()
            .unwrap()
            .to_owned();
        (dir, host, s)
    }
    fn push(host: &Arc<Host>, s: &str) -> String {
        call(host, "advo", "api.push", json!({"s": s, "bytes": 1}))
            .as_str()
            .unwrap()
            .to_owned()
    }
    /// Sets the provider of `advo-front`'s `mods:advo/` link to `provider` and runs the check.
    fn rebind(host: &Arc<Host>, provider: Option<&str>) {
        for m in &mut host
            .catalog
            .lock()
            .unwrap()
            .mods
            .iter_mut()
            .filter(|m| m.local.as_ref() == "advo-front")
        {
            for l in &mut m.links {
                if l.interface.to_string().starts_with("mods:advo/") {
                    l.provider = provider.map(|p| LocalName::parse(p).unwrap());
                }
            }
        }
        host.end_lost_streams();
    }
    fn both_gone(host: &Arc<Host>, s: &str) {
        let start = Instant::now();
        let line = json!([format!("{s} gone")]);
        while ended(host, "advo") != line || ended(host, "advo-front") != line {
            assert!(start.elapsed() < Duration::from_secs(10), "no gone on both");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// 19.3 step 5 (review r7, F7.1): the pending of the chains in flight counts each stream's
    /// own items and bytes, summed over the chains, beside the origin's and host's bytes.
    #[test]
    fn s100_fix19_pending_counts_each_streams_items_and_bytes_over_the_chains() {
        let push = |id: &str, n: usize, origin: &str| Effect::Push {
            id: id.into(),
            body: vec![0; n],
            hop: 0,
            by: ModRef {
                local: LocalName::parse("m").unwrap(),
                identity: "i".into(),
            },
            origin: origin.into(),
        };
        let one = pending_of(&[push("a", 3, "o"), push("a", 5, "o"), push("b", 7, "p")]);
        let two = pending_of(&[push("a", 11, "o")]);
        let (by_origin, all, by_stream) = sum_pending(vec![one, two]);
        assert_eq!(
            by_origin,
            std::collections::HashMap::from([("o".into(), 19), ("p".into(), 7)])
        );
        assert_eq!(all, 26);
        assert_eq!(
            by_stream,
            std::collections::HashMap::from([("a".into(), (3, 19)), ("b".into(), (1, 7))])
        );
    }

    #[test]
    fn ac5_a_link_that_is_unbound_ends_the_stream_gone() {
        let (_dir, host, s) = with_stream();
        host.end_lost_streams();
        assert_eq!(push(&host, &s), "ok");
        rebind(&host, None);
        assert!(push(&host, &s).contains("unknown-stream"));
        both_gone(&host, &s);
    }

    #[test]
    fn ac5_a_link_moved_to_another_provider_ends_the_stream_gone() {
        let (_dir, host, s) = with_stream();
        rebind(&host, Some("relay"));
        assert!(push(&host, &s).contains("unknown-stream"));
        both_gone(&host, &s);
    }
}
