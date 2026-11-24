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

/// The boot id of this machine, one token (19.3, The run: Process).
fn boot_id() -> Option<String> {
    #[cfg(target_os = "linux")]
    let text = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    #[cfg(not(target_os = "linux"))]
    let text = tool_output("/usr/sbin/sysctl", &["-n", "kern.boottime"])?;
    let token: String = text
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    (!token.is_empty()).then_some(token)
}

/// The start time of process `pid`, one token: field 22 of `/proc/<pid>/stat` on Linux, the
/// `lstart` that `ps` prints on macOS (no `unsafe`: `kp_proc.p_starttime` needs a raw `sysctl`).
fn start_time(pid: i32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        stat_fields(pid).map(|(_, start)| start)
    }
    #[cfg(not(target_os = "linux"))]
    {
        lstart(pid)
    }
}

/// The start time of `pid` as `start_time` reads it, only while `pid` leads its own group
/// (field 5 of the same read equals the pid): what startup checks while it holds the leader.
/// macOS holds no leader (`hold_leader`), so it does not check the group.
fn leader_start_time(pid: i32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        stat_fields(pid).and_then(|(pgrp, start)| (pgrp == pid).then_some(start))
    }
    #[cfg(not(target_os = "linux"))]
    {
        lstart(pid)
    }
}

/// Field 5 (pgrp) and field 22 (start time) of `/proc/<pid>/stat`, from one read.
#[cfg(target_os = "linux")]
fn stat_fields(pid: i32) -> Option<(i32, String)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The comm field may hold spaces and `)`: count from the last `)`; field 3 comes next.
    let rest = stat.get(stat.rfind(')')? + 1..)?;
    let mut fields = rest.split_whitespace();
    let pgrp = fields.nth(2)?.parse::<i32>().ok()?;
    Some((pgrp, fields.nth(16)?.to_owned()))
}

#[cfg(not(target_os = "linux"))]
fn lstart(pid: i32) -> Option<String> {
    let text = tool_output("/bin/ps", &["-o", "lstart=", "-p", &pid.to_string()])?;
    let token: String = text
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    (!token.is_empty()).then_some(token)
}

/// The standard output of a system tool (`/usr/sbin/sysctl`, `/bin/ps`), in the C locale and an
/// empty environment otherwise, so a `TZ` of the host does not change what `ps` prints.
#[cfg(not(target_os = "linux"))]
fn tool_output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The line of `runs/pids` for the group `pgid`, led by the process of the same id:
/// `pgid boot-id start-time`. `None` when the leader is gone.
pub(crate) fn pid_record(pgid: i32) -> Option<String> {
    Some(format!("{pgid} {} {}", boot_id()?, start_time(pgid)?))
}

/// One parsed line of `runs/pids`: `pgid boot-id start-time`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Record {
    pgid: i32,
    boot: String,
    start: String,
}

/// Whether `boot` is a boot id as `boot_id` prints it: the UUID form of
/// `/proc/sys/kernel/random/boot_id` on Linux, one non-empty token elsewhere.
fn valid_boot(boot: &str) -> bool {
    if !cfg!(target_os = "linux") {
        return !boot.is_empty();
    }
    let groups: Vec<&str> = boot.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')))
}

/// The one parser of a `runs/pids` line, run before any handle opens: UTF-8, three fields, a
/// group id of 2 or more, a valid boot id, a start time (Linux: a decimal tick count). Anything
/// else is `None` (3.1 step 3, review r7 F7.2 and r8 F8.1, F8.2).
fn parse_record(line: &[u8]) -> Option<Record> {
    let text = std::str::from_utf8(line).ok()?;
    let mut fields = text.split_whitespace();
    let pgid = fields.next()?.parse::<i32>().ok().filter(|p| *p >= 2)?;
    let (boot, start) = (fields.next()?, fields.next()?);
    if fields.next().is_some()
        || !valid_boot(boot)
        || (cfg!(target_os = "linux") && start.parse::<u64>().is_err())
    {
        return None;
    }
    Some(Record {
        pgid,
        boot: boot.to_owned(),
        start: start.to_owned(),
    })
}

/// Whether `record` is still that run: the boot id is this boot's and the leader has the recorded
/// start time.
fn still_run(record: &Record, boot: &str, start_of: impl Fn(i32) -> Option<String>) -> bool {
    record.boot == boot && start_of(record.pgid).is_some_and(|t| t == record.start)
}

/// The most that startup reads of `runs/pids`, and the most bad lines it logs per open.
const PIDS_CAP: u64 = 64 * 1024;
const BAD_LOGGED: usize = 16;

/// The log of one cleanup: the first `BAD_LOGGED` lines go to stderr, the rest are only counted
/// and `end` logs the count, so no planted `runs/` makes a startup log without end.
#[derive(Default)]
struct CleanupLog {
    said: usize,
    /// The lines that went to stderr, for the tests.
    lines: Vec<String>,
}

impl CleanupLog {
    fn say(&mut self, line: String) {
        self.said += 1;
        if self.said <= BAD_LOGGED {
            eprintln!("{line}");
            self.lines.push(line);
        }
    }

    /// Logs one count line when lines were held back.
    fn end(&mut self) {
        if self.said > BAD_LOGGED {
            let line = format!(
                "watane: {} more lines of this cleanup are not logged",
                self.said - BAD_LOGGED
            );
            eprintln!("{line}");
            self.lines.push(line);
        }
    }
}

/// A bad line for the log: its first 64 bytes, escaped after the cut.
fn bad_line(line: &[u8]) -> String {
    format!("{:?}", String::from_utf8_lossy(&line[..line.len().min(64)]))
}

/// Opens the folder of `path` without following a link, then `pids` in it through `openat` on that
/// descriptor, without following a link and without blocking, and reads at most `PIDS_CAP` bytes
/// of it when it is a regular file; a link or a non-folder for `runs`, or anything but a regular
/// file for `pids`, is logged once (a missing file is not). The flag is true when the file was
/// longer.
fn read_pids(path: &Path, log: &mut CleanupLog) -> Option<(Vec<u8>, bool)> {
    use rustix::fs::{Mode, OFlags, openat};
    let (dir, name) = (path.parent()?, path.file_name()?);
    let opened = openat(
        rustix::fs::CWD,
        dir,
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .and_then(|dir| {
        openat(
            dir,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
    });
    let file = match opened {
        Ok(fd) => std::fs::File::from(fd),
        Err(rustix::io::Errno::NOENT) => return None,
        Err(_) => {
            log.say("watane: runs/pids is not a regular file: it is not read".to_owned());
            return None;
        }
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        log.say("watane: runs/pids is not a regular file: it is not read".to_owned());
        return None;
    }
    let mut bytes = vec![];
    if let Err(e) = file.take(PIDS_CAP + 1).read_to_end(&mut bytes) {
        log.say(format!("watane: runs/pids cannot be read: {e}"));
        return None;
    }
    let over = bytes.len() as u64 > PIDS_CAP;
    if over {
        bytes.truncate(PIDS_CAP as usize);
        // The cut line is no record.
        bytes.truncate(bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1));
    }
    Some((bytes, over))
}

/// Handles every line of the contents of `runs/pids`, a repeated record once (by its parsed value); the bad ones are
/// logged through `log`.
fn clean_lines<H>(
    bytes: &[u8],
    boot: &str,
    hold: impl Fn(i32) -> Option<H>,
    start_of: impl Fn(i32) -> Option<String>,
    signal: impl Fn(&H),
    log: &mut CleanupLog,
) {
    let mut seen = std::collections::HashSet::<Record>::new();
    // An empty piece (after the last newline, or a blank line) is no record and not logged.
    for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        match parse_record(line) {
            None => log.say(format!(
                "watane: a bad record in runs/pids is dropped: {}",
                bad_line(line)
            )),
            Some(record) => {
                if seen.insert(record.clone()) {
                    clean_line(&record, boot, &hold, &start_of, &signal, log);
                }
            }
        }
    }
}

/// What startup does with one parsed record of `runs/pids` (`parse_record`, run by the caller
/// before any handle opens): the leader is held (`hold`), its identity is checked while it is
/// held, and the signal goes to that held process (`signal`), never to the group by its number.
/// Without a hold nothing is signalled, and a recorded group that still matches is logged as a
/// leftover; so are the other members of a group whose leader was signalled (3.1 step 3, review
/// r6, F6.3).
fn clean_line<H>(
    record: &Record,
    boot: &str,
    hold: impl FnOnce(i32) -> Option<H>,
    start_of: impl Fn(i32) -> Option<String>,
    signal: impl FnOnce(&H),
    log: &mut CleanupLog,
) {
    let pgid = record.pgid;
    let held = hold(pgid);
    match (still_run(record, boot, start_of), held) {
        (true, Some(held)) => {
            signal(&held);
            log.say(format!(
                "watane: the other processes of a claude group (pgid {pgid}) of an earlier run may be left running"
            ));
        }
        (true, None) => log.say(format!(
            "watane: a claude group (pgid {pgid}) of an earlier run may be left running"
        )),
        (false, _) => {}
    }
}

/// Holds the leader of a group: a pidfd, open while the identity is checked and the signal is
/// sent through it. A pidfd names one process, so its signal can reach no other; it does not pin
/// the pid number once the process is reaped, so the group is not signalled by its number.
#[cfg(target_os = "linux")]
fn hold_leader(pgid: i32) -> Option<std::os::fd::OwnedFd> {
    let pid = rustix::process::Pid::from_raw(pgid)?;
    rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).ok()
}
/// Sends SIGKILL to the process that `pidfd` names (`pidfd_send_signal`).
#[cfg(target_os = "linux")]
fn kill_held(pidfd: &std::os::fd::OwnedFd) {
    let _ = rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL);
}
/// macOS has no pidfd and `lstart` resolves to a second: no leader is held, nothing signalled.
#[cfg(not(target_os = "linux"))]
fn hold_leader(_pgid: i32) -> Option<()> {
    None
}
#[cfg(not(target_os = "linux"))]
fn kill_held(_held: &()) {}

/// Kills the leaders of the groups that a crashed host left in `runs/pids` and are still its runs,
/// and empties `runs/` (3.1 step 3).
pub(crate) fn clean_runs(home: &Path) {
    clean_runs_logged(home, &mut CleanupLog::default());
}

/// `clean_runs` with the log of the cleanup passed down.
fn clean_runs_logged(home: &Path, log: &mut CleanupLog) {
    let runs = home.join("runs");
    if let (Some((bytes, over)), Some(boot)) = (read_pids(&runs.join("pids"), log), boot_id()) {
        clean_lines(
            &bytes,
            &boot,
            hold_leader,
            leader_start_time,
            kill_held,
            log,
        );
        if over {
            log.say(format!(
                "watane: runs/pids is over {PIDS_CAP} bytes: the rest is not read"
            ));
        }
    }
    // `runs`, whatever it is, is moved aside, so `runs/` is free at once; a link is moved itself
    // and its target is kept.
    if runs.symlink_metadata().is_ok() {
        let aside = home.join(format!("runs.{}.gone", random_hex()));
        if let Err(e) = std::fs::rename(&runs, &aside) {
            log.say(format!("watane: runs/ cannot be moved aside: {e}"));
        }
    }
    remove_gone(home, log);
    log.end();
}

/// Removes every `runs.*.gone` of `home`: a directory with `remove_dir_all`, which does not follow
/// a link inside the tree, anything else with `remove_file`. A failure is logged once and tried
/// again at the next open (3.1 step 3).
fn remove_gone(home: &Path, log: &mut CleanupLog) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(entries) = std::fs::read_dir(home) else {
        return;
    };
    let mut failed = false;
    for entry in entries.flatten() {
        let bytes = entry.file_name();
        let bytes = bytes.as_bytes();
        if bytes.starts_with(b"runs.") && bytes.ends_with(b".gone") && bytes.len() > 9 {
            let path = entry.path();
            let removed = if path.symlink_metadata().is_ok_and(|m| m.is_dir()) {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            failed |= removed.is_err();
        }
    }
    if failed {
        log.say(
            "watane: a runs.*.gone folder is not removed: the next open tries again".to_owned(),
        );
    }
}

/// Kills the group of `child`, then reaps the child: the pid is held until the kill is sent, so
/// no kill reaches a reused pid (19.3 step 5). Waits at most `EXIT_WAIT` for the reap.
fn kill_then_reap(mut child: std::process::Child, pgid: i32, kill: impl FnOnce(i32)) {
    kill(pgid);
    let (reaped, wait) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = child.wait();
        let _ = reaped.send(());
    });
    let _ = wait.recv_timeout(EXIT_WAIT);
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
        let _ = tx.send(text);
    });
    let result = rx.recv_timeout(Duration::from_secs(15));
    // The child is still unreaped here, so its pid cannot belong to anyone else.
    kill_then_reap(child, pgid, killpg);
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
    /// The line of `runs/pids` that this run wrote.
    record: Option<String>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.pgid.is_some() {
            // Through the session: a group whose leader was reaped is cleared there.
            self.session.kill_group();
        }
        if let Some(record) = &self.record {
            self.sessions.forget_pid(&self.home, record);
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
            record: None,
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
        cleanup.record = self.sessions.record_pid(&self.home, pgid);
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
        kill_then_reap(child, pgid, killpg);
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

/// Item 5 of S100: a `runs/pids` record names a run by its boot id and start time, and the
/// version probe signals its group before its child is reaped.
#[cfg(test)]
mod pids_tests {
    use super::*;

    fn start(map: &'static [(i32, &'static str)]) -> impl Fn(i32) -> Option<String> {
        move |p| {
            map.iter()
                .find(|(q, _)| *q == p)
                .map(|(_, t)| (*t).to_owned())
        }
    }

    /// A boot id in the form that `boot_id` prints on Linux, a token elsewhere.
    const BOOT: &str = "0a1b2c3d-4e5f-6789-abcd-ef0123456789";
    const OTHER: &str = "ffffffff-4e5f-6789-abcd-ef0123456789";

    /// The pgid of `line` when it is a record of the run `boot` that `live` still shows.
    fn recorded_group(line: &str, boot: &str, live: impl Fn(i32) -> Option<String>) -> Option<i32> {
        let record = parse_record(line.as_bytes())?;
        still_run(&record, boot, live).then_some(record.pgid)
    }

    #[test]
    fn ac2_a_matching_line_is_killed() {
        let line = format!("4242 {BOOT} 99");
        assert_eq!(
            recorded_group(&line, BOOT, start(&[(4242, "99")])),
            Some(4242)
        );
    }

    #[test]
    fn ac2_other_lines_are_dropped() {
        let live = start(&[(4242, "99")]);
        for line in [
            "4242".to_owned(),
            format!("4242 {BOOT}"),
            format!("4242 {BOOT} 99 extra"),
            format!("4242 {OTHER} 99"),
            format!("4242 {BOOT} 100"),
            format!("4243 {BOOT} 99"),
            format!("abc {BOOT} 99"),
            format!("1 {BOOT} 99"),
            format!("-5 {BOOT} 99"),
            String::new(),
        ] {
            assert_eq!(recorded_group(&line, BOOT, &live), None, "{line:?}");
        }
    }

    /// 3.1 step 3: one parser; a boot id has the UUID form on Linux, bytes that are not UTF-8
    /// are no record.
    #[test]
    fn s100_fix20_ac3_parse_record_takes_bytes_and_checks_every_field() {
        assert_eq!(
            parse_record(format!("7 {BOOT} 100").as_bytes()),
            Some(Record {
                pgid: 7,
                boot: BOOT.to_owned(),
                start: "100".to_owned()
            })
        );
        assert_eq!(parse_record(b"\xff 7 b 1"), None);
        assert_eq!(parse_record(b"7 \xff 1"), None);
        if cfg!(target_os = "linux") {
            for boot in [
                "bogus",
                "0A1B2C3D-4e5f-6789-abcd-ef0123456789",
                "0a1b2c3d4e5f6789abcdef0123456789",
            ] {
                assert_eq!(
                    parse_record(format!("7 {boot} 1").as_bytes()),
                    None,
                    "{boot}"
                );
            }
        }
    }

    /// A guard that notes in `events` when it is closed.
    struct Held<'a>(&'a std::cell::RefCell<Vec<&'static str>>);
    impl Drop for Held<'_> {
        fn drop(&mut self) {
            self.0.borrow_mut().push("close");
        }
    }

    /// 3.1 step 3 (review r6, F6.3): the leader is held before its identity is checked, and
    /// stays held through the signal.
    #[test]
    fn s100_fix18_ac3_the_signal_goes_out_only_while_the_leader_is_held_and_checked() {
        let events = std::cell::RefCell::new(vec![]);
        clean_line(
            &parse_record(format!("4242 {BOOT} 99").as_bytes()).unwrap(),
            BOOT,
            |pgid| {
                assert_eq!(pgid, 4242);
                events.borrow_mut().push("open");
                Some(Held(&events))
            },
            |_| {
                events.borrow_mut().push("check");
                Some("99".to_owned())
            },
            |_held: &Held| events.borrow_mut().push("signal"),
            &mut CleanupLog::default(),
        );
        assert_eq!(*events.borrow(), ["open", "check", "signal", "close"]);
    }

    /// 3.1 step 3 (review r6, F6.3): with no hold on the leader (macOS, or a leader that is gone)
    /// nothing is signalled, whatever the record says.
    #[test]
    fn s100_fix18_ac3_a_leader_that_cannot_be_held_is_never_signalled() {
        let signalled = std::cell::Cell::new(false);
        clean_line(
            &parse_record(format!("4242 {BOOT} 99").as_bytes()).unwrap(),
            BOOT,
            |_| None::<Held>,
            |_| Some("99".to_owned()),
            |_: &Held| signalled.set(true),
            &mut CleanupLog::default(),
        );
        assert!(!signalled.get());
    }

    /// 3.1 step 3 (review r7, F7.2): a record is validated before any handle is opened; a bad
    /// one is dropped and the next line is still handled.
    #[test]
    fn s100_fix19_ac2_a_bad_record_is_dropped_before_any_handle_and_the_next_is_handled() {
        let held = std::cell::RefCell::new(vec![]);
        for bad in [
            "-5 bogus 100".to_owned(),
            format!("0 {BOOT} 1"),
            format!("1 {BOOT} 1"),
            format!("7 {BOOT}"),
            format!("x {BOOT} 1"),
            format!("7 {BOOT} 1 extra"),
            #[cfg(target_os = "linux")]
            format!("7 {BOOT} x"),
            #[cfg(target_os = "linux")]
            format!("7 {BOOT} -1"),
        ] {
            assert!(parse_record(bad.as_bytes()).is_none(), "{bad}");
            clean_lines(
                bad.as_bytes(),
                BOOT,
                |pgid| {
                    held.borrow_mut().push(pgid);
                    None::<Held>
                },
                |_| None,
                |_: &Held| {},
                &mut CleanupLog::default(),
            );
        }
        assert!(held.borrow().is_empty(), "{:?}", held.borrow());
        let home = tempfile::tempdir().unwrap();
        let mut run = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = i32::try_from(run.id()).unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let record = pid_record(pgid).expect("the leader is alive");
        std::fs::write(
            home.path().join("runs/pids"),
            format!("-5 bogus 100\n{record}\n"),
        )
        .unwrap();
        clean_runs(home.path());
        if cfg!(target_os = "linux") {
            assert_eq!(run.wait().unwrap().signal(), Some(9));
        } else {
            run.kill().unwrap();
            run.wait().unwrap();
        }
        assert!(!home.path().join("runs").exists());
    }

    /// 3.1 step 3 (review r8, F8.1): a boot id that is not a boot id opens no handle.
    #[test]
    fn s100_fix20_ac1_a_bogus_boot_id_opens_no_handle() {
        let opened = std::cell::Cell::new(0);
        clean_lines(
            b"7 bogus 100",
            "bogus",
            |_| {
                opened.set(opened.get() + 1);
                None::<Held>
            },
            |_| Some("100".to_owned()),
            |_: &Held| {},
            &mut CleanupLog::default(),
        );
        assert_eq!(opened.get(), if cfg!(target_os = "linux") { 0 } else { 1 });
    }

    /// 3.1 step 3 (review r8, F8.2): an undecodable line is skipped, the valid record after it is
    /// still handled, and `runs/` is removed.
    #[cfg(target_os = "linux")]
    #[test]
    fn s100_fix20_ac2_an_invalid_utf8_line_is_skipped_and_the_next_record_is_handled() {
        let home = tempfile::tempdir().unwrap();
        let mut run = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = i32::try_from(run.id()).unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let mut file = b"\xff\xfe 7 bad\n".to_vec();
        file.extend(pid_record(pgid).expect("the leader is alive").bytes());
        file.push(b'\n');
        std::fs::write(home.path().join("runs/pids"), file).unwrap();
        clean_runs(home.path());
        let killed = (0..50).find_map(|_| {
            std::thread::sleep(Duration::from_millis(20));
            run.try_wait().unwrap()
        });
        if killed.is_none() {
            run.kill().unwrap();
            run.wait().unwrap();
        }
        assert_eq!(killed.and_then(|s| s.signal()), Some(9));
        assert!(!home.path().join("runs").exists());
    }

    /// Linux: the leader is held with a pidfd and killed through it; another member of its group
    /// is left (no signal goes to the group's number). On macOS nothing is signalled (3.1 step 3).
    #[test]
    fn ac2_a_recorded_leader_is_killed_at_open_on_linux_and_left_alone_elsewhere() {
        let home = tempfile::tempdir().unwrap();
        let mut run = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = i32::try_from(run.id()).unwrap();
        let mut member = Command::new("sleep")
            .arg("30")
            .process_group(pgid)
            .spawn()
            .unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let record = pid_record(pgid).expect("the leader is alive");
        assert_eq!(record.split(' ').count(), 3);
        std::fs::write(home.path().join("runs/pids"), format!("{record}\n")).unwrap();
        clean_runs(home.path());
        if cfg!(target_os = "linux") {
            assert_eq!(run.wait().unwrap().signal(), Some(9));
        } else {
            std::thread::sleep(Duration::from_millis(200));
            assert!(run.try_wait().unwrap().is_none(), "no process is signalled");
            run.kill().unwrap();
            run.wait().unwrap();
        }
        assert!(
            member.try_wait().unwrap().is_none(),
            "the group is not signalled"
        );
        member.kill().unwrap();
        member.wait().unwrap();
        assert!(!home.path().join("runs").exists());
    }

    /// macOS: `sysctl` and `ps` give a boot id and a start time (CI's macos job is the check).
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn ac2_the_boot_id_and_a_start_time_are_read() {
        let me = i32::try_from(std::process::id()).unwrap();
        let (boot, start) = (boot_id(), start_time(me));
        assert!(
            boot.is_some() && start.is_some(),
            "boot {boot:?}, start {start:?}"
        );
        assert_eq!(start_time(me), start, "the start time is stable");
    }

    #[test]
    fn ac3_the_group_is_killed_before_the_child_is_reaped() {
        use rustix::process::{WaitId, WaitIdOptions, waitid};
        let child = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = i32::try_from(child.id()).unwrap();
        let mut order = vec![];
        kill_then_reap(child, pid, |pgid| {
            let own = rustix::process::Pid::from_raw(pid).unwrap();
            let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG;
            // Reaped would be ECHILD: the pid is still held when the kill goes out.
            let held = waitid(WaitId::Pid(own), options).is_ok();
            order.push(if held {
                "kill, unreaped"
            } else {
                "kill, reaped"
            });
            killpg(pgid);
        });
        assert_eq!(order, ["kill, unreaped"]);
        let own = rustix::process::Pid::from_raw(pid).unwrap();
        assert!(
            rustix::process::test_kill_process(own).is_err(),
            "reaped after"
        );
    }
}

/// Adversary step of S100, ninth pass (red): `clean_runs` signals every pgid in `runs/pids` at
/// open with no check that the group is still a run of this home. After a crash the recorded
/// `claude` is reaped by init and its pid can be reused (always so after a reboot), so the open
/// kills an unrelated process group of the participant: a signal to a reaped pid (19.3 step 5,
/// S100-CAP "no kill reaches a pid reused since").
#[cfg(test)]
mod adv_s100_clean_runs {
    use super::*;

    #[test]
    fn adv_s100_open_kills_a_group_that_reused_a_recorded_pid() {
        let home = tempfile::tempdir().unwrap();
        // A group the host never started, whose id a crashed run had recorded.
        let mut unrelated = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        std::fs::write(
            home.path().join("runs/pids"),
            format!("{}\n", unrelated.id()),
        )
        .unwrap();
        clean_runs(home.path());
        std::thread::sleep(Duration::from_millis(200));
        let alive = unrelated.try_wait().unwrap().is_none();
        let _ = unrelated.kill();
        let _ = unrelated.wait();
        assert!(
            alive,
            "the open killed a process group that is not a run of this home"
        );
    }
}

/// Adversary step of S100, pass 13 (red): the startup cleanup of `runs/pids` (3.1 step 3, review
/// r8) against a co-tenant who plants the file.
#[cfg(all(test, target_os = "linux"))]
mod adv_s100_p13 {
    use super::*;

    /// A FIFO at `runs/pids`: `std::fs::read` opens it and blocks until a writer comes, so
    /// `Host::open` never returns.
    #[test]
    fn adv_s100_p13_a_fifo_at_runs_pids_hangs_startup() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let fifo = home.path().join("runs/pids");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let path = home.path().to_owned();
        let (done, finished) = std::sync::mpsc::channel();
        let cleaner = std::thread::spawn(move || {
            clean_runs(&path);
            let _ = done.send(());
        });
        let hung = finished.recv_timeout(Duration::from_secs(2)).is_err();
        if hung {
            // Release the reader so the thread ends: a writer that writes nothing.
            drop(std::fs::OpenOptions::new().write(true).open(&fifo));
        }
        cleaner.join().unwrap();
        assert!(!hung, "a FIFO at runs/pids blocked the open");
    }

    /// A pid that leads no group (here a child in the test's own group) with its true start time
    /// and this boot's id: the host kills it, though the record names no group's leader.
    #[test]
    fn adv_s100_p13_a_pid_that_leads_no_group_is_killed() {
        let home = tempfile::tempdir().unwrap();
        let mut member = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = i32::try_from(member.id()).unwrap();
        assert_ne!(
            rustix::process::getpgid(rustix::process::Pid::from_raw(pid)).unwrap(),
            rustix::process::Pid::from_raw(pid).unwrap(),
            "the sleep must lead no group"
        );
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let record = pid_record(pid).expect("alive");
        std::fs::write(home.path().join("runs/pids"), format!("{record}\n")).unwrap();
        clean_runs(home.path());
        std::thread::sleep(Duration::from_millis(200));
        let alive = member.try_wait().unwrap().is_none();
        let _ = member.kill();
        let _ = member.wait();
        assert!(alive, "the open killed a process that leads no group");
    }

    /// Each bad line is logged with all its bytes, so the log grows with the planted file: here
    /// 4 MiB of bad lines; run in a child so its standard error can be measured.
    #[test]
    fn adv_s100_p13_bad_lines_flood_the_log_without_bound() {
        if let Some(home) = std::env::var_os("ADV_S100_P13_HOME") {
            clean_runs(Path::new(&home));
            return;
        }
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let line = format!("{}\n", "\u{1}".repeat(1023));
        std::fs::write(home.path().join("runs/pids"), line.repeat(4096)).unwrap();
        let out = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "agent::claude::run::adv_s100_p13::adv_s100_p13_bad_lines_flood_the_log_without_bound",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("ADV_S100_P13_HOME", home.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(
            out.stderr.len() < 64 * 1024,
            "{} bytes of log from a 4 MiB runs/pids",
            out.stderr.len()
        );
    }
}

/// S100 fix21: `runs/pids` is opened as a bounded regular file, and its bad lines are logged
/// within a bound (3.1 step 3).
#[cfg(all(test, target_os = "linux"))]
mod fix21_tests {
    use super::*;

    fn leader() -> (std::process::Child, String) {
        let run = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .expect("spawn");
        let record = pid_record(i32::try_from(run.id()).expect("pid")).expect("alive");
        (run, record)
    }

    fn killed(run: &mut std::process::Child) -> bool {
        let done = (0..50).find_map(|_| {
            std::thread::sleep(Duration::from_millis(20));
            run.try_wait().expect("wait")
        });
        if done.is_none() {
            run.kill().expect("kill");
            run.wait().expect("reap");
        }
        done.is_some()
    }

    #[test]
    fn s100_fix21_ac2_a_symlink_at_runs_pids_is_not_followed() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let target = home.path().join("elsewhere");
        std::fs::write(&target, "1 a 2\n").unwrap();
        std::os::unix::fs::symlink(&target, home.path().join("runs/pids")).unwrap();
        assert_eq!(
            read_pids(&home.path().join("runs/pids"), &mut CleanupLog::default()),
            None
        );
        clean_runs(home.path());
        assert!(!home.path().join("runs").exists());
        assert!(target.exists());
    }

    /// A FIFO is opened without blocking but not read; a file of exactly 64 KiB is not over.
    #[test]
    fn s100_fix21_ac2_only_a_regular_file_is_read_and_64_kib_is_not_over() {
        let home = tempfile::tempdir().unwrap();
        let fifo = home.path().join("fifo");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        assert_eq!(read_pids(&fifo, &mut CleanupLog::default()), None);
        let full = home.path().join("full");
        std::fs::write(&full, vec![b'\n'; 64 * 1024]).unwrap();
        assert_eq!(
            read_pids(&full, &mut CleanupLog::default()),
            Some((vec![b'\n'; 64 * 1024], false))
        );
    }

    #[test]
    fn s100_fix21_ac2_a_file_over_64_kib_is_read_to_the_cap_and_logged_once() {
        let home = tempfile::tempdir().unwrap();
        let (mut run, record) = leader();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        let mut file = format!("{record}\n").into_bytes();
        // A line across the cap: its first part is no record.
        file.resize(65_530, b'\n');
        file.resize(70_000, b'y');
        std::fs::write(home.path().join("runs/pids"), file).unwrap();
        let (bytes, over) =
            read_pids(&home.path().join("runs/pids"), &mut CleanupLog::default()).unwrap();
        assert!(over);
        assert_eq!(bytes.len(), 65_530, "cut after the last whole line");
        clean_runs(home.path());
        assert!(killed(&mut run), "a record in the first 64 KiB is handled");
        assert!(!home.path().join("runs").exists());
    }

    #[test]
    fn s100_fix21_ac3_a_pid_whose_pgrp_differs_has_no_start_time() {
        let mut member = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = i32::try_from(member.id()).unwrap();
        let none = leader_start_time(pid).is_none();
        let _ = member.kill();
        let _ = member.wait();
        assert!(none);
    }

    /// The log of one open, in a child so its standard error can be read.
    fn log_of(home: &Path) -> Vec<String> {
        let out = Command::new(std::env::current_exe().expect("exe"))
            .args([
                "--exact",
                "agent::claude::run::fix21_tests::s100_fix21_ac4_twenty_bad_lines_log_sixteen_and_a_count",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("FIX21_HOME", home)
            .output()
            .expect("child");
        let log = String::from_utf8(out.stderr).expect("utf-8");
        log.lines()
            .filter(|l| l.starts_with("watane:"))
            .map(str::to_owned)
            .collect()
    }

    /// Twenty bad lines of 200 bytes, then padding past 64 KiB: 16 are logged, cut at 64 bytes,
    /// then one line with the rest, then one line for the cap. A link at `runs/pids`: one line.
    #[test]
    fn s100_fix21_ac4_twenty_bad_lines_log_sixteen_and_a_count() {
        if let Some(home) = std::env::var_os("FIX21_HOME") {
            clean_runs(Path::new(&home));
            return;
        }
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        // Distinct lines: a repeated line is handled once (3.1 step 3, pass 14).
        let mut file: Vec<u8> = (0..20)
            .flat_map(|i| format!("{}{i:02}\n", "x".repeat(198)).into_bytes())
            .collect();
        file.resize(70_000, b'\n');
        std::fs::write(home.path().join("runs/pids"), file).unwrap();
        let ours = log_of(home.path());
        // One cap covers the bad lines and the over line (3.1 step 3, pass 14).
        assert_eq!(ours.len(), 17, "{ours:?}");
        assert!(
            ours[0].ends_with(&format!("\"{}\"", "x".repeat(64))),
            "{}",
            ours[0]
        );
        assert!(ours[16].contains("5 more"), "{}", ours[16]);
        std::fs::create_dir_all(home.path().join("runs")).unwrap();
        std::os::unix::fs::symlink("/dev/null", home.path().join("runs/pids")).unwrap();
        let ours = log_of(home.path());
        assert_eq!(ours.len(), 1, "{ours:?}");
        assert!(ours[0].contains("not a regular file"), "{}", ours[0]);
        assert!(
            log_of(home.path()).is_empty(),
            "a missing runs/pids is no event"
        );
    }
}

/// S100 adversary pass 14: what a co-tenant plants at `runs/` itself (3.1 step 3: `runs/` is
/// deleted at open).
#[cfg(all(test, target_os = "linux"))]
mod adv_s100_p14 {
    use super::*;

    /// A regular file or a FIFO planted at `runs/` is left by `clean_runs`; every later run then
    /// fails to make `runs/<random>` (ENOTDIR).
    #[test]
    fn adv_s100_p14_runs_as_a_file_or_fifo_is_left() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("runs"), "x").unwrap();
        clean_runs(home.path());
        let file_left = home.path().join("runs").symlink_metadata().is_ok();
        let _ = std::fs::remove_file(home.path().join("runs"));
        rustix::fs::mknodat(
            rustix::fs::CWD,
            home.path().join("runs"),
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        clean_runs(home.path());
        let fifo_left = home.path().join("runs").symlink_metadata().is_ok();
        assert!(
            !file_left && !fifo_left,
            "runs/ left: file {file_left}, fifo {fifo_left}"
        );
    }

    /// One cap covers every cleanup log line, and a repeated record is handled once.
    #[test]
    fn s100_fix22_ac2_a_repeated_record_is_handled_once_and_the_log_is_capped() {
        let Some(boot) = boot_id() else { return };
        let mut leader = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = i32::try_from(leader.id()).unwrap();
        let record = pid_record(pgid).expect("the leader is alive");
        let kills = std::cell::Cell::new(0);
        let mut log = CleanupLog::default();
        let mut bytes = vec![];
        for _ in 0..1000 {
            bytes.extend_from_slice(record.as_bytes());
            bytes.push(b'\n');
        }
        clean_lines(
            &bytes,
            &boot,
            |_| Some(()),
            leader_start_time,
            |_: &()| kills.set(kills.get() + 1),
            &mut log,
        );
        log.end();
        let lines = log.lines;
        leader.kill().unwrap();
        leader.wait().unwrap();
        assert_eq!(kills.get(), 1);
        assert_eq!(lines.len(), 1, "{lines:?}");

        let mut distinct = vec![];
        for i in 0..1000 {
            distinct.extend_from_slice(format!("bad {i}\n").as_bytes());
        }
        let mut log = CleanupLog::default();
        clean_lines(
            &distinct,
            &boot,
            |_| None::<()>,
            |_| None,
            |_: &()| {},
            &mut log,
        );
        log.end();
        let lines = log.lines;
        assert_eq!(lines.len(), 17, "{lines:?}");
        assert!(lines[16].contains(" 984 more "), "{}", lines[16]);
    }

    /// A link at `runs` is removed itself; its target is kept.
    #[test]
    fn s100_fix22_ac1_a_link_at_runs_is_removed_and_its_target_kept() {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("keep"), "x").unwrap();
        std::os::unix::fs::symlink(target.path(), home.path().join("runs")).unwrap();
        clean_runs(home.path());
        assert!(!home.path().join("runs").exists());
        assert!(target.path().join("keep").exists());
    }
}

#[cfg(test)]
mod adv_s100_p15 {
    use super::*;

    /// A co-tenant plants a folder tree under `runs/` deeper than the open-file limit:
    /// `remove_dir_all` holds one descriptor and one stack frame per level, fails with EMFILE and
    /// leaves `runs/` (and, with a higher limit or a smaller stack, overflows the stack and aborts
    /// the open). The cleanup runs in a child process, so an abort does not end this one, with the
    /// common soft limit of 1024 files and the main thread's 8 MiB stack.
    #[test]
    fn adv_s100_p15_a_deep_tree_under_runs_is_left() {
        const CHILD: &str = "WATANE_ADV_S100_P15_DEEP";
        if let Some(home) = std::env::var_os(CHILD) {
            let home = std::path::PathBuf::from(home);
            let limit = rustix::process::getrlimit(rustix::process::Resource::Nofile);
            rustix::process::setrlimit(
                rustix::process::Resource::Nofile,
                rustix::process::Rlimit {
                    current: Some(1024),
                    maximum: limit.maximum,
                },
            )
            .unwrap();
            std::thread::Builder::new()
                .stack_size(8 << 20)
                .spawn(move || clean_runs(&home))
                .unwrap()
                .join()
                .unwrap();
            return;
        }
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join("runs")).unwrap();
        let mut fd = rustix::fs::open(
            home.path().join("runs"),
            rustix::fs::OFlags::DIRECTORY,
            rustix::fs::Mode::empty(),
        )
        .unwrap();
        for _ in 0..3_000 {
            rustix::fs::mkdirat(&fd, "d", rustix::fs::Mode::from_raw_mode(0o700)).unwrap();
            fd = rustix::fs::openat(
                &fd,
                "d",
                rustix::fs::OFlags::DIRECTORY,
                rustix::fs::Mode::empty(),
            )
            .unwrap();
        }
        drop(fd);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "agent::claude::run::adv_s100_p15::adv_s100_p15_a_deep_tree_under_runs_is_left",
                "--test-threads=1",
            ])
            .env(CHILD, home.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        let left = home.path().join("runs").exists();
        // `rm` walks a deep tree without recursion; the temporary folder's drop would not.
        let _ = std::process::Command::new("rm")
            .arg("-rf")
            .arg(home.path().join("runs"))
            .status();
        assert!(
            status.success() && !left,
            "open: {status}, runs/ left: {left}"
        );
    }

    /// Lines that differ only in white space, a sign or a leading zero are one record (same
    /// pgid, boot id, start time), yet each is handled: the line set does not make a repeated
    /// record handled once (3.1 step 3, pass 14).
    #[test]
    fn adv_s100_p15_a_record_in_white_space_variants_is_handled_each_time() {
        let Some(boot) = boot_id() else { return };
        let mut leader = Command::new("sleep")
            .arg("30")
            .process_group(0)
            .spawn()
            .unwrap();
        let pgid = i32::try_from(leader.id()).unwrap();
        let record = pid_record(pgid).expect("the leader is alive");
        let mut bytes = vec![];
        for line in [
            record.clone(),
            format!(" {record}"),
            format!("{record} "),
            format!("+{record}"),
            format!("0{record}"),
            record.replacen(' ', "\t", 1),
        ] {
            bytes.extend_from_slice(line.as_bytes());
            bytes.push(b'\n');
        }
        let kills = std::cell::Cell::new(0);
        let mut log = CleanupLog::default();
        clean_lines(
            &bytes,
            &boot,
            |_| Some(()),
            leader_start_time,
            |_: &()| kills.set(kills.get() + 1),
            &mut log,
        );
        leader.kill().unwrap();
        leader.wait().unwrap();
        assert_eq!(kills.get(), 1, "{:?}", log.lines);
    }
}

#[cfg(test)]
mod fix24_tests {
    use super::*;

    /// A `runs` that is a link is moved aside unread: nothing of its target's `pids` reaches the
    /// log, the link is gone from the home and the target is kept (review r9, F9.1).
    #[test]
    fn s100_fix24_ac1_a_linked_runs_is_not_read_and_its_target_is_kept() {
        let home = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::write(target.path().join("pids"), "marker-f91 x y\n").unwrap();
        std::os::unix::fs::symlink(target.path(), home.path().join("runs")).unwrap();
        let mut log = CleanupLog::default();
        clean_runs_logged(home.path(), &mut log);
        assert!(
            !log.lines.iter().any(|l| l.contains("marker-f91")),
            "{:?}",
            log.lines
        );
        assert!(home.path().join("runs").symlink_metadata().is_err());
        assert!(target.path().join("pids").exists());
    }

    /// One open removes the whole tree: five folders and 20 000 entries in one level leave no
    /// `runs` and no `runs.*.gone` (review r9, F9.2).
    #[test]
    fn s100_fix24_ac2_one_open_removes_the_whole_tree() {
        let home = tempfile::tempdir().unwrap();
        let runs = home.path().join("runs");
        for i in 0..5 {
            std::fs::create_dir_all(runs.join(format!("child{i}"))).unwrap();
        }
        for i in 0..20_000 {
            std::fs::write(runs.join(format!("child0/f{i}")), "").unwrap();
        }
        let mut log = CleanupLog::default();
        clean_runs_logged(home.path(), &mut log);
        let left: Vec<_> = std::fs::read_dir(home.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert!(left.is_empty(), "{left:?} {:?}", log.lines);
    }
}
