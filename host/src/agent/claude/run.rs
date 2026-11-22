//! The run of `claude` (TECHNICAL 19.3, The run, Folders): the binary and its version, the
//! scratch workspace, the run folder, the process group, the fixed environment, the host's
//! `--settings=`, and the thread that reads the stream and ends it.

use super::control::{self, Line};
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

/// Sends SIGKILL to a process group. The workspace forbids `unsafe`, so this runs `kill`.
fn killpg(pgid: i32) {
    if pgid > 1 {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pgid}")])
            .env_clear()
            .env("PATH", PATH)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
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
    sessions: Arc<Sessions>,
    home: PathBuf,
    folder: Option<PathBuf>,
    pgid: Option<i32>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(pgid) = self.pgid {
            killpg(pgid);
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
        for id in &launch.extra {
            match self.held_scratch(id) {
                Ok(path) => args.push(format!("--add-dir={}", path.display())),
                Err(outcome) => return outcome,
            }
        }
        args.extend(launch.flags.iter().cloned());
        let mut cleanup = Cleanup {
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
        cleanup.folder = Some(folder);
        if let Err(e) = made {
            return Self::failed(format!("cannot make the run folder: {e}"));
        }
        args.push(format!("--plugin-dir={}", plugin.display()));
        let mut child = match Command::new(&program)
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
        let stdin = child.stdin.take().expect("piped");
        let mut stdout = std::io::BufReader::new(child.stdout.take().expect("piped"));
        // The writer thread is the one place that writes to stdin; a line goes through its queue.
        let rx = self.session.started(pgid);
        std::thread::spawn(move || {
            use std::io::Write;
            let mut stdin = stdin;
            for mut line in rx {
                line.push(b'\n');
                if stdin.write_all(&line).and_then(|()| stdin.flush()).is_err() {
                    break;
                }
            }
        });
        // The waiter reaps the child and then kills what is left of its group, so that a
        // grandchild cannot keep the pipe open.
        let waiter = std::thread::spawn(move || {
            let status = child.wait();
            killpg(pgid);
            status
        });
        let request_id = self.session.request_id.clone();
        let mut reader = Reading {
            job: self,
            held: Some(vec![]),
            request_id,
            workspace,
        };
        let ended = reader.read(&mut stdout);
        let status = waiter.join().ok().and_then(Result::ok);
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
    /// Lines that came before the answer to `initialize`: `initialized` is first (19.3, step 2).
    held: Option<Vec<Vec<u8>>>,
    request_id: String,
    workspace: String,
}
impl Reading<'_> {
    /// Pushes one item, waiting while the stream is full (a host stream stops reading its
    /// source, 19.2). `Err` when the stream is gone or the item cannot go.
    fn push(&self, body: Vec<u8>) -> Result<(), Value> {
        let session = &self.job.session;
        loop {
            match self.job.hub.host_push(&session.id, &body) {
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
    fn item(&mut self, body: Vec<u8>) -> Result<(), Value> {
        match &mut self.held {
            Some(held) => {
                held.push(body);
                Ok(())
            }
            None => self.push(body),
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
        match control::classify(raw) {
            Line::Item(body) => self.item(body),
            Line::Deny { request_id, hook } => {
                self.job.session.write(control::deny(&request_id, hook));
                Ok(())
            }
            Line::Init(value, body) => {
                if let Err(field) = init::check(&self.job.launch, &value) {
                    return Err(json!({"mismatch": field}));
                }
                self.item(body)
            }
            Line::Response { request_id, ok } => {
                if self.held.is_some() && request_id == self.request_id {
                    if !ok {
                        return Err(Job::failed("claude refused initialize"));
                    }
                    let workspace = Value::String(self.workspace.clone());
                    self.push(
                        format!(r#"{{"type":"initialized","workspace":{workspace}}}"#).into_bytes(),
                    )?;
                    for body in self.held.take().unwrap_or_default() {
                        self.push(body)?;
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
