//! H1: the stream table, FIFO delivery to consumers, batches, `taken`/`defer`, the lag
//! clock, the deferral backstop, byte budgets, ends and page connections
//! (TECHNICAL 19.2, 8.4, 9.3).

#![cfg_attr(
    not(feature = "test-streams"),
    allow(dead_code, reason = "the test streams of 19.9 use them")
)]

use super::{Effect, ModRef};
use crate::runtime::call::{Args, Start, Who, run_start};
use crate::{Caller, Host, LocalName};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::task::Waker;
use std::time::{Duration, Instant};
use wasmtime::component::Val;
use watane_pkg::InterfaceName;

pub(crate) const MAX_ITEMS: usize = 256;
pub(crate) const MAX_BYTES: usize = 4 << 20;
pub(crate) const MAX_ITEM: usize = 4 << 20;
const BATCH_ITEMS: usize = 256;
const BATCH_BYTES: usize = 1 << 20;
const ORIGIN_BUDGET: usize = 128 << 20;
const HOST_BUDGET: usize = 512 << 20;
const CONN_BUDGET: usize = 16 << 20;
pub(crate) const MAX_OPEN: usize = 32;
const LAG: Duration = Duration::from_secs(10);
const BACKSTOP: Duration = Duration::from_secs(300);
pub(crate) const STREAM_SINK: &str = "watane:host/stream-sink@0.1.0";
pub(crate) const STREAM_PRODUCER: &str = "watane:host/stream-producer@0.1.0";

/// The WIT `stream-error` (19.1).
#[derive(Debug, Clone, PartialEq)]
pub enum StreamError {
    NotYours,
    UnknownStream,
    Full,
    TooLarge,
}
impl StreamError {
    pub(crate) fn case(&self) -> &'static str {
        match self {
            Self::NotYours => "not-yours",
            Self::UnknownStream => "unknown-stream",
            Self::Full => "full",
            Self::TooLarge => "too-large",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Consumer {
    Mod(ModRef),
    Page {
        conn: String,
        local: LocalName,
    },
    /// Test only: takes a batch only when the test asks (19.9).
    Held {
        origin: String,
    },
}
impl Consumer {
    /// The key of the 32-streams limit; none for a held consumer.
    pub fn key(&self) -> Option<String> {
        match self {
            Self::Mod(m) => Some(m.identity.clone()),
            Self::Page { conn, .. } => Some(format!("page:{conn}")),
            Self::Held { .. } => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Producer {
    Host,
    Mod(ModRef),
}

#[derive(Clone, Debug)]
struct Item {
    seq: u64,
    body: Vec<u8>,
    hop: u8,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Idle,
    Queued,
    Running,
}
#[derive(Clone, Copy, Debug)]
enum Refused {
    Stream,
    Origin(usize),
    Host(usize),
}
#[derive(Debug)]
struct Stream {
    consumer: Consumer,
    producer: Producer,
    origin: String,
    /// The install generation of the origin client when the stream opened (19, Terms).
    origin_gen: Option<u64>,
    next_seq: u64,
    queue: VecDeque<Item>,
    bytes: usize,
    state: State,
    retry_len: Option<usize>,
    failed_once: bool,
    deferred: Option<Instant>,
    full_since: Option<Instant>,
    needs_ready: Option<Refused>,
    closing: Option<Value>,
    producer_told: bool,
    handed_off: bool,
    /// How many times the stream changed consumer: a `cancel` or `resume` recorded at another
    /// count is stale, even if the consumer is the same again (19.6, At apply).
    handoffs: u32,
    test: bool,
    /// The consumer's `Call` link interface that `open` ran through (19.2, Ends).
    via: Option<InterfaceName>,
}
impl Stream {
    fn new(
        consumer: Consumer,
        producer: Producer,
        (origin, origin_gen): (String, Option<u64>),
        test: bool,
        via: Option<InterfaceName>,
    ) -> Self {
        Self {
            consumer,
            producer,
            origin,
            origin_gen,
            next_seq: 1,
            queue: VecDeque::new(),
            bytes: 0,
            state: State::Idle,
            retry_len: None,
            failed_once: false,
            deferred: None,
            full_since: None,
            needs_ready: None,
            closing: None,
            producer_told: false,
            handed_off: false,
            handoffs: 0,
            test,
            via,
        }
    }
}

/// What a call may know of a stream (19.2, checks of the call).
#[derive(Clone, Debug)]
pub(crate) struct Info {
    pub consumer: Consumer,
    pub producer: Producer,
    pub origin: String,
    pub handed_off: bool,
    pub handoffs: u32,
}

#[derive(Debug)]
struct Conn {
    local: LocalName,
    streams: Vec<String>,
    ends: VecDeque<(String, Value)>,
    written: Option<(String, usize)>,
    waker: Option<Waker>,
    ended: bool,
    turn: usize,
}

#[derive(Debug)]
enum Notify {
    SinkEnded(Value),
    ProducerEnded(Value),
    Ready,
}
#[derive(Debug)]
enum Task {
    Items(String),
    Notify {
        sid: String,
        origin: String,
        origin_gen: Option<u64>,
        what: Notify,
    },
}

/// How an `items` call came back (19.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeliveryAnswer {
    Taken,
    Deferred,
    Failed,
}
/// One `items` call of a test stream (19.9).
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    pub seqs: Vec<u64>,
    pub answer: DeliveryAnswer,
}

#[derive(Debug)]
struct ClockShared {
    now: Mutex<Instant>,
    hub: Mutex<Option<Weak<Hub>>>,
}
/// The clock of the lag clock and the deferral backstop (19.2). Real unless a test replaces it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Clock(Option<Arc<ClockShared>>);
impl Clock {
    fn now(&self) -> Instant {
        self.0.as_ref().map_or_else(Instant::now, |c| {
            *c.now
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
        })
    }
}
/// A clock that tests move (19.9).
#[derive(Clone, Debug)]
pub struct TestClock(Arc<ClockShared>);
impl Default for TestClock {
    fn default() -> Self {
        Self::new()
    }
}
impl TestClock {
    pub fn new() -> Self {
        Self(Arc::new(ClockShared {
            now: Mutex::new(Instant::now()),
            hub: Mutex::new(None),
        }))
    }
    pub fn advance(&self, by: Duration) {
        *self
            .0
            .now
            .lock()
            .expect("lock poisoned: another thread panicked while holding it") += by;
        let hub = self
            .0
            .hub
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .clone();
        if let Some(hub) = hub.and_then(|h| h.upgrade()) {
            let _guard = hub.lock();
            hub.cv.notify_all();
        }
    }
}

#[derive(Default)]
struct Inner {
    streams: HashMap<String, Stream>,
    open: HashMap<String, usize>,
    origin_bytes: HashMap<String, usize>,
    host_bytes: usize,
    conns: HashMap<String, Conn>,
    tasks: BTreeMap<LocalName, VecDeque<Task>>,
    busy: HashSet<LocalName>,
    waiting: BTreeSet<String>,
    clock: Clock,
    ended_host: HashMap<String, Value>,
    deliveries: HashMap<String, Vec<Delivery>>,
    /// Run when a host stream ends: the claude session kills its process group (19.3).
    on_end: HashMap<String, Box<dyn FnOnce() + Send>>,
}

pub(crate) struct Hub {
    inner: Mutex<Inner>,
    cv: Condvar,
}
impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub").finish_non_exhaustive()
    }
}

pub(crate) fn random_id() -> String {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).expect("the system random source works");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
/// The case name of an outcome in the JSON of 6.9.
pub(crate) fn case_of(outcome: &Value) -> String {
    match outcome {
        Value::String(s) => s.clone(),
        Value::Object(o) => o.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}
fn batch_len(queue: &VecDeque<Item>) -> usize {
    let mut bytes = 0;
    let mut n = 0;
    for item in queue {
        if n >= BATCH_ITEMS || (n > 0 && bytes + item.body.len() > BATCH_BYTES) {
            break;
        }
        bytes += item.body.len();
        n += 1;
    }
    n
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner::default()),
            cv: Condvar::new(),
        })
    }
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }

    /// Starts the delivery worker. It holds a `Weak` of the host and stops with it (19.2, Driver).
    pub fn start(self: &Arc<Self>, host: &Arc<Host>) -> std::io::Result<()> {
        let hub = Arc::clone(self);
        let weak = Arc::downgrade(host);
        std::thread::Builder::new()
            .name("watane-streams".into())
            .spawn(move || hub.run(&weak))
            .map(|_| ())
    }
    fn run(self: &Arc<Self>, weak: &Weak<Host>) {
        let mut inner = self.lock();
        loop {
            if weak.strong_count() == 0 {
                return;
            }
            inner.check_clocks();
            let next = inner.next_task();
            let Some((local, task)) = next else {
                inner = self
                    .cv
                    .wait_timeout(inner, Duration::from_millis(50))
                    .expect("lock poisoned: another thread panicked while holding it")
                    .0;
                continue;
            };
            drop(inner);
            if let Some(host) = weak.upgrade() {
                let hub = Arc::clone(self);
                let spawned = std::thread::Builder::new()
                    .name("watane-delivery".into())
                    .stack_size(16 << 20)
                    .spawn(move || {
                        hub.exec(&host, &local, task);
                        drop(host);
                        hub.lock().busy.remove(&local);
                        hub.cv.notify_all();
                    });
                if spawned.is_err() {
                    eprintln!("watane: cannot start a stream delivery thread");
                }
            }
            inner = self.lock();
        }
    }

    // ---- calls: checks at the call (19.2, Commit) -------------------------------------

    pub fn info(&self, id: &str) -> Option<Info> {
        self.info_of(id, false)
    }
    /// Like `info`, also for a stream that its producer ended and whose consumer has not taken
    /// every item yet: the consumer may still `resume` or `cancel` it (19.2, Deferred batch).
    pub fn info_closing(&self, id: &str) -> Option<Info> {
        self.info_of(id, true)
    }
    fn info_of(&self, id: &str, closing: bool) -> Option<Info> {
        let inner = self.lock();
        let s = inner
            .streams
            .get(id)
            .filter(|s| closing || s.closing.is_none())?;
        Some(Info {
            consumer: s.consumer.clone(),
            producer: s.producer.clone(),
            origin: s.origin.clone(),
            handed_off: s.handed_off,
            handoffs: s.handoffs,
        })
    }
    pub fn open_count(&self, key: &str) -> usize {
        self.lock().open.get(key).copied().unwrap_or(0)
    }
    pub fn conn_live(&self, id: &str, local: &LocalName) -> bool {
        self.lock()
            .conns
            .get(id)
            .is_some_and(|c| &c.local == local && !c.ended)
    }
    /// The push check of a call: counts what the chain has queued already (`queued`: items and
    /// bytes of this stream in the chain, `origin_pending`, `host_pending`: bytes in the chain).
    /// A `full` is remembered for the lag clock and `ready` (19.2).
    pub fn check_push(
        &self,
        id: &str,
        len: usize,
        chain: (usize, usize),
        origin: &str,
        origin_pending: usize,
        host_pending: usize,
    ) -> Result<(), StreamError> {
        let mut inner = self.lock();
        let now = inner.clock.now();
        let origin_now = inner.origin_bytes.get(origin).copied().unwrap_or(0);
        let host_now = inner.host_bytes;
        let Some(s) = inner.streams.get_mut(id) else {
            return Ok(());
        };
        let refused =
            if s.queue.len() + chain.0 + 1 > MAX_ITEMS || s.bytes + chain.1 + len > MAX_BYTES {
                Some(Refused::Stream)
            } else if origin_now + origin_pending + len > ORIGIN_BUDGET {
                Some(Refused::Origin(len))
            } else if host_now + host_pending + len > HOST_BUDGET {
                Some(Refused::Host(len))
            } else {
                None
            };
        let Some(refused) = refused else {
            return Ok(());
        };
        if s.deferred.is_none() && s.full_since.is_none() {
            s.full_since = Some(now);
        }
        s.needs_ready = Some(refused);
        inner.waiting.insert(id.to_owned());
        Err(StreamError::Full)
    }

    // ---- applying effects (after the chain's commit) ------------------------------------

    pub fn apply(&self, host: &Host, effect: Effect, valid: bool) {
        let mut inner = self.lock();
        let hops = host.limits.hops;
        match effect {
            Effect::Open {
                id,
                consumer,
                producer,
                origin,
                origin_gen,
                via,
                ..
            } => {
                inner.create(
                    id.clone(),
                    consumer,
                    (Producer::Mod(producer), (origin, origin_gen), via),
                    false,
                );
                if !valid {
                    inner.end(&id, json!("gone"), false);
                }
            }
            Effect::Push {
                id, body, hop, by, ..
            } => {
                if hop >= hops {
                    eprintln!("watane: stream item hop limit reached");
                } else if inner
                    .streams
                    .get(&id)
                    .is_some_and(|s| s.producer == Producer::Mod(by.clone()))
                {
                    inner.enqueue(&id, body, hop);
                }
            }
            Effect::Close { id, outcome, by } => {
                if inner
                    .streams
                    .get(&id)
                    .is_some_and(|s| s.producer == Producer::Mod(by.clone()))
                {
                    let outcome = producer_outcome(outcome);
                    inner.end(&id, outcome, true);
                }
            }
            Effect::Cancel { id, by, handoffs } => {
                if inner
                    .streams
                    .get(&id)
                    .is_some_and(|s| s.consumer == by && s.handoffs == handoffs)
                {
                    inner.end(&id, json!("cancelled"), false);
                }
            }
            Effect::HandOff { id, .. } if !valid => {
                inner.end(&id, json!("gone"), false);
            }
            Effect::HandOff { id, to, .. } => {
                if let Some(s) = inner.streams.get_mut(&id) {
                    let old = s.consumer.key();
                    s.consumer = Consumer::Mod(to.clone());
                    s.handed_off = true;
                    s.handoffs += 1;
                    if let Some(old) = old {
                        inner.dec_open(&old);
                    }
                    *inner.open.entry(to.identity.clone()).or_default() += 1;
                    inner.schedule(&id);
                }
            }
            Effect::Resume { id, by, handoffs } => {
                if let Some(s) = inner.streams.get_mut(&id)
                    && s.consumer == by
                    && s.handoffs == handoffs
                    && s.deferred.take().is_some()
                {
                    inner.schedule(&id);
                }
            }
            Effect::TimerAfter { .. }
            | Effect::TimerCancel { .. }
            | Effect::ClaudeStart { .. }
            | Effect::ClaudeWrite { .. }
            | Effect::Fake { .. } => {}
        }
        drop(inner);
        self.cv.notify_all();
    }

    /// The mod streams that depend on a grant: (id, consumer, producer's local name, the
    /// consumer's link interface) (19.2, Ends).
    pub fn vias(&self) -> Vec<(String, ModRef, LocalName, InterfaceName)> {
        self.lock()
            .streams
            .iter()
            .filter_map(|(id, s)| match (&s.consumer, &s.producer, &s.via) {
                (Consumer::Mod(c), Producer::Mod(p), Some(via)) if s.closing.is_none() => {
                    Some((id.clone(), c.clone(), p.local.clone(), via.clone()))
                }
                _ => None,
            })
            .collect()
    }
    /// Ends these streams `gone` for both sides, at once.
    pub fn end_gone(&self, ids: &[String]) {
        if ids.is_empty() {
            return;
        }
        let mut inner = self.lock();
        for id in ids {
            inner.end(id, json!("gone"), false);
        }
        drop(inner);
        self.cv.notify_all();
    }
    /// Disable, removal or update of a mod: every stream it is a side or the origin client of
    /// ends `gone` (19.2, Ends).
    pub fn end_mod(&self, identity: &str) {
        let mut inner = self.lock();
        let ids: Vec<String> = inner
            .streams
            .iter()
            .filter(|(_, s)| {
                s.origin == identity
                    || matches!(&s.producer, Producer::Mod(p) if p.identity == identity)
                    || matches!(&s.consumer, Consumer::Mod(m) if m.identity == identity)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            inner.end(&id, json!("gone"), false);
        }
        drop(inner);
        self.cv.notify_all();
    }

    // ---- page connections (8.4, 9.3) ----------------------------------------------------

    pub fn open_conn(self: &Arc<Self>, local: &LocalName) -> ConnGuard {
        let id = random_id();
        self.lock().conns.insert(
            id.clone(),
            Conn {
                local: local.clone(),
                streams: vec![],
                ends: VecDeque::new(),
                written: None,
                waker: None,
                ended: false,
                turn: 0,
            },
        );
        ConnGuard {
            hub: Arc::clone(self),
            id,
        }
    }
    fn close_conn(&self, id: &str) {
        let mut inner = self.lock();
        let Some(conn) = inner.conns.remove(id) else {
            return;
        };
        for sid in conn.streams {
            inner.end(&sid, json!("gone"), false);
        }
        drop(inner);
        self.cv.notify_all();
    }
    /// The next message of a connection: a batch of items, or the end of a stream.
    pub fn poll_conn(&self, id: &str, waker: &Waker) -> ConnPoll {
        let mut inner = self.lock();
        let Some(conn) = inner.conns.get_mut(id) else {
            return ConnPoll::Closed;
        };
        if conn.ended {
            return ConnPoll::Closed;
        }
        // The previous batch's write completed: it is taken.
        if let Some((sid, n)) = conn.written.take() {
            inner.pop(&sid, n);
        }
        let conn = inner.conns.get_mut(id).expect("checked above");
        let count = conn.streams.len();
        let order: Vec<String> = (0..count)
            .map(|i| conn.streams[(conn.turn + i) % count].clone())
            .collect();
        for (i, sid) in order.into_iter().enumerate() {
            let Some(s) = inner.streams.get(&sid) else {
                continue;
            };
            if s.queue.is_empty() {
                continue;
            }
            let n = batch_len(&s.queue);
            let items: Vec<String> = s
                .queue
                .iter()
                .take(n)
                .map(|it| {
                    let body = Value::from(String::from_utf8_lossy(&it.body).into_owned());
                    format!(r#"{{"seq":{},"body":{body}}}"#, it.seq)
                })
                .collect();
            let message = format!(
                r#"{{"type":"stream","stream":"{sid}","items":[{}]}}"#,
                items.join(",")
            );
            let conn = inner.conns.get_mut(id).expect("checked above");
            conn.turn = (conn.turn + i + 1) % count.max(1);
            conn.written = Some((sid, n));
            return ConnPoll::Message(message);
        }
        let conn = inner.conns.get_mut(id).expect("checked above");
        if let Some((sid, outcome)) = conn.ends.pop_front() {
            return ConnPoll::Message(format!(
                r#"{{"type":"stream-end","stream":"{sid}","outcome":{outcome}}}"#
            ));
        }
        conn.waker = Some(waker.clone());
        ConnPoll::Pending
    }

    // ---- execution of deliveries --------------------------------------------------------

    fn exec(self: &Arc<Self>, host: &Arc<Host>, local: &LocalName, task: Task) {
        match task {
            Task::Items(sid) => self.exec_items(host, local, &sid),
            Task::Notify {
                sid,
                origin,
                origin_gen,
                what,
            } => {
                let (function, args) = match what {
                    Notify::SinkEnded(outcome) => (
                        format!("{STREAM_SINK}.ended"),
                        json!({"s": sid, "outcome": outcome}),
                    ),
                    Notify::ProducerEnded(outcome) => (
                        format!("{STREAM_PRODUCER}.ended"),
                        json!({"s": sid, "outcome": outcome}),
                    ),
                    Notify::Ready => (format!("{STREAM_PRODUCER}.ready"), json!({"s": sid})),
                };
                let mut start =
                    Start::internal(Who::Host, local.clone(), function, Args::Json(args), origin);
                start.origin_gen = origin_gen;
                let done = run_start(host, start);
                if let Err(error) = done.result {
                    eprintln!(
                        "watane: stream {sid} notification to {local} failed: {:?}: {}",
                        error.code, error.message
                    );
                }
            }
        }
    }
    fn exec_items(self: &Arc<Self>, host: &Arc<Host>, local: &LocalName, sid: &str) {
        let prepared = {
            let mut inner = self.lock();
            let Some(s) = inner.streams.get_mut(sid) else {
                return;
            };
            match &s.consumer {
                Consumer::Mod(m) if &m.local == local => {}
                Consumer::Mod(m) => {
                    // Handed off while queued: go to the new consumer's queue.
                    let to = m.local.clone();
                    inner
                        .tasks
                        .entry(to)
                        .or_default()
                        .push_back(Task::Items(sid.to_owned()));
                    return;
                }
                _ => {
                    s.state = State::Idle;
                    return;
                }
            }
            if s.deferred.is_some() || s.queue.is_empty() {
                s.state = State::Idle;
                return;
            }
            let n = s
                .retry_len
                .unwrap_or_else(|| batch_len(&s.queue))
                .min(s.queue.len());
            s.state = State::Running;
            let items: Vec<Item> = s.queue.iter().take(n).cloned().collect();
            (
                n,
                items,
                s.producer.clone(),
                (s.origin.clone(), s.origin_gen),
            )
        };
        let (n, items, producer, (origin, origin_gen)) = prepared;
        let hop = items
            .iter()
            .map(|i| i.hop)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let who = match &producer {
            Producer::Host => Who::Host,
            Producer::Mod(p) => Who::Caller(Caller::Mod(p.local.clone())),
        };
        let seqs: Vec<u64> = items.iter().map(|i| i.seq).collect();
        let vals = Val::List(
            items
                .iter()
                .map(|i| {
                    Val::Record(vec![
                        ("seq".into(), Val::U64(i.seq)),
                        (
                            "body".into(),
                            Val::List(i.body.iter().map(|b| Val::U8(*b)).collect()),
                        ),
                    ])
                })
                .collect(),
        );
        let mut start = Start::internal(
            who,
            local.clone(),
            format!("{STREAM_SINK}.items"),
            Args::Vals(vec![Val::String(sid.to_owned()), vals]),
            origin,
        );
        start.hop = hop;
        start.origin_gen = origin_gen;
        start.defers = true;
        let done = run_start(host, start);
        let taken = matches!(&done.result, Ok(Value::String(v)) if v == "taken");
        let mut inner = self.lock();
        let now = inner.clock.now();
        let Some(s) = inner.streams.get_mut(sid) else {
            return;
        };
        let record = |answer| Delivery {
            seqs: seqs.clone(),
            answer,
        };
        let test = s.test;
        if taken {
            s.failed_once = false;
            s.retry_len = None;
            s.state = State::Idle;
            if test {
                inner
                    .deliveries
                    .entry(sid.to_owned())
                    .or_default()
                    .push(record(DeliveryAnswer::Taken));
            }
            inner.pop(sid, n);
            inner.schedule(sid);
        } else if done.deferred {
            s.deferred = Some(now);
            s.full_since = None;
            s.failed_once = false;
            s.retry_len = None;
            s.state = State::Idle;
            if test {
                inner
                    .deliveries
                    .entry(sid.to_owned())
                    .or_default()
                    .push(record(DeliveryAnswer::Deferred));
            }
        } else {
            if let Err(error) = &done.result {
                eprintln!(
                    "watane: items of stream {sid} to {local} failed: {:?}: {}",
                    error.code, error.message
                );
            }
            let first_failure = !s.failed_once;
            if test {
                inner
                    .deliveries
                    .entry(sid.to_owned())
                    .or_default()
                    .push(record(DeliveryAnswer::Failed));
            }
            if first_failure {
                if let Some(s) = inner.streams.get_mut(sid) {
                    s.failed_once = true;
                    s.retry_len = Some(n);
                    s.state = State::Queued;
                }
                inner
                    .tasks
                    .entry(local.clone())
                    .or_default()
                    .push_front(Task::Items(sid.to_owned()));
            } else {
                inner.end(sid, json!("consumer-failed"), false);
            }
        }
        drop(inner);
        self.cv.notify_all();
    }

    // ---- host streams of `claude` (19.3) -----------------------------------------------

    /// Opens a host stream whose consumer is a mod; `on_end` runs once when it ends.
    pub fn open_host(
        &self,
        id: String,
        consumer: ModRef,
        origin: (String, Option<u64>),
        on_end: Box<dyn FnOnce() + Send>,
    ) {
        let mut inner = self.lock();
        inner.create(
            id.clone(),
            Consumer::Mod(consumer),
            (Producer::Host, origin, None),
            false,
        );
        inner.on_end.insert(id, on_end);
    }
    /// A push by the host as producer (19.2): `full` when the stream, the origin client or the
    /// host is at its limit, so that the source waits.
    pub fn host_push(&self, id: &str, body: &[u8]) -> Result<(), StreamError> {
        if body.len() > MAX_ITEM {
            return Err(StreamError::TooLarge);
        }
        let origin = {
            let inner = self.lock();
            match inner.streams.get(id) {
                Some(s) if s.closing.is_none() && s.producer == Producer::Host => s.origin.clone(),
                _ => return Err(StreamError::UnknownStream),
            }
        };
        self.check_push(id, body.len(), (0, 0), &origin, 0, 0)?;
        self.lock().enqueue(id, body.to_vec(), 0);
        self.cv.notify_all();
        Ok(())
    }
    /// The source of a host stream ended with `outcome`: the consumer takes what is queued,
    /// also after a deferral, and then gets `ended` (19.2, Deferred batch).
    pub fn finish(&self, id: &str, outcome: Value) {
        let mut inner = self.lock();
        if inner.streams.get(id).is_some_and(|s| s.closing.is_none()) {
            inner.end(id, outcome, true);
        }
        drop(inner);
        self.cv.notify_all();
    }

    // ---- test streams (19.9) -----------------------------------------------------------

    pub fn set_clock(self: &Arc<Self>, clock: &TestClock) {
        *clock
            .0
            .hub
            .lock()
            .expect("lock poisoned: another thread panicked while holding it") =
            Some(Arc::downgrade(self));
        self.lock().clock = Clock(Some(Arc::clone(&clock.0)));
        self.cv.notify_all();
    }
    pub fn test_open(&self, consumer: Consumer, origin: String) -> Result<String, StreamError> {
        let mut inner = self.lock();
        if let Some(key) = consumer.key()
            && inner.open.get(&key).copied().unwrap_or(0) >= MAX_OPEN
        {
            return Err(StreamError::Full);
        }
        let id = random_id();
        inner.create(
            id.clone(),
            consumer,
            (Producer::Host, (origin, None), None),
            true,
        );
        Ok(id)
    }
    /// A push by the host as producer, applied at once.
    pub fn test_push(&self, id: &str, body: &[u8]) -> Result<(), StreamError> {
        self.host_push(id, body)
    }
    pub fn test_paused(&self, id: &str) -> bool {
        self.lock().streams.get(id).is_some_and(|s| {
            s.deferred.is_some()
                || s.needs_ready.is_some()
                || s.queue.len() >= MAX_ITEMS
                || s.bytes >= MAX_BYTES
        })
    }
    pub fn test_close(&self, id: &str, outcome: Value) {
        let mut inner = self.lock();
        if inner
            .streams
            .get(id)
            .is_some_and(|s| s.producer == Producer::Host)
        {
            let outcome = producer_outcome(outcome);
            inner.end(id, outcome, true);
        }
        drop(inner);
        self.cv.notify_all();
    }
    pub fn test_ended(&self, id: &str) -> Option<Value> {
        self.lock().ended_host.get(id).cloned()
    }
    pub fn test_take(&self, id: &str) -> Vec<(u64, Vec<u8>)> {
        let mut inner = self.lock();
        let Some(s) = inner.streams.get(id) else {
            return vec![];
        };
        if !matches!(s.consumer, Consumer::Held { .. }) {
            return vec![];
        }
        let taken: Vec<(u64, Vec<u8>)> = s.queue.iter().map(|i| (i.seq, i.body.clone())).collect();
        let n = taken.len();
        inner.pop(id, n);
        taken
    }
    pub fn test_deliveries(&self, id: &str) -> Vec<Delivery> {
        self.lock().deliveries.get(id).cloned().unwrap_or_default()
    }
}

/// A producer's `close` may pass only `closed`, `replaced`, `lagging` or `failed` (19.1).
fn producer_outcome(outcome: Value) -> Value {
    let case = case_of(&outcome);
    if matches!(case.as_str(), "closed" | "replaced" | "lagging" | "failed") {
        outcome
    } else {
        json!({"failed": format!("{case} is the host's outcome")})
    }
}

impl Inner {
    fn create(
        &mut self,
        id: String,
        consumer: Consumer,
        (producer, origin, via): (Producer, (String, Option<u64>), Option<InterfaceName>),
        test: bool,
    ) {
        if let Some(key) = consumer.key() {
            *self.open.entry(key).or_default() += 1;
        }
        if let Consumer::Page { conn, .. } = &consumer {
            match self.conns.get_mut(conn) {
                Some(c) => c.streams.push(id.clone()),
                None => {
                    // The connection closed already: the stream is created and ends `gone`.
                    self.streams.insert(
                        id.clone(),
                        Stream::new(consumer, producer, origin, test, via),
                    );
                    self.end(&id, json!("gone"), false);
                    return;
                }
            }
        }
        self.streams
            .insert(id, Stream::new(consumer, producer, origin, test, via));
    }
    fn dec_open(&mut self, key: &str) {
        if let Some(n) = self.open.get_mut(key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.open.remove(key);
            }
        }
    }
    fn next_task(&mut self) -> Option<(LocalName, Task)> {
        let local = self
            .tasks
            .iter()
            .find(|(local, q)| !q.is_empty() && !self.busy.contains(*local))
            .map(|(local, _)| local.clone())?;
        let task = self.tasks.get_mut(&local)?.pop_front()?;
        self.busy.insert(local.clone());
        Some((local, task))
    }
    fn wake(&mut self, conn: &str) {
        if let Some(waker) = self.conns.get_mut(conn).and_then(|c| c.waker.take()) {
            waker.wake();
        }
    }
    /// Queues a batch delivery for a consumer mod, or wakes a page connection.
    fn schedule(&mut self, id: &str) {
        let Some(s) = self.streams.get_mut(id) else {
            return;
        };
        if s.queue.is_empty() {
            return;
        }
        match &s.consumer {
            Consumer::Mod(m) => {
                if s.state != State::Idle || s.deferred.is_some() {
                    return;
                }
                s.state = State::Queued;
                let local = m.local.clone();
                self.tasks
                    .entry(local)
                    .or_default()
                    .push_back(Task::Items(id.to_owned()));
            }
            Consumer::Page { conn, .. } => {
                let conn = conn.clone();
                self.wake(&conn);
            }
            Consumer::Held { .. } => {}
        }
    }
    fn enqueue(&mut self, id: &str, body: Vec<u8>, hop: u8) {
        let Some(s) = self.streams.get_mut(id) else {
            return;
        };
        if s.closing.is_some() {
            return;
        }
        let len = body.len();
        let seq = s.next_seq;
        s.next_seq += 1;
        s.bytes += len;
        s.queue.push_back(Item { seq, body, hop });
        let origin = s.origin.clone();
        let consumer = s.consumer.clone();
        *self.origin_bytes.entry(origin).or_default() += len;
        self.host_bytes += len;
        self.schedule(id);
        if let Consumer::Page { conn, .. } = consumer {
            self.check_conn_budget(&conn);
        }
    }
    /// A page connection past 16 MiB of queued items ends; its streams end `lagging` (9.3).
    fn check_conn_budget(&mut self, conn: &str) {
        let Some(c) = self.conns.get(conn) else {
            return;
        };
        let bytes: usize = c
            .streams
            .iter()
            .filter_map(|id| self.streams.get(id))
            .map(|s| s.bytes)
            .sum();
        if bytes <= CONN_BUDGET {
            return;
        }
        let ids = c.streams.clone();
        if let Some(c) = self.conns.get_mut(conn) {
            c.ended = true;
        }
        for id in ids {
            self.end(&id, json!("lagging"), false);
        }
        if let Some(c) = self.conns.get_mut(conn) {
            c.ends.clear();
        }
        self.wake(conn);
    }
    /// The consumer took the first `n` items.
    fn pop(&mut self, id: &str, n: usize) {
        let Some(s) = self.streams.get_mut(id) else {
            return;
        };
        let mut freed = 0;
        for _ in 0..n {
            if let Some(item) = s.queue.pop_front() {
                freed += item.body.len();
            }
        }
        s.bytes -= freed;
        let origin = s.origin.clone();
        let drained = s.queue.is_empty() && s.closing.is_some();
        self.free(&origin, freed);
        if drained {
            self.remove(id);
        }
        self.scan_ready();
    }
    fn free(&mut self, origin: &str, bytes: usize) {
        if let Some(b) = self.origin_bytes.get_mut(origin) {
            *b = b.saturating_sub(bytes);
            if *b == 0 {
                self.origin_bytes.remove(origin);
            }
        }
        self.host_bytes = self.host_bytes.saturating_sub(bytes);
    }
    /// `ready` for each producer whose refused push would now fit (19.2, In flight).
    fn scan_ready(&mut self) {
        let waiting: Vec<String> = self.waiting.iter().cloned().collect();
        for id in waiting {
            let Some(s) = self.streams.get(&id) else {
                self.waiting.remove(&id);
                continue;
            };
            let Some(refused) = s.needs_ready else {
                self.waiting.remove(&id);
                continue;
            };
            if s.deferred.is_some() || s.queue.len() * 2 >= MAX_ITEMS || s.bytes * 2 >= MAX_BYTES {
                continue;
            }
            let room = match refused {
                Refused::Stream => true,
                Refused::Origin(n) => {
                    self.origin_bytes.get(&s.origin).copied().unwrap_or(0) + n <= ORIGIN_BUDGET
                }
                Refused::Host(n) => self.host_bytes + n <= HOST_BUDGET,
            };
            if !room {
                continue;
            }
            let (origin, origin_gen, producer) =
                (s.origin.clone(), s.origin_gen, s.producer.clone());
            let s = self.streams.get_mut(&id).expect("checked above");
            s.needs_ready = None;
            s.full_since = None;
            self.waiting.remove(&id);
            if let Producer::Mod(p) = producer {
                self.tasks
                    .entry(p.local)
                    .or_default()
                    .push_back(Task::Notify {
                        sid: id,
                        origin,
                        origin_gen,
                        what: Notify::Ready,
                    });
            }
        }
    }
    fn check_clocks(&mut self) {
        let now = self.clock.now();
        let due: Vec<(String, &'static str)> = self
            .streams
            .iter()
            .filter_map(|(id, s)| {
                if s.deferred
                    .is_some_and(|d| now.saturating_duration_since(d) >= BACKSTOP)
                {
                    Some((id.clone(), "consumer-failed"))
                } else if s.deferred.is_none()
                    && s.full_since
                        .is_some_and(|f| now.saturating_duration_since(f) >= LAG)
                {
                    Some((id.clone(), "lagging"))
                } else {
                    None
                }
            })
            .collect();
        for (id, outcome) in due {
            self.end(&id, json!(outcome), false);
        }
    }
    /// Ends a stream: both sides get `ended`. With `drain`, a consumer first takes the items
    /// that are queued (a producer's `close`); every other end drops them.
    fn end(&mut self, id: &str, outcome: Value, drain: bool) {
        let Some(s) = self.streams.get_mut(id) else {
            return;
        };
        if !s.producer_told {
            s.producer_told = true;
            let (producer, origin, origin_gen, test) =
                (s.producer.clone(), s.origin.clone(), s.origin_gen, s.test);
            match producer {
                Producer::Mod(p) => {
                    self.tasks
                        .entry(p.local)
                        .or_default()
                        .push_back(Task::Notify {
                            sid: id.to_owned(),
                            origin,
                            origin_gen,
                            what: Notify::ProducerEnded(outcome.clone()),
                        });
                }
                Producer::Host => {
                    if test {
                        self.ended_host.insert(id.to_owned(), outcome.clone());
                    }
                    if let Some(hook) = self.on_end.remove(id) {
                        hook();
                    }
                }
            }
        }
        let s = self.streams.get_mut(id).expect("checked above");
        if drain && !s.queue.is_empty() && !matches!(s.consumer, Consumer::Held { .. }) {
            if s.closing.is_none() {
                s.closing = Some(outcome);
                if let Consumer::Page { conn, .. } = s.consumer.clone() {
                    self.wake(&conn);
                }
            }
            return;
        }
        s.closing = Some(outcome);
        self.remove(id);
    }
    /// Removes an ended stream whose queue is dropped or drained and tells its consumer.
    fn remove(&mut self, id: &str) {
        let Some(s) = self.streams.remove(id) else {
            return;
        };
        let outcome = s.closing.clone().unwrap_or(Value::Null);
        self.free(&s.origin, s.bytes);
        self.waiting.remove(id);
        if let Some(key) = s.consumer.key() {
            self.dec_open(&key);
        }
        match s.consumer {
            Consumer::Mod(m) => {
                self.tasks
                    .entry(m.local)
                    .or_default()
                    .push_back(Task::Notify {
                        sid: id.to_owned(),
                        origin: s.origin,
                        origin_gen: s.origin_gen,
                        what: Notify::SinkEnded(outcome),
                    });
            }
            Consumer::Page { conn, .. } => {
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.streams.retain(|x| x != id);
                    c.ends.push_back((id.to_owned(), outcome));
                }
                self.wake(&conn);
            }
            Consumer::Held { .. } => {}
        }
        self.scan_ready();
    }
}

pub(crate) enum ConnPoll {
    Message(String),
    Pending,
    Closed,
}
/// A page's `/events` connection; dropping it closes the connection (9.3).
pub(crate) struct ConnGuard {
    hub: Arc<Hub>,
    id: String,
}
impl ConnGuard {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn poll(&self, waker: &Waker) -> ConnPoll {
        self.hub.poll_conn(&self.id, waker)
    }
}
impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.hub.close_conn(&self.id);
    }
}
impl std::fmt::Debug for ConnGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnGuard").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Limits, Mode};

    fn modref(name: &str) -> ModRef {
        ModRef {
            local: LocalName::parse(name).unwrap(),
            identity: format!("local/{name}"),
        }
    }

    /// 19.6 (At apply): a stream handed from A to X and back to A is A's again, but a `cancel`
    /// (or `resume`) that A's call recorded before the hand-offs is stale and does nothing.
    #[test]
    fn ac3_a_stale_cancel_or_resume_does_nothing_after_a_hand_off_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        let (a, x) = (modref("a"), modref("x"));
        let id = host
            .hub
            .test_open(Consumer::Mod(a.clone()), "local/a".into())
            .unwrap();
        let handoffs = host.hub.info(&id).unwrap().handoffs;
        assert_eq!(handoffs, 0);
        for to in [&x, &a] {
            host.hub.apply(
                &host,
                Effect::HandOff {
                    id: id.clone(),
                    to: to.clone(),
                    to_gen: None,
                },
                true,
            );
        }
        let info = host.hub.info(&id).unwrap();
        assert_eq!(
            (info.consumer.clone(), info.handoffs),
            (Consumer::Mod(a.clone()), 2)
        );
        let cancel = |handoffs| Effect::Cancel {
            id: id.clone(),
            by: Consumer::Mod(a.clone()),
            handoffs,
        };
        host.hub.apply(&host, cancel(0), true);
        assert!(
            host.hub.info(&id).is_some(),
            "a stale cancel ended the stream"
        );
        // The same for `resume`: a stale one leaves the stream deferred.
        let deferred = || host.hub.lock().streams[&id].deferred.is_some();
        host.hub.lock().streams.get_mut(&id).unwrap().deferred = Some(Instant::now());
        let resume = |handoffs| Effect::Resume {
            id: id.clone(),
            by: Consumer::Mod(a.clone()),
            handoffs,
        };
        host.hub.apply(&host, resume(0), true);
        assert!(deferred(), "a stale resume resumed the stream");
        host.hub.apply(&host, resume(2), true);
        assert!(!deferred(), "a current resume did nothing");
        host.hub.apply(&host, cancel(2), true);
        assert!(host.hub.info(&id).is_none(), "a current cancel did nothing");
    }
}
