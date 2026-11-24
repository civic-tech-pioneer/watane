//! H2, Claude Code (TECHNICAL 19.3): `claude-code.start`, `claude-code-shell.start` and the
//! functions of `claude-session` at the call, the table of live sessions, and the start that
//! a committed chain applies.

#![cfg_attr(
    windows,
    allow(
        dead_code,
        reason = "runner states are only used by Unix process groups"
    )
)]

mod asks;
pub(crate) mod control;
mod init;
pub(crate) mod launch;
mod paths;
#[cfg(unix)] // The runner relies on Unix process groups and verified PID records.
pub(crate) mod run;

#[cfg(windows)]
pub(crate) mod run {
    // A Windows launch is refused before it can create a process or a PID record.
    pub(crate) fn clean_runs(_: &std::path::Path) {}

    pub(super) fn kill_group(_: i32) {
        unreachable!("Windows Claude sessions never start a process group")
    }

    pub(super) fn pid_record(_: i32) -> Option<String> {
        unreachable!("Windows Claude sessions never write process records")
    }

    pub(crate) enum Job {}
}

use super::streams::{Consumer, MAX_OPEN};
use super::{Effect, ModRef};
use crate::runtime::call::Invocation;
use crate::{CallError, Host};
use asks::{Asks, Decision};
use launch::{Kind, Launch, Workspace};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use wasmtime::component::Val;

/// `claude` processes live at once: per origin client, and in all (19.2, Limits).
const PER_ORIGIN: usize = 4;
const IN_ALL: usize = 8;

/// What a `claude-session` call asks the host to write (19.3, step 5).
#[derive(Clone, Debug)]
pub(crate) enum Write {
    Say(String),
    Compact,
    Interrupt,
    Answer { ask: String, decision: Decision },
}

/// One line for `claude`'s stdin. `answered` is `(ask id, outcome)` when the line is the
/// answer to an ask or check: the writer reports it after the line is written (19.3, step 5).
#[derive(Debug)]
pub(crate) struct Out {
    pub line: Vec<u8>,
    pub answered: Option<(String, &'static str)>,
}

#[derive(Debug, Default)]
struct Running {
    pgid: Option<i32>,
    stopped: bool,
}

/// One `claude` run: its process group, and the queue of lines for its stdin.
#[derive(Debug)]
pub(crate) struct Session {
    pub id: String,
    pub origin: String,
    /// The mod whose request waits, the `via` of a workspace consent (19.7).
    pub via: Option<ModRef>,
    /// The install generation of the origin client at the call (19.6, At apply).
    pub origin_gen: Option<u64>,
    /// The picked folders of the launch, canonical: what a withdrawal matches (19.7).
    pub folders: Vec<String>,
    /// The `initialize` request's id: the answer to it is the one the run waits for.
    pub request_id: String,
    /// The group and the stop flag under one lock: `stop` and `started` each read what the
    /// other wrote, so one of them always kills the group.
    state: Mutex<Running>,
    tx: Mutex<Option<Sender<Out>>>,
    rx: Mutex<Option<Receiver<Out>>>,
    /// The open asks and checks (19.3, steps 4 and 5).
    asks: Mutex<Asks>,
}
impl Session {
    /// `hook_timeout`: the `pre-tool-check`'s `timeout-s`, for the hook of `initialize`.
    pub(crate) fn new(id: String, origin: String, hook_timeout: Option<u64>) -> Self {
        let (tx, rx) = channel();
        let request_id = super::streams::random_id();
        // `initialize` is the first line, whatever else is queued while the process starts.
        let _ = tx.send(Out {
            line: control::initialize(&request_id, hook_timeout),
            answered: None,
        });
        Self {
            id,
            origin,
            via: None,
            origin_gen: None,
            folders: vec![],
            request_id,
            state: Mutex::new(Running::default()),
            tx: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
            asks: Mutex::default(),
        }
    }
    /// Names what the session's consent depends on (19.7).
    pub(crate) fn scoped(
        mut self,
        via: ModRef,
        origin_gen: Option<u64>,
        folders: Vec<String>,
    ) -> Self {
        self.via = Some(via);
        self.origin_gen = origin_gen;
        self.folders = folders;
        self
    }
    fn running(&self) -> std::sync::MutexGuard<'_, Running> {
        self.state
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    pub(crate) fn stopped(&self) -> bool {
        self.running().stopped
    }
    /// Kills the group and closes the queue. Safe to call again.
    pub(crate) fn stop(&self) {
        {
            let mut state = self.running();
            state.stopped = true;
            Self::kill_in(&state)
        };
        self.tx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take();
    }
    /// Kills the recorded group, if any. The lock is held across the signal, and `exited` clears
    /// the group under it before `claude` is reaped, so no kill reaches a pid reused since.
    fn kill_in(state: &Running) {
        if let Some(pgid) = state.pgid {
            run::kill_group(pgid);
        }
    }
    /// Kills the recorded group, if `claude` is not reaped yet.
    pub(crate) fn kill_group(&self) {
        Self::kill_in(&self.running());
    }
    /// `claude` has exited and is not reaped yet (its pid is still held): kills what is left of
    /// its group, and clears the group, so no later kill signals one (19.3 step 5, S100-CAP).
    pub(crate) fn exited(&self) {
        let mut state = self.running();
        Self::kill_in(&state);
        state.pgid = None;
    }
    /// The process started: its group, and the queue of its stdin.
    pub(crate) fn started(&self, pgid: i32) -> Receiver<Out> {
        let stopped = {
            let mut state = self.running();
            state.pgid = Some(pgid);
            state.stopped
        };
        if stopped {
            run::kill_group(pgid);
        }
        self.rx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take()
            .unwrap_or_else(|| channel().1)
    }
    /// Queues one line for stdin (the writer adds the newline). `false` when it could not be
    /// queued (the stream is stopped, or the writer has gone because stdin failed): the session
    /// is then stopped, so the stream ends as the process does (19.3, step 5).
    pub(crate) fn write(&self, line: Vec<u8>) -> bool {
        self.enqueue(Out {
            line,
            answered: None,
        })
    }
    /// Closes the queue: the writer ends after the lines already in it.
    pub(crate) fn close_queue(&self) {
        self.tx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take();
    }
    fn enqueue(&self, out: Out) -> bool {
        let sent = self
            .tx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .as_ref()
            .is_some_and(|tx| tx.send(out).is_ok());
        if !sent {
            self.stop();
        }
        sent
    }
}

/// The live sessions of a host, and `runs/pids`.
#[derive(Default, Debug)]
pub(crate) struct Sessions {
    live: Mutex<HashMap<String, Arc<Session>>>,
    pids: Mutex<()>,
}
impl Sessions {
    /// Registers a session, unless the limits of 19.2 are reached.
    pub(crate) fn register(&self, session: &Arc<Session>) -> Result<(), ()> {
        let mut live = self
            .live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let same_origin = live.values().filter(|s| s.origin == session.origin).count();
        if same_origin >= PER_ORIGIN || live.len() >= IN_ALL {
            return Err(());
        }
        live.insert(session.id.clone(), Arc::clone(session));
        Ok(())
    }
    /// The ids of the live sessions of `origin` started through `via` whose launch names `folder`
    /// (19.7, Withdrawal).
    pub(crate) fn depending(&self, origin: &str, via: &str, folder: &str) -> Vec<String> {
        self.live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .values()
            .filter(|s| {
                s.origin == origin
                    && s.via.as_ref().is_some_and(|v| v.identity == via)
                    && s.folders.iter().any(|f| f == folder)
            })
            .map(|s| s.id.clone())
            .collect()
    }
    pub(crate) fn get(&self, id: &str) -> Option<Arc<Session>> {
        self.live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .get(id)
            .cloned()
    }
    pub(crate) fn remove(&self, id: &str) {
        self.live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .remove(id);
    }
    /// Kills every group: the host is closing.
    pub(crate) fn stop_all(&self) {
        let all: Vec<Arc<Session>> = self
            .live
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .values()
            .cloned()
            .collect();
        for session in all {
            session.stop();
        }
    }
    /// Adds a process group to `<home>/runs/pids` as `pgid boot-id start-time` (19.3, The run),
    /// and gives the line, for `forget_pid`. `None` when the leader is gone: nothing is recorded.
    pub(crate) fn record_pid(&self, home: &Path, pgid: i32) -> Option<String> {
        let _guard = self
            .pids
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let file = home.join("runs/pids");
        // No record when the leader is gone: there is no run to find again.
        let record = run::pid_record(pgid)?;
        let mut text = std::fs::read_to_string(&file).unwrap_or_default();
        text.push_str(&format!("{record}\n"));
        let _ = std::fs::write(file, text);
        Some(record)
    }
    /// Removes the lines of `runs/pids` that are `record`, the whole line (pgid, boot id, start
    /// time): a newer run that reused the pgid keeps its own (19.3 step 5, review r6, F6.4).
    pub(crate) fn forget_pid(&self, home: &Path, record: &str) {
        let _guard = self
            .pids
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let file = home.join("runs/pids");
        if let Ok(text) = std::fs::read_to_string(&file) {
            let kept: String = text
                .lines()
                .filter(|l| *l != record)
                .map(|l| format!("{l}\n"))
                .collect();
            let _ = std::fs::write(file, kept);
        }
    }
}

fn text_arg(args: &[Val], i: usize, what: &str) -> Result<String, CallError> {
    match args.get(i) {
        Some(Val::String(s)) => Ok(s.clone()),
        _ => Err(CallError::internal(format!("{what}: bad argument"))),
    }
}
fn bad_launch(field: &str, reason: &str) -> Value {
    json!({"err": {"bad-launch": {"field": field, "reason": reason}}})
}

/// The WIT `decision`: `allow(option<string>)`, `deny(string)` or `none`.
fn decision_arg(v: Option<&Val>) -> Result<Decision, CallError> {
    let bad = || CallError::internal("answer: bad decision");
    match v {
        Some(Val::Variant(case, payload)) => match (case.as_str(), payload.as_deref()) {
            ("none", None) => Ok(Decision::None),
            ("deny", Some(Val::String(why))) => Ok(Decision::Deny(why.clone())),
            ("allow", Some(Val::Option(None))) => Ok(Decision::Allow(None)),
            ("allow", Some(Val::Option(Some(inner)))) => match &**inner {
                Val::String(json) => Ok(Decision::Allow(Some(json.clone()))),
                _ => Err(bad()),
            },
            _ => Err(bad()),
        },
        _ => Err(bad()),
    }
}

fn platform_refusal() -> Option<Value> {
    cfg!(windows).then(|| {
        bad_launch(
            "runtime",
            "Claude sessions require Unix process-group controls",
        )
    })
}

#[cfg(all(test, windows))]
#[test]
fn s110_refuses_claude_without_process_group_controls() {
    assert_eq!(
        platform_refusal().expect("Windows must refuse a Claude launch"),
        json!({"err": {"bad-launch": {"field": "runtime", "reason": "Claude sessions require Unix process-group controls"}}})
    );
}

/// `claude-code.start` and `claude-code-shell.start` at the call (19.3): the checks that
/// need nothing outside, then the request.
pub(crate) fn start(inv: &Invocation, kind: Kind, args: &[Val]) -> Result<Value, CallError> {
    if let Some(refusal) = platform_refusal() {
        return Ok(refusal);
    }
    let text = text_arg(args, 0, "start")?;
    let mut launch = match Launch::parse(&text, kind) {
        Ok(l) => l,
        Err(b) => return Ok(bad_launch(&b.field, &b.reason)),
    };
    let host = &inv.chain.host;
    // Picked folders: resolved now; the workspace id is the canonical path (19.3, Folders).
    let user_home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let forbidden = launch::Forbidden::new(host.home(), user_home.as_deref());
    if let Workspace::Picked(raw) = &launch.workspace {
        match launch::resolve_picked(raw, &forbidden) {
            Ok(path) => launch.workspace = Workspace::Picked(path),
            Err(reason) => return Ok(bad_launch("workspace", &reason)),
        }
    }
    let mut resolved: Vec<String> = vec![];
    for raw in &launch.extra_picked {
        match launch::resolve_picked(raw, &forbidden) {
            Ok(path) => {
                if !resolved.contains(&path) {
                    resolved.push(path);
                }
            }
            Err(reason) => return Ok(bad_launch("extra-workspaces", &reason)),
        }
    }
    launch.extra_picked = resolved;
    // Sessions are the origin client's (19.3): another identity's id is refused.
    if let Some(used) = &launch.session {
        let (id, must_hold) = used.id();
        let owner = host.state.session_owner(id)?;
        let ok = match owner.as_deref() {
            Some(o) => o == inv.origin,
            None => !must_hold,
        };
        if !ok {
            return Ok(bad_launch(
                "session",
                "the session is unknown, or another mod's",
            ));
        }
    }
    let held = |id: &str| -> Result<bool, CallError> {
        Ok(host.state.scratch_owner(id)?.as_deref() == Some(inv.origin.as_str()))
    };
    if let Workspace::Scratch(id) = &launch.workspace
        && !held(id)?
    {
        return Ok(bad_launch(
            "workspace",
            "the scratch workspace is unknown, or another mod's",
        ));
    }
    for id in &launch.extra {
        if !held(id)? {
            return Ok(bad_launch(
                "extra-workspaces",
                "the scratch workspace is unknown, or another mod's",
            ));
        }
    }
    // A handle is the origin client's (19.5); one opened in this chain is not applied yet.
    if let Some(handle) = &launch.agent_handle
        && !host.handles.held_by(handle, &inv.origin)
    {
        return Ok(bad_launch(
            "agent-handle",
            "the origin client holds no such handle",
        ));
    }
    let me = inv.me();
    let key = Consumer::Mod(me.clone()).key().unwrap_or_default();
    let gate = inv.gate();
    let mut log = inv.log();
    let mut opened = 0;
    let mut queued = |e: &Effect| {
        if e.consumer_key().as_deref() == Some(key.as_str()) {
            opened += 1;
        }
    };
    gate.each(&mut queued);
    log.iter().for_each(&mut queued);
    if host.hub.open_count(&key) + opened >= MAX_OPEN {
        return Ok(json!({"err": "full"}));
    }
    let id = super::streams::random_id();
    log.push(Effect::ClaudeStart {
        id: id.clone(),
        consumer: me.clone(),
        consumer_gen: inv.package_of(&inv.local),
        origin: inv.origin.clone(),
        origin_gen: inv.origin_gen,
        launch: Arc::new(launch),
        shell: kind == Kind::Shell,
    });
    Ok(json!({"ok": id}))
}

/// The functions of `claude-session` at the call (19.3, The session functions).
pub(crate) fn session_call(
    inv: &Invocation,
    function: &str,
    args: &[Val],
) -> Result<Value, CallError> {
    let err = |case: &str| Ok(json!({"err": case}));
    let id = text_arg(args, 0, function)?;
    let host = &inv.chain.host;
    let me = inv.me();
    let mut log = inv.log();
    let Some(view) = inv.view(&log, &id) else {
        return err("unknown-stream");
    };
    // A stream started in this chain has no session yet: it is one when a `claude-code.start`
    // of this chain opened it (a shell's or a mod's stream is not).
    let session = if view.in_chain {
        log.iter()
            .any(|e| matches!(e, Effect::ClaudeStart { id: i, shell: false, .. } if *i == id))
    } else {
        host.sessions.get(&id).is_some()
    };
    if view.consumer != Consumer::Mod(me.clone()) || !session {
        return err("not-yours");
    }
    let what = match function {
        "say" => {
            let content = text_arg(args, 1, function)?;
            if control::is_slash_command(&content) {
                return err("slash-command");
            }
            Write::Say(content)
        }
        "answer" => {
            let ask = text_arg(args, 1, function)?;
            let decision = decision_arg(args.get(2))?;
            // A stream started in this chain has no ask open yet.
            let live = (!view.in_chain).then(|| host.sessions.get(&id)).flatten();
            match live.map(|s| s.check_answer(&host.hub, &ask, &decision)) {
                Some(Ok(())) => Write::Answer { ask, decision },
                Some(Err(refused)) => return err(refused.case()),
                None => return err("unknown-ask"),
            }
        }
        "interrupt" => Write::Interrupt,
        "compact" => Write::Compact,
        other => {
            return Err(CallError::internal(format!(
                "no host function claude-session.{other}"
            )));
        }
    };
    log.push(Effect::ClaudeWrite { id, what, by: me });
    Ok(json!({"ok": null}))
}

impl Host {
    /// Applies a `start` of a committed chain: the stream, and the thread that runs `claude`
    /// (19.6, At apply).
    #[expect(clippy::too_many_arguments, reason = "the fields of one effect")]
    pub(crate) fn claude_start(
        &self,
        id: String,
        consumer: ModRef,
        origin: String,
        origin_gen: Option<u64>,
        launch: Arc<Launch>,
        shell: bool,
        valid: bool,
    ) -> Option<run::Job> {
        let session = Arc::new(
            Session::new(id.clone(), origin.clone(), launch.check).scoped(
                consumer.clone(),
                origin_gen,
                launch.picked(),
            ),
        );
        let hook = Arc::clone(&session);
        self.hub.open_host(
            id.clone(),
            consumer,
            (origin, origin_gen),
            Box::new(move || hook.stop()),
        );
        if !valid {
            self.hub.end_gone(&[id]);
            return None;
        }
        #[cfg(windows)]
        {
            let _ = shell;
            self.hub.finish(
                &id,
                json!({"failed": "Claude sessions require Unix process-group controls"}),
            );
            None
        }
        #[cfg(unix)]
        {
            if shell {
                self.hub.finish(&id, run::shell_waits());
                return None;
            }
            if self.sessions.register(&session).is_err() {
                self.hub
                    .finish(&id, json!({"failed": "too many claude processes"}));
                return None;
            }
            Some(run::Job {
                weak: std::sync::Weak::<Host>::clone(&self.weak),
                hub: Arc::clone(&self.hub),
                session: Arc::clone(&session),
                sessions: Arc::clone(&self.sessions),
                launch,
                home: self.home().to_path_buf(),
                mode: self.home.mode.clone(),
                fake: self
                    .fake_claude
                    .lock()
                    .expect("lock poisoned: another thread panicked while holding it")
                    .clone(),
            })
        }
    }

    #[cfg(windows)]
    pub(crate) fn claude_run(&self, job: run::Job) {
        match job {}
    }

    /// Starts the thread of a session, unless its stream ended first (19.3, At apply).
    #[cfg(unix)]
    pub(crate) fn claude_run(&self, job: run::Job) {
        let id = job.session.id.clone();
        if job.session.stopped() {
            self.sessions.remove(&id);
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("watane-claude".into())
            .spawn(move || job.go());
        if spawned.is_err() {
            self.sessions.remove(&id);
            self.hub
                .finish(&id, json!({"failed": "cannot start a thread for claude"}));
        }
    }

    /// Applies a write of a committed chain, if the stream is still the caller's (19.3).
    pub(crate) fn claude_write(&self, id: &str, what: Write, by: &ModRef) {
        let Some(session) = self.sessions.get(id) else {
            return;
        };
        if self
            .hub
            .info(id)
            .is_none_or(|i| i.consumer != Consumer::Mod(by.clone()))
        {
            return;
        }
        match what {
            Write::Say(text) => {
                session.write(control::user(&text));
            }
            Write::Compact => {
                session.write(control::user("/compact"));
            }
            Write::Interrupt => {
                session.interrupt(&self.hub, control::interrupt(&super::streams::random_id()));
            }
            Write::Answer { ask, decision } => session.apply_answer(&self.hub, &ask, &decision),
        }
    }
}

#[cfg(all(test, unix))] // Exercises Unix process groups and PID records.
mod tests {
    /// 3.1 step 3, 19.3 step 5 (review r6, F6.4): a run removes only its own line of `runs/pids`.
    #[test]
    fn s100_fix18_ac4_a_run_removes_its_own_record_and_not_a_newer_one_of_the_same_pgid() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let (old, new) = ("4242 boot-a 100", "4242 boot-a 200");
        std::fs::write(home.path().join("runs/pids"), format!("{old}\n{new}\n")).unwrap();
        Sessions::default().forget_pid(home.path(), old);
        assert_eq!(
            std::fs::read_to_string(home.path().join("runs/pids")).unwrap(),
            format!("{new}\n")
        );
    }
    use super::*;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::time::Duration;

    /// 19.3 step 5 (S100-CAP): when `claude` has exited, its group is killed while its pid is
    /// still held, then cleared, so a later `stop()` signals no group.
    #[test]
    fn s100_fix16_ac3_exited_kills_the_group_then_clears_it() {
        let group = || {
            std::process::Command::new("sleep")
                .arg("30")
                .process_group(0)
                .spawn()
                .unwrap()
        };
        let mut left = group();
        let session = Session::new("s".into(), "o".into(), None);
        let _rx = session.started(i32::try_from(left.id()).unwrap());
        session.exited();
        assert_eq!(session.running().pgid, None);
        let status = left.wait().unwrap();
        assert_eq!(
            status.signal(),
            Some(9),
            "the group left by claude is killed"
        );
        // A later `stop()` (and the run's cleanup) signals nothing.
        let mut other = group();
        session.stop();
        session.kill_group();
        std::thread::sleep(Duration::from_millis(200));
        let alive = other.try_wait().unwrap().is_none();
        let _ = other.kill();
        let _ = other.wait();
        assert!(alive);
    }
}
