//! H2, Claude Code (TECHNICAL 19.3): `claude-code.start`, `claude-code-shell.start` and the
//! functions of `claude-session` at the call, the table of live sessions, and the start that
//! a committed chain applies.

pub(crate) mod control;
mod init;
pub(crate) mod launch;
pub(crate) mod run;

use super::streams::{Consumer, MAX_OPEN};
use super::{Effect, ModRef};
use crate::runtime::call::Invocation;
use crate::{CallError, Host};
use launch::{Kind, Launch, Workspace};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
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
}

/// One `claude` run: its process group, and the queue of lines for its stdin.
#[derive(Debug)]
pub(crate) struct Session {
    pub id: String,
    pub origin: String,
    /// The `initialize` request's id: the answer to it is the one the run waits for.
    pub request_id: String,
    pgid: AtomicI32,
    stopped: AtomicBool,
    tx: Mutex<Option<Sender<Vec<u8>>>>,
    rx: Mutex<Option<Receiver<Vec<u8>>>>,
}
impl Session {
    pub(crate) fn new(id: String, origin: String) -> Self {
        let (tx, rx) = channel();
        let request_id = super::streams::random_id();
        // `initialize` is the first line, whatever else is queued while the process starts.
        let _ = tx.send(control::initialize(&request_id));
        Self {
            id,
            origin,
            request_id,
            pgid: AtomicI32::new(0),
            stopped: AtomicBool::new(false),
            tx: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
        }
    }
    pub(crate) fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    /// Kills the group and closes the queue. Safe to call again.
    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
        run::kill_group(self.pgid.load(Ordering::Acquire));
        self.tx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take();
    }
    /// The process started: its group, and the queue of its stdin.
    pub(crate) fn started(&self, pgid: i32) -> Receiver<Vec<u8>> {
        self.pgid.store(pgid, Ordering::Release);
        if self.stopped() {
            run::kill_group(pgid);
        }
        self.rx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .take()
            .unwrap_or_else(|| channel().1)
    }
    /// Queues one line for stdin (the writer adds the newline).
    pub(crate) fn write(&self, line: Vec<u8>) {
        if let Some(tx) = self
            .tx
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .as_ref()
        {
            let _ = tx.send(line);
        }
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
    /// Adds a process group to `<home>/runs/pids` (19.3, The run).
    pub(crate) fn record_pid(&self, home: &Path, pgid: i32) {
        let _guard = self
            .pids
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let file = home.join("runs/pids");
        let mut text = std::fs::read_to_string(&file).unwrap_or_default();
        text.push_str(&format!("{pgid}\n"));
        let _ = std::fs::write(file, text);
    }
    pub(crate) fn forget_pid(&self, home: &Path, pgid: i32) {
        let _guard = self
            .pids
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        let file = home.join("runs/pids");
        if let Ok(text) = std::fs::read_to_string(&file) {
            let kept: String = text
                .lines()
                .filter(|l| l.trim() != pgid.to_string())
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

/// `claude-code.start` and `claude-code-shell.start` at the call (19.3): the checks that
/// need nothing outside, then the request.
pub(crate) fn start(inv: &Invocation, kind: Kind, args: &[Val]) -> Result<Value, CallError> {
    let text = text_arg(args, 0, "start")?;
    let launch = match Launch::parse(&text, kind) {
        Ok(l) => l,
        Err(b) => return Ok(bad_launch(&b.field, &b.reason)),
    };
    let host = &inv.chain.host;
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
    let Some(info) = host.hub.info(&id) else {
        return err("unknown-stream");
    };
    if info.consumer != Consumer::Mod(me.clone()) || host.sessions.get(&id).is_none() {
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
        "answer" => return err("unknown-ask"),
        "interrupt" => Write::Interrupt,
        "compact" => Write::Compact,
        other => {
            return Err(CallError::internal(format!(
                "no host function claude-session.{other}"
            )));
        }
    };
    inv.log().push(Effect::ClaudeWrite { id, what, by: me });
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
    ) {
        let session = Arc::new(Session::new(id.clone(), origin.clone()));
        let hook = Arc::clone(&session);
        self.hub.open_host(
            id.clone(),
            consumer,
            (origin, origin_gen),
            Box::new(move || hook.stop()),
        );
        if !valid {
            self.hub.end_gone(&[id]);
            return;
        }
        if shell {
            self.hub.finish(&id, run::shell_waits());
            return;
        }
        if self.sessions.register(&session).is_err() {
            self.hub
                .finish(&id, json!({"failed": "too many claude processes"}));
            return;
        }
        let job = run::Job {
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
        };
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
        session.write(match what {
            Write::Say(text) => control::user(&text),
            Write::Compact => control::user("/compact"),
            Write::Interrupt => control::interrupt(&super::streams::random_id()),
        });
    }
}
