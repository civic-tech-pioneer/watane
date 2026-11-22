//! The fake `claude` of a maker's preview and of the agent layer tests: stream-json and control
//! protocol from a script, checks the host's arguments, logs what the host wrote (TECHNICAL 18.2).
#![allow(clippy::print_stderr, reason = "a program reports on stderr")]
use serde_json::{Map, Value, json};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Mutex, mpsc};
use std::time::Duration;

const FIXED: [&str; 7] = [
    "-p",
    "--input-format=stream-json",
    "--output-format=stream-json",
    "--verbose",
    "--restricted",
    "--strict-mcp-config",
    "--system-prompt-snapshot=off",
];
const FORBIDDEN: [&str; 15] = [
    "--safe-mode",
    "--bare",
    "--plugin-url",
    "--worktree",
    "--bg",
    "--cloud",
    "--remote-control",
    "--chrome",
    "--ide",
    "--file",
    "--betas",
    "--permission-mode=auto",
    "--permission-mode=bypassPermissions",
    "--dangerously-skip-permissions",
    "--allow-dangerously-skip-permissions",
];
const SESSION: &str = "fake-session";

/// Set by `hang`: the reader then reads, logs and checks nothing more (TECHNICAL 18.2).
static HANGING: AtomicBool = AtomicBool::new(false);
static LOG: Mutex<Option<File>> = Mutex::new(None);

fn log(value: &Value) {
    if let Ok(mut guard) = LOG.lock()
        && let Some(file) = guard.as_mut()
    {
        let _ = writeln!(file, "{value}");
    }
}
fn die(code: i32, message: &str) -> ! {
    log(&json!({"error": message}));
    eprintln!("fake-claude: {message}");
    std::process::exit(code)
}
fn emit(value: &Value) {
    emit_bytes(value.to_string().as_bytes());
}
fn emit_bytes_raw(bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let mut out = std::io::stdout().lock();
    if out.write_all(bytes).and_then(|()| out.flush()).is_err() {
        std::process::exit(0);
    }
}
fn emit_bytes(bytes: &[u8]) {
    let mut out = std::io::stdout().lock();
    if out
        .write_all(bytes)
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .is_err()
    {
        std::process::exit(0);
    }
}

/// The host's lines, checked by the reader thread: a line that no step could await is fatal.
fn spawn_reader(mut input: impl BufRead + Send + 'static) -> Receiver<Value> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        loop {
            let mut raw = Vec::new();
            if input.read_until(b'\n', &mut raw).unwrap_or(0) == 0 {
                return;
            }
            if HANGING.load(Ordering::SeqCst) {
                return;
            }
            let text = String::from_utf8_lossy(&raw).trim_end().to_owned();
            log(&json!({"stdin": text}));
            let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let allowed = match value["type"].as_str().unwrap_or("") {
                "user" => true,
                // An allow answer never changes the session's permissions (host 19.3).
                "control_response" => value["response"]["response"]
                    .get("updatedPermissions")
                    .is_none(),
                "control_request" => value["request"]["subtype"] == "interrupt",
                _ => false,
            };
            if !allowed {
                die(70, &format!("a line that no step awaits: {text}"));
            }
            if tx.send(value).is_err() {
                return;
            }
        }
    });
    rx
}

struct Fake {
    rx: Receiver<Value>,
    queue: VecDeque<Value>,
    counter: u64,
    requests: HashMap<String, String>,
    callback: String,
    /// The turn ended (a deny or an interrupt): steps are skipped up to the next turn step.
    skip: bool,
    /// A later step waits for a `user` message, so a deny may end the turn.
    later_turn: bool,
}
fn is_interrupt(line: &Value) -> bool {
    line["type"] == "control_request" && line["request"]["subtype"] == "interrupt"
}
fn said(line: &Value) -> String {
    let content = &line["message"]["content"];
    content.as_str().map_or_else(
        || {
            content
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|b| b["text"].as_str())
                        .collect::<String>()
                })
                .unwrap_or_default()
        },
        str::to_owned,
    )
}
fn answer_interrupt(line: &Value) {
    emit(
        &json!({"type":"control_response","response":{"subtype":"success",
        "request_id":line["request_id"],"response":{}}}),
    );
}
fn assistant(content: &[Value]) {
    emit(&assistant_line(content));
}
fn assistant_line(content: &[Value]) -> Value {
    json!({"type":"assistant","session_id":SESSION,"message":{
        "id":"msg_fake","type":"message","role":"assistant","model":"fake-claude",
        "content":content,"stop_reason":"end_turn","stop_sequence":null,
        "usage":{"input_tokens":1,"output_tokens":1}}})
}
fn result_line(subtype: &str, text: &str, usage: &Value) -> Value {
    json!({"type":"result","subtype":subtype,"is_error":subtype != "success",
        "duration_ms":1,"duration_api_ms":1,"num_turns":1,"result":text,
        "session_id":SESSION,"total_cost_usd":0,"usage":usage})
}
fn idle_line() -> Value {
    json!({"type":"system","subtype":"session_state_changed","state":"idle"})
}
impl Fake {
    /// The next line, whatever it is; `None` when standard input is closed.
    fn next_any(&mut self) -> Option<Value> {
        self.queue.pop_front().or_else(|| self.rx.recv().ok())
    }
    /// The next line that is not an `interrupt` or an answer no step awaits: an `interrupt` is
    /// answered (host 19.3 step 5), an answer to a written ask or check is consumed.
    fn next_unawaited(&mut self) -> Option<Value> {
        loop {
            let line = self.next_any()?;
            if is_interrupt(&line) {
                answer_interrupt(&line);
            } else if !self.consumed_answer(&line) {
                return Some(line);
            }
        }
    }
    /// A `control_response` that no step awaits is consumed when it answers a request the fake
    /// wrote; one that names none is fatal (TECHNICAL 18.2).
    fn consumed_answer(&self, line: &Value) -> bool {
        if line["type"] != "control_response" {
            return false;
        }
        let id = line["response"]["request_id"].as_str().unwrap_or("");
        if !self.requests.values().any(|r| r == id) {
            die(70, &format!("a line that no step awaits: {line}"));
        }
        true
    }
    fn await_line(&mut self, what: &str) -> Value {
        self.next_unawaited()
            .unwrap_or_else(|| die(70, &format!("stdin closed while waiting for {what}")))
    }
    /// Lines that arrive for `wait` (none: only those already there); a `user` message or an
    /// answer is kept for the next step, an `interrupt` is answered and reported.
    fn poll(&mut self, wait: Duration) -> bool {
        let deadline = std::time::Instant::now() + wait;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let line = if left.is_zero() {
                self.rx.try_recv().ok()
            } else {
                self.rx.recv_timeout(left).ok()
            };
            match line {
                Some(line) if is_interrupt(&line) => {
                    answer_interrupt(&line);
                    return true;
                }
                Some(line) if self.consumed_answer(&line) => {}
                Some(line) => self.queue.push_back(line),
                None => return false,
            }
        }
    }
    fn end_turn(&mut self, subtype: &str) {
        emit(&result_line(
            subtype,
            "",
            &json!({"input_tokens":1,"output_tokens":1}),
        ));
        emit(&idle_line());
    }
    /// `long-turn` and `load` (TECHNICAL 18.2).
    fn long_turn(&mut self, step: &Value, bytes: u64, burst: bool) {
        let entries = step["entries"].as_u64().unwrap_or(100);
        let bytes = usize::try_from(step["bytes"].as_u64().unwrap_or(bytes)).unwrap_or(32);
        let every = Duration::from_millis(step["every-ms"].as_u64().unwrap_or(0));
        let mut pending: Vec<u8> = vec![];
        let mut lines = 0;
        let mut interrupted = false;
        for i in 1..=entries {
            if self.poll(if i == 1 { Duration::ZERO } else { every }) {
                interrupted = true;
                break;
            }
            let mut text = format!("entry {i} ");
            while text.len() < bytes {
                text.push('x');
            }
            let line = assistant_line(&[json!({"type":"text","text":text})]);
            if burst {
                pending.extend_from_slice(line.to_string().as_bytes());
                pending.push(b'\n');
                lines += 1;
                if lines == 64 {
                    emit_bytes_raw(&pending);
                    pending.clear();
                    lines = 0;
                }
            } else {
                emit(&line);
            }
        }
        emit_bytes_raw(&pending);
        self.end_turn(if interrupted {
            "error_during_execution"
        } else {
            "success"
        });
    }
    fn reply_per_say(&mut self, step: &Value) {
        let prefix = step["text"].as_str().unwrap_or("you said: ");
        let turns = step["turns"].as_u64();
        let mut done = 0;
        while turns.is_none_or(|n| done < n) {
            let Some(line) = self.next_unawaited() else {
                if turns.is_none() {
                    std::process::exit(0);
                }
                die(70, "stdin closed while waiting for a user message");
            };
            if line["type"] != "user" {
                die(70, &format!("expected a user message, got {line}"));
            }
            let reply = format!("{prefix}{}", said(&line));
            emit(
                &json!({"type":"stream_event","session_id":SESSION,"uuid":"fake-uuid",
                "event":{"type":"content_block_delta","index":0,
                    "delta":{"type":"text_delta","text":reply}}}),
            );
            assistant(&[json!({"type":"text","text":reply})]);
            self.end_turn("success");
            done += 1;
        }
    }
    fn control(&mut self, step: &Value, mut request: Value, kind: &str) {
        self.counter += 1;
        let tool_use_id = step["tool_use_id"]
            .as_str()
            .map_or_else(|| format!("toolu_fake_{}", self.counter), str::to_owned);
        let request_id = format!("fake-req-{}", self.counter);
        request["tool_use_id"] = json!(tool_use_id);
        emit(&json!({"type":"control_request","request_id":request_id,"request":request}));
        self.requests.insert(tool_use_id.clone(), request_id);
        if kind == "check" && step["error"] == true {
            emit(
                &json!({"type":"system","subtype":"hook_response","hook_event":"PreToolUse",
                "outcome":"error","tool_use_id":tool_use_id}),
            );
        }
        if step["await"] == true {
            let Some(line) = self.next_any() else {
                die(70, "stdin closed while waiting for a control_response");
            };
            if kind == "ask" && is_interrupt(&line) {
                answer_interrupt(&line);
                self.end_turn("error_during_execution");
                self.skip = true;
                return;
            }
            if line["type"] != "control_response" {
                die(70, &format!("expected a control_response, got {line}"));
            }
            let answer = &line["response"]["response"];
            if kind == "ask" && answer["behavior"] == "deny" && self.later_turn {
                let why = answer["message"].as_str().unwrap_or("");
                assistant(&[json!({"type":"text","text":format!("denied: {why}")})]);
                self.end_turn("success");
                self.skip = true;
            }
        }
    }

    fn run(&mut self, step: &Value) {
        let text = step["text"].as_str().unwrap_or("");
        let input = step.get("input").cloned().unwrap_or(json!({}));
        match step["do"].as_str().unwrap_or("") {
            "expect-user" => {
                let line = self.await_line("a user message");
                if line["type"] != "user" {
                    die(70, &format!("expected a user message, got {line}"));
                }
                let said = said(&line);
                if let Some(want) = step["contains"].as_str()
                    && !said.contains(want)
                {
                    die(70, &format!("the user message {said:?} lacks {want:?}"));
                }
            }
            "delta" => {
                let size = step["chunk"].as_u64().map_or(usize::MAX, |n| {
                    usize::try_from(n.max(1)).unwrap_or(usize::MAX)
                });
                let mut rest = text;
                while !rest.is_empty() {
                    let mut end = size.min(rest.len());
                    while !rest.is_char_boundary(end) {
                        end += 1;
                    }
                    let (piece, tail) = rest.split_at(end);
                    emit(
                        &json!({"type":"stream_event","session_id":SESSION,"uuid":"fake-uuid",
                        "event":{"type":"content_block_delta","index":0,
                            "delta":{"type":"text_delta","text":piece}}}),
                    );
                    rest = tail;
                }
            }
            "message" => {
                let mut content = vec![];
                if !text.is_empty() {
                    content.push(json!({"type":"text","text":text}));
                }
                for block in step["tool_use"].as_array().into_iter().flatten() {
                    let mut block = block.clone();
                    block["type"] = json!("tool_use");
                    content.push(block);
                }
                assistant(&content);
            }
            "result" => {
                let subtype = step["subtype"].as_str().unwrap_or("success");
                let usage = step
                    .get("usage")
                    .cloned()
                    .unwrap_or(json!({"input_tokens":1,"output_tokens":1}));
                emit(&result_line(subtype, text, &usage));
            }
            "idle" => {
                emit(&idle_line());
            }
            "ask" => {
                let request =
                    json!({"subtype":"can_use_tool","tool_name":step["tool"],"input":input});
                self.control(step, request, "ask");
            }
            "check" => {
                let request = json!({"subtype":"hook_callback","callback_id":self.callback,
                    "input":{"hook_event_name":"PreToolUse","tool_name":step["tool"],
                        "tool_input":input}});
                self.control(step, request, "check");
            }
            "cancel" => {
                let id = step["tool_use_id"].as_str().unwrap_or("");
                let Some(request_id) = self.requests.get(id) else {
                    die(70, &format!("cancel names no ask or check: {id}"));
                };
                emit(&json!({"type":"control_cancel_request","request_id":request_id}));
            }
            "raw" => {
                if let Some(hex) = step["hex"].as_str() {
                    emit_bytes(&unhex(hex));
                } else {
                    emit_bytes(step["line"].as_str().unwrap_or("").as_bytes());
                }
            }
            "expect-interrupt" => {
                let Some(line) = self.next_any() else {
                    die(70, "stdin closed while waiting for an interrupt");
                };
                if !is_interrupt(&line) {
                    die(70, &format!("expected an interrupt, got {line}"));
                }
                answer_interrupt(&line);
            }
            "long-turn" => self.long_turn(step, 32, false),
            "load" => self.long_turn(step, 1024, true),
            "reply-per-say" => self.reply_per_say(step),
            "sleep" => {
                // A line that arrives meanwhile is kept for the next step.
                let deadline = std::time::Instant::now()
                    + Duration::from_millis(step["ms"].as_u64().unwrap_or(0));
                loop {
                    let left = deadline.saturating_duration_since(std::time::Instant::now());
                    match self.rx.recv_timeout(left) {
                        Ok(line) => self.queue.push_back(line),
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => {
                            std::thread::sleep(left);
                            break;
                        }
                    }
                }
            }
            "exit" => {
                std::process::exit(i32::try_from(step["code"].as_i64().unwrap_or(0)).unwrap_or(1))
            }
            "crash" => {
                let _ = std::process::Command::new("/bin/kill")
                    .arg("-9")
                    .arg(std::process::id().to_string())
                    .status();
                loop {
                    std::thread::park();
                }
            }
            "hang" => {
                HANGING.store(true, Ordering::SeqCst);
                loop {
                    std::thread::park();
                }
            }
            other => die(70, &format!("unknown step {other:?}")),
        }
    }
}
fn unhex(hex: &str) -> Vec<u8> {
    let digits: Vec<u8> = hex.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits
        .chunks(2)
        .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

fn check_arguments(args: &[String]) {
    for fixed in FIXED {
        if !args.iter().any(|a| a == fixed) {
            die(64, &format!("missing argument {fixed}"));
        }
    }
    if !args.iter().any(|a| a.starts_with("--settings=")) {
        die(64, "missing argument --settings=");
    }
    for arg in args {
        for bad in FORBIDDEN {
            if arg == bad || (!bad.contains('=') && arg.starts_with(&format!("{bad}="))) {
                die(64, &format!("forbidden argument {arg}"));
            }
        }
    }
    // Claude Code's names only: a host name such as `manual` or `accept-edits` is a host bug.
    if let Some(mode) = value_of(args, "--permission-mode=")
        && !["default", "acceptEdits", "plan", "dontAsk"].contains(&mode)
    {
        die(64, &format!("unknown argument --permission-mode={mode}"));
    }
}
fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(flag))
}

/// What the launch arguments say, as the `system/init` line holds it.
fn init_line(args: &[String], version: &str, overrides: &Value) -> Value {
    let tools: Vec<&str> = value_of(args, "--tools=")
        .unwrap_or("")
        .split(',')
        .filter(|t| !t.is_empty())
        .collect();
    let agents: Vec<String> = value_of(args, "--agents=")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .map(|v| match v {
            Value::Object(map) => map.keys().cloned().collect(),
            Value::Array(items) => items
                .iter()
                .filter_map(|i| i["name"].as_str().map(str::to_owned))
                .collect(),
            _ => vec![],
        })
        .unwrap_or_default();
    let mode = value_of(args, "--permission-mode=").unwrap_or("default");
    let servers = value_of(args, "--mcp-config=").map_or_else(Vec::new, mcp_servers);
    let mut line = json!({"type":"system","subtype":"init","session_id":SESSION,
        "cwd":std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        "model":"fake-claude","tools":tools,"mcp_servers":servers,"agents":agents,
        "slash_commands":[],"skills":[],"apiKeySource":"none","permissionMode":mode,
        "claude_code_version":version,
        "plugins":[{"name":"watane-run","path":value_of(args, "--plugin-dir=").unwrap_or("")}]});
    for (key, value) in overrides.as_object().into_iter().flatten() {
        line[key] = value.clone();
    }
    line
}
/// The servers of `--mcp-config=` (the host gives a pipe, `/dev/fd/3`: read it, but never wait long).
fn mcp_servers(path: &str) -> Vec<Value> {
    let path = path.to_owned();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(std::fs::read_to_string(path));
    });
    let Ok(Ok(text)) = rx.recv_timeout(Duration::from_secs(2)) else {
        return vec![];
    };
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v["mcpServers"].as_object().cloned())
        .map(|servers| {
            servers
                .keys()
                .map(|name| json!({"name":name,"status":"connected"}))
                .collect()
        })
        .unwrap_or_default()
}

/// The folder of the program as it was started: `argv[0]`, because a link must not resolve.
fn script_folder(argv0: &Path) -> PathBuf {
    let own = if argv0.components().count() > 1 {
        argv0.to_path_buf()
    } else {
        std::env::current_exe().unwrap_or_else(|_| argv0.to_path_buf())
    };
    own.parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn main() {
    let mut raw = std::env::args();
    let argv0 = PathBuf::from(raw.next().unwrap_or_default());
    let args: Vec<String> = raw.collect();
    let folder = script_folder(&argv0);
    let script: Value = std::fs::read(folder.join("fake-claude.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(json!({}));
    let version = script["version"].as_str().unwrap_or("2.1.295").to_owned();
    if args.len() == 1 && args[0] == "--version" {
        println!("{version} (Claude Code)");
        return;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(folder.join("fake-claude.log"))
        .ok();
    if let Ok(mut guard) = LOG.lock() {
        *guard = file;
    }
    log(&json!({"args": args}));
    for (name, value) in std::env::vars_os() {
        log(&json!({"env": {name.to_string_lossy(): value.to_string_lossy()}}));
    }
    check_arguments(&args);

    let mut input = BufReader::new(std::io::stdin());
    let mut first = String::new();
    if input.read_line(&mut first).unwrap_or(0) == 0 {
        die(70, "stdin closed before initialize");
    }
    log(&json!({"stdin": first.trim_end()}));
    let request: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    if request["type"] != "control_request" || request["request"]["subtype"] != "initialize" {
        die(
            70,
            &format!("the first line is not initialize: {}", first.trim_end()),
        );
    }
    let hooks = request["request"]["hooks"]
        .as_object()
        .is_some_and(|h| !h.is_empty());
    let mut answer = Map::new();
    answer.insert("hook_on_failure_supported".into(), json!(true));
    if hooks {
        answer.insert("hooks_applied".into(), json!(true));
    }
    for (key, value) in script["initialize"].as_object().into_iter().flatten() {
        if value.is_null() {
            answer.remove(key);
        } else {
            answer.insert(key.clone(), value.clone());
        }
    }
    emit(
        &json!({"type":"control_response","response":{"subtype":"success",
        "request_id":request["request_id"],"response":answer}}),
    );
    emit(&init_line(&args, &version, &script["init"]));

    let callback = request["request"]["hooks"]["PreToolUse"][0]["hookCallbackIds"][0]
        .as_str()
        .unwrap_or("fake-hook")
        .to_owned();
    let mut fake = Fake {
        rx: spawn_reader(input),
        queue: VecDeque::new(),
        counter: 0,
        requests: HashMap::new(),
        callback,
        skip: false,
        later_turn: false,
    };
    let steps = script["steps"].as_array().cloned().unwrap_or_default();
    let is_turn =
        |s: &Value| ["expect-user", "reply-per-say"].contains(&s["do"].as_str().unwrap_or(""));
    for (i, step) in steps.iter().enumerate() {
        if fake.skip {
            if !is_turn(step) {
                continue;
            }
            fake.skip = false;
        }
        fake.later_turn = steps[i + 1..].iter().any(is_turn);
        fake.run(step);
    }
    // Steps over: read on until the host closes stdin, so that a stray line is caught.
    while fake.next_unawaited().is_some() {}
}
