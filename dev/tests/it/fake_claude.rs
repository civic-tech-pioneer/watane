#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Tests of the program `fake-claude` (TECHNICAL 18.2), driven over pipes.

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const DEFAULT: &str = r#"{"steps":[{"do":"expect-user"},{"do":"delta","text":"This is the fake claude."},{"do":"message","text":"This is the fake claude."},{"do":"result"},{"do":"idle"}]}"#;
const IDLE: &str = r#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#;

struct Folder {
    dir: tempfile::TempDir,
}
impl Folder {
    /// A folder of its own: the program linked in, as `serve` does, and its script.
    fn new(script: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(
            env!("CARGO_BIN_EXE_fake-claude"),
            dir.path().join("fake-claude"),
        )
        .unwrap();
        fs::write(dir.path().join("fake-claude.json"), script).unwrap();
        Self { dir }
    }
    fn path(&self) -> &Path {
        self.dir.path()
    }
    fn program(&self) -> PathBuf {
        self.path().join("fake-claude")
    }
    fn log(&self) -> Vec<Value> {
        fs::read_to_string(self.path().join("fake-claude.log"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
    fn start(&self, extra: &[&str]) -> Session {
        let mut args = fixed();
        args.extend(extra.iter().map(|s| (*s).to_owned()));
        self.start_with(&args)
    }
    fn start_with(&self, args: &[String]) -> Session {
        let mut child = Command::new(self.program())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).split(b'\n') {
                if tx.send(line.unwrap()).is_err() {
                    return;
                }
            }
        });
        Session {
            child,
            stdin: Some(stdin),
            rx,
        }
    }
}
fn fixed() -> Vec<String> {
    [
        "-p",
        "--input-format=stream-json",
        "--output-format=stream-json",
        "--verbose",
        "--restricted",
        "--strict-mcp-config",
        "--system-prompt-snapshot=off",
        "--settings={}",
    ]
    .map(String::from)
    .to_vec()
}
struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Vec<u8>>,
}
impl Session {
    fn send(&mut self, line: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{line}").unwrap();
        stdin.flush().unwrap();
    }
    fn bytes(&self) -> Vec<u8> {
        self.rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a line from fake-claude")
    }
    fn line(&self) -> Value {
        serde_json::from_slice(&self.bytes()).unwrap()
    }
    /// `initialize` sent; the answer and the `system/init` line read.
    fn initialize(&mut self, hooks: bool) -> (Value, Value) {
        let mut request = json!({"subtype":"initialize"});
        if hooks {
            request["hooks"] =
                json!({"PreToolUse":[{"matcher":"*","hookCallbackIds":["cb1"],"timeout":30}]});
        }
        self.send(&json!({"type":"control_request","request_id":"i1","request":request}));
        let answer = self.line();
        (answer, self.line())
    }
    fn user(&mut self, text: &str) {
        self.send(&json!({"type":"user","message":{"role":"user","content":text}}));
    }
    fn status(&mut self) -> i32 {
        self.stdin = None;
        self.child.wait().unwrap().code().unwrap_or(-1)
    }
    fn stderr(&mut self) -> String {
        let mut text = String::new();
        std::io::Read::read_to_string(self.child.stderr.as_mut().unwrap(), &mut text).unwrap();
        text
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn ac1_version_default_and_from_script() {
    let f = Folder::new("{}");
    let out = Command::new(f.program()).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "2.1.295 (Claude Code)\n"
    );
    fs::write(
        f.path().join("fake-claude.json"),
        r#"{"version":"2.1.400"}"#,
    )
    .unwrap();
    let out = Command::new(f.program()).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "2.1.400 (Claude Code)\n"
    );
}

#[test]
fn ac2_default_script_over_pipes() {
    let f = Folder::new(DEFAULT);
    let mut s = f.start(&[
        "--tools=Read,Grep",
        "--permission-mode=default",
        "--agents={\"scout\":{\"description\":\"d\",\"prompt\":\"p\"}}",
    ]);
    let (answer, init) = s.initialize(false);
    assert_eq!(
        answer,
        json!({"type":"control_response","response":{"subtype":"success","request_id":"i1",
            "response":{"hook_on_failure_supported":true}}})
    );
    assert_eq!(init["type"], "system");
    assert_eq!(init["subtype"], "init");
    assert_eq!(init["tools"], json!(["Read", "Grep"]));
    assert_eq!(init["mcp_servers"], json!([]));
    assert_eq!(init["agents"], json!(["scout"]));
    assert_eq!(init["permissionMode"], "default");
    assert_eq!(init["plugins"].as_array().unwrap().len(), 1);
    assert_eq!(init["plugins"][0]["name"], "watane-run");
    s.user("hi");
    let rest: Vec<Value> = (0..4).map(|_| s.line()).collect();
    assert_eq!(
        rest[0],
        json!({"type":"stream_event","session_id":"fake-session","uuid":"fake-uuid","event":{"type":"content_block_delta",
            "index":0,"delta":{"type":"text_delta","text":"This is the fake claude."}}})
    );
    assert_eq!(
        rest[1],
        json!({"type":"assistant","session_id":"fake-session","message":{
            "id":"msg_fake","type":"message","role":"assistant","model":"fake-claude",
            "content":[{"type":"text","text":"This is the fake claude."}],
            "stop_reason":"end_turn","stop_sequence":null,
            "usage":{"input_tokens":1,"output_tokens":1}}})
    );
    assert_eq!(
        rest[2],
        json!({"type":"result","subtype":"success","is_error":false,"duration_ms":1,
            "duration_api_ms":1,"num_turns":1,"result":"","session_id":"fake-session",
            "total_cost_usd":0,"usage":{"input_tokens":1,"output_tokens":1}})
    );
    assert_eq!(rest[3], serde_json::from_str::<Value>(IDLE).unwrap());
    // The kernel's own types read every line it wrote.
    for line in [&answer, &init, &rest[0], &rest[1], &rest[2], &rest[3]] {
        claude_codes::ClaudeOutput::parse_json(&line.to_string()).unwrap();
    }
    // A system line parses whatever it holds; the init line must read as the crate's init.
    let Ok(claude_codes::ClaudeOutput::System(system)) =
        claude_codes::ClaudeOutput::parse_json(&init.to_string())
    else {
        panic!("{init}")
    };
    assert!(system.as_init().is_some(), "{init}");
    assert_eq!(s.status(), 0);
}

#[test]
fn ac2_script_overrides_and_hooks() {
    let script = json!({"initialize":{"hooks_applied":false,"extra":1},
        "init":{"plugins":[{"name":"watane-run","path":""},{"name":"evil","path":""}],"tools":["Bash"]},
        "steps":[]});
    let f = Folder::new(&script.to_string());
    let mut s = f.start(&["--tools=Read", "--permission-mode=acceptEdits"]);
    let (answer, init) = s.initialize(true);
    assert_eq!(
        answer["response"]["response"],
        json!({"hook_on_failure_supported":true,"hooks_applied":false,"extra":1})
    );
    assert_eq!(init["tools"], json!(["Bash"]));
    assert_eq!(init["plugins"][1]["name"], "evil");
    assert_eq!(init["permissionMode"], "acceptEdits");
    // Hooks in the request: `hooks_applied` is true unless the script says otherwise.
    let f = Folder::new(r#"{"steps":[]}"#);
    let mut s2 = f.start(&[]);
    let (answer, _) = s2.initialize(true);
    assert_eq!(answer["response"]["response"]["hooks_applied"], true);
    drop(s);
}

#[test]
fn ac3_missing_and_forbidden_arguments_exit_64() {
    let f = Folder::new("{}");
    for missing in fixed() {
        let args: Vec<String> = fixed().into_iter().filter(|a| *a != missing).collect();
        let mut s = f.start_with(&args);
        assert_eq!(s.status(), 64, "without {missing}");
        let name = missing.split('=').next().unwrap().to_owned();
        assert!(s.stderr().contains(&name), "names {name}");
    }
    for bad in [
        "--safe-mode",
        "--bare",
        "--plugin-url=x",
        "--worktree",
        "--bg",
        "--cloud",
        "--remote-control",
        "--chrome",
        "--ide",
        "--file=a",
        "--betas=b",
        "--permission-mode=auto",
        "--permission-mode=bypassPermissions",
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        // Host names: `claude` knows only its own (default, acceptEdits, plan, dontAsk).
        "--permission-mode=manual",
        "--permission-mode=accept-edits",
    ] {
        let mut s = f.start(&[bad]);
        assert_eq!(s.status(), 64, "{bad}");
        assert!(
            s.stderr().contains(bad.split('=').next().unwrap()),
            "names {bad}"
        );
    }
}

#[test]
fn ac4_log_holds_arguments_environment_and_stdin_in_order() {
    let f = Folder::new(DEFAULT);
    let mut s = f.start(&["--tools=Read"]);
    s.initialize(false);
    s.user("hi");
    for _ in 0..4 {
        s.line();
    }
    assert_eq!(s.status(), 0);
    let log = f.log();
    let mut args = fixed();
    args.push("--tools=Read".into());
    assert_eq!(log[0], json!({ "args": args }));
    let envs = log.iter().filter(|l| l.get("env").is_some()).count();
    assert_eq!(envs, std::env::vars_os().count());
    let stdin: Vec<&Value> = log.iter().filter(|l| l.get("stdin").is_some()).collect();
    assert_eq!(stdin.len(), 2);
    let first: Value = serde_json::from_str(stdin[0]["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(first["request"]["subtype"], "initialize");
    let second: Value = serde_json::from_str(stdin[1]["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(second["message"]["content"], "hi");
    let last_env = log.iter().rposition(|l| l.get("env").is_some()).unwrap();
    let first_stdin = log.iter().position(|l| l.get("stdin").is_some()).unwrap();
    assert!(last_env < first_stdin);
}

#[test]
fn ac5_lines_no_step_awaits_exit_70() {
    for bad in [
        json!({"type":"control_request","request_id":"i2","request":{"subtype":"initialize"}}),
        json!({"type":"control_request","request_id":"p","request":{"subtype":"set_permission_mode","mode":"plan"}}),
        json!({"type":"control_request","request_id":"p","request":{"subtype":"apply_flag_settings"}}),
        json!({"type":"control_request","request_id":"p","request":{"subtype":"mcp_set_servers"}}),
        json!({"type":"control_response","response":{"subtype":"success","request_id":"r",
            "response":{"behavior":"allow","updatedPermissions":[]}}}),
    ] {
        let f = Folder::new(r#"{"steps":[{"do":"sleep","ms":5000}]}"#);
        let mut s = f.start(&[]);
        s.initialize(false);
        s.send(&bad);
        assert_eq!(s.child.wait().unwrap().code(), Some(70), "{bad}");
        let log = f.log();
        let text = bad.to_string();
        assert!(log.iter().any(|l| l["stdin"] == text.as_str()), "logged");
        assert!(log.last().unwrap()["error"].is_string());
    }
}

#[test]
fn ac5_expect_user_contains_mismatch_exits_70() {
    let f = Folder::new(r#"{"steps":[{"do":"expect-user","contains":"hello"},{"do":"idle"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    s.user("goodbye");
    assert_eq!(s.child.wait().unwrap().code(), Some(70));
    assert!(
        f.log().last().unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("hello")
    );
    // A match goes on.
    let f = Folder::new(r#"{"steps":[{"do":"expect-user","contains":"hello"},{"do":"idle"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    s.user("oh hello there");
    assert_eq!(s.line(), serde_json::from_str::<Value>(IDLE).unwrap());
}

#[test]
fn ac6_raw_line_and_hex() {
    let f = Folder::new(
        r#"{"steps":[{"do":"raw","line":"not json"},{"do":"raw","hex":"ff fe 41"},{"do":"idle"}]}"#,
    );
    let mut s = f.start(&[]);
    s.initialize(false);
    assert_eq!(s.bytes(), b"not json");
    assert_eq!(s.bytes(), [0xff, 0xfe, 0x41]);
    assert_eq!(s.line()["state"], "idle");
}

#[test]
fn ac6_sleep_exit_and_crash() {
    let f =
        Folder::new(r#"{"steps":[{"do":"sleep","ms":50},{"do":"idle"},{"do":"exit","code":3}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    assert_eq!(s.line()["state"], "idle");
    assert_eq!(s.child.wait().unwrap().code(), Some(3));
    let f = Folder::new(r#"{"steps":[{"do":"crash"},{"do":"idle"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    let status = s.child.wait().unwrap();
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&status),
        Some(9)
    );
}

#[test]
fn ac6_hang_ignores_stdin_until_killed() {
    let f = Folder::new(r#"{"steps":[{"do":"hang"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    std::thread::sleep(Duration::from_millis(100));
    // A line that a reading fake would refuse (exit 70): the hanging one reads and logs nothing.
    let stray = json!({"type":"control_request","request_id":"p","request":{"subtype":"set_permission_mode","mode":"plan"}});
    s.send(&stray);
    std::thread::sleep(Duration::from_millis(200));
    assert!(s.child.try_wait().unwrap().is_none());
    assert!(
        !f.log()
            .iter()
            .any(|l| l["stdin"] == stray.to_string().as_str())
    );
    s.child.kill().unwrap();
}

#[test]
fn ac6_ask_check_cancel_and_answers() {
    let f = Folder::new(
        r#"{"steps":[
        {"do":"ask","tool":"Edit","input":{"file_path":"a"},"tool_use_id":"t1","await":true},
        {"do":"check","tool":"Bash","input":{"command":"ls"},"tool_use_id":"t2","error":true},
        {"do":"ask","tool":"Read","tool_use_id":"t3"},
        {"do":"cancel","tool_use_id":"t3"},
        {"do":"idle"}]}"#,
    );
    let mut s = f.start(&[]);
    s.initialize(true);
    let ask = s.line();
    assert_eq!(ask["type"], "control_request");
    assert_eq!(ask["request"]["subtype"], "can_use_tool");
    assert_eq!(ask["request"]["tool_name"], "Edit");
    assert_eq!(ask["request"]["input"], json!({"file_path":"a"}));
    assert_eq!(ask["request"]["tool_use_id"], "t1");
    // Nothing more until the host answers.
    s.rx.recv_timeout(Duration::from_millis(200)).unwrap_err();
    let deny = json!({"type":"control_response","response":{"subtype":"success",
        "request_id":ask["request_id"],"response":{"behavior":"deny","message":"no"}}});
    let denied = deny.to_string();
    s.send(&deny);
    let check = s.line();
    assert_eq!(check["request"]["subtype"], "hook_callback");
    assert_eq!(check["request"]["callback_id"], "cb1");
    assert_eq!(check["request"]["input"]["tool_name"], "Bash");
    assert_eq!(
        s.line(),
        json!({"type":"system","subtype":"hook_response","hook_event":"PreToolUse",
            "outcome":"error","tool_use_id":"t2"})
    );
    let third = s.line();
    assert_eq!(third["request"]["tool_use_id"], "t3");
    let cancel = s.line();
    assert_eq!(
        cancel,
        json!({"type":"control_cancel_request","request_id":third["request_id"]})
    );
    for line in [&ask, &check, &third, &cancel] {
        claude_codes::ClaudeOutput::parse_json(&line.to_string()).unwrap();
    }
    assert_eq!(s.line()["state"], "idle");
    assert!(
        f.log()
            .iter()
            .any(|l| l["stdin"].as_str() == Some(denied.as_str())),
        "the answer is logged"
    );
}

#[test]
fn ac6_expect_interrupt_answers_it() {
    let f = Folder::new(r#"{"steps":[{"do":"expect-interrupt"},{"do":"idle"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    s.send(&json!({"type":"control_request","request_id":"x9","request":{"subtype":"interrupt"}}));
    assert_eq!(
        s.line(),
        json!({"type":"control_response","response":{"subtype":"success","request_id":"x9","response":{}}})
    );
    assert_eq!(s.line()["state"], "idle");
}

#[test]
fn ac6_delta_chunks() {
    let f = Folder::new(r#"{"steps":[{"do":"delta","text":"abcde","chunk":2}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    let texts: Vec<String> = (0..3)
        .map(|_| {
            s.line()["event"]["delta"]["text"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(texts, ["ab", "cd", "e"]);
}

struct Serve {
    root: tempfile::TempDir,
}
impl Serve {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let (code, _) = Self::run_in(root.path(), &["new", "hello", "--json"], None);
        assert_eq!(code, 0);
        Self { root }
    }
    fn run_in(cwd: &Path, args: &[&str], fake_claude: Option<&str>) -> (i32, Value) {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        c.current_dir(cwd)
            .env("WATANE_DEV_CACHE", crate::support::cache())
            .env("WATANE_BIN", env!("CARGO_BIN_EXE_fake-watane"))
            .args(args);
        if let Some(path) = fake_claude {
            c.env("WATANE_FAKE_CLAUDE", path);
        }
        let out = c.output().unwrap();
        let v = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| panic!("{out:?}"));
        (out.status.code().unwrap(), v)
    }
    fn serve(&self, fake_claude: Option<&str>) -> Value {
        let (code, v) = Self::run_in(
            &self.root.path().join("hello"),
            &["serve", "--json"],
            fake_claude,
        );
        assert_eq!(code, 0, "{v}");
        v
    }
    fn preview(v: &Value) -> PathBuf {
        PathBuf::from(v["result"]["home"].as_str().unwrap())
            .parent()
            .unwrap()
            .to_path_buf()
    }
    fn started_with(v: &Value) -> Vec<String> {
        let home = PathBuf::from(v["result"]["home"].as_str().unwrap());
        serde_json::from_slice(&fs::read(home.join("args.json")).unwrap()).unwrap()
    }
}
impl Drop for Serve {
    fn drop(&mut self) {
        let _ = Serve::run_in(
            &self.root.path().join("hello"),
            &["serve", "--stop", "--json"],
            None,
        );
    }
}

#[test]
#[ignore = "slow: full serve pipeline: the spawned host gets --fake-claude"]
fn ac7_serve_passes_the_flag_to_the_host() {
    let s = Serve::new();
    let v = s.serve(None);
    let p = Serve::preview(&v);
    let link = p.join("claude/fake-claude");
    let home = v["result"]["home"].as_str().unwrap();
    assert_eq!(
        Serve::started_with(&v),
        ["--preview", home, "--fake-claude", link.to_str().unwrap()]
    );
    assert!(v["warnings"].as_array().is_none_or(Vec::is_empty), "{v}");
    // Without the program: the warning, and the host starts without the flag.
    let dir = s.root.path().join("hello");
    let (code, _) = Serve::run_in(&dir, &["serve", "--stop", "--json"], None);
    assert_eq!(code, 0);
    let v = s.serve(Some("/nonexistent/fake-claude"));
    assert_eq!(
        v["warnings"][0]["code"], "preview.fake_claude_missing",
        "{v}"
    );
    assert_eq!(Serve::started_with(&v), ["--preview", home]);
}

const PROBE_WORLD: &str = include_str!("../fixtures/probe/world.wit");
const PROBE_LOGIC: &str = include_str!("../fixtures/probe/lib.rs");

/// One `/call` of the mod's page: the answer's JSON.
fn page_call(v: &Value, function: &str, args: &Value) -> Value {
    use std::net::TcpStream;
    let page = v["result"]["pages"][0]["url"].as_str().unwrap();
    let origin = page.trim_end_matches('/');
    let authority = origin.strip_prefix("http://").unwrap();
    let port: u16 = authority.rsplit(':').next().unwrap().parse().unwrap();
    let body = json!({"fn": function, "args": args}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream, "POST /call HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut bytes).unwrap();
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    assert!(
        bytes.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes[split + 4..]).unwrap()
}

#[test]
#[ignore = "slow: needs WATANE_BIN with the host of s99-2; the real host starts the fake and writes to it"]
fn ac9_real_preview_runs_a_conversation_on_the_fake() {
    let s = Serve::new();
    let dir = s.root.path().join("hello");
    fs::write(
        dir.join("wit/world.wit"),
        PROBE_WORLD.replace("probe", "hello"),
    )
    .unwrap();
    fs::write(
        dir.join("logic/src/lib.rs"),
        PROBE_LOGIC.replace("probe", "hello"),
    )
    .unwrap();
    fs::write(
        dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"chat.seen","returns":[]}]}"#,
    )
    .unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(&dir)
        .env("WATANE_DEV_CACHE", crate::support::cache())
        .env(
            "WATANE_BIN",
            std::env::var_os("WATANE_BIN").expect("set WATANE_BIN"),
        )
        .args(["serve", "--json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{v}");
    let log = Serve::preview(&v).join("claude/fake-claude.log");
    page_call(&v, "chat.start", &json!([]));
    let wait = |what: &str, done: &dyn Fn() -> bool| {
        for _ in 0..100 {
            if done() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        panic!("{what}");
    };
    let seen = || page_call(&v, "chat.seen", &json!([])).to_string();
    wait("initialized", &|| seen().contains("initialized"));
    page_call(&v, "chat.say", &json!(["hi"]));
    wait("the fake's reply", &|| {
        seen().contains("This is the fake claude.")
    });
    let written = fs::read_to_string(log).unwrap();
    assert!(
        written
            .lines()
            .any(|l| l.contains(r#"\"type\":\"user\""#) && l.contains(r#"\"content\":\"hi\""#)),
        "{written}"
    );
}

fn parsed(s: &Session) -> Value {
    let v = s.line();
    claude_codes::ClaudeOutput::parse_json(&v.to_string()).unwrap();
    v
}
fn text_of(v: &Value) -> &str {
    v["message"]["content"][0]["text"].as_str().unwrap()
}
fn interrupt(s: &mut Session, id: &str) {
    s.send(&json!({"type":"control_request","request_id":id,"request":{"subtype":"interrupt"}}));
}
fn interrupt_answer(id: &str) -> Value {
    json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":{}}})
}

#[test]
fn s100_ac1_long_turn_entries_result_idle() {
    let f = Folder::new(r#"{"steps":[{"do":"long-turn","entries":3,"bytes":16}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    for want in ["entry 1 xxxxxxxx", "entry 2 xxxxxxxx", "entry 3 xxxxxxxx"] {
        let v = parsed(&s);
        assert_eq!(v["type"], "assistant");
        assert_eq!(text_of(&v), want);
    }
    let result = parsed(&s);
    assert_eq!(result["type"], "result");
    assert_eq!(result["subtype"], "success");
    assert_eq!(s.line()["state"], "idle");
}

#[test]
fn s100_ac1_long_turn_defaults() {
    let f = Folder::new(r#"{"steps":[{"do":"long-turn"}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    for i in 1..=100 {
        let v = s.line();
        let text = text_of(&v);
        assert_eq!(text.len(), 32);
        assert!(text.starts_with(&format!("entry {i} x")));
    }
    assert_eq!(s.line()["type"], "result");
}

#[test]
fn s100_ac2_long_turn_interrupted_session_goes_on() {
    let f = Folder::new(
        r#"{"steps":[{"do":"long-turn","entries":1000,"every-ms":5},{"do":"expect-user","contains":"next"},{"do":"message","text":"ok"}]}"#,
    );
    let mut s = f.start(&[]);
    s.initialize(false);
    assert_eq!(text_of(&parsed(&s)), format!("entry 1 {}", "x".repeat(24)));
    interrupt(&mut s, "x1");
    let mut entries = 1;
    loop {
        let v = parsed(&s);
        if v["type"] == "assistant" {
            entries += 1;
        } else {
            assert_eq!(v, interrupt_answer("x1"));
            break;
        }
    }
    assert!(entries < 1000);
    let result = parsed(&s);
    assert_eq!(result["type"], "result");
    assert_eq!(result["subtype"], "error_during_execution");
    assert_eq!(s.line()["state"], "idle");
    s.user("next please");
    assert_eq!(text_of(&parsed(&s)), "ok");
}

#[test]
fn s100_ac3_reply_per_say() {
    let f = Folder::new(r#"{"steps":[{"do":"reply-per-say","turns":2}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    for said in ["a", "b"] {
        s.user(said);
        let d = parsed(&s);
        assert_eq!(d["event"]["delta"]["text"], format!("you said: {said}"));
        assert_eq!(text_of(&parsed(&s)), format!("you said: {said}"));
        assert_eq!(parsed(&s)["type"], "result");
        assert_eq!(s.line()["state"], "idle");
    }
}

#[test]
fn s100_ac3_reply_per_say_text_until_stdin_closes_and_interrupt() {
    let f = Folder::new(r#"{"steps":[{"do":"reply-per-say","text":"echo "}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    interrupt(&mut s, "q");
    assert_eq!(s.line(), interrupt_answer("q"));
    s.user("hi");
    assert_eq!(s.line()["event"]["delta"]["text"], "echo hi");
    assert_eq!(text_of(&s.line()), "echo hi");
    s.line();
    s.line();
    assert_eq!(s.status(), 0);
}

fn ask_script() -> Folder {
    Folder::new(
        r#"{"steps":[{"do":"ask","tool":"Edit","tool_use_id":"t1","await":true},
        {"do":"message","text":"after"},{"do":"result"},{"do":"idle"},
        {"do":"expect-user"},{"do":"message","text":"next"}]}"#,
    )
}
fn answer(id: &Value, response: &Value) -> Value {
    json!({"type":"control_response","response":{"subtype":"success",
        "request_id":id,"response":response}})
}

#[test]
fn s100_ac4_ask_allow_goes_on() {
    let f = ask_script();
    let mut s = f.start(&[]);
    s.initialize(true);
    let ask = parsed(&s);
    let allow = answer(
        &ask["request_id"],
        &json!({"behavior":"allow","updatedInput":{}}),
    );
    s.send(&allow);
    assert_eq!(text_of(&parsed(&s)), "after");
    assert_eq!(parsed(&s)["type"], "result");
    let text = allow.to_string();
    assert!(f.log().iter().any(|l| l["stdin"] == text.as_str()));
}

#[test]
fn s100_ac4_ask_deny_ends_the_turn() {
    let f = ask_script();
    let mut s = f.start(&[]);
    s.initialize(true);
    let ask = parsed(&s);
    s.send(&answer(
        &ask["request_id"],
        &json!({"behavior":"deny","message":"no"}),
    ));
    assert_eq!(text_of(&parsed(&s)), "denied: no");
    assert_eq!(parsed(&s)["type"], "result");
    assert_eq!(s.line()["state"], "idle");
    s.user("go");
    assert_eq!(text_of(&parsed(&s)), "next");
}

#[test]
fn s100_ac4_ask_deny_interrupted_then_interrupt() {
    let f = ask_script();
    let mut s = f.start(&[]);
    s.initialize(true);
    let ask = parsed(&s);
    s.send(&answer(
        &ask["request_id"],
        &json!({"behavior":"deny","message":"interrupted"}),
    ));
    interrupt(&mut s, "i1");
    assert_eq!(text_of(&parsed(&s)), "denied: interrupted");
    assert_eq!(parsed(&s)["type"], "result");
    assert_eq!(s.line()["state"], "idle");
    s.user("go");
    assert_eq!(s.line(), interrupt_answer("i1"));
    assert_eq!(text_of(&parsed(&s)), "next");
}

#[test]
fn s100_ac4_interrupt_in_place_of_the_answer() {
    let f = ask_script();
    let mut s = f.start(&[]);
    s.initialize(true);
    parsed(&s);
    interrupt(&mut s, "i2");
    assert_eq!(s.line(), interrupt_answer("i2"));
    assert_eq!(parsed(&s)["type"], "result");
    assert_eq!(s.line()["state"], "idle");
    s.user("go");
    assert_eq!(text_of(&parsed(&s)), "next");
}

#[test]
fn s100_ac5_load_writes_ten_thousand_entries() {
    let f = Folder::new(r#"{"steps":[{"do":"load","entries":10000}]}"#);
    let mut s = f.start(&[]);
    s.initialize(false);
    let start = std::time::Instant::now();
    for i in 1..=10_000 {
        let bytes = s.bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        let text = text_of(&v);
        assert_eq!(text.len(), 1024);
        assert!(text.starts_with(&format!("entry {i} x")));
    }
    assert_eq!(s.line()["type"], "result");
    assert_eq!(s.line()["state"], "idle");
    let took = start.elapsed();
    println!("load of 10000 entries took {took:?}");
    assert!(took < Duration::from_secs(5), "{took:?}");
}

#[test]
fn s100_ac6_host_permission_arguments_and_unawaited_interrupt() {
    let f = Folder::new(r#"{"steps":[{"do":"sleep","ms":300},{"do":"expect-user"}]}"#);
    let mut s = f.start(&[
        "--permission-prompts=host",
        "--permission-prompt-tool=stdio",
    ]);
    s.initialize(false);
    interrupt(&mut s, "u1");
    s.user("x");
    assert_eq!(s.line(), interrupt_answer("u1"));
    assert_eq!(s.status(), 0);
    let log = f.log();
    assert!(
        log.iter()
            .any(|l| l["stdin"].as_str().is_some_and(|t| t.contains("\"u1\"")))
    );
    assert!(log.iter().all(|l| l.get("error").is_none()));
}
