//! Agent tools (TECHNICAL 19.5): the handles of `agent-tools`, their toolset consents, their
//! tokens and the jobs that run a tool; the pure rules for the consents' function lists.

use super::consent::{self, Asked};
use super::streams::{Hub, MAX_ITEM, MAX_OPEN, StreamError};
use super::{Effect, ModRef, live_mod};
use crate::runtime::call::Invocation;
use crate::tools::Tool;
use crate::{CallError, Consent, ConsentRequest, Handle, Host, LocalName, Mode};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use wasmtime::component::Val;

/// Live handles per (origin client, opening mod) (19.5, Limits).
const MAX_HANDLES: usize = 32;

/// Tests only (19.9): holds the runs of agent tools just before their authority is read.
#[cfg(feature = "test-streams")]
#[derive(Debug, Default)]
pub(crate) struct AgentGate {
    /// `(held, waiting)`.
    state: Mutex<(bool, usize)>,
    cv: std::sync::Condvar,
    /// The runs of agent tools that ended, denied or not.
    finished: std::sync::atomic::AtomicUsize,
}
#[cfg(feature = "test-streams")]
impl AgentGate {
    pub(crate) fn hold(&self) {
        self.state.lock().expect("gate lock").0 = true;
    }
    pub(crate) fn release(&self) {
        self.state.lock().expect("gate lock").0 = false;
        self.cv.notify_all();
    }
    pub(crate) fn waiting(&self) -> usize {
        self.state.lock().expect("gate lock").1
    }
    pub(crate) fn finished(&self) -> usize {
        self.finished.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub(crate) fn finish(&self) {
        self.finished
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    /// Blocks while the gate is held.
    pub(crate) fn pass(&self) {
        let mut s = self.state.lock().expect("gate lock");
        s.1 += 1;
        while s.0 {
            s = self.cv.wait(s).expect("gate lock");
        }
        s.1 -= 1;
    }
}

/// Why the host did not start an agent's tool (19.5, Authority at the run).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Denied {
    /// The handle, its opening mod or its origin client is gone.
    Gone,
    /// The tool is not in the handle's tools now, or its mod is not the install it resolved to.
    UnknownTool,
}
impl Denied {
    /// The `code` of a job's `err` item.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Gone => "gone",
            Self::UnknownTool => "unknown-tool",
        }
    }
}
/// What an agent's call of a tool decided at the lookup (19.5): the handle by its caller name,
/// the tool by its name, and the install of the tool's mod that the lookup resolved to.
#[derive(Clone, Debug)]
pub(crate) struct AgentRun {
    pub(crate) handle: String,
    pub(crate) tool: String,
    pub(crate) install: Option<u64>,
}
/// The end of an agent's call of a tool: the host did not start it, or it ran and failed.
pub(crate) enum AgentFailure {
    Denied(Denied),
    Call(CallError),
}

/// Where a handle stands with the consents of its toolsets (19.5).
#[derive(Clone, Debug, PartialEq)]
enum Wait {
    /// The `what` of the consent that waits.
    Pending(String),
    /// The `what` of the consent that was refused; the first use reports it and closes the handle.
    Refused(String),
    Ready,
}

#[derive(Debug)]
struct Entry {
    /// The caller name `h<30 hex>`, never the handle itself.
    name: String,
    owner: ModRef,
    owner_gen: Option<u64>,
    origin: String,
    origin_gen: Option<u64>,
    toolsets: Vec<String>,
    wait: Wait,
    /// The bearer token of this handle and its id, in memory only (10.2).
    token: String,
    token_id: crate::TokenId,
    /// Live jobs: key to stream id.
    jobs: HashMap<String, String>,
}

/// The live handles of a host, by handle (19.5).
#[derive(Debug, Default)]
pub(crate) struct Handles {
    live: Mutex<HashMap<String, Entry>>,
    /// The consent questions that wait: (origin client, toolset) to the handles that wait on
    /// the answer (19.5, Limits). Taken before `live` is never held with it.
    asking: Mutex<HashMap<(String, String), Vec<String>>>,
}
impl Handles {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    /// What `describe` and `call` need of the handle that (owner, origin client) holds.
    fn view(&self, id: &str, owner: &ModRef, origin: &str) -> Option<(Vec<String>, Wait)> {
        self.lock()
            .get(id)
            .filter(|e| e.owner == *owner && e.origin == origin)
            .map(|e| (e.toolsets.clone(), e.wait.clone()))
    }
    /// The live handles of this (origin client, opening mod).
    fn count(&self, owner: &ModRef, origin: &str) -> usize {
        self.lock()
            .values()
            .filter(|e| e.owner == *owner && e.origin == origin)
            .count()
    }
    fn asking(&self) -> std::sync::MutexGuard<'_, HashMap<(String, String), Vec<String>>> {
        self.asking
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    /// Whether the origin client holds the handle (19.3, `agent-handle`).
    pub(crate) fn held_by(&self, id: &str, origin: &str) -> bool {
        self.lock().get(id).is_some_and(|e| e.origin == origin)
    }
    fn live(&self, id: &str) -> bool {
        self.lock().contains_key(id)
    }
    fn job(&self, id: &str, key: &str) -> Option<String> {
        self.lock().get(id).and_then(|e| e.jobs.get(key).cloned())
    }
    /// The job of `key` is over, if it is still the one with this stream.
    fn job_done(&self, id: &str, key: &str, stream: &str) {
        if let Some(e) = self.lock().get_mut(id)
            && e.jobs.get(key).is_some_and(|s| s == stream)
        {
            e.jobs.remove(key);
        }
    }
    fn set_wait(&self, id: &str, wait: Wait) {
        if let Some(e) = self.lock().get_mut(id) {
            e.wait = wait;
        }
    }
    /// Takes the handle out of the table.
    fn take(&self, id: &str) -> Option<Entry> {
        self.lock().remove(id)
    }
    fn take_where(&self, mut f: impl FnMut(&Entry) -> bool) -> Vec<Entry> {
        let mut live = self.lock();
        let ids: Vec<String> = live
            .iter()
            .filter(|(_, e)| f(e))
            .map(|(id, _)| id.clone())
            .collect();
        ids.iter().filter_map(|id| live.remove(id)).collect()
    }
}

/// What a run needs of a handle before it starts `claude` (19.3, `agent-handle`).
pub(crate) enum Readiness {
    /// The bearer token of the handle.
    Ready(String),
    /// The `what` of the consent that waits.
    Pending(String),
    /// The `what` of the refusal.
    Refused(String),
    Gone,
}

/// Whether a toolset needs its consent asked: the origin client holds none, or one of the
/// toolset's current functions is not in the stored list (19.7).
pub(crate) fn needs_consent(stored: Option<&[String]>, current: &[String]) -> bool {
    stored.is_none_or(|stored| current.iter().any(|f| !stored.contains(f)))
}

/// Drops the names `<local>_…` from the function list of each toolset consent (4.4): a later
/// mod under that local name, or a mod that became a manager, reaches no agent until a new
/// consent (S65, S68).
pub(crate) fn drop_names(consents: &mut [Consent], local: &str) {
    let prefix = format!("{local}_");
    for c in consents {
        if let Consent::Toolset { functions, .. } = c {
            functions.retain(|f| !f.starts_with(&prefix));
        }
    }
}

fn err(case: &str) -> Value {
    json!({"err": case})
}
fn err_with(case: &str, what: &str) -> Value {
    json!({"err": {case: what}})
}

fn text(args: &[Val], i: usize, what: &str) -> Result<String, CallError> {
    match args.get(i) {
        Some(Val::String(s)) => Ok(s.clone()),
        _ => Err(CallError::internal(format!(
            "agent-tools.{what}: bad argument"
        ))),
    }
}

/// `agent-tools` at the call (19.5, 19.6): `open` and `call` are requests, `describe` a read,
/// `close` acts at once.
pub(crate) fn call(inv: &Invocation, function: &str, args: &[Val]) -> Result<Value, CallError> {
    let host = &inv.chain.host;
    let me = inv.me();
    match function {
        "open" => {
            let Some(Val::List(items)) = args.first() else {
                return Err(CallError::internal("agent-tools.open: bad argument"));
            };
            let mut toolsets: Vec<String> = items
                .iter()
                .map(|v| match v {
                    Val::String(s) => s.clone(),
                    _ => String::new(),
                })
                .collect();
            toolsets.sort();
            toolsets.dedup();
            let known = host.toolsets();
            if toolsets.is_empty()
                || toolsets.iter().any(|t| {
                    // `agent` is the route of handle tokens: a legacy toolset of that name is
                    // listed but no handle opens it (10.1).
                    t == "agent"
                        || !crate::ToolsetName::parse(t).is_ok_and(|name| known.contains_key(&name))
                })
            {
                return Ok(err("not-granted"));
            }
            // At most 32 live handles per (origin client, opening mod), counting those that
            // chains in flight have queued (19.5, Limits).
            let gate = inv.gate();
            let mut log = inv.log();
            let mut queued = host.handles.count(&me, &inv.origin);
            let mut count = |e: &Effect| {
                if let Effect::AgentOpen { owner, origin, .. } = e
                    && *owner == me
                    && *origin == inv.origin
                {
                    queued += 1;
                }
            };
            gate.each(&mut count);
            log.iter().for_each(&mut count);
            if queued >= MAX_HANDLES {
                return Ok(err("not-granted"));
            }
            let id = super::streams::random_id();
            let name = format!("h{}", &super::streams::random_id()[..30]);
            log.push(Effect::AgentOpen {
                id: id.clone(),
                name,
                owner: me,
                owner_gen: inv.package_of(&inv.local),
                origin: inv.origin.clone(),
                origin_gen: inv.origin_gen,
                toolsets,
            });
            Ok(json!({"ok": id}))
        }
        "describe" => {
            let handle = text(args, 0, function)?;
            let toolsets = match usable(host, &handle, &me, &inv.origin) {
                Ok(t) => t,
                Err(e) => return Ok(e),
            };
            let tools = host.handle_tools(&toolsets, &inv.origin);
            Ok(json!({"ok": tools.into_iter().map(|(name, tool)| {
                let (parallel, idempotent) = tool.annotations();
                json!({
                    "name": name,
                    "description": tool.description(),
                    "input-schema": tool.schema.to_string(),
                    "parallel": parallel,
                    "idempotent": idempotent,
                })
            }).collect::<Vec<_>>()}))
        }
        "call" => {
            let (handle, key, tool, input) = (
                text(args, 0, function)?,
                text(args, 1, function)?,
                text(args, 2, function)?,
                text(args, 3, function)?,
            );
            let toolsets = match usable(host, &handle, &me, &inv.origin) {
                Ok(t) => t,
                Err(e) => return Ok(e),
            };
            let Some(found) = host.handle_tools(&toolsets, &inv.origin).remove(&tool) else {
                return Ok(err("unknown-tool"));
            };
            // The gate holds the chains still: a key lives once, also across chains in flight.
            let gate = inv.gate();
            let mut log = inv.log();
            let mut same = host.handles.job(&handle, &key);
            let mut queued = |e: &Effect| {
                if let Effect::AgentCall {
                    handle: h,
                    key: k,
                    id,
                    ..
                } = e
                    && *h == handle
                    && *k == key
                {
                    same.get_or_insert_with(|| id.clone());
                }
            };
            gate.each(&mut queued);
            log.iter().for_each(&mut queued);
            if let Some(id) = same {
                return Ok(json!({"ok": id}));
            }
            let id = super::streams::random_id();
            log.push(Effect::AgentCall {
                handle,
                key,
                id: id.clone(),
                local: found.local,
                function: found.function,
                tool,
                tool_install: found.install,
                args: input,
                owner: me,
                owner_gen: inv.package_of(&inv.local),
                origin: inv.origin.clone(),
                origin_gen: inv.origin_gen,
            });
            Ok(json!({"ok": id}))
        }
        "close" => {
            let handle = text(args, 0, function)?;
            if host.handles.view(&handle, &me, &inv.origin).is_some() {
                host.close_handle(&handle);
            }
            Ok(Value::Null)
        }
        other => Err(CallError::internal(format!(
            "no host function agent-tools.{other}"
        ))),
    }
}

/// The toolsets of a handle that (owner, origin client) may use now, or the error to return:
/// `unknown-handle`, `consent-pending`, `consent-refused` (the first use after a refusal, which
/// closes the handle).
fn usable(host: &Host, id: &str, owner: &ModRef, origin: &str) -> Result<Vec<String>, Value> {
    match host.handles.view(id, owner, origin) {
        None => Err(err("unknown-handle")),
        Some((_, Wait::Pending(what))) => Err(err_with("consent-pending", &what)),
        Some((_, Wait::Refused(what))) => {
            host.close_handle(id);
            Err(err_with("consent-refused", &what))
        }
        Some((toolsets, Wait::Ready)) => Ok(toolsets),
    }
}

impl Host {
    /// The decision to run an agent's tool, made again just before its call starts, under the one
    /// read of the catalog that the caller holds (19.5, Authority at the run): the handle is
    /// live, its opening mod and origin client are the installs it had, the tool is in the
    /// handle's tools as the stored consents stand now, and its mod is the install the lookup
    /// resolved to. The origin client of the call, with its install generation.
    pub(crate) fn agent_authority(
        &self,
        catalog: &crate::catalog::Catalog,
        run: &AgentRun,
        local: &LocalName,
        function: &str,
    ) -> Result<(String, Option<u64>), Denied> {
        let (owner, owner_gen, origin, origin_gen, toolsets) = {
            let live = self.handles.lock();
            let e = live
                .values()
                .find(|e| e.name == run.handle && e.wait == Wait::Ready)
                .ok_or(Denied::Gone)?;
            (
                e.owner.clone(),
                e.owner_gen,
                e.origin.clone(),
                e.origin_gen,
                e.toolsets.clone(),
            )
        };
        if !(live_mod(catalog, Some(&owner.local), &owner.identity, owner_gen)
            && live_mod(catalog, None, &origin, origin_gen))
        {
            return Err(Denied::Gone);
        }
        // A ready handle holds a stored consent for each of its toolsets. One that is gone was
        // withdrawn: the withdrawal writes the catalog first and closes the handle after, so
        // the handle is as good as closed and the job ends `gone`, as it would a moment later.
        let held = catalog
            .mods
            .iter()
            .find(|m| m.identity == origin)
            .is_some_and(|m| {
                toolsets.iter().all(|t| {
                    m.consents
                        .iter()
                        .any(|c| matches!(c, Consent::Toolset { toolset, .. } if toolset == t))
                })
            });
        if !held {
            return Err(Denied::Gone);
        }
        let tools = self.handle_tools_in(catalog, &toolsets, &origin);
        match tools.get(&run.tool) {
            Some(t) if t.local == *local && t.function == function && t.install == run.install => {
                Ok((origin, origin_gen))
            }
            _ => Err(Denied::UnknownTool),
        }
    }

    /// The tools of a ready handle by its caller name (10.3); `None` for a handle that closed or
    /// has no consent yet.
    pub(crate) fn agent_tools_of(&self, name: &str) -> Option<BTreeMap<String, Tool>> {
        let (toolsets, origin) = {
            let live = self.handles.lock();
            let e = live
                .values()
                .find(|e| e.name == name && e.wait == Wait::Ready)?;
            (e.toolsets.clone(), e.origin.clone())
        };
        Some(self.handle_tools(&toolsets, &origin))
    }

    /// Closes a handle (19.5): its token ends, its live jobs' streams end `gone`.
    pub(crate) fn close_handle(&self, id: &str) {
        if let Some(entry) = self.handles.take(id) {
            self.ended_handle(&entry);
        }
    }
    fn ended_handle(&self, entry: &Entry) {
        self.end_token(&entry.token_id);
        let streams: Vec<String> = entry.jobs.values().cloned().collect();
        self.hub.end_gone(&streams);
    }
    /// A consent was withdrawn: the handles of that origin client that hold the toolset close (19.7).
    pub(crate) fn close_handles_of_toolset(&self, origin: &str, toolset: &str) {
        for entry in self
            .handles
            .take_where(|e| e.origin == origin && e.toolsets.iter().any(|t| t == toolset))
        {
            self.ended_handle(&entry);
        }
    }
    /// Closes the handles whose owner or origin client is no longer the enabled install that
    /// opened it (19.5): called after each catalog write.
    pub(crate) fn end_lost_handles(&self) {
        let catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let lost = self.handles.take_where(|e| {
            !(live_mod(
                &catalog,
                Some(&e.owner.local),
                &e.owner.identity,
                e.owner_gen,
            ) && live_mod(&catalog, None, &e.origin, e.origin_gen))
        });
        drop(catalog);
        for entry in lost {
            self.ended_handle(&entry);
        }
    }

    /// What a run of `claude` needs of the handle `id` (19.3): its token once its consents are
    /// held.
    pub(crate) fn handle_readiness(&self, id: &str, origin: &str) -> Readiness {
        match self.handles.lock().get(id) {
            Some(e) if e.origin == origin => match &e.wait {
                Wait::Ready => Readiness::Ready(e.token.clone()),
                Wait::Pending(what) => Readiness::Pending(what.clone()),
                Wait::Refused(what) => Readiness::Refused(what.clone()),
            },
            _ => Readiness::Gone,
        }
    }

    /// Applies an `open` of a committed chain (19.5, 19.6): the handle, and the questions of the
    /// toolsets that need their consent, on a thread of their own.
    #[expect(clippy::too_many_arguments, reason = "the fields of one effect")]
    pub(crate) fn agent_open(
        &self,
        id: String,
        name: String,
        owner: ModRef,
        owner_gen: Option<u64>,
        origin: String,
        origin_gen: Option<u64>,
        toolsets: Vec<String>,
        valid: bool,
    ) {
        if !valid {
            return;
        }
        let Ok(token) = self.issue_agent_token(&name) else {
            return;
        };
        self.handles.lock().insert(
            id.clone(),
            Entry {
                name,
                owner,
                owner_gen,
                origin,
                origin_gen,
                toolsets,
                wait: Wait::Ready,
                token: token.as_str().to_owned(),
                token_id: token.id(),
                jobs: HashMap::new(),
            },
        );
        // The checks of the call are made again where the effect lands (19.6, At apply).
        self.end_lost_handles();
        self.advance_handle(&id);
    }

    /// Takes a handle to its next toolset that needs its consent, or to ready. The question of
    /// a (origin client, toolset) is asked once: a handle that needs the same one waits on it
    /// (19.5, Limits).
    pub(crate) fn advance_handle(&self, id: &str) {
        self.advance(id, None);
    }
    /// `advance_handle`, skipping `answered`: the toolset whose question just ended with an
    /// answer for a list that changed while it waited (19.5). It is asked again on a later `open`.
    fn advance(&self, id: &str, answered: Option<&str>) {
        let (origin, toolsets) = {
            let live = self.handles.lock();
            let Some(e) = live.get(id) else { return };
            (e.origin.clone(), e.toolsets.clone())
        };
        let Some(toolset) = self.first_to_ask(&origin, &toolsets, answered) else {
            self.handles.set_wait(id, Wait::Ready);
            return;
        };
        let what = format!("toolset:{toolset}");
        self.handles.set_wait(id, Wait::Pending(what.clone()));
        let key = (origin, toolset);
        let first = {
            let mut asking = self.handles.asking();
            match asking.get_mut(&key) {
                Some(waiters) => {
                    waiters.push(id.to_owned());
                    Some(false)
                }
                // A question that ended stored its answer before it left `asking`: read the
                // catalog again under this lock, so a handle that missed it asks nothing more.
                None if !self.toolset_needs_consent(&key.0, &key.1) => None,
                None => {
                    asking.insert(key.clone(), vec![id.to_owned()]);
                    Some(true)
                }
            }
        };
        match first {
            None => return self.advance(id, answered),
            Some(false) => return,
            Some(true) => {}
        }
        let question = Question {
            weak: Weak::<Host>::clone(&self.weak),
            handles: Arc::clone(&self.handles),
            hub: Arc::clone(&self.hub),
            mode: self.home.mode.clone(),
            key: key.clone(),
        };
        if std::thread::Builder::new()
            .name("watane-consent".into())
            .spawn(move || question.run())
            .is_err()
        {
            // No thread, no question: the consent is refused.
            let waiters = self.handles.asking().remove(&key).unwrap_or_default();
            for w in waiters {
                self.refuse_handle(&w, &what);
            }
        }
    }

    /// The consent of a handle was refused: its token and jobs end, the first use reports why.
    fn refuse_handle(&self, id: &str, what: &str) {
        let taken = {
            let mut live = self.handles.lock();
            live.get_mut(id).map(|e| {
                e.wait = Wait::Refused(what.to_owned());
                (e.token_id.clone(), std::mem::take(&mut e.jobs))
            })
        };
        if let Some((token_id, jobs)) = taken {
            self.end_token(&token_id);
            self.hub.end_gone(&jobs.into_values().collect::<Vec<_>>());
        }
    }

    /// The first toolset of `toolsets` that needs its consent, if any.
    fn first_to_ask(
        &self,
        origin: &str,
        toolsets: &[String],
        answered: Option<&str>,
    ) -> Option<String> {
        toolsets
            .iter()
            .find(|t| Some(t.as_str()) != answered && self.toolset_needs_consent(origin, t))
            .cloned()
    }
    fn drop_mark(&self) -> u64 {
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .drop_mark()
    }
    fn toolset_needs_consent(&self, origin: &str, toolset: &str) -> bool {
        let current = self.toolset_functions(toolset);
        let stored = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .find(|m| m.identity == origin)
            .and_then(|m| {
                m.consents.iter().find_map(|c| match c {
                    Consent::Toolset {
                        toolset: t,
                        functions,
                    } if t == toolset => Some(functions.clone()),
                    _ => None,
                })
            });
        needs_consent(stored.as_deref(), &current)
    }

    /// Applies a `call` of a committed chain (19.5, Job): the stream, and the thread that runs the
    /// tool as a top-level call of its own.
    #[expect(clippy::too_many_arguments, reason = "the fields of one effect")]
    pub(crate) fn agent_call(
        &self,
        handle: String,
        key: String,
        id: String,
        tool: ToolRun,
        input: String,
        owner: ModRef,
        origin: (String, Option<u64>),
        valid: bool,
    ) {
        let hooked = {
            let (handles, handle, key, id) = (
                Arc::clone(&self.handles),
                handle.clone(),
                key.clone(),
                id.clone(),
            );
            Box::new(move || handles.job_done(&handle, &key, &id))
        };
        self.hub
            .open_host(id.clone(), owner.clone(), origin.clone(), hooked);
        let name = {
            let live = self.handles.lock();
            live.get(&handle)
                .filter(|e| {
                    valid
                        && e.owner == owner
                        && e.origin == origin.0
                        && e.wait == Wait::Ready
                        && !e.jobs.contains_key(&key)
                })
                .map(|e| e.name.clone())
        };
        let Some(name) = name else {
            self.hub.end_gone(&[id]);
            return;
        };
        if self.hub.open_count(&owner.identity) > MAX_OPEN {
            self.hub
                .finish(&id, json!({"failed": "too many streams open"}));
            return;
        }
        // A withdrawal, a close or a mod's end may have taken the handle since the check above:
        // then the job is a dependent that nothing else will end, so it ends `gone` here (19.7).
        if let Some(e) = self.handles.lock().get_mut(&handle) {
            e.jobs.insert(key.clone(), id.clone());
        } else {
            self.hub.end_gone(&[id]);
            return;
        }
        let job = Job {
            weak: Weak::<Host>::clone(&self.weak),
            handles: Arc::clone(&self.handles),
            hub: Arc::clone(&self.hub),
            handle,
            key,
            id: id.clone(),
            name,
            tool,
            input,
        };
        let spawned = std::thread::Builder::new()
            .name("watane-tool".into())
            .stack_size(16 << 20)
            .spawn(move || job.run());
        if spawned.is_err() {
            self.hub
                .finish(&id, json!({"failed": "cannot start a thread for a tool"}));
        }
    }
}

/// The thread of one consent question of (origin client, toolset): it asks the app once, and
/// its answer goes to every handle that waits (19.7, Asking; 19.5, Limits).
struct Question {
    weak: Weak<Host>,
    handles: Arc<Handles>,
    hub: Arc<Hub>,
    mode: Mode,
    key: (String, String),
}
impl Question {
    /// The live handles that wait.
    fn waiters(&self) -> Vec<String> {
        let ids = self
            .handles
            .asking()
            .get(&self.key)
            .cloned()
            .unwrap_or_default();
        let live = self.handles.lock();
        ids.into_iter().filter(|id| live.contains_key(id)).collect()
    }
    fn run(&self) {
        let (origin, toolset) = (&self.key.0, &self.key.1);
        let Some(host) = self.weak.upgrade() else {
            return;
        };
        let first = self.waiters().into_iter().next().and_then(|id| {
            let live = self.handles.lock();
            live.get(&id).map(|e| (e.owner.clone(), e.origin_gen))
        });
        let Some(origin_local) = host.local_of(origin) else {
            self.end(&host, |host, id| host.close_handle(id));
            return;
        };
        // No live waiter: a handle that joined since goes on with a question of its own.
        let Some((owner, origin_gen)) = first else {
            self.end(&host, |host, id| host.advance_handle(id));
            return;
        };
        let mark = host.drop_mark();
        let functions = host.toolset_functions(toolset);
        let consent = Consent::Toolset {
            toolset: toolset.clone(),
            functions: vec![],
        };
        let what = consent.to_string();
        drop(host);
        let request = ConsentRequest {
            origin: origin_local,
            via: owner.local,
            consent,
            profile: None,
            functions: functions.clone(),
        };
        let asked = consent::ask(&self.mode, &self.hub, || self.waiters().is_empty(), request);
        let Some(host) = self.weak.upgrade() else {
            return;
        };
        match asked {
            // Every waiter had gone; one that joined since must not be lost.
            Asked::Stopped => self.end(&host, |host, id| host.advance_handle(id)),
            Asked::Refused => self.end(&host, |host, id| host.refuse_handle(id, &what)),
            Asked::Granted => {
                // A mod of the list removed or made a manager while the question waited: the
                // participant did not see its names as they are now. The store leaves them out
                // under the catalog's lock, so no drop slips between a check and the write; they
                // are asked again on a later `open` like gained ones (19.5).
                let stored = Consent::Toolset {
                    toolset: toolset.clone(),
                    functions,
                };
                if host
                    .store_consent(origin, origin_gen, stored, Some(mark))
                    .is_err()
                {
                    self.end(&host, |host, id| host.close_handle(id));
                } else if host.drop_mark() != mark {
                    self.end(&host, |host, id| host.advance(id, Some(toolset)));
                } else {
                    // The question is over: each handle goes on to its next toolset.
                    self.end(&host, |host, id| host.advance_handle(id));
                }
            }
        }
    }
    /// The question is over: `then` runs for each handle that waited and is still live.
    fn end(&self, host: &Host, then: impl Fn(&Host, &str)) {
        let waiters = self.handles.asking().remove(&self.key).unwrap_or_default();
        for id in waiters {
            if self.handles.live(&id) {
                then(host, &id);
            }
        }
    }
}

/// The thread that runs one tool and gives its result as the one item of the job's stream.
struct Job {
    weak: Weak<Host>,
    handles: Arc<Handles>,
    hub: Arc<Hub>,
    handle: String,
    key: String,
    id: String,
    /// The caller name of the handle.
    name: String,
    tool: ToolRun,
    input: String,
}
/// The tool of a job as the call resolved it: its mod, its function `<interface>.<function>`, its
/// name and the install of its mod.
pub(crate) struct ToolRun {
    pub(crate) local: LocalName,
    pub(crate) function: String,
    pub(crate) name: String,
    pub(crate) install: Option<u64>,
}
impl Job {
    fn run(&self) {
        let item = match serde_json::from_str::<Value>(&self.input) {
            Err(e) => {
                json!({"err": {"code": "bad_request", "message": format!("arguments are not JSON: {e}")}})
            }
            Ok(args) => {
                let (Some(host), Ok(handle)) = (self.weak.upgrade(), Handle::parse(&self.name))
                else {
                    return;
                };
                // The tool runs as a top-level call with its own budget, the agent as caller and
                // the handle's origin client as origin client; the host decides again whether it
                // may run just before it starts (19.5, Authority at the run).
                let run = AgentRun {
                    handle: self.name.clone(),
                    tool: self.tool.name.clone(),
                    install: self.tool.install,
                };
                let result = crate::runtime::call::run_agent(
                    &host,
                    &run,
                    handle,
                    &self.tool.local,
                    &self.tool.function,
                    args,
                );
                match result {
                    Ok(value) => json!({"ok": value}),
                    // The handle, its opener or its origin client went before the tool could run:
                    // the job's stream ends `gone`, as a live job's does (19.5, 19.7), no item.
                    Err(AgentFailure::Denied(Denied::Gone)) => {
                        self.handles.job_done(&self.handle, &self.key, &self.id);
                        self.hub.end_gone(std::slice::from_ref(&self.id));
                        return;
                    }
                    Err(AgentFailure::Denied(d)) => {
                        json!({"err": {"code": d.code(), "message": "the tool may not run now"}})
                    }
                    Err(AgentFailure::Call(e)) => {
                        let code = serde_json::to_value(&e)
                            .ok()
                            .and_then(|v| v["code"].as_str().map(str::to_owned))
                            .unwrap_or_else(|| "internal".into());
                        json!({"err": {"code": code, "message": e.message}})
                    }
                }
            }
        };
        let mut body = item.to_string().into_bytes();
        if body.len() > MAX_ITEM {
            body = json!({"err": {"code": "too_large", "message": "the result is over 4 MiB"}})
                .to_string()
                .into_bytes();
        }
        // The key is free once the result is there: the same key then starts a new job.
        self.handles.job_done(&self.handle, &self.key, &self.id);
        loop {
            match self.hub.host_push(&self.id, &body) {
                Err(StreamError::Full) => std::thread::sleep(Duration::from_millis(10)),
                Ok(()) => break,
                Err(_) => return,
            }
        }
        self.hub.finish(&self.id, json!("closed"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    /// 19.7: a toolset asks when it is new, or when a current function is not in the stored list.
    #[test]
    fn ac1_a_toolset_needs_its_consent_when_new_or_when_it_gained_a_function() {
        let current = names(&["tool_api_a", "tool_api_b"]);
        assert!(needs_consent(None, &current));
        assert!(needs_consent(None, &[]));
        assert!(needs_consent(Some(&names(&["tool_api_a"])), &current));
        assert!(needs_consent(Some(&[]), &current));
        assert!(!needs_consent(Some(&current), &current));
        // A stored name that the toolset no longer has asks nothing.
        assert!(!needs_consent(
            Some(&names(&["tool_api_a", "tool_api_b", "tool_api_c"])),
            &current
        ));
        assert!(!needs_consent(Some(&[]), &[]));
    }

    /// 4.4: the names `<local>_…` of one mod go from every toolset consent, and only those.
    #[test]
    fn ac4_drop_names_removes_one_mods_names_from_each_toolset_consent() {
        let mut consents = vec![
            Consent::Toolset {
                toolset: "t".into(),
                functions: names(&["tool_api_a", "tool-x_api_a", "tooly_api_a", "other_api_b"]),
            },
            Consent::Workspace {
                via: "local/p".into(),
                folder: "/tool_x".into(),
            },
            Consent::Toolset {
                toolset: "u".into(),
                functions: names(&["tool_api_b"]),
            },
        ];
        drop_names(&mut consents, "tool");
        assert_eq!(
            consents,
            vec![
                Consent::Toolset {
                    toolset: "t".into(),
                    functions: names(&["tool-x_api_a", "tooly_api_a", "other_api_b"]),
                },
                Consent::Workspace {
                    via: "local/p".into(),
                    folder: "/tool_x".into(),
                },
                Consent::Toolset {
                    toolset: "u".into(),
                    functions: vec![],
                },
            ]
        );
    }
}
