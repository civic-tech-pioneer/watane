#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Stage A2, first slice (TECHNICAL 19.3, 19.9; FUNCTIONAL S63, S64, S67, S69): `claude-code`
//! on host's own fake `claude` (`tests/fixtures/fake-claude/claude`). Asks and checks wait for
//! S100: nothing here tests an `ask` or a `check` item.

use crate::streams::{call_as, fixture, local, wait};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use watane_host::{Caller, Host, Limits, Mode};

pub(crate) const INIT: &str = r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"agents":[],"permissionMode":"default","plugins":[{"name":"watane-run"}]}"#;
pub(crate) const LAUNCH: &str = r#"{"version":1,"workspace":"scratch"}"#;

/// A preview home with the `claude` fixture, and a folder with the fake `claude` and its files.
pub(crate) struct Rig {
    pub home: TempDir,
    pub fake: TempDir,
    pub host: Option<Arc<Host>>,
}
impl Rig {
    /// `files`: the files beside the fake (`init`, `lines`, ...). `init` defaults to `INIT`.
    pub fn new(files: &[(&str, &[u8])]) -> Self {
        let home = tempfile::tempdir().unwrap();
        let fake = tempfile::tempdir().unwrap();
        let program = fake.path().join("claude");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude/claude"),
            &program,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut rig = Self {
            home,
            fake,
            host: None,
        };
        if !files.iter().any(|(n, _)| *n == "init") {
            rig.put("init", format!("{INIT}\n").as_bytes());
        }
        for (name, body) in files {
            rig.put(name, body);
        }
        rig.open();
        rig
    }
    pub fn put(&self, name: &str, body: &[u8]) {
        std::fs::write(self.fake.path().join(name), body).unwrap();
    }
    pub fn program(&self) -> PathBuf {
        self.fake.path().join("claude")
    }
    pub fn open(&mut self) {
        let host = Host::open(self.home.path(), Mode::Preview, Limits::default()).unwrap();
        host.set_fake_claude(&self.program()).unwrap();
        if host.mods().is_empty() {
            host.install_folders(&[fixture("claude"), fixture("claude-b")], 0)
                .unwrap();
        }
        self.host = Some(host);
    }
    pub fn host(&self) -> &Arc<Host> {
        self.host.as_ref().unwrap()
    }
    /// Closes the host and waits until its workers have let go of the state database.
    pub fn close(&mut self) {
        self.host = None;
        let path = self.home.path().join("state/state.redb");
        wait("the state database", Duration::from_secs(10), || {
            redb::Database::open(&path).is_ok()
        });
    }
    pub fn call(&self, function: &str, args: Value) -> Value {
        self.call_as(Caller::Page, "claude", function, args)
            .unwrap()
    }
    pub fn call_as(
        &self,
        caller: Caller,
        target: &str,
        function: &str,
        args: Value,
    ) -> Result<Value, watane_host::CallError> {
        call_as(self.host(), caller, target, function, args)
    }
    /// `api.start`: the stream id, or the error text.
    pub fn start(&self, launch: &str) -> String {
        self.call("api.start", json!({"launch": launch}))
            .as_str()
            .unwrap()
            .to_owned()
    }
    pub fn start_ok(&self, launch: &str) -> String {
        let s = self.start(launch);
        assert_eq!(s.len(), 32, "no stream id: {s}");
        s
    }
    pub fn items(&self, s: &str) -> Vec<String> {
        serde_json::from_value(self.call("api.items", json!({"s": s}))).unwrap()
    }
    pub fn wait_items(&self, s: &str, n: usize) -> Vec<String> {
        wait("items", Duration::from_secs(20), || {
            self.items(s).len() >= n
        });
        self.items(s)
    }
    pub fn ended(&self) -> Vec<String> {
        serde_json::from_value(self.call("api.ended", Value::Null)).unwrap()
    }
    pub fn wait_ended(&self, n: usize) -> Vec<String> {
        wait("ended", Duration::from_secs(20), || self.ended().len() >= n);
        self.ended()
    }
    /// The fake's log, one entry per line.
    pub fn log(&self) -> Vec<String> {
        std::fs::read_to_string(self.fake.path().join("claude.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    pub fn wait_log(&self, what: &str, check: impl Fn(&[String]) -> bool) -> Vec<String> {
        wait(what, Duration::from_secs(20), || check(&self.log()));
        self.log()
    }
    /// The entries of one kind: `ARG`, `ENV`, `IN`, `PID`, `CHILD`, `CWD`.
    pub fn entries(&self, kind: &str) -> Vec<String> {
        let prefix = format!("{kind} ");
        self.log()
            .iter()
            .filter_map(|l| l.strip_prefix(&prefix).map(str::to_owned))
            .collect()
    }
}

/// The scratch id from the `initialized` item.
pub(crate) fn workspace_of(item: &str) -> String {
    let v: Value = serde_json::from_str(item).unwrap();
    assert_eq!(v["type"], "initialized");
    v["workspace"].as_str().unwrap().to_owned()
}
/// The owner of a scratch id in the `scratch` table; the host must be closed.
pub(crate) fn scratch_owner(rig: &Rig, id: &str) -> Option<String> {
    let db = redb::Database::open(rig.home.path().join("state/state.redb")).unwrap();
    let read = redb::ReadableDatabase::begin_read(&db).unwrap();
    let table = read
        .open_table(redb::TableDefinition::<&str, &str>::new("scratch"))
        .unwrap();
    let owner = redb::ReadableTable::get(&table, id).unwrap();
    owner.map(|v| v.value().to_owned())
}
/// Adds a `scratch` row; the host must be closed.
pub(crate) fn add_scratch(rig: &Rig, id: &str, owner: &str) {
    let db = redb::Database::open(rig.home.path().join("state/state.redb")).unwrap();
    let txn = db.begin_write().unwrap();
    txn.open_table(redb::TableDefinition::<&str, &str>::new("scratch"))
        .unwrap()
        .insert(id, owner)
        .unwrap();
    txn.commit().unwrap();
}
/// The `generation` row of the `host` table; the host must be closed.
pub(crate) fn stored_generation(rig: &Rig) -> u64 {
    let db = redb::Database::open(rig.home.path().join("state/state.redb")).unwrap();
    let read = redb::ReadableDatabase::begin_read(&db).unwrap();
    let table = read
        .open_table(redb::TableDefinition::<&str, u64>::new("host"))
        .unwrap();
    let value = redb::ReadableTable::get(&table, "generation").unwrap();
    value.unwrap().value()
}
fn alive(pid: i32) -> bool {
    // A zombie is gone for this purpose.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| {
            s.rsplit_once(')')
                .map(|(_, rest)| rest.trim_start().starts_with('Z'))
        })
        .is_some_and(|zombie| !zombie)
}

#[test]
fn ac5_a_scratch_launch_runs_the_fake_with_the_fixed_arguments_and_environment() {
    let rig = Rig::new(&[]);
    let s =
        rig.start_ok(r#"{"version":1,"workspace":"scratch","model":"sonnet","tools":["Read"]}"#);
    let items = rig.wait_items(&s, 2);
    let workspace = workspace_of(&items[0]);
    let id = workspace.strip_prefix("scratch:").unwrap();
    assert_eq!(id.len(), 32);
    let home = rig.host().home().to_path_buf();
    let scratch = home.join("scratch").join(id);
    assert_eq!(
        std::fs::metadata(&scratch).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let args = rig.entries("ARG");
    assert_eq!(
        args[..7],
        [
            "-p",
            "--input-format=stream-json",
            "--output-format=stream-json",
            "--verbose",
            "--restricted",
            "--strict-mcp-config",
            "--system-prompt-snapshot=off"
        ]
    );
    assert!(args[7].starts_with("--settings={"));
    assert_eq!(
        args[8..11],
        [
            "--model=sonnet",
            "--tools=Read",
            "--permission-prompts=none"
        ]
    );
    let plugin = args[11].strip_prefix("--plugin-dir=").unwrap();
    let plugin = Path::new(plugin);
    assert_eq!(plugin.file_name().unwrap(), "watane-run");
    assert!(plugin.starts_with(home.join("runs")));
    assert!(plugin.join(".claude-plugin/plugin.json").is_file());
    assert_eq!(args.len(), 12);
    // The program's own working folder, the group and its pid record.
    assert_eq!(
        rig.entries("CWD"),
        [scratch.canonicalize().unwrap().display().to_string()]
    );
    let pid = rig.entries("PID")[0].clone();
    assert_eq!(
        std::fs::read_to_string(home.join("runs/pids")).unwrap(),
        format!("{pid}\n")
    );
    // The group: the program is its own leader.
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    assert_eq!(fields[2], pid, "process group differs from the pid");
    // The environment: only the variables of 19.3.
    let env = rig.entries("ENV");
    let names: Vec<&str> = env.iter().map(|e| e.split('=').next().unwrap()).collect();
    for name in &names {
        assert!(
            matches!(
                *name,
                "HOME"
                    | "USER"
                    | "LOGNAME"
                    | "LANG"
                    | "TMPDIR"
                    | "SHELL"
                    | "PATH"
                    | "PWD"
                    | "OLDPWD"
                    | "SHLVL"
                    | "_"
                    | "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH"
                    | "CLAUDE_CODE_OAUTH_TOKEN"
            ) || name.starts_with("LC_"),
            "the environment holds {name}"
        );
    }
    assert!(env.contains(
        &"PATH=/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin".to_owned()
    ));
    assert!(env.contains(&"CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=1".to_owned()));
    assert!(!names.contains(&"CARGO_MANIFEST_DIR"));
}

#[test]
fn ac6_the_settings_hold_the_protected_paths_the_protected_reads_and_no_unsandboxed_commands() {
    let rig = Rig::new(&[]);
    let s = rig.start_ok(LAUNCH);
    rig.wait_items(&s, 2);
    let args = rig.entries("ARG");
    let settings = args
        .iter()
        .find_map(|a| a.strip_prefix("--settings="))
        .unwrap();
    let v: Value = serde_json::from_str(settings).unwrap();
    assert_eq!(v["sandbox"], json!({"allowUnsandboxedCommands": false}));
    let deny: Vec<String> = v["permissions"]["deny"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_owned())
        .collect();
    for tool in ["Edit", "Write", "NotebookEdit"] {
        for p in [
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
        ] {
            assert!(deny.contains(&format!("{tool}({p})")), "{tool}({p})");
        }
    }
    let home = rig.host().home().canonicalize().unwrap();
    let user = PathBuf::from(std::env::var("HOME").unwrap())
        .canonicalize()
        .unwrap();
    let mut reads: Vec<String> = [
        "app/**",
        "dist/**",
        "log/**",
        "net/**",
        "state/**",
        "installed.json",
        "toolsets.json",
    ]
    .iter()
    .map(|p| format!("/{}", home.join(p).display()))
    .collect();
    reads.extend(
        [
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
        ]
        .iter()
        .map(|p| format!("/{}", user.join(p).display())),
    );
    for path in reads {
        assert!(path.starts_with("//"), "{path}");
        for tool in ["Read", "Glob", "Grep", "Edit", "Write", "NotebookEdit"] {
            assert!(deny.contains(&format!("{tool}({path})")), "{tool}({path})");
        }
    }
}

#[test]
fn ac7_initialized_comes_first_and_lines_are_items_or_wrapped() {
    let lines: Vec<u8> = [
        &br#"{"type":"assistant","n":1}"#[..],
        br#"{"type":"ask","id":"x"}"#,
        b"{\"type\":\"x\",\"t\":\"\xff\"}",
        b"not json",
        br#"{"a":1,"a":2}"#,
        br#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#,
    ]
    .iter()
    .flat_map(|l| l.iter().copied().chain(std::iter::once(b'\n')))
    .collect();
    let rig = Rig::new(&[("lines", &lines)]);
    let s = rig.start_ok(LAUNCH);
    let items = rig.wait_items(&s, 8);
    let ws = workspace_of(&items[0]);
    assert_eq!(
        items[0],
        format!(r#"{{"type":"initialized","workspace":"{ws}"}}"#)
    );
    assert_eq!(items[1], INIT);
    assert_eq!(items[2], r#"{"type":"assistant","n":1}"#);
    let wrapped = |i: usize| -> Value { serde_json::from_str(&items[i]).unwrap() };
    assert_eq!(
        wrapped(3),
        json!({"type":"wrapped","line":r#"{"type":"ask","id":"x"}"#})
    );
    assert_eq!(
        wrapped(4),
        json!({"type":"wrapped","line":"{\"type\":\"x\",\"t\":\"\u{fffd}\"}"})
    );
    assert_eq!(wrapped(5), json!({"type":"wrapped","line":"not json"}));
    assert_eq!(
        wrapped(6),
        json!({"type":"wrapped","line":r#"{"a":1,"a":2}"#})
    );
    assert_eq!(
        items[7],
        r#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#
    );
}

#[test]
fn ac7_an_init_that_differs_from_the_launch_kills_the_group_and_ends_mismatch() {
    let cases = [
        (
            "tools",
            r#"{"type":"system","subtype":"init","tools":["Bash"],"mcp_servers":[],"agents":[],"permissionMode":"default","plugins":[]}"#,
        ),
        (
            "mcp_servers",
            r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[{"name":"x","status":"connected"}],"agents":[],"permissionMode":"default","plugins":[]}"#,
        ),
        (
            "agents",
            r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"agents":["spy"],"permissionMode":"default","plugins":[]}"#,
        ),
        (
            "plugins",
            r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"agents":[],"permissionMode":"default","plugins":[{"name":"evil"}]}"#,
        ),
        (
            "permissionMode",
            r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[],"agents":[],"permissionMode":"acceptEdits","plugins":[]}"#,
        ),
    ];
    for (field, init) in cases {
        let rig = Rig::new(&[("init", format!("{init}\n").as_bytes()), ("spawn", b"")]);
        let s = rig.start_ok(LAUNCH);
        assert_eq!(rig.wait_ended(1), [format!("{s} mismatch({field})")]);
        let pid: i32 = rig.entries("PID")[0].parse().unwrap();
        wait("the group to die", Duration::from_secs(10), || !alive(pid));
        let child: i32 = rig.entries("CHILD")[0].parse().unwrap();
        wait("the child to die", Duration::from_secs(10), || {
            !alive(child)
        });
    }
}

#[test]
fn ac7_a_plugin_of_claude_codes_own_and_the_mode_dont_ask_match() {
    let init = r#"{"type":"system","subtype":"init","tools":["Read"],"mcp_servers":[],"agents":["general-purpose","rev","Explore"],"permissionMode":"dontAsk","plugins":[{"name":"watane-run"},{"name":"cc-plugin-x"}]}"#;
    let rig = Rig::new(&[("init", format!("{init}\n").as_bytes())]);
    let launch = r#"{"version":1,"workspace":"scratch","mode":"dont-ask","tools":["Read"],"subagents":[{"name":"rev","description":"d","prompt":"p","tools":["Read"],"model":"sonnet"}]}"#;
    let s = rig.start_ok(launch);
    let items = rig.wait_items(&s, 2);
    assert_eq!(items[1], init);
    assert!(rig.ended().is_empty());
    let args = rig.entries("ARG");
    assert!(args.contains(&"--permission-mode=dontAsk".to_owned()));
    assert!(args.contains(&r#"--agents={"rev":{"description":"d","model":"sonnet","prompt":"p","tools":["Read"]}}"#.to_owned()));
}

#[test]
fn ac8_a_can_use_tool_and_a_hook_callback_are_denied_at_once_and_push_no_item() {
    // Asks and checks wait for S100 (agentic-harness case 10, 14, 15 host parts).
    let lines = br#"{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"Read","input":{}}}
{"type":"control_request","request_id":"r2","request":{"subtype":"hook_callback","callback_id":"c"}}
{"type":"assistant","n":1}
"#;
    let rig = Rig::new(&[("lines", lines)]);
    let s = rig.start_ok(LAUNCH);
    let items = rig.wait_items(&s, 3);
    assert_eq!(items[2], r#"{"type":"assistant","n":1}"#);
    assert_eq!(items.len(), 3);
    let log = rig.wait_log("the denies", |l| {
        l.iter().filter(|e| e.contains("control_response")).count() >= 2
    });
    let ins: Vec<Value> = log
        .iter()
        .filter_map(|l| l.strip_prefix("IN "))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(ins[1]["response"]["response"]["behavior"], "deny");
    assert_eq!(ins[1]["response"]["request_id"], "r1");
    assert_eq!(ins[2]["response"]["request_id"], "r2");
    assert_eq!(
        ins[2]["response"]["response"]["hookSpecificOutput"]["permissionDecision"],
        "deny"
    );
}

#[test]
fn ac9_say_interrupt_and_compact_write_to_stdin_after_the_commit() {
    let rig = Rig::new(&[]);
    let s = rig.start_ok(LAUNCH);
    rig.wait_items(&s, 2);
    assert_eq!(
        rig.call("api.say", json!({"s": s, "text": "hello"})),
        json!("ok")
    );
    let log = rig.wait_log("the user line", |l| {
        l.iter().any(|e| e.contains(r#""type":"user""#))
    });
    let user: Vec<&String> = log
        .iter()
        .filter(|l| l.contains(r#""type":"user""#))
        .collect();
    assert_eq!(user.len(), 1);
    let v: Value = serde_json::from_str(user[0].strip_prefix("IN ").unwrap()).unwrap();
    assert_eq!(
        v,
        json!({"type":"user","message":{"role":"user","content":"hello"}})
    );
    // A call that traps after `say` writes nothing.
    let failed = rig.call_as(
        Caller::Page,
        "claude",
        "api.say-then-fail",
        json!({"s": s, "text": "lost"}),
    );
    failed.unwrap_err();
    assert_eq!(rig.call("api.interrupt", json!({"s": s})), json!("ok"));
    assert_eq!(rig.call("api.compact", json!({"s": s})), json!("ok"));
    // stdin is one queue: a written "lost" would be in the log before `/compact`.
    let log = rig.wait_log("compact", |l| l.iter().any(|e| e.contains("/compact")));
    assert!(!log.iter().any(|l| l.contains("lost")));
    let ins: Vec<Value> = log
        .iter()
        .filter_map(|l| l.strip_prefix("IN "))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(ins[0]["request"], json!({"subtype":"initialize"}));
    assert_eq!(ins[2]["request"], json!({"subtype":"interrupt"}));
    assert_eq!(ins[2]["type"], "control_request");
    assert_eq!(
        ins[3],
        json!({"type":"user","message":{"role":"user","content":"/compact"}})
    );
    assert_eq!(ins.len(), 4);
}

#[test]
fn ac9_say_refuses_slash_commands_strangers_and_unknown_streams() {
    let rig = Rig::new(&[]);
    let s = rig.start_ok(LAUNCH);
    rig.wait_items(&s, 2);
    for text in [" /x", "\u{feff}/x", "\u{200b}/x", "/x"] {
        assert_eq!(
            rig.call("api.say", json!({"s": s, "text": text})),
            json!("slash-command"),
            "{text:?}"
        );
    }
    assert_eq!(
        rig.call("api.say", json!({"s": s, "text": "a /x"})),
        json!("ok")
    );
    let b = rig
        .call_as(
            Caller::Page,
            "claude-b",
            "api.say",
            json!({"s": s, "text": "hi"}),
        )
        .unwrap();
    assert_eq!(b, json!("not-yours"));
    let zero = "0".repeat(32);
    assert_eq!(
        rig.call("api.say", json!({"s": zero, "text": "hi"})),
        json!("unknown-stream")
    );
    assert_eq!(
        rig.call("api.answer", json!({"s": s, "ask": "1"})),
        json!("unknown-ask")
    );
    rig.call("api.cancel", json!({"s": s}));
    rig.wait_ended(1);
    assert_eq!(
        rig.call("api.say", json!({"s": s, "text": "hi"})),
        json!("unknown-stream")
    );
    // Only the one `say` reached the program.
    let says = rig
        .entries("IN")
        .iter()
        .filter(|l| l.contains(r#""type":"user""#))
        .count();
    assert_eq!(says, 1);
}

#[test]
fn ac10_no_fake_gives_no_binary_and_an_old_version_gives_version() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("claude")], 0).unwrap();
    let s = call_as(
        &host,
        Caller::Page,
        "claude",
        "api.start",
        json!({"launch": LAUNCH}),
    )
    .unwrap();
    let s = s.as_str().unwrap().to_owned();
    wait("no-binary", Duration::from_secs(10), || {
        call_as(&host, Caller::Page, "claude", "api.ended", Value::Null).unwrap()
            == json!([format!("{s} no-binary")])
    });
    assert!(!home.path().join("scratch").exists());
    let rig = Rig::new(&[("version", b"2.1.294 (Claude Code)\n")]);
    let s = rig.start_ok(LAUNCH);
    assert_eq!(
        rig.wait_ended(1),
        [format!("{s} version(2.1.294 need 2.1.295)")]
    );
    assert!(rig.log().is_empty(), "the program ran");
    assert!(!rig.home.path().join("scratch").exists());
}

#[test]
fn ac10_set_fake_claude_outside_a_preview_is_bad_request() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Home { app: None }, Limits::default()).unwrap();
    let e = host.set_fake_claude(Path::new("/bin/true")).unwrap_err();
    assert_eq!(e.code(), "bad_request");
}

/// A home with an app that approves every install, and the given accounts.
struct App(Option<Arc<dyn watane_host::AccountSource>>);
impl watane_host::Confirm for App {
    fn install(
        &self,
        _: watane_host::InstallRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = watane_host::Answer> + Send>> {
        Box::pin(async { watane_host::Answer::Approve { withhold: vec![] } })
    }
    fn accounts(&self) -> Option<Arc<dyn watane_host::AccountSource>> {
        self.0.clone()
    }
}
struct Accounts(Result<PathBuf, watane_host::AccountError>);
impl watane_host::AccountSource for Accounts {
    fn openai_token(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, watane_host::AccountError>> + Send>,
    > {
        Box::pin(async { Err(watane_host::AccountError::NoAccount) })
    }
    fn judge(&self, _: &str) -> Result<watane_host::JudgeAccount, watane_host::AccountError> {
        Err(watane_host::AccountError::NoAccount)
    }
    fn claude_binary(&self) -> Result<PathBuf, watane_host::AccountError> {
        self.0.clone()
    }
}
async fn home_with(accounts: Option<Accounts>) -> (TempDir, Arc<Host>) {
    let dir = tempfile::tempdir().unwrap();
    let app = App(accounts.map(|a| Arc::new(a) as Arc<dyn watane_host::AccountSource>));
    let host = Host::open(
        dir.path(),
        Mode::Home {
            app: Some(Arc::new(app)),
        },
        Limits::default(),
    )
    .unwrap();
    let plan = host
        .plan(vec![watane_host::Source::Folder(fixture("claude"))])
        .unwrap();
    host.install(plan, watane_host::Requester::App)
        .await
        .unwrap();
    (dir, host)
}
fn ended_of(host: &Arc<Host>) -> Vec<String> {
    serde_json::from_value(call_as(host, Caller::Page, "claude", "api.ended", Value::Null).unwrap())
        .unwrap()
}
async fn start_in(host: &Arc<Host>) -> String {
    let host = Arc::clone(host);
    tokio::task::spawn_blocking(move || {
        let s = call_as(
            &host,
            Caller::Page,
            "claude",
            "api.start",
            json!({"launch": LAUNCH}),
        )
        .unwrap();
        let s = s.as_str().unwrap().to_owned();
        wait("an end", Duration::from_secs(20), || {
            !ended_of(&host).is_empty()
        });
        s
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn ac10_a_home_without_accounts_ends_no_app_and_a_missing_pick_ends_no_binary() {
    let (_dir, host) = home_with(None).await;
    let s = start_in(&host).await;
    assert_eq!(ended_of(&host), [format!("{s} no-app")]);
    let (_dir, host) = home_with(Some(Accounts(Err(watane_host::AccountError::NoAccount)))).await;
    let s = start_in(&host).await;
    assert_eq!(ended_of(&host), [format!("{s} no-binary")]);
    let (_dir, host) = home_with(Some(Accounts(Err(watane_host::AccountError::Unavailable(
        "offline".into(),
    )))))
    .await;
    let s = start_in(&host).await;
    assert_eq!(
        ended_of(&host),
        [format!("{s} failed(account unavailable: offline)")]
    );
}

#[tokio::test]
async fn ac10_a_home_runs_the_program_that_the_account_source_names() {
    let rig = Rig::new(&[("exit", b"0")]);
    let (_dir, host) = home_with(Some(Accounts(Ok(rig.program())))).await;
    let s = start_in(&host).await;
    assert_eq!(ended_of(&host), [format!("{s} closed")]);
    assert_eq!(rig.entries("ARG")[0], "-p");
}

#[test]
fn ac10_the_shell_grant_checks_its_launch_then_its_stream_ends_failed() {
    let rig = Rig::new(&[]);
    let s = rig.call("api.start-shell", json!({"launch": LAUNCH}));
    assert!(
        s.as_str().unwrap().starts_with("bad-launch:sandbox:"),
        "{s}"
    );
    let launch = r#"{"version":1,"workspace":"scratch","sandbox":{"enabled":true},"allow":["Bash(ls:*)","Bash(npm test)"]}"#;
    let s = rig
        .call("api.start-shell", json!({"launch": launch}))
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        rig.wait_ended(1),
        [format!(
            "{s} failed(the shell grant waits on section 18, Q39 and Q42)"
        )]
    );
    assert!(rig.log().is_empty());
}

#[test]
fn ac11_the_program_exiting_ends_the_stream_closed_and_deletes_the_run_folder() {
    let rig = Rig::new(&[
        ("exit", b"0"),
        ("lines", b"{\"type\":\"assistant\",\"n\":1}\n"),
    ]);
    let s = rig.start_ok(LAUNCH);
    assert_eq!(rig.wait_ended(1), [format!("{s} closed")]);
    // The last line came first.
    assert_eq!(
        rig.items(&s).last().unwrap(),
        r#"{"type":"assistant","n":1}"#
    );
    let runs = rig.host().home().join("runs");
    assert!(
        std::fs::read_dir(&runs)
            .unwrap()
            .all(|e| e.unwrap().file_name() == "pids")
    );
    assert_eq!(std::fs::read_to_string(runs.join("pids")).unwrap(), "");
    let ws = workspace_of(&rig.items(&s)[0]);
    assert!(
        rig.host()
            .home()
            .join("scratch")
            .join(ws.strip_prefix("scratch:").unwrap())
            .is_dir()
    );
}

#[test]
fn ac11_a_non_zero_exit_ends_failed_naming_the_status() {
    let rig = Rig::new(&[("exit", b"3")]);
    let s = rig.start_ok(LAUNCH);
    assert_eq!(
        rig.wait_ended(1),
        [format!("{s} failed(claude exited with status 3)")]
    );
}

#[test]
fn ac11_cancel_and_disable_kill_the_group_and_keep_the_scratch_folder() {
    let rig = Rig::new(&[("spawn", b"")]);
    let s = rig.start_ok(LAUNCH);
    let ws = workspace_of(&rig.wait_items(&s, 2)[0]);
    let scratch = rig
        .host()
        .home()
        .join("scratch")
        .join(ws.strip_prefix("scratch:").unwrap());
    let pid: i32 = rig.entries("PID")[0].parse().unwrap();
    let child: i32 = rig.entries("CHILD")[0].parse().unwrap();
    assert!(alive(pid) && alive(child));
    rig.call("api.cancel", json!({"s": s}));
    assert_eq!(rig.wait_ended(1), [format!("{s} cancelled")]);
    wait("the group to die", Duration::from_secs(10), || {
        !alive(pid) && !alive(child)
    });
    assert!(scratch.is_dir());
    wait("the run folder to go", Duration::from_secs(10), || {
        std::fs::read_dir(rig.host().home().join("runs"))
            .unwrap()
            .all(|e| e.unwrap().file_name() == "pids")
    });
    // Disabling the mod: the same, `gone`.
    std::fs::remove_file(rig.fake.path().join("claude.log")).unwrap();
    let s2 = rig.start_ok(&format!(r#"{{"version":1,"workspace":"{ws}"}}"#));
    rig.wait_items(&s2, 2);
    let pid2: i32 = rig.entries("PID")[0].parse().unwrap();
    rig.host().disable(&local("claude")).unwrap();
    wait("the group to die", Duration::from_secs(10), || !alive(pid2));
    assert!(scratch.is_dir());
}

#[test]
fn ac11_open_kills_the_recorded_groups_and_empties_runs() {
    let mut rig = Rig::new(&[]);
    rig.close();
    let runs = rig.home.path().join("runs");
    std::fs::create_dir_all(runs.join("old")).unwrap();
    use std::os::unix::process::CommandExt;
    let child = std::process::Command::new("sleep")
        .arg("300")
        .process_group(0)
        .spawn()
        .unwrap();
    let pid = i32::try_from(child.id()).unwrap();
    std::fs::write(runs.join("pids"), format!("{pid}\n")).unwrap();
    assert!(alive(pid));
    rig.open();
    wait("the group to die", Duration::from_secs(10), || !alive(pid));
    assert!(!runs.join("old").exists());
    assert!(
        !runs.join("pids").exists()
            || std::fs::read_to_string(runs.join("pids"))
                .unwrap()
                .is_empty()
    );
    let mut child = child;
    let _ = child.wait();
}

#[test]
fn ac11_removing_the_owner_deletes_its_scratch_folders() {
    let rig = Rig::new(&[("exit", b"0")]);
    let s = rig.start_ok(LAUNCH);
    let ws = workspace_of(&rig.wait_items(&s, 1)[0]);
    let scratch = rig
        .host()
        .home()
        .join("scratch")
        .join(ws.strip_prefix("scratch:").unwrap());
    assert!(scratch.is_dir());
    rig.wait_ended(1);
    rig.host().remove(&local("claude"), true).unwrap();
    assert!(!scratch.exists());
}

#[test]
fn ac12_a_second_launch_on_a_held_scratch_id_runs_in_the_same_folder_also_after_a_restart() {
    let mut rig = Rig::new(&[("exit", b"0")]);
    let s = rig.start_ok(LAUNCH);
    let ws = workspace_of(&rig.wait_items(&s, 1)[0]);
    rig.wait_ended(1);
    let launch = format!(r#"{{"version":1,"workspace":"{ws}"}}"#);
    let s2 = rig.start_ok(&launch);
    assert_eq!(workspace_of(&rig.wait_items(&s2, 1)[0]), ws);
    let cwd = rig.entries("CWD");
    assert_eq!(cwd.len(), 2);
    assert_eq!(cwd[0], cwd[1]);
    rig.close();
    // The row names the origin client.
    assert_eq!(
        scratch_owner(&rig, ws.strip_prefix("scratch:").unwrap()).as_deref(),
        Some("local/claude")
    );
    std::fs::remove_file(rig.fake.path().join("claude.log")).unwrap();
    rig.open();
    let s3 = rig.start_ok(&launch);
    assert_eq!(workspace_of(&rig.wait_items(&s3, 1)[0]), ws);
    assert_eq!(rig.entries("CWD"), [cwd[0].clone()]);
}

#[test]
fn limits_a_fifth_process_of_one_origin_client_ends_failed() {
    // 4 processes per origin client (19.2, Limits): the fifth ends failed.
    let rig = Rig::new(&[]);
    let ids: Vec<String> = (0..5).map(|_| rig.start_ok(LAUNCH)).collect();
    let ended = rig.wait_ended(1);
    assert_eq!(
        ended,
        [format!("{} failed(too many claude processes)", ids[4])]
    );
}
