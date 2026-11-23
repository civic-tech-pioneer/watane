//! The run of `claude` (TECHNICAL 19.3, The run, Folders): the binary and its version, the
//! scratch workspace, the run folder, the process group, the fixed environment, the host's
//! `--settings=`, and the thread that reads the stream and ends it.

use super::asks::ASK_TTL;
use super::control::{self, Fields, Line};
use super::launch::{Launch, Workspace};
use super::{Session, Sessions, init};
use crate::agent::streams::{Hub, MAX_ITEM, StreamError};
use crate::{Host, Mode};
use serde_json::{Value, json};
use std::io::{BufRead, Read};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Weak};
use std::time::Duration;

/// The oldest `claude` that the host accepts (19.3, The run: Binary).
const MIN_VERSION: (u64, u64, u64) = (2, 1, 295);
const NEED: &str = "2.1.295";
const PATH: &str = "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin";
/// The reason of a deny at once.
const REFUSED: &str =
    "the host refused: the launch did not ask for this, or the request is malformed";
const SHELL_WAITS: &str = "the shell grant waits on section 18, Q39 and Q42";

/// Files that no launch can let `claude` write (19.3, Protected paths), as rules for `Edit`,
/// `Write` and `NotebookEdit`.
const PROTECTED_PATHS: [&str; 14] = [
    ".git/**",
    ".claude/**",
    ".agents/**",
    ".codex/**",
    "**/.aws/**",
    "**/CLAUDE.md",
    "**/AGENTS.md",
    ".mcp.json",
    ".envrc",
    "**/.github/workflows/**",
    "build.rs",
    "package.json",
    "Makefile",
    ".husky/**",
];
/// The participant's files that `claude` may not read or write (19.3, Protected reads), under
/// `$HOME`.
const PROTECTED_HOME: [&str; 12] = [
    ".claude/**",
    ".claude.json",
    ".codex/**",
    ".ssh/**",
    ".aws/**",
    ".gnupg/**",
    ".netrc",
    ".git-credentials",
    ".config/gh/**",
    ".docker/**",
    ".kube/**",
    "Library/Keychains/**",
];
/// The watane home's own files, under the home.
const PROTECTED_WATANE: [&str; 7] = [
    "app/**",
    "dist/**",
    "log/**",
    "net/**",
    "state/**",
    "installed.json",
    "toolsets.json",
];
const FILE_TOOLS: [&str; 6] = ["Read", "Glob", "Grep", "Edit", "Write", "NotebookEdit"];

/// The `--settings=` JSON of every start (19.3): the deny rules and no unsandboxed commands.
pub(crate) fn settings(watane_home: &Path, user_home: Option<&Path>) -> String {
    let mut deny: Vec<String> = vec![];
    for tool in ["Edit", "Write", "NotebookEdit"] {
        deny.extend(PROTECTED_PATHS.iter().map(|p| format!("{tool}({p})")));
    }
    let mut roots: Vec<(PathBuf, &[&str])> = vec![(watane_home.to_path_buf(), &PROTECTED_WATANE)];
    if let Some(home) = user_home {
        roots.push((home.to_path_buf(), &PROTECTED_HOME));
    }
    for (root, patterns) in roots {
        // An absolute rule starts with `//`; the root is canonical, and so is each existing
        // folder under it, in a second spelling when it differs (a symlink).
        let root = root.canonicalize().unwrap_or(root);
        for pattern in patterns {
            let mut paths = vec![root.join(pattern)];
            let stem = pattern.trim_end_matches("/**");
            if let Ok(real) = root.join(stem).canonicalize()
                && real != root.join(stem)
            {
                paths.push(if stem == *pattern {
                    real
                } else {
                    real.join("**")
                });
            }
            for path in paths {
                for tool in FILE_TOOLS {
                    deny.push(format!("{tool}(/{})", path.display()));
                }
            }
        }
    }
    json!({"permissions": {"deny": deny}, "sandbox": {"allowUnsandboxedCommands": false}})
        .to_string()
}

/// The environment of `claude` (19.3, The run): nothing of the host's but these.
pub(crate) fn environment(depth: u8) -> Vec<(String, String)> {
    let mut env = vec![];
    for (name, value) in std::env::vars() {
        let kept = matches!(
            name.as_str(),
            "HOME" | "USER" | "LOGNAME" | "LANG" | "TMPDIR" | "SHELL" | "CLAUDE_CODE_OAUTH_TOKEN"
        ) || name.starts_with("LC_");
        if kept {
            env.push((name, value));
        }
    }
    env.push(("PATH".into(), PATH.into()));
    env.push((
        "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH".into(),
        depth.to_string(),
    ));
    env
}

/// Sends SIGKILL to a process group.
fn killpg(pgid: i32) {
    if pgid > 1
        && let Some(pid) = rustix::process::Pid::from_raw(pgid)
    {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}
pub(crate) fn kill_group(pgid: i32) {
    killpg(pgid);
}

fn random_hex() -> String {
    crate::agent::streams::random_id()
}

/// Kills the groups that a crashed host left in `runs/pids` and empties `runs/` (3.1 step 3).
pub(crate) fn clean_runs(home: &Path) {
    let runs = home.join("runs");
    if let Ok(text) = std::fs::read_to_string(runs.join("pids")) {
        for pgid in text.lines().filter_map(|l| l.trim().parse::<i32>().ok()) {
            killpg(pgid);
        }
    }
    let _ = std::fs::remove_dir_all(&runs);
}

/// `<program> --version` with the fixed environment, at most 15 s.
fn probe(program: &Path) -> Result<String, String> {
    let mut child = Command::new(program)
        .arg("--version")
        .env_clear()
        .envs(environment(1))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    let pgid = i32::try_from(child.id()).unwrap_or(0);
    let mut out = child.stdout.take().expect("piped");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = out.by_ref().take(4096).read_to_string(&mut text);
        let _ = child.wait();
        let _ = tx.send(text);
    });
    let result = rx.recv_timeout(Duration::from_secs(15));
    killpg(pgid);
    result.map_err(|_| "it did not answer".to_owned())
}
/// `x.y.z` from the first word of `2.1.295 (Claude Code)`.
fn parse_version(text: &str) -> Option<((u64, u64, u64), String)> {
    let word = text.split_whitespace().next()?;
    let mut parts = word.split('.');
    let n = |p: Option<&str>| p?.parse::<u64>().ok();
    let v = (n(parts.next())?, n(parts.next())?, n(parts.next())?);
    Some((v, word.to_owned()))
}

/// The line of a new scratch workspace: `scratch:<id>`.
fn scratch_name(id: &str) -> String {
    format!("scratch:{id}")
}

/// Everything the thread of one `claude` run owns.
pub(crate) struct Job {
    pub weak: Weak<Host>,
    pub hub: Arc<Hub>,
    pub session: Arc<Session>,
    pub sessions: Arc<Sessions>,
    pub launch: Arc<Launch>,
    pub home: PathBuf,
    pub mode: Mode,
    pub fake: Option<PathBuf>,
}

/// Removes the run folder and forgets the pid when the run ends, however it ends.
struct Cleanup {
    session: Arc<Session>,
    sessions: Arc<Sessions>,
    home: PathBuf,
    folder: Option<PathBuf>,
    pgid: Option<i32>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(pgid) = self.pgid {
            // Through the session: a group whose leader was reaped is cleared there.
            self.session.kill_group();
            self.sessions.forget_pid(&self.home, pgid);
        }
        if let Some(folder) = &self.folder {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}

impl Job {
    /// Runs on its own thread; ends the stream with the outcome (19.3).
    pub(crate) fn go(self) {
        let id = self.session.id.clone();
        let outcome = self.run();
        self.sessions.remove(&id);
        self.hub.finish(&id, outcome);
    }

    fn failed(message: impl std::fmt::Display) -> Value {
        json!({"failed": message.to_string()})
    }

    /// The program to start: the preview's fake, or the participant's pick (19.3, 19.4, 19.9).
    fn binary(&self) -> Result<PathBuf, Value> {
        match &self.mode {
            Mode::Preview => self.fake.clone().ok_or_else(|| json!("no-binary")),
            Mode::Home { app } => {
                let accounts = app
                    .as_ref()
                    .and_then(|app| app.accounts())
                    .ok_or_else(|| json!("no-app"))?;
                accounts.claude_binary().map_err(|e| match e {
                    crate::AccountError::NoAccount => json!("no-binary"),
                    crate::AccountError::Unavailable(m) => {
                        Self::failed(format!("account unavailable: {m}"))
                    }
                })
            }
        }
    }

    /// The path of a scratch workspace that the origin client holds (19.3, Folders).
    fn held_scratch(&self, id: &str) -> Result<PathBuf, Value> {
        let host = self.weak.upgrade().ok_or_else(|| json!("gone"))?;
        let owner = host
            .state
            .scratch_owner(id)
            .map_err(|e| Self::failed(e.message))?;
        let path = self.home.join("scratch").join(id);
        if owner.as_deref() == Some(self.session.origin.as_str()) && path.is_dir() {
            Ok(path)
        } else {
            Err(json!("gone"))
        }
    }
    /// Makes a new scratch workspace: 32 hex chars, mode 0700, owned by the origin client.
    fn new_scratch(&self) -> Result<(String, PathBuf), Value> {
        let host = self.weak.upgrade().ok_or_else(|| json!("gone"))?;
        let id = random_hex();
        let scratch = self.home.join("scratch");
        let path = scratch.join(&id);
        std::fs::create_dir_all(&scratch)
            .and_then(|()| std::fs::DirBuilder::new().mode(0o700).create(&path))
            .map_err(|e| Self::failed(format!("cannot make the scratch workspace: {e}")))?;
        host.state
            .scratch_add(&id, &self.session.origin)
            .map_err(|e| Self::failed(e.message))?;
        // The owner may have been removed since the last check: delete this folder and this
        // row only, and end `gone` (19.3, Folders, Lifetime).
        if self.session.stopped() {
            let _ = std::fs::remove_dir_all(&path);
            let _ = host.state.scratch_delete(&id);
            return Err(json!("gone"));
        }
        Ok((id, path))
    }

    #[expect(clippy::too_many_lines, reason = "one run, in the steps of 19.3")]
    fn run(&self) -> Value {
        if self.session.stopped() {
            return json!("gone");
        }
        let launch = &self.launch;
        let program = match self.binary() {
            Ok(p) => p,
            Err(outcome) => return outcome,
        };
        let text = match probe(&program) {
            Ok(t) => t,
            Err(_) => return json!("no-binary"),
        };
        let Some((version, found)) = parse_version(&text) else {
            return Self::failed("cannot read the version of claude");
        };
        if version < MIN_VERSION {
            return json!({"version": {"found": found, "need": NEED}});
        }
        // The owner may have been removed while the probe ran: a stopped start makes nothing.
        if self.session.stopped() {
            return json!("gone");
        }
        let (workspace, cwd) = match &launch.workspace {
            Workspace::NewScratch => match self.new_scratch() {
                Ok((id, path)) => (scratch_name(&id), path),
                Err(outcome) => return outcome,
            },
            Workspace::Scratch(id) => match self.held_scratch(id) {
                Ok(path) => (scratch_name(id), path),
                Err(outcome) => return outcome,
            },
        };
        let mut args: Vec<String> = [
            "-p",
            "--input-format=stream-json",
            "--output-format=stream-json",
            "--verbose",
            "--restricted",
            "--strict-mcp-config",
            "--system-prompt-snapshot=off",
        ]
        .map(str::to_owned)
        .to_vec();
        let user_home = std::env::var_os("HOME").map(PathBuf::from);
        args.push(format!(
            "--settings={}",
            settings(&self.home, user_home.as_deref())
        ));
        let mut extra_paths = vec![];
        for id in &launch.extra {
            match self.held_scratch(id) {
                Ok(path) => {
                    args.push(format!("--add-dir={}", path.display()));
                    extra_paths.push(path);
                }
                Err(outcome) => return outcome,
            }
        }
        args.extend(launch.flags.iter().cloned());
        if self.session.stopped() {
            return json!("gone");
        }
        let mut cleanup = Cleanup {
            session: Arc::clone(&self.session),
            sessions: Arc::clone(&self.sessions),
            home: self.home.clone(),
            folder: None,
            pgid: None,
        };
        let folder = self.home.join("runs").join(random_hex());
        let plugin = folder.join("watane-run");
        let made = std::fs::create_dir_all(plugin.join(".claude-plugin")).and_then(|()| {
            std::fs::write(
                plugin.join(".claude-plugin/plugin.json"),
                r#"{"name":"watane-run","version":"0.1.0","description":"The host's plugin for one run."}"#,
            )
        });
        let canon = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let run_root = canon(&plugin);
        let mut roots = vec![canon(&cwd), run_root.clone()];
        roots.extend(extra_paths.iter().map(|p| canon(p)));
        cleanup.folder = Some(folder);
        if let Err(e) = made {
            return Self::failed(format!("cannot make the run folder: {e}"));
        }
        args.push(format!("--plugin-dir={}", plugin.display()));
        if self.session.stopped() {
            return json!("gone");
        }
        let child = match Command::new(&program)
            .args(&args)
            .current_dir(&cwd)
            .env_clear()
            .envs(environment(launch.depth))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return Self::failed(format!("cannot start claude: {e}")),
        };
        let pgid = i32::try_from(child.id()).unwrap_or(0);
        cleanup.pgid = Some(pgid);
        self.sessions.record_pid(&self.home, pgid);
        let (mut child, stdin) = match take_stdin(child, pgid, make_non_blocking) {
            Ok(taken) => taken,
            Err(failed) => return failed,
        };
        self.session.set_roots(cwd.clone(), roots, run_root);
        let ticker = Arc::clone(&self.session);
        let hub = Arc::clone(&self.hub);
        std::thread::spawn(move || {
            while !ticker.stopped() {
                ticker.tick(&hub);
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let mut stdout = std::io::BufReader::new(child.stdout.take().expect("piped"));
        // The writer thread is the one place that writes to stdin; a line goes through its queue.
        // `initialize` goes at once; every later line waits for `release`, which the reader sends
        // once the answer passed step 2. When the reader ends without it, `release` drops and
        // nothing more is written (19.3, step 5).
        let rx = self.session.started(pgid);
        let writer_session = Arc::clone(&self.session);
        let writer_hub = Arc::clone(&self.hub);
        let (release, held) = std::sync::mpsc::channel::<Vec<Vec<u8>>>();
        let giveup = Arc::new(std::sync::OnceLock::new());
        let writer = {
            let giveup = Arc::clone(&giveup);
            std::thread::spawn(move || {
                write_lines(&stdin, &rx, &held, &writer_session, &writer_hub, &giveup);
            })
        };
        // The waiter waits for the child to exit, kills what is left of its group, so that a
        // grandchild cannot keep the pipe open, and only then reaps it: until the reap the pid
        // is held, so no kill reaches a process that reused it (19.3 step 5, S100-CAP).
        let (exited, exit) = std::sync::mpsc::channel();
        let session = Arc::clone(&self.session);
        let waiter = std::thread::spawn(move || {
            wait_unreaped(pgid);
            session.exited();
            let _ = exited.send(child.wait());
        });
        let request_id = self.session.request_id.clone();
        let mut reader = Reading {
            job: self,
            answered: false,
            request_id,
            workspace,
            awaiting_init: false,
            early: vec![],
            release: Some(release),
        };
        let ended = reader.read(&mut stdout);
        // The output is at its end: the process gets `EXIT_WAIT` to exit, then its group is
        // killed (19.3, step 5).
        let status = exit.recv_timeout(EXIT_WAIT).or_else(|_| {
            self.session.kill_group();
            exit.recv_timeout(EXIT_WAIT)
        });
        if status.is_ok() {
            let _ = waiter.join();
        }
        let status = status.ok().and_then(Result::ok);
        // Before the stream ends: the writer reports every answer it wrote, then each waiting
        // `ask-resolved` goes on the stream, so `ended` never overtakes one (19.3, step 5).
        drop(reader);
        end_writer(&self.session, writer, &giveup, WRITER_WAIT);
        self.session.drain(&self.hub);
        self.session.seal();
        if let Some(outcome) = ended {
            return outcome;
        }
        match status {
            Some(s) if s.success() => json!("closed"),
            Some(s) => match (s.code(), s.signal()) {
                (Some(code), _) => Self::failed(format!("claude exited with status {code}")),
                (None, Some(sig)) => Self::failed(format!("claude was killed by signal {sig}")),
                _ => Self::failed("claude exited"),
            },
            None => Self::failed("claude exited"),
        }
    }
}

/// The state of reading the stdout of one run.
struct Reading<'a> {
    job: &'a Job,
    /// The answer to `initialize` came: before it only control lines may come, and none is
    /// pushed, so `initialized` is the first item (19.3, step 2).
    answered: bool,
    request_id: String,
    workspace: String,
    /// The answer to `initialize` came and the `system/init` line has not: only control lines
    /// may come before it (19.3, step 3).
    awaiting_init: bool,
    /// The denies of requests that came before the answer to `initialize`: nothing is written
    /// before it passed step 2, so they go first once it did, and are dropped when it fails
    /// (19.3, step 5).
    early: Vec<Vec<u8>>,
    /// Lets the writer thread go on after `initialize`, with `early`; dropped unsent when the
    /// check fails.
    release: Option<std::sync::mpsc::Sender<Vec<Vec<u8>>>>,
}
impl Reading<'_> {
    /// Pushes one item, waiting while the stream is full (a host stream stops reading its
    /// source, 19.2). `Err` when the stream is gone or the item cannot go.
    fn push(&self, body: Vec<u8>) -> Result<(), Value> {
        let session = &self.job.session;
        loop {
            match session.push(&self.job.hub, &body) {
                Ok(()) => return Ok(()),
                Err(StreamError::Full) if !session.stopped() => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(StreamError::TooLarge) => {
                    return Err(Job::failed("a line of claude is over 4 MiB"));
                }
                Err(_) => return Err(json!("gone")),
            }
        }
    }
    /// An ask or a check, or a deny at once when the launch did not ask for it, the request
    /// is malformed, or its item is too large to push (19.3, step 4).
    fn request(
        &mut self,
        request_id: &str,
        hook: bool,
        fields: Option<&Fields>,
    ) -> Result<(), Value> {
        let session = &self.job.session;
        let launch = &self.job.launch;
        let wanted = if hook {
            launch.check.is_some()
        } else {
            launch.ask
        };
        // Nothing is opened before `system/init` passed (19.3, step 5).
        let (true, true, Some(fields)) = (wanted, self.answered && !self.awaiting_init, fields)
        else {
            let deny = control::deny(request_id, hook, REFUSED);
            if self.answered {
                session.write(deny);
            } else {
                self.early.push(deny);
            }
            return Ok(());
        };
        let ttl = match launch.check {
            Some(t) if hook => Duration::from_secs(t),
            _ => ASK_TTL,
        };
        loop {
            match session.open_request(&self.job.hub, request_id, hook, fields, ttl) {
                Ok(()) => return Ok(()),
                Err(StreamError::Full) if !session.stopped() => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(StreamError::TooLarge) => {
                    session.write(control::deny(request_id, hook, REFUSED));
                    return Ok(());
                }
                Err(_) => return Err(json!("gone")),
            }
        }
    }
    fn item(&self, body: Vec<u8>) -> Result<(), Value> {
        if self.answered {
            self.push(body)
        } else {
            Ok(())
        }
    }
    /// Reads until the end of the output. `Some(outcome)` when the run must end with it.
    fn read(&mut self, stdout: &mut impl BufRead) -> Option<Value> {
        loop {
            let mut raw = vec![];
            let limit = MAX_ITEM as u64 + 2;
            let n = stdout
                .by_ref()
                .take(limit)
                .read_until(b'\n', &mut raw)
                .ok()?;
            if n == 0 {
                return None;
            }
            if raw.last() == Some(&b'\n') {
                raw.pop();
            } else if raw.len() > MAX_ITEM {
                self.job.session.stop();
                return Some(Job::failed("a line of claude is over 4 MiB"));
            }
            if let Err(outcome) = self.line(&raw) {
                self.job.session.stop();
                return Some(outcome);
            }
        }
    }
    fn line(&mut self, raw: &[u8]) -> Result<(), Value> {
        let line = control::classify(raw);
        if (!self.answered || self.awaiting_init)
            && !matches!(line, Line::Init(..))
            && !control::is_control(&line)
        {
            return Err(json!({"mismatch": "init"}));
        }
        match line {
            Line::Item(body) => self.item(body),
            Line::Request {
                request_id,
                hook,
                fields,
            } => self.request(&request_id, hook, fields.as_ref()),
            Line::Cancel { request_id } => {
                self.job.session.cancel_request(&self.job.hub, &request_id);
                Ok(())
            }
            Line::Init(value, body) => {
                if !self.answered {
                    return Err(json!({"mismatch": "init"}));
                }
                self.awaiting_init = false;
                if let Err(field) = init::check(&self.job.launch, &value) {
                    return Err(json!({"mismatch": field}));
                }
                self.item(body)
            }
            Line::Response {
                request_id,
                ok,
                body,
            } => {
                if !self.answered && request_id == self.request_id {
                    if !ok {
                        return Err(Job::failed("claude refused initialize"));
                    }
                    // With a callback, the answer must hold the hook support (19.3, step 2).
                    if self.job.launch.check.is_some()
                        && (body.get("hook_on_failure_supported") != Some(&Value::Bool(true))
                            || body.get("hooks_applied") == Some(&Value::Bool(false)))
                    {
                        return Err(json!({"mismatch": "hooks"}));
                    }
                    let workspace = Value::String(self.workspace.clone());
                    self.push(
                        format!(r#"{{"type":"initialized","workspace":{workspace}}}"#).into_bytes(),
                    )?;
                    self.answered = true;
                    self.awaiting_init = true;
                    if let Some(release) = self.release.take() {
                        let _ = release.send(std::mem::take(&mut self.early));
                    }
                    Ok(())
                } else {
                    self.item(raw.to_vec())
                }
            }
        }
    }
}

/// `claude-code-shell.start`: its launch is checked, then its stream ends (19.3, Grants).
pub(crate) fn shell_waits() -> Value {
    Job::failed(SHELL_WAITS)
}

/// How long the end of a run waits for the process to exit once its output is at its end, and
/// then for the writer thread (19.3, step 5).
const EXIT_WAIT: Duration = Duration::from_secs(2);
const WRITER_WAIT: Duration = Duration::from_secs(2);

/// When the writer gives up a write that blocks: set by `end_writer`.
type Giveup = std::sync::OnceLock<std::time::Instant>;

/// Writes all of `line` to the non-blocking `stdin`. On a full pipe it waits for room in slices of
/// 20 ms and gives up (`false`) once `giveup` has passed. `false` too when the write fails.
fn write_all_by(stdin: &impl std::os::fd::AsFd, mut line: &[u8], giveup: &Giveup) -> bool {
    use rustix::event::{PollFd, PollFlags, poll};
    use rustix::io::Errno;
    while !line.is_empty() {
        // Before every write call, whatever the last one returned (19.3, step 5).
        if giveup
            .get()
            .is_some_and(|d| std::time::Instant::now() >= *d)
        {
            return false;
        }
        match rustix::io::write(stdin, line) {
            Ok(n) => line = &line[n..],
            Err(Errno::INTR) => {}
            Err(Errno::AGAIN) => {
                let slice = rustix::fs::Timespec {
                    tv_sec: 0,
                    tv_nsec: 20_000_000,
                };
                let mut fds = [PollFd::new(stdin, PollFlags::OUT)];
                let _ = poll(&mut fds, Some(&slice));
            }
            Err(_) => return false,
        }
    }
    true
}

/// Waits until the child `pid` has exited, leaving it unreaped (`WNOWAIT`).
fn wait_unreaped(pid: i32) {
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    let Some(pid) = rustix::process::Pid::from_raw(pid) else {
        return;
    };
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT;
    while let Err(rustix::io::Errno::INTR) = waitid(WaitId::Pid(pid), options) {}
}

/// Takes the write end of the child's stdin and sets it up. When `setup` fails, the group is
/// killed and a thread reaps the child, so no zombie is left; the start ends `failed("stdin")`
/// after at most `EXIT_WAIT`, the reaper staying only for a process stuck in the kernel
/// (19.3, step 5).
fn take_stdin(
    mut child: std::process::Child,
    pgid: i32,
    setup: impl FnOnce(&std::process::ChildStdin) -> Result<(), Value>,
) -> Result<(std::process::Child, std::process::ChildStdin), Value> {
    let stdin = child.stdin.take().expect("piped");
    if let Err(failed) = setup(&stdin) {
        killpg(pgid);
        let (reaped, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = child.wait();
            let _ = reaped.send(());
        });
        let _ = wait.recv_timeout(EXIT_WAIT);
        return Err(failed);
    }
    Ok((child, stdin))
}

/// Makes the write end of stdin non-blocking; the start ends `failed("stdin")` when it cannot.
fn make_non_blocking(stdin: &impl std::os::fd::AsFd) -> Result<(), Value> {
    rustix::fs::fcntl_setfl(stdin, rustix::fs::OFlags::NONBLOCK).map_err(|_| Job::failed("stdin"))
}

/// The writer thread: the one place that writes to stdin; a line goes through its queue.
/// `initialize` goes at once; every later line waits for `held`, which the reader sends once the
/// answer passed step 2 (with the denies of the requests that came before it). When the reader
/// ends without it, `held` drops and nothing more is written (19.3, step 5). A write that does not
/// complete by `giveup` ends the writer with no `answered`.
fn write_lines(
    stdin: &impl std::os::fd::AsFd,
    rx: &std::sync::mpsc::Receiver<super::Out>,
    held: &std::sync::mpsc::Receiver<Vec<Vec<u8>>>,
    session: &Session,
    hub: &Hub,
    giveup: &Giveup,
) {
    let mut released = false;
    for mut out in rx {
        out.line.push(b'\n');
        if !write_all_by(stdin, &out.line, giveup) {
            return;
        }
        // `answered` only after the answer reached `claude` (19.3, step 5).
        if let Some((id, outcome)) = &out.answered {
            session.answered(hub, id, outcome);
        }
        if !released {
            // The denies of the requests that came before the answer go first.
            let Ok(early) = held.recv() else {
                return;
            };
            released = true;
            for mut line in early {
                line.push(b'\n');
                if !write_all_by(stdin, &line, giveup) {
                    return;
                }
            }
        }
    }
}

/// Closes the queue of the writer and joins it: a write that blocks gives up at `limit`, so the
/// join ends within it and no thread of the run outlives the bound (19.3, step 5).
fn end_writer(
    session: &Session,
    writer: std::thread::JoinHandle<()>,
    giveup: &Giveup,
    limit: Duration,
) {
    let _ = giveup.set(std::time::Instant::now() + limit);
    session.close_queue();
    let _ = writer.join();
}

#[cfg(test)]
mod join_tests {
    use super::*;
    use crate::agent::streams::Consumer;

    /// A held stream, a session on it, and a real non-blocking pipe for the writer to write to.
    struct Rig {
        hub: Arc<Hub>,
        stream: String,
        session: Arc<Session>,
        writer: Option<std::thread::JoinHandle<()>>,
        giveup: Arc<Giveup>,
        reader: Option<std::io::PipeReader>,
        release: Option<std::sync::mpsc::Sender<Vec<Vec<u8>>>>,
    }
    fn rig() -> Rig {
        let hub = Hub::new();
        let stream = hub
            .test_open(Consumer::Held { origin: "o".into() }, "o".into())
            .unwrap();
        let session = Arc::new(Session::new(stream.clone(), "o".into(), None));
        let rx = session.started(0);
        let (reader, writer_end) = std::io::pipe().unwrap();
        rustix::fs::fcntl_setfl(&writer_end, rustix::fs::OFlags::NONBLOCK).unwrap();
        let (release, held) = std::sync::mpsc::channel();
        let giveup = Arc::new(Giveup::new());
        let thread = {
            let (hub, session, giveup) =
                (Arc::clone(&hub), Arc::clone(&session), Arc::clone(&giveup));
            std::thread::spawn(move || {
                write_lines(&writer_end, &rx, &held, &session, &hub, &giveup)
            })
        };
        Rig {
            hub,
            stream,
            session,
            writer: Some(thread),
            giveup,
            reader: Some(reader),
            release: Some(release),
        }
    }
    impl Rig {
        fn out(&self, line: Vec<u8>) {
            self.session.enqueue(super::super::Out {
                line,
                answered: Some(("7".into(), "allow")),
            });
        }
        fn end(&mut self, limit: Duration) -> Duration {
            // As the run does: the reader is dropped first, and with it the release.
            self.release.take();
            let start = std::time::Instant::now();
            end_writer(
                &self.session,
                self.writer.take().unwrap(),
                &self.giveup,
                limit,
            );
            start.elapsed()
        }
        fn items(&self) -> Vec<String> {
            self.session.drain(&self.hub);
            self.session.seal();
            self.hub
                .test_take(&self.stream)
                .into_iter()
                .map(|(_, b)| String::from_utf8(b).unwrap())
                .collect()
        }
    }

    fn pipe() -> (std::io::PipeReader, std::io::PipeWriter) {
        let (reader, writer) = std::io::pipe().unwrap();
        rustix::fs::fcntl_setfl(&writer, rustix::fs::OFlags::NONBLOCK).unwrap();
        (reader, writer)
    }

    #[test]
    fn s100_fix12_ac1_an_expired_deadline_rejects_a_write_on_a_writable_pipe() {
        let (mut reader, writer) = pipe();
        let giveup = Giveup::new();
        giveup.set(std::time::Instant::now()).unwrap();
        assert!(!write_all_by(&writer, b"line\n", &giveup));
        drop(writer);
        let mut left = vec![];
        std::io::Read::read_to_end(&mut reader, &mut left).unwrap();
        assert!(left.is_empty());
    }

    #[test]
    fn s100_fix12_ac1_a_partial_write_checks_the_deadline_before_the_next_call() {
        // 200 KB into an unread pipe: the first call writes part of it (the pipe holds 64 KB),
        // the reader frees room, and the deadline that passed meanwhile stops the rest.
        let (mut reader, writer) = pipe();
        let giveup = Arc::new(Giveup::new());
        let g = Arc::clone(&giveup);
        let t = std::thread::spawn(move || write_all_by(&writer, &vec![b'x'; 200_000], &g));
        std::thread::sleep(Duration::from_millis(100));
        giveup.set(std::time::Instant::now()).unwrap();
        let mut total = 0;
        let mut buf = vec![0; 8192];
        // Drain what is in the pipe until the writer reports; it must be false.
        while !t.is_finished() {
            total += std::io::Read::read(&mut reader, &mut buf).unwrap();
        }
        assert!(!t.join().unwrap());
        assert!(total < 200_000, "{total}");
    }

    #[test]
    fn s100_fix12_ac1_the_next_queued_line_is_not_started_after_the_deadline() {
        let mut rig = rig();
        rig.release.as_ref().unwrap().send(vec![]).unwrap();
        let mut reader = rig.reader.take().unwrap();
        giveup_now(&rig);
        rig.out(b"first".to_vec());
        rig.out(b"second".to_vec());
        rig.end(Duration::from_secs(5));
        drop(rig.release.take());
        let mut left = vec![];
        let _ = std::io::Read::read_to_end(&mut reader, &mut left);
        assert!(left.is_empty(), "{:?}", String::from_utf8_lossy(&left));
    }
    fn giveup_now(rig: &Rig) {
        let _ = rig.giveup.set(std::time::Instant::now());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn s100_fix12_ac2_a_write_end_that_cannot_be_made_non_blocking_fails_the_start_stdin() {
        let fd = rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::PATH,
            rustix::fs::Mode::empty(),
        )
        .unwrap();
        assert_eq!(
            make_non_blocking(&fd).unwrap_err(),
            serde_json::json!({"failed": "stdin"})
        );
        let (_reader, writer) = std::io::pipe().unwrap();
        make_non_blocking(&writer).unwrap();
    }

    /// 19.3 step 5: a stdin setup that fails after the spawn leaves no zombie child.
    #[cfg(target_os = "linux")]
    #[test]
    fn s100_fix15_ac1_a_failed_stdin_setup_reaps_the_child() {
        use std::os::unix::process::CommandExt;
        let child = Command::new("sleep")
            .arg("60")
            .stdin(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let failed = take_stdin(child, i32::try_from(pid).unwrap(), |_| {
            Err(Job::failed("stdin"))
        })
        .unwrap_err();
        assert_eq!(failed, serde_json::json!({"failed": "stdin"}));
        let waited = rustix::process::waitpid(
            rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()),
            rustix::process::WaitOptions::NOHANG,
        );
        assert_eq!(waited.unwrap_err(), rustix::io::Errno::CHILD);
    }

    #[test]
    fn s100_fix12_ac3_a_close_waits_on_no_consumer_and_goes_before_ended_in_order() {
        let rig = rig();
        let bash = super::super::control::Fields {
            tool_use_id: "t".into(),
            tool: "Bash".into(),
            input: serde_json::json!({"command": "ls"}),
        };
        for rid in ["r7", "r8"] {
            rig.session
                .open_request(&rig.hub, rid, false, &bash, Duration::from_secs(60))
                .unwrap();
        }
        while rig.hub.test_push(&rig.stream, b"{}").is_ok() {}
        assert!(rig.hub.test_push(&rig.stream, b"{}").is_err());
        rig.session.answered(&rig.hub, "1", "allow");
        rig.session.answered(&rig.hub, "2", "deny");
        let start = std::time::Instant::now();
        rig.session.drain(&rig.hub);
        assert!(start.elapsed() < Duration::from_secs(1));
        rig.session.seal();
        let items: Vec<String> = rig
            .hub
            .test_take(&rig.stream)
            .into_iter()
            .map(|(_, b)| String::from_utf8(b).unwrap())
            .collect();
        // The stream was full, and both resolutions are past its limit, in order, last.
        assert_eq!(items.len(), crate::agent::streams::MAX_ITEMS + 2);
        assert_eq!(
            items[crate::agent::streams::MAX_ITEMS..],
            [
                r#"{"type":"ask-resolved","id":"1","by":"answered","outcome":"allow"}"#,
                r#"{"type":"ask-resolved","id":"2","by":"answered","outcome":"deny"}"#
            ]
        );
    }

    #[test]
    fn s100_fix9_ac4_a_writer_that_waits_on_its_queue_ends_when_the_queue_closes() {
        let mut rig = rig();
        let _reader = rig.reader.take();
        assert!(rig.end(Duration::from_secs(5)) < Duration::from_secs(1));
    }

    #[test]
    fn s100_fix9_ac4_a_write_that_completes_during_the_teardown_gets_its_answered_before_ended() {
        let mut rig = rig();
        rig.release.as_ref().unwrap().send(vec![]).unwrap();
        // The reader takes the pipe empty slowly, so the 300 KB line completes only while the
        // teardown waits for the writer.
        let mut reader = rig.reader.take().unwrap();
        let taker = std::thread::spawn(move || {
            let mut buf = vec![0; 4096];
            let mut total = 0;
            while total < 300_000 {
                std::thread::sleep(Duration::from_millis(2));
                match reader.read(&mut buf).unwrap() {
                    0 => break, // the writer ended: the assertions below tell why
                    n => total += n,
                }
            }
        });
        rig.out(vec![b'x'; 300_000]);
        assert!(rig.end(Duration::from_secs(10)) < Duration::from_secs(10));
        taker.join().unwrap();
        assert_eq!(
            rig.items(),
            [r#"{"type":"ask-resolved","id":"7","by":"answered","outcome":"allow"}"#]
        );
    }

    #[test]
    fn s100_fix9_ac4_a_write_that_fails_pushes_no_answered() {
        let mut rig = rig();
        rig.release.as_ref().unwrap().send(vec![]).unwrap();
        // The read end is gone: the write fails (EPIPE).
        drop(rig.reader.take());
        rig.out(b"x".to_vec());
        assert!(rig.end(Duration::from_secs(5)) < Duration::from_secs(2));
        assert_eq!(rig.items(), Vec::<String>::new());
    }

    #[test]
    fn s100_fix9_ac4_a_write_blocked_on_a_pipe_nobody_reads_gives_up_at_the_limit() {
        let mut rig = rig();
        rig.release.as_ref().unwrap().send(vec![]).unwrap();
        // The read end is held and never read: a 1 MB line cannot complete.
        let _reader = rig.reader.take();
        rig.out(vec![b'x'; 1_000_000]);
        std::thread::sleep(Duration::from_millis(100));
        let took = rig.end(Duration::from_millis(300));
        assert!(
            took >= Duration::from_millis(150) && took < Duration::from_secs(2),
            "{took:?}"
        );
        assert_eq!(rig.items(), Vec::<String>::new());
    }
}

#[cfg(test)]
mod tests {
    use super::{Job, Launch, Session, Workspace};
    use crate::{Host, Limits, Mode};
    use serde_json::json;
    use std::sync::Arc;

    /// A start stopped while it made its scratch folder deletes that folder and its row, and
    /// no other row of the owner (19.3, Folders, Lifetime).
    #[test]
    fn a_start_stopped_while_it_made_its_scratch_deletes_that_folder_and_row_only() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        host.state.scratch_add("kept", "owner").unwrap();
        let session = Arc::new(Session::new("s".into(), "owner".into(), None));
        session.stop();
        let job = Job {
            weak: Arc::downgrade(&host),
            hub: Arc::clone(&host.hub),
            session,
            sessions: Arc::clone(&host.sessions),
            launch: Arc::new(Launch {
                workspace: Workspace::NewScratch,
                extra: vec![],
                flags: vec![],
                tools: vec![],
                agents: vec![],
                init_mode: "default",
                depth: 0,
                ask: false,
                check: None,
            }),
            home: host.home().to_path_buf(),
            mode: Mode::Preview,
            fake: None,
        };
        assert_eq!(job.new_scratch().unwrap_err(), json!("gone"));
        let left: Vec<_> = std::fs::read_dir(host.home().join("scratch"))
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert!(left.is_empty(), "scratch folders left: {left:?}");
        assert_eq!(host.state.scratch_removed("owner").unwrap(), ["kept"]);
    }
}
