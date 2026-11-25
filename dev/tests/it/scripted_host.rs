//! `ScriptedResponses` behind a real host: a mod streams a scripted OpenAI turn through
//! `https-stream` with the account `openai`, and a refused request reaches no server
//! (dev TECHNICAL 18.3, 15 stage D; host TECHNICAL 19.4, 19.9). This module is the example of how
//! a test uses the pair: `Host::open` of a temp home with a `Confirm` whose `accounts()` is a fake
//! `AccountSource`, `set_test_net` with the two addresses and the authority of the fake, and host's
//! own `https` fixture (read from `../host/tests/fixtures/mods/https`, no copy) as the mod.
#![cfg(feature = "fake-claude")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use watane_dev::scripted::{Reply, ScriptedResponses, SseEvent};
use watane_host::{
    Account, AccountError, AccountSource, Caller, Confirm, ConsentAnswer, ConsentRequest, Host,
    JudgeAccount, Limits, LocalName, Mode, Source, TestNet,
};

const TOKEN: &str = "sk-watane-fake-token-7f3a91";
const RESPONSES: &str = "https://api.openai.com/v1/responses";

struct Fake;
impl AccountSource for Fake {
    fn openai_token(&self) -> Pin<Box<dyn Future<Output = Result<String, AccountError>> + Send>> {
        Box::pin(async { Ok(TOKEN.to_owned()) })
    }
    fn judge(&self, _: &str) -> Result<JudgeAccount, AccountError> {
        Err(AccountError::NoAccount)
    }
    fn claude_binary(&self) -> Result<PathBuf, AccountError> {
        Err(AccountError::NoAccount)
    }
}
struct App;
impl Confirm for App {
    fn install(
        &self,
        _: watane_host::InstallRequest,
    ) -> Pin<Box<dyn Future<Output = watane_host::Answer> + Send>> {
        Box::pin(async { watane_host::Answer::Approve { withhold: vec![] } })
    }
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
        Some(Arc::new(Fake))
    }
    fn consent(&self, _: ConsentRequest) -> Pin<Box<dyn Future<Output = ConsentAnswer> + Send>> {
        Box::pin(async { ConsentAnswer::Granted })
    }
}

struct Rig {
    dir: tempfile::TempDir,
    host: Option<Arc<Host>>,
    server: ScriptedResponses,
}
impl Rig {
    fn new() -> Self {
        let server = ScriptedResponses::start().unwrap();
        server.expect_token(TOKEN);
        let dir = tempfile::tempdir().unwrap();
        let confirm: Arc<dyn Confirm> = Arc::new(App);
        let host = Host::open(
            dir.path(),
            Mode::Home { app: Some(confirm) },
            Limits::default(),
        )
        .unwrap();
        host.set_test_net(TestNet {
            routes: vec![
                ("api.openai.com".to_owned(), server.responses_addr()),
                ("judge.test".to_owned(), server.judge_addr()),
            ],
            root_pem: server.ca_pem(),
        });
        let folder =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../host/tests/fixtures/mods/https");
        let plan = host.plan(vec![Source::Folder(folder)]).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(host.install(plan, watane_host::Requester::App))
            .unwrap();
        Self {
            dir,
            host: Some(host),
            server,
        }
    }
    fn host(&self) -> &Arc<Host> {
        self.host.as_ref().unwrap()
    }
    /// A call of the fixture `https` as the page, on a thread with a stack the host's calls need.
    fn call(&self, function: &str, args: Value) -> Value {
        let host = Arc::clone(self.host());
        let function = function.to_owned();
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || {
                host.call(
                    &Caller::Page,
                    &LocalName::parse("https").unwrap(),
                    &function,
                    args,
                )
            })
            .unwrap()
            .join()
            .unwrap()
            .unwrap()
    }
    /// `https-stream.start` by the mod: the stream id, or the fixture's text of the `start-error`.
    fn start(&self, request: &Value) -> String {
        self.call("api.start", json!({"request": request.to_string()}))
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn items(&self, s: &str) -> Vec<String> {
        serde_json::from_value(self.call("api.items", json!({"s": s}))).unwrap()
    }
    /// The remaining items after checking the first account request's consent notice.
    fn after_consent(&self, s: &str) -> Vec<String> {
        let mut items = self.items(s);
        assert_eq!(
            items.first().map(|item| json_of(item)),
            Some(json!({"type":"consent-pending","what":"account:local/https:openai"})),
            "{items:?}"
        );
        items.remove(0);
        assert!(
            items
                .iter()
                .all(|item| !item.starts_with(r#"{"type":"consent-pending""#))
        );
        items
    }
    fn ended(&self) -> Vec<String> {
        serde_json::from_value(self.call("api.ended", Value::Null)).unwrap()
    }
    /// Waits for the end of `s` and returns its outcome.
    fn outcome(&self, s: &str) -> String {
        let prefix = format!("{s} ");
        let start = Instant::now();
        loop {
            if let Some(line) = self.ended().into_iter().find(|l| l.starts_with(&prefix)) {
                return line[prefix.len()..].to_owned();
            }
            assert!(start.elapsed() < Duration::from_secs(20), "{s} did not end");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn wait(&self, what: &str, check: impl Fn() -> bool) {
        let start = Instant::now();
        while !check() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "timed out waiting for {what}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn account_post(body: &str) -> Value {
    json!({"url": RESPONSES, "method": "POST", "account": "openai", "body": body})
}
fn json_of(item: &str) -> Value {
    serde_json::from_str(item).unwrap()
}
fn header<'a>(r: &'a watane_dev::scripted::Recorded, name: &str) -> Vec<&'a str> {
    r.headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
        .collect()
}

#[test]
fn ac1_case_3_a_scripted_turn_streams_into_the_mod_with_the_accounts_token() {
    let rig = Rig::new();
    rig.server.script(vec![Reply::Sse(vec![
        SseEvent::Delta("he".into()),
        SseEvent::Delta("llo".into()),
        SseEvent::Completed {
            input_tokens: 3,
            output_tokens: 2,
        },
    ])]);
    let body = r#"{"model":"gpt-5","input":[{"role":"user","content":"hi"}],"stream":true}"#;
    let s = rig.start(&account_post(body));
    assert_eq!(rig.outcome(&s), "closed");
    let items = rig.after_consent(&s);
    assert_eq!(items.len(), 4, "{items:?}");
    let head = json_of(&items[0]);
    assert_eq!(head["status"], 200);
    assert!(
        head["headers"]["content-type"]
            .as_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let delta = |t: &str| {
        json!({"type":"response.output_text.delta","item_id":"msg_1","output_index":0,
               "content_index":0,"delta":t})
    };
    assert_eq!(json_of(&items[1]), delta("he"));
    assert_eq!(json_of(&items[2]), delta("llo"));
    assert_eq!(
        json_of(&items[3]),
        json!({"type":"response.completed","response":{"id":"resp_1","object":"response",
               "status":"completed",
               "usage":{"input_tokens":3,"output_tokens":2,"total_tokens":5}}})
    );
    let seen = rig.server.requests();
    assert_eq!(seen.len(), 1);
    let r = &seen[0];
    assert_eq!(
        (r.method.as_str(), r.path.as_str()),
        ("POST", "/v1/responses")
    );
    assert_eq!(header(r, "authorization"), [format!("Bearer {TOKEN}")]);
    assert_eq!(r.body, body.as_bytes());
    for name in [
        "cookie",
        "openai-organization",
        "openai-project",
        "chatgpt-account-id",
        "proxy-authorization",
    ] {
        assert!(header(r, name).is_empty(), "{name}");
    }
    assert_eq!(header(r, "host"), ["api.openai.com"]);
    assert_eq!(rig.server.judge().count(), 0);
}

#[test]
fn ac2_case_19_models_with_the_account_streams_the_models_answer() {
    let rig = Rig::new();
    rig.server.models(&["gpt-5", "gpt-5-mini"]);
    let s = rig.start(
        &json!({"url": "https://api.openai.com/v1/models", "method": "GET",
                              "account": "openai"}),
    );
    assert_eq!(rig.outcome(&s), "closed");
    let items = rig.after_consent(&s);
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(json_of(&items[0])["status"], 200);
    let model = |id: &str| json!({"id": id, "object": "model", "created": 0, "owned_by": "openai"});
    assert_eq!(
        json_of(&items[1]),
        json!({"object": "list", "data": [model("gpt-5"), model("gpt-5-mini")]})
    );
    let seen = rig.server.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].path.as_str()),
        ("GET", "/v1/models")
    );
    assert_eq!(
        header(&seen[0], "authorization"),
        [format!("Bearer {TOKEN}")]
    );
}

#[test]
fn ac2_case_19_every_refused_form_reaches_no_server() {
    let rig = Rig::new();
    let post = |url: &str| json!({"url": url, "method": "POST", "account": "openai", "body": "{}"});
    let cases = [
        (post("https://api.openai.com/v1/other"), "destination"),
        (
            post("https://api.openai.com/v1/responses?x=1"),
            "destination",
        ),
        (
            json!({"url": RESPONSES, "method": "GET", "account": "openai"}),
            "destination",
        ),
        (post("https://second.test/v1/responses"), "destination"),
        (post("https://judge.test/v1/responses"), "destination"),
        (post("http://api.openai.com/v1/responses"), "destination"),
        (
            json!({"url": RESPONSES, "method": "POST", "account": "judge", "body": "{}"}),
            "destination",
        ),
        (
            json!({"url": RESPONSES, "method": "POST", "account": "openai",
                   "headers": {"Cookie": "a=b"}}),
            "forbidden-header:Cookie",
        ),
        (
            json!({"url": RESPONSES, "method": "POST", "account": "openai",
                   "headers": {"authorization": "Bearer mine"}}),
            "forbidden-header:authorization",
        ),
        (
            json!({"url": RESPONSES, "method": "POST", "account": "openai",
                   "headers": {":authority": "judge.test"}}),
            "forbidden-header::authority",
        ),
    ];
    for (request, error) in cases {
        assert_eq!(rig.start(&request), error, "{request}");
    }
    assert!(rig.server.requests().is_empty());
    assert_eq!(rig.server.judge().count(), 0);
    assert!(rig.ended().is_empty());
}

/// Starts the case's one request with `reply` scripted and returns the items and the outcome.
fn run(rig: &Rig, reply: Reply, idle_s: u64) -> (Vec<String>, String) {
    rig.server.script(vec![reply]);
    let mut request = account_post("{}");
    request["idle-s"] = json!(idle_s);
    let s = rig.start(&request);
    let outcome = rig.outcome(&s);
    (rig.after_consent(&s), outcome)
}

#[test]
fn ac2_case_19_a_redirect_is_an_item_and_is_not_followed() {
    let rig = Rig::new();
    let (items, outcome) = run(&rig, Reply::Redirect("https://judge.test/x".into()), 30);
    assert_eq!(outcome, "closed");
    assert_eq!(items.len(), 1, "{items:?}");
    let head = json_of(&items[0]);
    assert_eq!(head["status"], 307);
    assert_eq!(head["headers"]["location"], "https://judge.test/x");
    assert_eq!(rig.server.requests().len(), 1);
    assert_eq!(rig.server.judge().count(), 0);
}

#[test]
fn ac2_case_19_a_usage_limit_and_a_server_error_arrive_as_items_and_the_stream_closes() {
    let rig = Rig::new();
    let (items, outcome) = run(&rig, Reply::UsageLimit { resets_in_s: 60 }, 30);
    assert_eq!(outcome, "closed");
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(json_of(&items[0])["status"], 429);
    assert_eq!(
        json_of(&items[1]),
        json!({"error": {"type": "usage_limit_reached", "code": "usage_limit_reached",
               "message": "The usage limit has been reached", "plan_type": "plus",
               "resets_in_seconds": 60}})
    );

    let rig = Rig::new();
    let (items, outcome) = run(
        &rig,
        Reply::Status {
            code: 500,
            body: r#"{"error":"boom"}"#.into(),
        },
        30,
    );
    assert_eq!(outcome, "closed");
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(json_of(&items[0])["status"], 500);
    assert_eq!(items[1], r#"{"error":"boom"}"#);
    assert_eq!(rig.server.requests().len(), 1, "no retry by the host");
}

#[test]
fn ac2_case_19_stall_ends_idle_timeout_cut_ends_closed_and_reset_ends_failed() {
    let rig = Rig::new();
    let begun = Instant::now();
    let (items, outcome) = run(&rig, Reply::Stall, 1);
    assert_eq!(outcome, "idle-timeout");
    assert!(items.is_empty(), "{items:?}");
    assert!(begun.elapsed() < Duration::from_secs(15));

    let rig = Rig::new();
    let (items, outcome) = run(
        &rig,
        Reply::Sse(vec![SseEvent::Delta("a".into()), SseEvent::Cut]),
        30,
    );
    assert_eq!(outcome, "closed");
    assert_eq!(items.len(), 2, "{items:?}");
    assert_eq!(json_of(&items[1])["delta"], "a");

    let rig = Rig::new();
    let (items, outcome) = run(&rig, Reply::Reset, 30);
    assert!(
        outcome.starts_with("failed(HTTP: connection ended before the response head:"),
        "{outcome}"
    );
    assert!(items.is_empty(), "{items:?}");
    assert!(!outcome.contains(TOKEN));
}

#[test]
fn ac3_the_token_reaches_no_byte_the_mod_recorded_or_stored() {
    let rig = Rig::new();
    rig.server.script(vec![
        Reply::Sse(vec![SseEvent::Delta("x".into())]),
        Reply::Status {
            code: 401,
            body: "no".into(),
        },
    ]);
    let a = rig.start(&account_post("{}"));
    assert_eq!(rig.outcome(&a), "closed");
    let b = rig.start(&account_post("{}"));
    assert_eq!(rig.outcome(&b), "closed");
    let mut seen = rig.after_consent(&a).concat();
    let second = rig.items(&b);
    assert_eq!(second.len(), 2, "{second:?}");
    assert_eq!(json_of(&second[0])["status"], 401);
    assert_eq!(second[1], "no");
    seen.push_str(&second.concat());
    seen.push_str(&rig.ended().concat());
    assert!(!seen.is_empty() && !seen.contains(TOKEN), "{seen}");
    // The mod's state and every other file of the home, after the host is gone.
    let Rig { dir, host, server } = rig;
    drop(host);
    assert_eq!(server.requests().len(), 2);
    let mut files = vec![dir.path().to_owned()];
    let mut count = 0;
    while let Some(path) = files.pop() {
        if path.is_dir() {
            files.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            continue;
        }
        let start = Instant::now();
        let bytes = loop {
            // The state database may stay locked a moment after the drop.
            match std::fs::read(&path) {
                Ok(bytes) => break bytes,
                Err(e) => assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "{}: {e}",
                    path.display()
                ),
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        count += 1;
        assert!(
            !bytes.windows(TOKEN.len()).any(|w| w == TOKEN.as_bytes()),
            "{}",
            path.display()
        );
    }
    assert!(count > 0);
}

#[test]
fn ac3_a_sign_out_during_a_stall_ends_the_stream_gone() {
    let rig = Rig::new();
    rig.server.script(vec![Reply::Stall]);
    let mut request = account_post("{}");
    request["idle-s"] = json!(120);
    let s = rig.start(&request);
    rig.wait("the request at the server", || {
        !rig.server.requests().is_empty()
    });
    rig.wait("the consent notice at the mod", || {
        !rig.items(&s).is_empty()
    });
    rig.host().account_changed(&Account::OpenAi);
    assert_eq!(rig.outcome(&s), "gone");
    assert!(rig.after_consent(&s).is_empty());
}
