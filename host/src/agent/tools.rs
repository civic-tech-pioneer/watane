//! Agent tools (TECHNICAL 19.5): the handles of `agent-tools`, their toolset consents, their
//! tokens and the jobs that run a tool; the pure rules for the consents' function lists.

use super::consent::{self, Asked};
use super::streams::{Hub, MAX_ITEM, MAX_OPEN, StreamError};
use super::{Effect, ModRef, live_mod};
use crate::runtime::call::Invocation;
use crate::tools::Tool;
use crate::{CallError, Caller, Consent, ConsentRequest, Handle, Host, LocalName, Mode};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use wasmtime::component::Val;

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
                    !crate::ToolsetName::parse(t).is_ok_and(|name| known.contains_key(&name))
                })
            {
                return Ok(err("not-granted"));
            }
            let id = super::streams::random_id();
            let name = format!("h{}", &super::streams::random_id()[..30]);
            inv.log().push(Effect::AgentOpen {
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
        let first = self
            .first_to_ask(&origin, &toolsets)
            .map(|toolset| format!("toolset:{toolset}"));
        let wait = first.clone().map_or(Wait::Ready, Wait::Pending);
        self.handles.lock().insert(
            id.clone(),
            Entry {
                name,
                owner,
                owner_gen,
                origin,
                origin_gen,
                toolsets,
                wait,
                token: token.as_str().to_owned(),
                token_id: token.id(),
                jobs: HashMap::new(),
            },
        );
        // The checks of the call are made again where the effect lands (19.6, At apply).
        self.end_lost_handles();
        if let Some(what) = first {
            let ask = Asker {
                weak: Weak::<Host>::clone(&self.weak),
                handles: Arc::clone(&self.handles),
                hub: Arc::clone(&self.hub),
                mode: self.home.mode.clone(),
                id: id.clone(),
            };
            let spawned = std::thread::Builder::new()
                .name("watane-consent".into())
                .spawn(move || ask.run());
            if spawned.is_err() {
                // No thread, no question: the consent is refused.
                self.handles.set_wait(&id, Wait::Refused(what));
            }
        }
    }

    /// The first toolset of `toolsets` that needs its consent, if any.
    fn first_to_ask(&self, origin: &str, toolsets: &[String]) -> Option<String> {
        toolsets
            .iter()
            .find(|t| self.toolset_needs_consent(origin, t))
            .cloned()
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
        tool: (LocalName, String),
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
        if let Some(e) = self.handles.lock().get_mut(&handle) {
            e.jobs.insert(key.clone(), id.clone());
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

/// The thread that asks the consents of a handle's toolsets, one after the other (19.7, Asking).
struct Asker {
    weak: Weak<Host>,
    handles: Arc<Handles>,
    hub: Arc<Hub>,
    mode: Mode,
    id: String,
}
impl Asker {
    fn run(&self) {
        let toolsets = {
            let live = self.handles.lock();
            let Some(e) = live.get(&self.id) else { return };
            e.toolsets.clone()
        };
        for toolset in toolsets {
            // Each question is made from what the catalog holds when it is asked.
            let Some(host) = self.weak.upgrade() else {
                return;
            };
            let (origin, origin_gen, owner) = {
                let live = self.handles.lock();
                let Some(e) = live.get(&self.id) else { return };
                (e.origin.clone(), e.origin_gen, e.owner.clone())
            };
            if !host.toolset_needs_consent(&origin, &toolset) {
                continue;
            }
            let functions = host.toolset_functions(&toolset);
            let Some(origin_local) = host.local_of(&origin) else {
                host.close_handle(&self.id);
                return;
            };
            let consent = Consent::Toolset {
                toolset: toolset.clone(),
                functions: vec![],
            };
            let what = consent.to_string();
            self.handles.set_wait(&self.id, Wait::Pending(what.clone()));
            drop(host);
            let request = ConsentRequest {
                origin: origin_local,
                via: owner.local,
                consent,
                profile: None,
                functions: functions.clone(),
            };
            let asked = consent::ask(
                &self.mode,
                &self.hub,
                || !self.handles.live(&self.id),
                request,
            );
            let Some(host) = self.weak.upgrade() else {
                return;
            };
            match asked {
                Asked::Stopped => return,
                Asked::Refused => {
                    self.handles.set_wait(&self.id, Wait::Refused(what));
                    // The handle is closed: its token and jobs end; the first use reports why.
                    if let Some(entry) = self.handles.lock().get_mut(&self.id) {
                        let (token_id, jobs) =
                            (entry.token_id.clone(), std::mem::take(&mut entry.jobs));
                        host.end_token(&token_id);
                        host.hub.end_gone(&jobs.into_values().collect::<Vec<_>>());
                    }
                    return;
                }
                Asked::Granted => {
                    let stored = Consent::Toolset { toolset, functions };
                    if host.store_consent(&origin, origin_gen, stored).is_err() {
                        host.close_handle(&self.id);
                        return;
                    }
                }
            }
        }
        self.handles.set_wait(&self.id, Wait::Ready);
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
    tool: (LocalName, String),
    input: String,
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
                // The tool runs as a top-level call with its own budget and the agent as caller.
                let result = host.call(&Caller::Agent(handle), &self.tool.0, &self.tool.1, args);
                match result {
                    Ok(value) => json!({"ok": value}),
                    Err(e) => {
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
