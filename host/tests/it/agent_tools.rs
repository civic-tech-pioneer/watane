#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Agent tools (TECHNICAL 19.5, 19.7, 10.2, 10.3; FUNCTIONAL S56, S60, S65, S68): a home with an
//! app whose fake `Confirm` holds each consent answer until the test gives it, the fake `claude`,
//! and the installed mods `toolbox` (imports `agent-tools`), `toolbox-front`, `tool`, `manager`.

use crate::streams::{call_as, fixture, local, wait};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use watane_host::{
    AccountError, AccountSource, Caller, Confirm, Consent, ConsentAnswer, ConsentRequest, Host,
    JudgeAccount, Limits, Mode, ToolsetName,
};

type Held = tokio::sync::oneshot::Sender<ConsentAnswer>;

/// The app: approves installs, names the fake `claude`, and holds each consent question.
#[derive(Default)]
struct App {
    program: Mutex<Option<PathBuf>>,
    asked: Mutex<Vec<ConsentRequest>>,
    held: Mutex<Vec<Option<Held>>>,
}
struct Asking(Arc<App>);
struct Accounts(PathBuf);
impl AccountSource for Accounts {
    fn openai_token(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, AccountError>> + Send>> {
        Box::pin(async { Err(AccountError::NoAccount) })
    }
    fn judge(&self, _: &str) -> Result<JudgeAccount, AccountError> {
        Err(AccountError::NoAccount)
    }
    fn claude_binary(&self) -> Result<PathBuf, AccountError> {
        Ok(self.0.clone())
    }
}
impl Confirm for Asking {
    fn install(
        &self,
        _: watane_host::InstallRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = watane_host::Answer> + Send>> {
        Box::pin(async { watane_host::Answer::Approve { withhold: vec![] } })
    }
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
        let program = self.0.program.lock().unwrap().clone()?;
        Some(Arc::new(Accounts(program)))
    }
    fn consent(
        &self,
        request: ConsentRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = ConsentAnswer> + Send>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0.asked.lock().unwrap().push(request);
        self.0.held.lock().unwrap().push(Some(tx));
        Box::pin(async move { rx.await.unwrap_or(ConsentAnswer::Refused) })
    }
}

const MCP_INIT: &str = r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[{"name":"watane","status":"connected"}],"agents":[],"permissionMode":"default","plugins":[{"name":"watane-run"}]}"#;
/// The tools of `tool:api`, sorted by name (10.1).
const TOOL_NAMES: [&str; 6] = [
    "tool_api_boom",
    "tool_api_double",
    "tool_api_hello",
    "tool_api_refuse",
    "tool_api_slow",
    "tool_api_who",
];

struct Home {
    dir: TempDir,
    fake: TempDir,
    app: Arc<App>,
    host: Arc<Host>,
    served: Option<watane_host::Served>,
    client: reqwest::Client,
    rt: tokio::runtime::Runtime,
}
impl Home {
    fn new() -> Self {
        let fake = tempfile::tempdir().unwrap();
        let program = fake.path().join("claude");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude/claude"),
            &program,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(fake.path().join("init"), format!("{MCP_INIT}\n")).unwrap();
        // The fake exits after its lines: the run ends `closed`.
        std::fs::write(fake.path().join("exit"), "0").unwrap();
        let app = Arc::new(App::default());
        *app.program.lock().unwrap() = Some(program);
        let dir = tempfile::tempdir().unwrap();
        let confirm: Arc<dyn Confirm> = Arc::new(Asking(Arc::clone(&app)));
        let host = Host::open(
            dir.path(),
            Mode::Home { app: Some(confirm) },
            Limits::default(),
        )
        .unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let plan = host
            .plan(
                ["toolbox", "toolbox-front", "tool", "manager"]
                    .iter()
                    .map(|n| watane_host::Source::Folder(fixture(n)))
                    .collect(),
            )
            .unwrap();
        rt.block_on(host.install(plan, watane_host::Requester::App))
            .unwrap();
        host.set_toolset(
            &ToolsetName::parse("t").unwrap(),
            vec!["tool:api".to_owned()],
        )
        .unwrap();
        host.set_toolset(
            &ToolsetName::parse("m").unwrap(),
            vec!["manager:api".to_owned()],
        )
        .unwrap();
        Self {
            dir,
            fake,
            app,
            host,
            served: None,
            client: reqwest::Client::new(),
            rt,
        }
    }
    /// A home that also serves, for `/mcp/agent`.
    fn serving() -> Self {
        let mut home = Self::new();
        let served = home
            .rt
            .block_on(home.host.serve(0, axum::Router::new()))
            .unwrap();
        home.client = reqwest::Client::builder()
            .resolve("localhost", ([127, 0, 0, 1], served.port).into())
            .build()
            .unwrap();
        home.served = Some(served);
        home
    }
    fn call(&self, target: &str, function: &str, args: Value) -> Value {
        call_as(&self.host, Caller::Page, target, function, args).unwrap()
    }
    /// `toolbox.open` as the page: the handle, or the error text.
    fn open_in(&self, target: &str, toolsets: &[&str]) -> Result<String, String> {
        result(self.call(target, "api.open", json!({"toolsets": toolsets})))
    }
    fn open(&self, toolsets: &[&str]) -> Result<String, String> {
        self.open_in("toolbox", toolsets)
    }
    /// A handle of `toolbox` for `toolsets`, after the app granted the question `i`.
    fn granted(&self, toolsets: &[&str], i: usize) -> String {
        let h = self.open(toolsets).unwrap();
        self.wait_asked(i + 1);
        self.answer(i, ConsentAnswer::Granted);
        self.wait_ready(&h);
        h
    }
    fn describe(&self, h: &str) -> Result<Vec<String>, String> {
        result(self.call("toolbox", "api.describe", json!({"handle": h})))
    }
    fn wait_ready(&self, h: &str) {
        wait("a handle that is ready", Duration::from_secs(20), || {
            self.describe(h).is_ok()
        });
    }
    fn call_tool(&self, h: &str, key: &str, tool: &str, args: &str) -> Result<String, String> {
        result(self.call(
            "toolbox",
            "api.call",
            json!({"handle": h, "key": key, "tool": tool, "args": args}),
        ))
    }
    fn items(&self, s: &str) -> Vec<String> {
        serde_json::from_value(self.call("toolbox", "api.items", json!({"s": s}))).unwrap()
    }
    fn wait_items(&self, s: &str, n: usize) -> Vec<String> {
        wait("items", Duration::from_secs(20), || {
            self.items(s).len() >= n
        });
        self.items(s)
    }
    fn ended(&self) -> Vec<String> {
        serde_json::from_value(self.call("toolbox", "api.ended", Value::Null)).unwrap()
    }
    fn wait_ended(&self, line: &str) {
        let start = std::time::Instant::now();
        while !self.ended().iter().any(|l| l == line) {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "no ended line {line:?} in {:?}; log: {:?}",
                self.ended(),
                std::fs::read_to_string(self.fake.path().join("claude.log"))
                    .unwrap_or_default()
                    .lines()
                    .filter(|l| !l.starts_with("ENV"))
                    .collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn wait_asked(&self, n: usize) {
        wait("a question", Duration::from_secs(20), || {
            self.app.asked.lock().unwrap().len() >= n
        });
    }
    fn asked(&self) -> Vec<ConsentRequest> {
        self.app.asked.lock().unwrap().clone()
    }
    fn answer(&self, i: usize, answer: ConsentAnswer) {
        let tx = self.app.held.lock().unwrap()[i].take().unwrap();
        let _ = tx.send(answer);
    }
    fn consents_of(&self, local: &str) -> Value {
        let text = std::fs::read_to_string(self.dir.path().join("installed.json")).unwrap();
        let all: Value = serde_json::from_str(&text).unwrap();
        all["mods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == local)
            .unwrap()
            .get("consents")
            .cloned()
            .unwrap_or_else(|| json!([]))
    }
    /// Starts a `claude` with the handle through `toolbox`; its stream id.
    fn launch(&self, h: &str) -> String {
        let launch = json!({"version": 1, "workspace": "scratch", "agent-handle": h});
        self.call(
            "toolbox",
            "api.start",
            json!({"launch": launch.to_string()}),
        )
        .as_str()
        .unwrap()
        .to_owned()
    }
    /// The MCP config that the fake `claude` read on fd 3 (the file of the last launch).
    fn mcp_config(&self) -> Value {
        let file = self.fake.path().join("mcp.json");
        wait("the MCP config", Duration::from_secs(20), || file.exists());
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap()
    }
    /// The bearer token of a handle: a launch, and what `claude` read on fd 3.
    fn token_of(&self, h: &str) -> String {
        let _ = std::fs::remove_file(self.fake.path().join("mcp.json"));
        let s = self.launch(h);
        assert_eq!(s.len(), 32, "no stream id: {s}");
        let config = self.mcp_config();
        let header = config["mcpServers"]["watane"]["headers"]["Authorization"]
            .as_str()
            .unwrap();
        header.strip_prefix("Bearer ").unwrap().to_owned()
    }
    fn port(&self) -> u16 {
        self.served.as_ref().unwrap().port
    }
    /// One JSON-RPC request to `/mcp/<name>`: the status and the body.
    fn mcp(&self, name: &str, token: &str, method: &str, params: Value) -> (u16, Value) {
        let url = format!("http://localhost:{}/mcp/{name}", self.port());
        let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
        self.rt.block_on(async {
            let r = self
                .client
                .post(url)
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap();
            let status = r.status().as_u16();
            (status, r.json().await.unwrap_or(Value::Null))
        })
    }
    fn mcp_names(&self, token: &str) -> Vec<String> {
        let (status, body) = self.mcp("agent", token, "tools/list", json!({}));
        assert_eq!(status, 200, "{body}");
        body["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect()
    }
}
/// A `result<T, string>` of a fixture, as `Ok(T)` or `Err(text)`.
fn result<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    match (v.get("ok"), v.get("err")) {
        (Some(ok), None) => Ok(serde_json::from_value(ok.clone()).unwrap()),
        (None, Some(err)) => Err(err.as_str().unwrap().to_owned()),
        _ => panic!("not a result: {v}"),
    }
}
fn is_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn names(r: Result<Vec<String>, String>) -> Vec<String> {
    r.unwrap()
        .iter()
        .map(|l| l.split('|').next().unwrap().to_owned())
        .collect()
}

// ---- AC1 -----------------------------------------------------------------------------------

#[test]
fn ac1_open_with_no_toolset_or_one_that_toolsets_json_lacks_is_not_granted() {
    let home = Home::new();
    assert_eq!(home.open(&[]).unwrap_err(), "not-granted");
    assert_eq!(home.open(&["zzz"]).unwrap_err(), "not-granted");
    assert_eq!(home.open(&["t", "zzz"]).unwrap_err(), "not-granted");
    assert_eq!(home.open(&["Not A Name"]).unwrap_err(), "not-granted");
    assert!(home.asked().is_empty());
}

/// The first answer of `describe` that is not `consent-pending`: the one that reads the refusal.
fn first_after_pending(home: &Home, h: &str, use_call: bool) -> String {
    let start = std::time::Instant::now();
    loop {
        let e = if use_call {
            home.call_tool(h, "k", "tool_api_hello", r#"{"name":"x"}"#)
                .unwrap_err()
        } else {
            home.describe(h).unwrap_err()
        };
        if e != "consent-pending:toolset:t" {
            return e;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "still pending");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn ac1_open_returns_a_handle_at_once_asks_once_and_a_refusal_closes_the_handle() {
    let home = Home::new();
    let h = home.open(&["t"]).unwrap();
    assert!(is_hex(&h, 32), "{h}");
    home.wait_asked(1);
    assert_eq!(
        home.asked(),
        vec![ConsentRequest {
            consent: Consent::Toolset {
                toolset: "t".into(),
                functions: vec![]
            },
            functions: TOOL_NAMES.map(str::to_owned).to_vec(),
            via: local("toolbox"),
            origin: local("toolbox"),
            profile: None,
        }]
    );
    // While the question waits, both fail `consent-pending`.
    assert_eq!(home.describe(&h).unwrap_err(), "consent-pending:toolset:t");
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_hello", r#"{"name":"x"}"#)
            .unwrap_err(),
        "consent-pending:toolset:t"
    );
    assert_eq!(home.asked().len(), 1);
    home.answer(0, ConsentAnswer::Refused);
    // The first use after the refusal reads it; then the handle is closed.
    assert_eq!(
        first_after_pending(&home, &h, false),
        "consent-refused:toolset:t"
    );
    assert_eq!(home.consents_of("toolbox"), json!([]));
    assert_eq!(home.describe(&h).unwrap_err(), "unknown-handle");
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_hello", "{}").unwrap_err(),
        "unknown-handle"
    );
}

#[test]
fn ac1_a_call_reads_the_refusal_too() {
    let home = Home::new();
    let h = home.open(&["t"]).unwrap();
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Refused);
    assert_eq!(
        first_after_pending(&home, &h, true),
        "consent-refused:toolset:t"
    );
    assert_eq!(home.describe(&h).unwrap_err(), "unknown-handle");
}

#[test]
fn ac1_granted_stores_the_consent_on_the_origin_client_and_a_second_open_asks_nothing() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":TOOL_NAMES}])
    );
    assert_eq!(names(home.describe(&h)), TOOL_NAMES);
    // A second open of the same toolset asks nothing and is ready at once.
    let h2 = home.open(&["t"]).unwrap();
    assert_ne!(h, h2);
    assert_eq!(names(home.describe(&h2)), TOOL_NAMES);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(home.asked().len(), 1);
}

#[test]
fn ac1_the_request_names_the_opening_mod_as_via_and_the_origin_client() {
    let home = Home::new();
    let h = result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]})))
        .unwrap();
    home.wait_asked(1);
    let asked = home.asked();
    assert_eq!(asked[0].via, local("toolbox"));
    assert_eq!(asked[0].origin, local("toolbox-front"));
    home.answer(0, ConsentAnswer::Granted);
    // The consent is the origin client's, not the opening mod's.
    // The handle is ready once the consent is stored (the asking thread then marks it).
    wait("the front's handle", Duration::from_secs(20), || {
        result::<Vec<String>>(home.call("toolbox-front", "api.describe", json!({"handle": h})))
            .is_ok()
    });
    assert_ne!(home.consents_of("toolbox-front"), json!([]));
    assert_eq!(home.consents_of("toolbox"), json!([]));
    // The handle is the origin client's: the front reaches it, `toolbox` as page does not.
    let described =
        result::<Vec<String>>(home.call("toolbox-front", "api.describe", json!({"handle": h})))
            .unwrap();
    assert_eq!(described.len(), 6);
    assert_eq!(home.describe(&h).unwrap_err(), "unknown-handle");
}

// ---- AC2 -----------------------------------------------------------------------------------

#[test]
fn ac2_describe_carries_the_annotations_the_description_and_the_schema() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    let none = json!({"type":"object","properties":{},"required":[],"additionalProperties":false});
    let uint = json!({"type":"integer","minimum":0,"maximum":4294967295u32});
    let one = |param: &str, schema: &Value| json!({"type":"object","properties":{param:schema},"required":[param],"additionalProperties":false});
    let expected: [(&str, bool, bool, &str, Value); 6] = [
        ("tool_api_boom", false, false, "boom: func()", none.clone()),
        (
            "tool_api_double",
            false,
            true,
            "Doubles a number.\n@idempotent",
            one("n", &uint),
        ),
        (
            "tool_api_hello",
            true,
            false,
            "Says hello.\n@parallel",
            one("name", &json!({"type":"string"})),
        ),
        (
            "tool_api_refuse",
            false,
            false,
            "refuse: func() -> result<string, string>",
            none.clone(),
        ),
        (
            "tool_api_slow",
            false,
            false,
            "Waits, then answers.",
            one("ms", &uint),
        ),
        ("tool_api_who", false, false, "who: func() -> string", none),
    ];
    let lines = home.describe(&h).unwrap();
    assert_eq!(lines.len(), 6);
    for (line, (name, parallel, idempotent, description, schema)) in lines.iter().zip(expected) {
        let fields: Vec<&str> = line.splitn(5, '|').collect();
        assert_eq!(fields[0], name);
        assert_eq!(fields[1], parallel.to_string(), "{name}");
        assert_eq!(fields[2], idempotent.to_string(), "{name}");
        assert_eq!(fields[3], description);
        assert_eq!(
            serde_json::from_str::<Value>(fields[4]).unwrap(),
            schema,
            "{name}"
        );
    }
}

#[test]
fn ac2_a_key_returns_one_stream_while_the_job_lives_and_the_item_is_the_result() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    // In one chain: the second `call` finds the first in the chain's own effects.
    let twice = serde_json::from_value::<Vec<String>>(home.call(
        "toolbox",
        "api.call-twice",
        json!({"handle": h, "key": "k0", "tool": "tool_api_hello", "args": r#"{"name":"a"}"#}),
    ))
    .unwrap();
    assert_eq!(twice[0], twice[1], "{twice:?}");
    assert!(is_hex(&twice[0], 32), "{twice:?}");
    // In two chains, while the job lives.
    let args = r#"{"ms":600}"#;
    let s1 = home.call_tool(&h, "k1", "tool_api_slow", args).unwrap();
    let s2 = home.call_tool(&h, "k1", "tool_api_slow", args).unwrap();
    assert_eq!(s1, s2);
    assert!(is_hex(&s1, 32));
    let other = home.call_tool(&h, "k2", "tool_api_slow", args).unwrap();
    assert_ne!(s1, other);
    assert_eq!(home.wait_items(&s1, 1), vec![r#"{"ok":"slept"}"#]);
    home.wait_ended(&format!("{s1} closed"));
    assert_eq!(home.items(&s1), vec![r#"{"ok":"slept"}"#]);
    // Once the job is over, the key starts a new one.
    let again = home
        .call_tool(&h, "k1", "tool_api_slow", r#"{"ms":1}"#)
        .unwrap();
    assert_ne!(again, s1);
    assert_eq!(home.wait_items(&again, 1), vec![r#"{"ok":"slept"}"#]);
    assert_eq!(home.wait_items(&twice[0], 1), vec![r#"{"ok":"hello a"}"#]);
}

#[test]
fn ac2_a_tool_that_fails_or_returns_err_is_an_item_and_the_caller_is_an_agent_name() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    let s = home.call_tool(&h, "e", "tool_api_refuse", "{}").unwrap();
    assert_eq!(home.wait_items(&s, 1), vec![r#"{"ok":{"err":"refused"}}"#]);
    let s = home.call_tool(&h, "b", "tool_api_boom", "{}").unwrap();
    let item: Value = serde_json::from_str(&home.wait_items(&s, 1)[0]).unwrap();
    assert_eq!(item["err"]["code"], "trap", "{item}");
    assert!(item["err"]["message"].is_string());
    let s = home.call_tool(&h, "w", "tool_api_who", "{}").unwrap();
    let item: Value = serde_json::from_str(&home.wait_items(&s, 1)[0]).unwrap();
    let who = item["ok"].as_str().unwrap();
    let name = who.strip_prefix("agent:h").unwrap();
    assert!(is_hex(name, 30), "{who}");
    assert!(!h.contains(name), "the handle is not the name: {h} {who}");
    // Arguments that are not JSON are an item too, and no call runs.
    let s = home
        .call_tool(&h, "j", "tool_api_hello", "not json")
        .unwrap();
    let item: Value = serde_json::from_str(&home.wait_items(&s, 1)[0]).unwrap();
    assert_eq!(item["err"]["code"], "bad_request", "{item}");
}

#[test]
fn ac2_unknown_foreign_and_same_chain_handles_and_unknown_tools() {
    let home = Home::new();
    assert_eq!(
        home.describe(&"a".repeat(32)).unwrap_err(),
        "unknown-handle"
    );
    assert_eq!(
        home.call_tool(&"a".repeat(32), "k", "tool_api_who", "{}")
            .unwrap_err(),
        "unknown-handle"
    );
    let h = home.granted(&["t"], 0);
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_nope", "{}").unwrap_err(),
        "unknown-tool"
    );
    // A handle of another origin client is foreign: the front calls `toolbox` with itself as origin.
    let foreign =
        result::<Vec<String>>(home.call("toolbox-front", "api.describe", json!({"handle": h})));
    assert_eq!(foreign.unwrap_err(), "unknown-handle");
    // A handle opened in the same chain is not applied yet.
    assert_eq!(
        home.call("toolbox", "api.open-describe", json!({"toolsets": ["t"]})),
        json!("unknown-handle")
    );
    // `close` acts at once.
    home.call("toolbox", "api.close", json!({"handle": h}));
    assert_eq!(home.describe(&h).unwrap_err(), "unknown-handle");
}
// ---- AC3 -----------------------------------------------------------------------------------

#[test]
fn ac3_a_manager_function_is_listed_when_the_stored_consent_names_it() {
    let home = Home::new();
    // `manager` holds `Manage` (installed with its grants): its functions are in the question,
    // and a handle lists them because the stored consent names them.
    let h = home.granted(&["m"], 0);
    let asked = home.asked();
    assert!(
        asked[0]
            .functions
            .contains(&"manager_api_disable".to_owned())
    );
    assert!(asked[0].functions.contains(&"manager_api_who".to_owned()));
    let listed = names(home.describe(&h));
    assert_eq!(listed, asked[0].functions);
    let stored = home.consents_of("toolbox");
    assert_eq!(stored[0]["toolset"], "m");
    assert_eq!(stored[0]["functions"], json!(asked[0].functions));
}

// ---- AC4 -----------------------------------------------------------------------------------

#[test]
fn ac4_a_gained_function_is_left_out_until_a_new_consent_with_the_whole_list() {
    let home = Home::serving();
    let h = home.granted(&["t"], 0);
    let token = home.token_of(&h);
    assert_eq!(home.mcp_names(&token), TOOL_NAMES);
    // `tool` is updated: `api` gains `extra`.
    let plan = home
        .host
        .plan(vec![watane_host::Source::Folder(fixture("tool-plus"))])
        .unwrap();
    home.rt
        .block_on(home.host.install(plan, watane_host::Requester::App))
        .unwrap();
    assert_eq!(names(home.describe(&h)), TOOL_NAMES);
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_extra", "{}").unwrap_err(),
        "unknown-tool"
    );
    assert_eq!(home.mcp_names(&token), TOOL_NAMES);
    let (_, body) = home.mcp(
        "agent",
        &token,
        "tools/call",
        json!({"name": "tool_api_extra", "arguments": {}}),
    );
    assert_eq!(body["error"]["code"], -32602, "{body}");
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":TOOL_NAMES}])
    );
    // A new open asks with the whole current list; after `Granted` both are listed.
    let all = [
        "tool_api_boom",
        "tool_api_double",
        "tool_api_extra",
        "tool_api_hello",
        "tool_api_refuse",
        "tool_api_slow",
        "tool_api_who",
    ];
    let h2 = home.granted(&["t"], 1);
    assert_eq!(home.asked()[1].functions, all);
    assert_eq!(names(home.describe(&h2)), all);
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":all}])
    );
    assert_eq!(home.mcp_names(&token), all);
}

#[test]
fn ac4_remove_of_the_tool_mod_leaves_an_empty_list_and_the_names_are_asked_again() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    home.host.remove(&local("tool"), false).unwrap();
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":[]}])
    );
    // The same names come back with a mod installed under that local name.
    let plan = home
        .host
        .plan(vec![watane_host::Source::Folder(fixture("tool"))])
        .unwrap();
    home.rt
        .block_on(home.host.install(plan, watane_host::Requester::App))
        .unwrap();
    assert_eq!(names(home.describe(&h)), Vec::<String>::new());
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_hello", r#"{"name":"x"}"#)
            .unwrap_err(),
        "unknown-tool"
    );
    let h2 = home.open(&["t"]).unwrap();
    home.wait_asked(2);
    assert_eq!(home.asked()[1].functions, TOOL_NAMES);
    home.answer(1, ConsentAnswer::Granted);
    home.wait_ready(&h2);
    assert_eq!(names(home.describe(&h2)), TOOL_NAMES);
}

#[test]
fn ac4_a_grant_of_manage_leaves_an_empty_list_until_a_new_consent() {
    let home = Home::new();
    let manage = watane_host::watane_pkg::Grant::Manage;
    home.host.revoke(&local("manager"), &manage).unwrap();
    let h = home.granted(&["m"], 0);
    let asked = home.asked()[0].functions.clone();
    assert!(!asked.is_empty());
    assert_eq!(names(home.describe(&h)), asked);
    // The participant grants `Manage` (no update): the stored names go.
    home.host.grant(&local("manager"), &manage).unwrap();
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"m","functions":[]}])
    );
    assert_eq!(names(home.describe(&h)), Vec::<String>::new());
    assert_eq!(
        home.call_tool(&h, "k", "manager_api_who", "{}")
            .unwrap_err(),
        "unknown-tool"
    );
    // A grant of what was already granted again is no change to another mod's names.
    let again = home.open(&["m"]).unwrap();
    home.wait_asked(2);
    assert_eq!(home.asked()[1].functions, asked);
    home.answer(1, ConsentAnswer::Granted);
    home.wait_ready(&again);
}

// ---- AC5 -----------------------------------------------------------------------------------

#[test]
fn ac5_a_launch_with_a_handle_gets_the_mcp_config_on_fd_3_and_the_token_works_on_mcp_agent() {
    let home = Home::serving();
    let h = home.granted(&["t"], 0);
    let _ = std::fs::remove_file(home.fake.path().join("mcp.json"));
    let s = home.launch(&h);
    assert_eq!(s.len(), 32, "{s}");
    let config = home.mcp_config();
    let url = format!("http://localhost:{}/mcp/agent", home.port());
    assert_eq!(config["mcpServers"]["watane"]["type"], "http");
    assert_eq!(config["mcpServers"]["watane"]["url"], url);
    let header = config["mcpServers"]["watane"]["headers"]["Authorization"]
        .as_str()
        .unwrap()
        .to_owned();
    let token = header.strip_prefix("Bearer wt_").unwrap();
    assert_eq!(token.len(), 43, "{header}");
    let token = format!("wt_{token}");
    // `claude` got the flag, and the stream ended with the run.
    home.wait_ended(&format!("{s} closed"));
    let log = std::fs::read_to_string(home.fake.path().join("claude.log")).unwrap();
    assert!(log.contains("ARG --mcp-config=/dev/fd/3\n"), "{log}");
    // The token lists the handle's tools, and calls one as the agent.
    assert_eq!(home.mcp_names(&token), TOOL_NAMES);
    let (status, body) = home.mcp(
        "agent",
        &token,
        "tools/call",
        json!({"name": "tool_api_hello", "arguments": {"name": "mcp"}}),
    );
    assert_eq!(status, 200);
    assert_eq!(
        body["result"],
        json!({"content": [{"type": "text", "text": "hello mcp"}], "isError": false})
    );
    let (_, body) = home.mcp(
        "agent",
        &token,
        "tools/call",
        json!({"name": "tool_api_who", "arguments": {}}),
    );
    let who = body["result"]["content"][0]["text"].as_str().unwrap();
    assert!(who.starts_with("agent:h") && who.len() == 7 + 30, "{who}");
    assert!(!who.contains(&h));
    // The token is for `/mcp/agent` only.
    assert_eq!(home.mcp("t", &token, "tools/list", json!({})).0, 403);
    // It is in no item, no argument and no file but the pipe's copy.
    assert!(!home.items(&s).iter().any(|i| i.contains(&token)));
    assert!(!log.contains(&token));
    assert!(!tree_holds(home.dir.path(), &token));
    // `close` ends the token.
    home.call("toolbox", "api.close", json!({"handle": h}));
    assert_eq!(home.mcp("agent", &token, "tools/list", json!({})).0, 401);
}
#[test]
fn ac5_a_launch_whose_handle_waits_says_consent_pending_once_then_runs_or_ends_refused() {
    let home = Home::serving();
    let pending = r#"{"type":"consent-pending","what":"toolset:t"}"#;
    let h = home.open(&["t"]).unwrap();
    home.wait_asked(1);
    let s = home.launch(&h);
    assert_eq!(home.wait_items(&s, 1), vec![pending]);
    std::thread::sleep(Duration::from_millis(200));
    // No `claude` while the consent waits, and the item is said once.
    assert!(!home.fake.path().join("claude.log").exists());
    assert_eq!(home.items(&s), vec![pending]);
    home.answer(0, ConsentAnswer::Granted);
    assert!(home.mcp_config()["mcpServers"]["watane"]["url"].is_string());
    home.wait_ended(&format!("{s} closed"));
    let items = home.items(&s);
    assert_eq!(items.iter().filter(|i| *i == pending).count(), 1);
    // A refused handle: the run ends without `claude` (`consent-refused` reads `other` here).
    std::fs::remove_file(home.fake.path().join("claude.log")).unwrap();
    let h = home.open(&["m"]).unwrap();
    home.wait_asked(2);
    let s = home.launch(&h);
    home.answer(1, ConsentAnswer::Refused);
    home.wait_ended(&format!("{s} other"));
    assert!(!home.fake.path().join("claude.log").exists());
}
fn tree_holds(dir: &Path, needle: &str) -> bool {
    std::fs::read_dir(dir).unwrap().flatten().any(|e| {
        let path = e.path();
        if path.is_dir() {
            tree_holds(&path, needle)
        } else {
            std::fs::read(&path)
                .map(|b| String::from_utf8_lossy(&b).contains(needle))
                .unwrap_or(false)
        }
    })
}

#[test]
fn ac5_a_handle_of_another_origin_client_or_an_unknown_one_is_bad_launch() {
    let home = Home::new();
    // A handle of `toolbox-front` (origin `toolbox-front`), launched by `toolbox`.
    let h = result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]})))
        .unwrap();
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Granted);
    wait("the consent", Duration::from_secs(20), || {
        home.consents_of("toolbox-front") != json!([])
    });
    for handle in [h.as_str(), &"a".repeat(32), "h", ""] {
        let r = home.launch(handle);
        assert!(r.starts_with("bad-launch:agent-handle:"), "{handle}: {r}");
        assert!(!r.contains("stage"), "{r}");
    }
}

#[test]
fn ac5_the_end_of_either_mod_ends_the_token() {
    let home = Home::serving();
    // The owner and the origin client are one mod: its disable ends the token.
    let h = home.granted(&["t"], 0);
    let token = home.token_of(&h);
    assert_eq!(home.mcp_names(&token), TOOL_NAMES);
    home.host.disable(&local("toolbox")).unwrap();
    assert_eq!(home.mcp("agent", &token, "tools/list", json!({})).0, 401);
    assert!(home.host.tokens().is_empty(), "{:?}", home.host.tokens());
    home.host.enable(&local("toolbox")).unwrap();
    assert_eq!(home.describe(&h).unwrap_err(), "unknown-handle");
    // A front's handle (origin `toolbox-front`, owner `toolbox`): the end of either ends it.
    for gone in ["toolbox-front", "toolbox"] {
        let before = home.host.tokens().len();
        let _ =
            result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]})))
                .unwrap();
        assert_eq!(home.host.tokens().len(), before + 1);
        home.host.disable(&local(gone)).unwrap();
        assert_eq!(home.host.tokens().len(), before, "{gone}");
        home.host.enable(&local(gone)).unwrap();
    }
}

// ---- AC6 -----------------------------------------------------------------------------------

#[test]
fn ac6_withdrawing_a_toolset_consent_closes_the_handles_and_ends_the_live_jobs() {
    let home = Home::serving();
    let h1 = home.granted(&["t"], 0);
    let h2 = home.open(&["t"]).unwrap();
    let token = home.token_of(&h1);
    assert_eq!(home.mcp_names(&token), TOOL_NAMES);
    // A handle of another origin client keeps working.
    let front =
        result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]})))
            .unwrap();
    home.wait_asked(2);
    home.answer(1, ConsentAnswer::Granted);
    wait("the front's handle", Duration::from_secs(20), || {
        result::<Vec<String>>(home.call("toolbox-front", "api.describe", json!({"handle": front})))
            .is_ok()
    });
    let job = home
        .call_tool(&h1, "slow", "tool_api_slow", r#"{"ms":1500}"#)
        .unwrap();
    home.host
        .withdraw_consent(&local("toolbox"), &Consent::parse("toolset:t").unwrap())
        .unwrap();
    // At once: no wait between the withdrawal and what it ends.
    assert_eq!(home.describe(&h1).unwrap_err(), "unknown-handle");
    assert_eq!(home.describe(&h2).unwrap_err(), "unknown-handle");
    assert_eq!(home.mcp("agent", &token, "tools/list", json!({})).0, 401);
    assert_eq!(home.consents_of("toolbox"), json!([]));
    home.wait_ended(&format!("{job} gone"));
    assert!(home.items(&job).is_empty());
    let kept =
        result::<Vec<String>>(home.call("toolbox-front", "api.describe", json!({"handle": front})))
            .unwrap();
    assert_eq!(kept.len(), 6);
    // A consent that is not held is `bad_request`, as before.
    let e = home
        .host
        .withdraw_consent(&local("toolbox"), &Consent::parse("toolset:t").unwrap())
        .unwrap_err();
    assert_eq!(e.code(), "bad_request");
}

// ---- Harness (19.1) --------------------------------------------------------------------------

#[test]
fn harness_fakes_agent_tools_and_records_open_and_call_as_requests() {
    use watane_host::{Fakes, Harness, RequestRecord};
    let h = Harness::open(&fixture("toolbox"), Fakes::new()).unwrap();
    let out = h.call("api.open", json!({"toolsets": ["t"]})).unwrap();
    assert_eq!(out.value, json!({"ok": format!("{:032x}", 1)}));
    assert_eq!(
        out.requests,
        vec![RequestRecord {
            function: "agent-tools.open".into(),
            args: json!({"toolsets": ["t"]}),
        }]
    );
    let args = json!({"handle": "h", "key": "k", "tool": "x", "args": "{}"});
    let out = h.call("api.call", args.clone()).unwrap();
    assert_eq!(out.value, json!({"ok": "s2"}));
    assert_eq!(
        out.requests,
        vec![RequestRecord {
            function: "agent-tools.call".into(),
            args,
        }]
    );
    let out = h.call("api.describe", json!({"handle": "h"})).unwrap();
    assert_eq!((out.value, out.requests), (json!({"ok": []}), vec![]));
}

// ---- Round 1: an update that gains `Manage`, and the limits of 19.5 ---------------------------

#[test]
fn ac4_an_update_whose_approval_grants_manage_leaves_an_empty_list() {
    let home = Home::new();
    let h = home.granted(&["t"], 0);
    assert_eq!(names(home.describe(&h)), TOOL_NAMES);
    // `tool` is updated to a version that declares `Manage`; the install grants it.
    let plan = home
        .host
        .plan(vec![watane_host::Source::Folder(fixture("tool-manage"))])
        .unwrap();
    home.rt
        .block_on(home.host.install(plan, watane_host::Requester::App))
        .unwrap();
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":[]}])
    );
    assert_eq!(names(home.describe(&h)), Vec::<String>::new());
    assert_eq!(
        home.call_tool(&h, "k", "tool_api_hello", r#"{"name":"x"}"#)
            .unwrap_err(),
        "unknown-tool"
    );
}

#[test]
fn limits_the_33rd_live_handle_of_an_origin_client_and_opener_is_not_granted() {
    let home = Home::new();
    let handles: Vec<String> = (0..32).map(|_| home.open(&["t"]).unwrap()).collect();
    assert_eq!(home.open(&["t"]).unwrap_err(), "not-granted");
    // Another origin client has its own 32.
    result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]}))).unwrap();
    // Closing one frees a slot.
    home.call("toolbox", "api.close", json!({"handle": handles[0]}));
    home.open(&["t"]).unwrap();
    assert_eq!(home.open(&["t"]).unwrap_err(), "not-granted");
}

#[test]
fn limits_the_opens_a_chain_queued_count_too() {
    let home = Home::new();
    for _ in 0..30 {
        home.open(&["t"]).unwrap();
    }
    let opened: Vec<String> = serde_json::from_value(home.call(
        "toolbox",
        "api.open-many",
        json!({"toolsets": ["t"], "n": 3}),
    ))
    .unwrap();
    assert!(
        is_hex(&opened[0], 32) && is_hex(&opened[1], 32),
        "{opened:?}"
    );
    assert_eq!(opened[2], "not-granted");
    // Another opener's queued handles are not this one's: the front still opens.
    result::<String>(home.call("toolbox-front", "api.open", json!({"toolsets": ["t"]}))).unwrap();
}

#[test]
fn limits_one_question_waits_per_origin_client_and_toolset_and_one_answer_serves_all() {
    let home = Home::new();
    let (a, b) = (home.open(&["t"]).unwrap(), home.open(&["t"]).unwrap());
    home.wait_asked(1);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(home.asked().len(), 1);
    assert_eq!(home.describe(&b).unwrap_err(), "consent-pending:toolset:t");
    home.answer(0, ConsentAnswer::Granted);
    home.wait_ready(&a);
    home.wait_ready(&b);
    assert_eq!(names(home.describe(&b)), TOOL_NAMES);
    assert_eq!(home.asked().len(), 1);
}

#[test]
fn limits_one_refusal_refuses_every_handle_that_waited_on_the_question() {
    let home = Home::new();
    let (a, b) = (home.open(&["t"]).unwrap(), home.open(&["t"]).unwrap());
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Refused);
    assert_eq!(
        first_after_pending(&home, &a, false),
        "consent-refused:toolset:t"
    );
    assert_eq!(
        first_after_pending(&home, &b, false),
        "consent-refused:toolset:t"
    );
    assert_eq!(home.asked().len(), 1);
    // Neither waiter keeps a live token.
    wait("no token", Duration::from_secs(20), || {
        home.host.tokens().is_empty()
    });
    // A later open asks again: the question is over.
    let _ = home.open(&["t"]).unwrap();
    home.wait_asked(2);
}

#[test]
fn limits_a_question_lives_while_one_waiter_does_and_is_dropped_with_the_last() {
    let home = Home::new();
    let (a, b) = (home.open(&["t"]).unwrap(), home.open(&["t"]).unwrap());
    home.wait_asked(1);
    let dropped = |i: usize| {
        home.app.held.lock().unwrap()[i]
            .as_ref()
            .unwrap()
            .is_closed()
    };
    home.call("toolbox", "api.close", json!({"handle": a}));
    std::thread::sleep(Duration::from_millis(200));
    assert!(!dropped(0));
    home.call("toolbox", "api.close", json!({"handle": b}));
    wait("the dropped question", Duration::from_secs(20), || {
        dropped(0)
    });
    // The next open is not lost behind the dropped question: it asks, and its answer serves it.
    let c = home.open(&["t"]).unwrap();
    home.wait_asked(2);
    home.answer(1, ConsentAnswer::Granted);
    home.wait_ready(&c);
}

// ---- Adversary S103 ------------------------------------------------------------------------

/// 19.5 (Manager functions), 4.4: a grant of `Manage` drops the mod's names from every stored
/// list, because the name alone does not show that its mod changed. A grant that lands while
/// the toolset question waits must not let the answer store those names again: the participant
/// was shown them as the functions of a mod that was no manager.
#[test]
fn adv_s103_a_manage_grant_while_the_question_waits_is_not_undone_by_the_answer() {
    let home = Home::new();
    let manage = watane_host::watane_pkg::Grant::Manage;
    home.host.revoke(&local("manager"), &manage).unwrap();
    let h = home.open(&["m"]).unwrap();
    home.wait_asked(1);
    assert!(
        home.asked()[0]
            .functions
            .contains(&"manager_api_disable".to_owned())
    );
    // While the question waits, `manager` becomes a manager.
    home.host.grant(&local("manager"), &manage).unwrap();
    home.answer(0, ConsentAnswer::Granted);
    wait("the handle leaves pending", Duration::from_secs(20), || {
        !home
            .describe(&h)
            .is_err_and(|e| e.starts_with("consent-pending"))
    });
    let listed = home.describe(&h).map(|_| names(home.describe(&h)));
    assert!(
        !listed
            .as_ref()
            .is_ok_and(|l| l.contains(&"manager_api_disable".to_owned())),
        "a handle reaches a manager's function that no consent named as one: {listed:?}; stored {}",
        home.consents_of("toolbox")
    );
}

/// S103 fix2 AC2 (19.5): the tool mod is removed while the toolset question waits. `Granted`
/// stores an empty list (or none); the handle is ready and nothing more is asked, since the
/// toolset now has no function.
#[test]
fn ac2_s103_fix2_a_removal_while_the_question_waits_stores_nothing() {
    let home = Home::new();
    let h = home.open(&["t"]).unwrap();
    home.wait_asked(1);
    assert_eq!(home.asked()[0].functions, TOOL_NAMES);
    home.host.remove(&local("tool"), false).unwrap();
    home.answer(0, ConsentAnswer::Granted);
    home.wait_ready(&h);
    assert_eq!(names(home.describe(&h)), Vec::<String>::new());
    let stored = home.consents_of("toolbox");
    assert!(
        stored == json!([]) || stored == json!([{"kind":"toolset","toolset":"t","functions":[]}]),
        "stored {stored}"
    );
    assert_eq!(home.asked().len(), 1);
}

/// S103 fix2 (19.5): the tool mod is removed and installed again under the same local name
/// while the question of a toolset of two mods waits. `Granted` stores the other mod's names
/// only; the handle is ready with them, and the next `open` asks the whole list again.
#[test]
fn s103_fix2_a_reinstall_while_the_question_waits_keeps_only_the_other_mods_names() {
    let home = Home::new();
    home.host
        .set_toolset(
            &ToolsetName::parse("tm").unwrap(),
            vec!["tool:api".to_owned(), "manager:api".to_owned()],
        )
        .unwrap();
    let h = home.open(&["tm"]).unwrap();
    home.wait_asked(1);
    let all = home.asked()[0].functions.clone();
    let others: Vec<String> = all
        .iter()
        .filter(|f| !f.starts_with("tool_"))
        .cloned()
        .collect();
    assert!(!others.is_empty() && others.len() < all.len(), "{all:?}");
    home.host.remove(&local("tool"), false).unwrap();
    let plan = home
        .host
        .plan(vec![watane_host::Source::Folder(fixture("tool"))])
        .unwrap();
    home.rt
        .block_on(home.host.install(plan, watane_host::Requester::App))
        .unwrap();
    home.answer(0, ConsentAnswer::Granted);
    home.wait_ready(&h);
    assert_eq!(names(home.describe(&h)), others);
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"tm","functions":others}])
    );
    assert_eq!(home.asked().len(), 1);
    let again = home.open(&["tm"]).unwrap();
    home.wait_asked(2);
    assert_eq!(home.asked()[1].functions, all);
    home.answer(1, ConsentAnswer::Granted);
    home.wait_ready(&again);
    assert_eq!(names(home.describe(&again)), all);
}

/// S103 fix2 (19.5): `tool` is updated to a version granted `Manage` while the question waits.
/// `Granted` stores none of its names; the handle is ready without them.
#[test]
fn s103_fix2_an_update_that_grants_manage_while_the_question_waits_is_not_undone() {
    let home = Home::new();
    let h = home.open(&["t"]).unwrap();
    home.wait_asked(1);
    assert_eq!(home.asked()[0].functions, TOOL_NAMES);
    let plan = home
        .host
        .plan(vec![watane_host::Source::Folder(fixture("tool-manage"))])
        .unwrap();
    home.rt
        .block_on(home.host.install(plan, watane_host::Requester::App))
        .unwrap();
    home.answer(0, ConsentAnswer::Granted);
    home.wait_ready(&h);
    assert_eq!(names(home.describe(&h)), Vec::<String>::new());
    assert_eq!(
        home.consents_of("toolbox"),
        json!([{"kind":"toolset","toolset":"t","functions":[]}])
    );
}
