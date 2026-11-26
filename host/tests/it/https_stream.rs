#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Stages A4 and A5 `https-stream` (TECHNICAL 19.4, 19.5, 19.7, 19.9; FUNCTIONAL S64): the `https`
//! fixture, a TLS test server of this module that the host reaches through `set_test_net`, a fake
//! `AccountSource` and a fake `Confirm`. Oracles are literals: the bytes the server sent, the
//! items the fixture recorded, the requests the server received.

use crate::streams::{call_as, fixture, local, wait};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use watane_host::watane_pkg::Grant;
use watane_host::{
    Account, AccountError, AccountSource, Caller, Confirm, Consent, ConsentAnswer, ConsentRequest,
    Host, JudgeAccount, Limits, Mode, TestClock, TestNet,
};

// ---- the TLS test server -------------------------------------------------------------------

struct Pki {
    root_pem: String,
    config: Arc<rustls::ServerConfig>,
}
/// One root and one certificate for `api.openai.com` and `second.test`, made once.
fn pki() -> &'static Pki {
    static PKI: OnceLock<Pki> = OnceLock::new();
    PKI.get_or_init(|| make_pki(&["api.openai.com", "second.test"]))
}

#[cfg(all(unix, watane_dev_profile, debug_assertions))]
pub(super) fn scripted_preview_root() -> &'static str {
    &pki().root_pem
}

fn make_pki(names: &[&str]) -> Pki {
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    let ca_key = KeyPair::generate().unwrap();
    let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca.distinguished_name
        .push(DnType::CommonName, "watane test root");
    let ca_cert = ca.self_signed(&ca_key).unwrap();
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>(),
    )
    .unwrap()
    .signed_by(&leaf_key, &Issuer::from_params(&ca, &ca_key))
    .unwrap();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(leaf_key.serialize_der());
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![leaf.der().clone()], key.into())
    .unwrap();
    Pki {
        root_pem: ca_cert.pem(),
        config: Arc::new(config),
    }
}

/// A request as the server read it.
#[derive(Debug, Clone)]
struct Seen {
    method: String,
    target: String,
    /// Names in lower case, values as sent.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
impl Seen {
    fn header(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .collect()
    }
}
#[derive(Default)]
struct Stats {
    /// TCP connections accepted.
    connections: AtomicUsize,
    /// Connections that the client closed, as a handler saw (`until_closed`).
    closed: AtomicUsize,
    /// Events or bytes that a handler wrote, as it counts them.
    written: AtomicUsize,
    requests: Mutex<Vec<Seen>>,
}
impl Stats {
    fn requests(&self) -> Vec<Seen> {
        self.requests.lock().unwrap().clone()
    }
}
struct Conn {
    tls: tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
    stats: Arc<Stats>,
}
impl Conn {
    async fn head(&mut self, status: &str, headers: &[(&str, &str)]) {
        let mut text = format!("HTTP/1.1 {status}\r\n");
        for (n, v) in headers {
            text.push_str(&format!("{n}: {v}\r\n"));
        }
        text.push_str("\r\n");
        self.write(text.as_bytes()).await;
    }
    /// Writes, and returns whether the client was still there.
    async fn write(&mut self, bytes: &[u8]) -> bool {
        self.tls.write_all(bytes).await.is_ok() && self.tls.flush().await.is_ok()
    }
    /// Waits until the client closes the connection.
    async fn until_closed(&mut self) {
        let mut buf = [0_u8; 64];
        while matches!(self.tls.read(&mut buf).await, Ok(n) if n > 0) {}
        self.stats.closed.fetch_add(1, Ordering::SeqCst);
    }
    async fn finish(mut self) {
        let _ = self.tls.shutdown().await;
    }
}
struct Server {
    addr: SocketAddr,
    stats: Arc<Stats>,
    _rt: tokio::runtime::Runtime,
}
impl Server {
    fn start<F, Fut>(handler: F) -> Self
    where
        F: Fn(Conn) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self::start_with_pki(pki(), handler)
    }

    fn start_with_pki<F, Fut>(pki: &Pki, handler: F) -> Self
    where
        F: Fn(Conn) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let listener = rt
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let stats = Arc::new(Stats::default());
        let (acceptor, handler) = (
            tokio_rustls::TlsAcceptor::from(Arc::clone(&pki.config)),
            Arc::new(handler),
        );
        let seen_stats = Arc::clone(&stats);
        rt.spawn(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    return;
                };
                seen_stats.connections.fetch_add(1, Ordering::SeqCst);
                let (acceptor, handler, stats) = (
                    acceptor.clone(),
                    Arc::clone(&handler),
                    Arc::clone(&seen_stats),
                );
                tokio::spawn(async move {
                    let Ok(mut tls) = acceptor.accept(tcp).await else {
                        return;
                    };
                    let Some(seen) = read_request(&mut tls).await else {
                        return;
                    };
                    stats.requests.lock().unwrap().push(seen.clone());
                    handler(Conn { tls, stats }).await;
                });
            }
        });
        Self {
            addr,
            stats,
            _rt: rt,
        }
    }
}
/// Reads the head and the `content-length` body of one request.
async fn read_request(
    tls: &mut tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
) -> Option<Seen> {
    let mut buf = Vec::new();
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        let mut chunk = [0_u8; 4096];
        let n = tls.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, target) = (first.next()?.to_owned(), first.next()?.to_owned());
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(n, _)| n == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = buf[end + 4..].to_vec();
    while body.len() < length {
        let mut chunk = [0_u8; 4096];
        let n = tls.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    Some(Seen {
        method,
        target,
        headers,
        body,
    })
}

const SSE: &[(&str, &str)] = &[
    ("content-type", "text/event-stream"),
    ("cache-control", "no-cache"),
    ("x-request-id", "abc"),
    ("connection", "close"),
];
/// The first item for a response with the headers `SSE`.
const SSE_HEAD: &str = r#"{"status":200,"headers":{"content-type":"text/event-stream","cache-control":"no-cache","x-request-id":"abc","connection":"close"}}"#;

// ---- the app: accounts and consent ---------------------------------------------------------

#[derive(Default)]
enum TokenMode {
    #[default]
    Fresh,
    NoAccount,
    Unavailable,
}
/// The fake `AccountSource`: `tok-1`, `tok-2`, ..., one per call.
#[derive(Default)]
struct Tokens {
    calls: AtomicUsize,
    mode: Mutex<TokenMode>,
    program: Mutex<Option<PathBuf>>,
    judge: Mutex<Option<(String, JudgeAccount)>>,
    judge_after_read: Mutex<Option<JudgeAccount>>,
}
impl AccountSource for Tokens {
    fn openai_token(&self) -> Pin<Box<dyn Future<Output = Result<String, AccountError>> + Send>> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let answer = match *self.mode.lock().unwrap() {
            TokenMode::Fresh => Ok(format!("tok-{n}")),
            TokenMode::NoAccount => Err(AccountError::NoAccount),
            TokenMode::Unavailable => Err(AccountError::Unavailable("offline".into())),
        };
        Box::pin(async move { answer })
    }
    fn judge(&self, provider: &str) -> Result<JudgeAccount, AccountError> {
        let mut entry = self.judge.lock().unwrap();
        let (_, account) = entry
            .as_mut()
            .filter(|(stored_provider, _)| stored_provider == provider)
            .ok_or(AccountError::NoAccount)?;
        if let Some(replacement) = self.judge_after_read.lock().unwrap().take() {
            return Ok(std::mem::replace(account, replacement));
        }
        Ok(JudgeAccount {
            key: account.key.clone(),
            origin: account.origin.clone(),
        })
    }
    fn claude_binary(&self) -> Result<PathBuf, AccountError> {
        self.program
            .lock()
            .unwrap()
            .clone()
            .ok_or(AccountError::NoAccount)
    }
}
type Held = tokio::sync::oneshot::Sender<ConsentAnswer>;
struct App {
    /// `accounts()`: none for a home with an app that has no accounts.
    tokens: Mutex<Option<Arc<Tokens>>>,
    /// `Some`: every question is answered at once. `None`: the test answers.
    auto: Mutex<Option<ConsentAnswer>>,
    asked: Mutex<Vec<ConsentRequest>>,
    held: Mutex<Vec<Option<Held>>>,
}
struct Asking(Arc<App>);
impl Confirm for Asking {
    fn install(
        &self,
        _: watane_host::InstallRequest,
    ) -> Pin<Box<dyn Future<Output = watane_host::Answer> + Send>> {
        Box::pin(async { watane_host::Answer::Approve { withhold: vec![] } })
    }
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
        let tokens = self.0.tokens.lock().unwrap().clone()?;
        Some(tokens)
    }
    fn consent(
        &self,
        request: ConsentRequest,
    ) -> Pin<Box<dyn Future<Output = ConsentAnswer> + Send>> {
        if let Some(answer) = *self.0.auto.lock().unwrap() {
            self.0.asked.lock().unwrap().push(request);
            return Box::pin(async move { answer });
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        // `held` before `asked`: a test that sees a question may answer it at once (S118).
        self.0.held.lock().unwrap().push(Some(tx));
        self.0.asked.lock().unwrap().push(request);
        Box::pin(async move { rx.await.unwrap_or(ConsentAnswer::Refused) })
    }
}

// ---- the rig -------------------------------------------------------------------------------

struct Rig {
    dir: TempDir,
    host: Option<Arc<Host>>,
    app: Arc<App>,
    tokens: Arc<Tokens>,
    /// Kept alive for the host's async work.
    rt: tokio::runtime::Runtime,
    servers: Vec<(String, Server)>,
}
fn hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
fn grant(host: &str) -> Grant {
    Grant::Http {
        host: host.parse().unwrap(),
    }
}
impl Rig {
    /// A home with an app that has accounts. `auto` answers every consent question at once.
    fn new(auto: Option<ConsentAnswer>) -> Self {
        let tokens = Arc::new(Tokens::default());
        let app = Arc::new(App {
            tokens: Mutex::new(Some(Arc::clone(&tokens))),
            auto: Mutex::new(auto),
            asked: Mutex::default(),
            held: Mutex::default(),
        });
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
                ["https", "toolbox"]
                    .iter()
                    .map(|n| watane_host::Source::Folder(fixture(n)))
                    .collect(),
            )
            .unwrap();
        rt.block_on(host.install(plan, watane_host::Requester::App))
            .unwrap();
        Self {
            dir,
            host: Some(host),
            app,
            tokens,
            rt,
            servers: vec![],
        }
    }
    fn host(&self) -> &Arc<Host> {
        self.host.as_ref().unwrap()
    }
    /// Starts a server for `name` and routes the name to it (19.9).
    fn serve<F, Fut>(&mut self, name: &str, handler: F) -> Arc<Stats>
    where
        F: Fn(Conn) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let server = Server::start(handler);
        let stats = Arc::clone(&server.stats);
        self.servers.push((name.to_owned(), server));
        self.host().set_test_net(TestNet {
            routes: self
                .servers
                .iter()
                .map(|(n, s)| (n.clone(), s.addr))
                .collect(),
            root_pem: pki().root_pem.clone(),
        });
        stats
    }
    fn call(&self, caller: Caller, target: &str, function: &str, args: Value) -> Value {
        call_as(self.host(), caller, target, function, args).unwrap()
    }
    /// `https.start` as the page: the stream id, or the error text of the fixture.
    fn start(&self, request: &Value) -> String {
        self.start_as(Caller::Page, request)
    }
    fn start_as(&self, caller: Caller, request: &Value) -> String {
        self.call(
            caller,
            "https",
            "api.start",
            json!({"request": request.to_string()}),
        )
        .as_str()
        .unwrap()
        .to_owned()
    }
    /// A stream whose origin client is `toolbox`, with `https` as `via`.
    fn start_via(&self, request: &Value) -> String {
        self.start_as(Caller::Mod(local("toolbox")), request)
    }
    fn items(&self, s: &str) -> Vec<String> {
        serde_json::from_value(self.call(Caller::Page, "https", "api.items", json!({"s": s})))
            .unwrap()
    }
    fn ended(&self) -> Vec<String> {
        serde_json::from_value(self.call(Caller::Page, "https", "api.ended", Value::Null)).unwrap()
    }
    fn wait_ended_with_prefix(&self, s: &str, outcome: &str) {
        let prefix = format!("{s} {outcome}");
        wait(&prefix, Duration::from_secs(20), || {
            self.ended().iter().any(|line| line.starts_with(&prefix))
        });
    }
    fn wait_ended(&self, s: &str, outcome: &str) {
        let line = format!("{s} {outcome}");
        wait(&line, Duration::from_secs(20), || {
            self.ended().contains(&line)
        });
    }
    fn wait_items(&self, s: &str, n: usize) -> Vec<String> {
        wait("items", Duration::from_secs(20), || {
            self.items(s).len() >= n
        });
        self.items(s)
    }
    fn mode(&self, m: &str) {
        self.call(Caller::Page, "https", "api.mode", json!({"m": m}));
    }
    fn tally(&self, s: &str) -> String {
        self.call(Caller::Page, "https", "api.tally", json!({"s": s}))
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn wait_asked(&self, n: usize) {
        wait("a question", Duration::from_secs(20), || {
            self.app.asked.lock().unwrap().len() >= n
        });
    }
    fn answer(&self, i: usize, answer: ConsentAnswer) {
        let tx = self.app.held.lock().unwrap()[i].take().unwrap();
        let _ = tx.send(answer);
    }
    /// The consents of `local` in `installed.json`.
    fn consents_of(&self, local_name: &str) -> Value {
        let text = std::fs::read_to_string(self.dir.path().join("installed.json")).unwrap();
        let all: Value = serde_json::from_str(&text).unwrap();
        all["mods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == local_name)
            .unwrap()
            .get("consents")
            .cloned()
            .unwrap_or_else(|| json!([]))
    }
    /// Closes the host and reads the bytes of its state database.
    fn state_bytes(&mut self) -> Vec<u8> {
        self.host = None;
        let path = self.dir.path().join("state/state.redb");
        wait("the state database", Duration::from_secs(10), || {
            redb::Database::open(&path).is_ok()
        });
        std::fs::read(path).unwrap()
    }
}
fn get(url: &str) -> Value {
    json!({"url": url, "method": "GET"})
}
fn with(mut request: Value, key: &str, value: Value) -> Value {
    request[key] = value;
    request
}
const MODELS: &str = "https://api.openai.com/v1/models";
const RESPONSES: &str = "https://api.openai.com/v1/responses";
fn post(url: &str) -> Value {
    json!({"url": url, "method": "POST", "body": "{}"})
}

// ---- AC1: items ----------------------------------------------------------------------------

#[test]
fn ac1_a_granted_host_streams_the_head_then_one_item_per_event() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", SSE).await;
        c.write(
            b": a comment\r\ndata: first\r\n\r\ndata: second\n\nevent: x\ndata: l1\ndata: l2\n\n",
        )
        .await;
        c.finish().await;
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    assert!(hex32(&s), "{s}");
    rig.wait_ended(&s, "closed");
    assert_eq!(
        rig.items(&s),
        [SSE_HEAD, "first", "second", "l1\nl2"].map(str::to_owned)
    );
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn ac1_response_head_lowercases_and_joins_repeated_header_names() {
    let mut rig = Rig::new(None);
    rig.serve("api.openai.com", |mut c| async move {
        c.head(
            "200 OK",
            &[
                ("X-Sample", "first"),
                ("x-SAMPLE", "second"),
                ("Connection", "close"),
            ],
        )
        .await;
        c.finish().await;
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended(&s, "closed");
    let items = rig.items(&s);
    assert_eq!(items.len(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(&items[0]).unwrap(),
        json!({"status": 200, "headers": {"x-sample": "first, second", "connection": "close"}})
    );
}

#[test]
fn ac1_a_connection_closed_before_the_response_head_reports_why() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", Conn::finish);
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended_with_prefix(
        &s,
        "failed(HTTP: connection ended before the response head:",
    );
    assert!(rig.items(&s).is_empty());
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn s126_an_invalid_response_head_reports_a_parse_failure() {
    let mut rig = Rig::new(None);
    rig.serve("api.openai.com", |mut c| async move {
        c.write(b"HTTP/1.1 200 OK\r\nInvalid-Header\r\n\r\n").await;
        c.finish().await;
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended_with_prefix(&s, "failed(HTTP: invalid response head:");
    assert!(rig.items(&s).is_empty());
}

#[test]
fn ac1_an_event_over_four_mib_ends_the_stream() {
    let mut rig = Rig::new(None);
    rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", SSE).await;
        let mut event = b"data: ".to_vec();
        event.resize(event.len() + (4 << 20) + 1, b'x');
        event.extend_from_slice(b"\n\n");
        c.write(&event).await;
        c.finish().await;
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended(&s, "failed(a server-sent event is over 4 MiB)");
    assert_eq!(rig.items(&s), [SSE_HEAD]);
}

#[test]
fn ac1_a_body_that_is_not_sse_arrives_in_items_of_64_kib() {
    let mut rig = Rig::new(None);
    let body: String = (0..150 * 1024)
        .map(|i| char::from(b'a' + u8::try_from(i % 26).unwrap()))
        .collect();
    let sent = body.clone();
    rig.serve("api.openai.com", move |mut c| {
        let body = sent.clone();
        async move {
            let length = body.len().to_string();
            c.head(
                "200 OK",
                &[
                    ("content-type", "application/octet-stream"),
                    ("content-length", &length),
                ],
            )
            .await;
            c.write(body.as_bytes()).await;
            c.finish().await;
        }
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended(&s, "closed");
    let items = rig.items(&s);
    assert_eq!(
        items[0],
        r#"{"status":200,"headers":{"content-type":"application/octet-stream","content-length":"153600"}}"#
    );
    assert_eq!(
        items[1..].iter().map(String::len).collect::<Vec<_>>(),
        [65536, 65536, 22528]
    );
    assert_eq!(items[1..].concat(), body);
}

#[test]
fn ac1_a_server_that_sends_headers_then_nothing_ends_idle_timeout() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", SSE).await;
        c.until_closed().await;
    });
    let started = Instant::now();
    let s = rig.start(&with(
        get("https://api.openai.com/v1/x"),
        "idle-s",
        json!(1),
    ));
    rig.wait_ended(&s, "idle-timeout");
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(rig.items(&s), [SSE_HEAD]);
    wait("the connection closed", Duration::from_secs(10), || {
        server.closed.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn ac1_idle_s_of_0_or_over_300_is_refused_bad_launch_at_the_call() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    for idle in [json!(0), json!(301)] {
        let request = with(get("https://api.openai.com/v1/x"), "idle-s", idle);
        assert_eq!(
            rig.start(&request),
            "bad-launch:idle-s:an integer from 1 to 300"
        );
    }
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

// ---- AC2: refusals at the call ---------------------------------------------------------------

#[test]
fn ac2_every_refusal_comes_at_the_call_before_a_byte_leaves() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let ok = "https://api.openai.com/v1/x";
    let mut cases: Vec<(Value, String)> = vec![];
    for url in [
        "http://api.openai.com/v1/x",
        "https://denied.test/v1/x",
        "https://u:p@api.openai.com/v1/x",
        "https://u@api.openai.com/v1/x",
        "https://api.openai.com../v1/x",
        "https://api.openai.com:8443/v1/x",
    ] {
        cases.push((get(url), "destination".into()));
    }
    for name in [
        "Authorization",
        "cookie",
        "HOST",
        "Proxy-Authorization",
        "proxy-anything",
        "OpenAI-Organization",
        "openai-project",
        "ChatGPT-Account-Id",
        ":authority",
        "bad name",
        "bad\u{1}name",
        "Content-Length",
    ] {
        let request = with(get(ok), "headers", json!({"accept": "x", name: "v"}));
        cases.push((request, format!("forbidden-header:{name}")));
    }
    let big = with(post(ok), "body", json!("x".repeat((1 << 20) + 1)));
    cases.push((big, "too-large".into()));
    let max = with(post(ok), "body", json!("x".repeat(1 << 20)));
    assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), 0);
    for (url, method) in [
        (MODELS, "POST"),
        (RESPONSES, "GET"),
        ("https://api.openai.com/v1/other", "POST"),
        ("https://api.openai.com/v1/responses?x=1", "POST"),
        ("https://second.test/v1/responses", "POST"),
        ("http://api.openai.com/v1/responses", "POST"),
    ] {
        let request = json!({"url": url, "method": method, "account": "openai"});
        cases.push((request, "destination".into()));
    }
    for account in ["other", "OpenAI", ""] {
        let request = with(post(RESPONSES), "account", json!(account));
        cases.push((request, "destination".into()));
    }
    for (request, expected) in cases {
        assert_eq!(rig.start(&request), expected, "{request}");
    }
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
    assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), 0);
    // The largest body that is allowed leaves the host.
    let s = rig.start(&max);
    assert!(hex32(&s), "{s}");
    wait("the request", Duration::from_secs(20), || {
        server.connections.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn ac2_the_host_is_read_as_6_8_reads_it_and_the_request_leaves_with_the_plain_name() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", SSE).await;
        c.finish().await;
    });
    for url in [
        "https://API.OpenAI.com/v1/x?q=1#frag",
        "https://api.openai.com./v1/x?q=1",
        "https://api.openai.com:443/v1/x?q=1",
    ] {
        let s = rig.start(&get(url));
        assert!(hex32(&s), "{url}: {s}");
        rig.wait_ended(&s, "closed");
    }
    let seen = server.requests();
    assert_eq!(seen.len(), 3);
    for r in &seen {
        assert_eq!(r.method, "GET");
        assert_eq!(r.target, "/v1/x?q=1");
        assert_eq!(r.header("host"), ["api.openai.com"]);
        assert_eq!(r.header("accept-encoding"), ["identity"]);
    }
}

#[test]
fn ac2_a_port_grant_sends_the_port_in_the_host_header() {
    use std::fs;

    let mut rig = Rig::new(None);
    let source = fixture("https");
    let folder = tempfile::tempdir().unwrap();
    fs::create_dir_all(folder.path().join("wit/deps/watane-host")).unwrap();
    for path in [
        "logic.wasm",
        "wit/world.wit",
        "wit/deps/watane-host/host.wit",
    ] {
        fs::copy(source.join(path), folder.path().join(path)).unwrap();
    }
    fs::write(
        folder.path().join("mod.toml"),
        r#"name = "https"
summary = "HTTPS fixture with a port-specific grant."
[http]
hosts = ["api.openai.com:8443"]
"#,
    )
    .unwrap();
    let plan = rig
        .host()
        .plan(vec![watane_host::Source::Folder(
            folder.path().to_path_buf(),
        )])
        .unwrap();
    rig.rt
        .block_on(rig.host().install(plan, watane_host::Requester::App))
        .unwrap();
    let server = rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", SSE).await;
        c.finish().await;
    });
    assert_eq!(
        rig.start(&get("https://api.openai.com/v1/x")),
        "destination"
    );
    let s = rig.start(&get("https://api.openai.com:8443/v1/x"));
    rig.wait_ended(&s, "closed");
    let seen = server.requests();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].header("host"), ["api.openai.com:8443"]);
}

#[test]
fn ac2_a_trusted_certificate_for_the_wrong_name_sends_no_request() {
    let rig = Rig::new(None);
    let wrong_name = make_pki(&["second.test"]);
    let server = Server::start_with_pki(&wrong_name, Conn::finish);
    rig.host().set_test_net(TestNet {
        routes: vec![("api.openai.com".into(), server.addr)],
        root_pem: wrong_name.root_pem,
    });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended_with_prefix(&s, "failed(TLS with api.openai.com failed:");
    assert!(rig.items(&s).is_empty());
    assert_eq!(server.stats.connections.load(Ordering::SeqCst), 1);
    assert!(server.stats.requests().is_empty());
}

#[test]
fn ac2_no_redirect_is_followed() {
    let mut rig = Rig::new(None);
    let first = rig.serve("api.openai.com", |mut c| async move {
        c.head(
            "307 Temporary Redirect",
            &[
                ("location", "https://second.test/target"),
                ("content-length", "0"),
            ],
        )
        .await;
        c.finish().await;
    });
    let target = rig.serve("second.test", |c| async move { c.finish().await });
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended(&s, "closed");
    assert_eq!(
        rig.items(&s),
        [
            r#"{"status":307,"headers":{"location":"https://second.test/target","content-length":"0"}}"#
        ]
    );
    assert_eq!(first.requests().len(), 1);
    assert_eq!(target.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac2_the_request_leaves_as_the_mod_wrote_it() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |mut c| async move {
        c.head("200 OK", &[("content-length", "0")]).await;
        c.finish().await;
    });
    let request = json!({
        "url": "https://api.openai.com/v1/things", "method": "PUT", "body": "payload",
        "headers": {"Content-Type": "text/plain", "x-mod": "1"},
    });
    let s = rig.start(&request);
    rig.wait_ended(&s, "closed");
    let seen = &server.requests()[0];
    assert_eq!(
        (seen.method.as_str(), seen.target.as_str()),
        ("PUT", "/v1/things")
    );
    assert_eq!(seen.body, b"payload");
    assert_eq!(seen.header("content-type"), ["text/plain"]);
    assert_eq!(seen.header("x-mod"), ["1"]);
    assert_eq!(seen.header("authorization"), Vec::<&str>::new());
}

// ---- the chain's own streams (19.2, Commit; the limit of 32) ---------------------------------

#[test]
fn a_start_counts_the_starts_already_queued_in_its_chain_toward_the_limit_of_32() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let request = get("https://api.openai.com/v1/x").to_string();
    let out = rig.call(
        Caller::Page,
        "https",
        "api.start-many",
        json!({"request": request, "n": 33}),
    );
    let out: Vec<String> = serde_json::from_value(out).unwrap();
    assert!(out[..32].iter().all(|s| hex32(s)), "{out:?}");
    assert_eq!(out[32], "full");
    wait("32 connections", Duration::from_secs(60), || {
        server.connections.load(Ordering::SeqCst) == 32
    });
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(server.connections.load(Ordering::SeqCst), 32);
    // With 32 open, the next call is `full` too.
    let one = rig.start(&get("https://api.openai.com/v1/x"));
    assert_eq!(one, "full");
}

#[test]
fn a_stream_started_and_cancelled_in_one_call_ends_cancelled_and_sends_nothing() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let s = rig
        .call(
            Caller::Page,
            "https",
            "api.start-cancel",
            json!({"request": get("https://api.openai.com/v1/x").to_string()}),
        )
        .as_str()
        .unwrap()
        .to_owned();
    assert!(hex32(&s), "no unknown-stream, no failure: {s}");
    rig.wait_ended(&s, "cancelled");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
    assert_eq!(rig.items(&s), Vec::<String>::new());
}

#[test]
fn a_stream_started_and_handed_off_in_one_call_is_handed_off() {
    let mut rig = Rig::new(None);
    rig.serve("api.openai.com", hold);
    let out = rig
        .call(
            Caller::Mod(local("toolbox")),
            "https",
            "api.start-hand-off",
            json!({"request": get("https://api.openai.com/v1/x").to_string()}),
        )
        .as_str()
        .unwrap()
        .to_owned();
    let (s, result) = out.split_once(' ').unwrap();
    assert!(hex32(s), "{out}");
    assert_eq!(result, "ok", "{out}");
    // The new consumer, `toolbox`, gets the stream's items.
    wait("the head", Duration::from_secs(20), || {
        let items = rig.call(Caller::Page, "toolbox", "api.items", json!({"s": s}));
        items.as_array().is_some_and(|a| a.len() >= 2)
    });
    assert_eq!(rig.items(s), Vec::<String>::new());
}

// ---- AC3: apply, and a live stream -----------------------------------------------------------

#[test]
fn ac3_a_grant_revoked_between_the_call_and_the_apply_ends_the_stream_gone() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let host = Arc::clone(rig.host());
    let request = get("https://api.openai.com/v1/x").to_string();
    let chain = std::thread::spawn(move || {
        call_as(
            &host,
            Caller::Page,
            "https",
            "api.start-then-sleep",
            json!({"request": request, "ms": 1000}),
        )
        .unwrap()
    });
    std::thread::sleep(Duration::from_millis(400));
    rig.host()
        .revoke(&local("https"), &grant("api.openai.com"))
        .unwrap();
    let s = chain.join().unwrap().as_str().unwrap().to_owned();
    assert!(hex32(&s), "{s}");
    rig.wait_ended(&s, "gone");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac3_an_origin_client_disabled_between_the_call_and_the_apply_ends_the_stream_gone() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let host = Arc::clone(rig.host());
    let request = get("https://api.openai.com/v1/x").to_string();
    let chain = std::thread::spawn(move || {
        call_as(
            &host,
            Caller::Mod(local("toolbox")),
            "https",
            "api.start-then-sleep",
            json!({"request": request, "ms": 1000}),
        )
        .unwrap()
    });
    std::thread::sleep(Duration::from_millis(400));
    rig.host().disable(&local("toolbox")).unwrap();
    let s = chain.join().unwrap().as_str().unwrap().to_owned();
    assert!(hex32(&s), "{s}");
    rig.wait_ended(&s, "gone");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

/// A server that sends the head and holds the connection until the client closes it.
async fn hold(mut c: Conn) {
    c.head("200 OK", SSE).await;
    c.write(b"data: hello\n\n").await;
    c.until_closed().await;
}

#[test]
fn ac3_a_grant_revoked_while_the_stream_is_live_ends_it_gone_and_closes_the_connection() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_items(&s, 2);
    rig.host()
        .revoke(&local("https"), &grant("api.openai.com"))
        .unwrap();
    rig.wait_ended(&s, "gone");
    wait("the connection closed", Duration::from_secs(10), || {
        server.closed.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn ac3_a_mod_disabled_while_the_stream_is_live_ends_it_gone_and_closes_the_connection() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let s = rig.start_via(&get("https://api.openai.com/v1/x"));
    rig.wait_items(&s, 2);
    rig.host().disable(&local("toolbox")).unwrap();
    rig.wait_ended(&s, "gone");
    wait("the connection closed", Duration::from_secs(10), || {
        server.closed.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn ac3_disabling_the_consumer_closes_its_live_connection() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let s = rig.start_via(&get("https://api.openai.com/v1/x"));
    rig.wait_items(&s, 2);
    rig.host().disable(&local("https")).unwrap();
    wait("the connection closed", Duration::from_secs(10), || {
        server.closed.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn the_host_closing_closes_the_connection_of_a_live_stream() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", hold);
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_items(&s, 2);
    // 19.4, Ends: the host closing closes the connection at once.
    rig.host = None;
    wait("the connection closed", Duration::from_secs(10), || {
        server.closed.load(Ordering::SeqCst) == 1
    });
}

// ---- AC4: the account `openai` ---------------------------------------------------------------

fn answer_one(text: &'static str) -> impl Fn(Conn) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    move |mut c| {
        Box::pin(async move {
            c.head("200 OK", SSE).await;
            c.write(format!("data: {text}\n\n").as_bytes()).await;
            c.finish().await;
        })
    }
}

#[test]
fn ac4_each_request_carries_its_own_fresh_token_once_and_no_mod_sees_it() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let server = rig.serve("api.openai.com", answer_one("pong"));
    let request = with(post(RESPONSES), "account", json!("openai"));
    let request = with(
        request,
        "headers",
        json!({"content-type": "application/json"}),
    );
    for n in 1..=2 {
        let s = rig.start_via(&request);
        rig.wait_ended(&s, "closed");
        // The first request of the origin client asks the consent (19.7), which is granted.
        let first = if n == 1 {
            vec![PENDING.to_owned()]
        } else {
            vec![]
        };
        assert_eq!(
            rig.items(&s),
            [first, vec![SSE_HEAD.into(), "pong".into()]].concat()
        );
        assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), n);
    }
    let seen = server.requests();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].header("authorization"), ["Bearer tok-1"]);
    assert_eq!(seen[1].header("authorization"), ["Bearer tok-2"]);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].target.as_str()),
        ("POST", "/v1/responses")
    );
    assert_eq!(seen[0].header("content-type"), ["application/json"]);
    let models = with(get(MODELS), "account", json!("openai"));
    let s = rig.start_via(&models);
    rig.wait_ended(&s, "closed");
    assert_eq!(
        server.requests()[2].header("authorization"),
        ["Bearer tok-3"]
    );
    // No recorded byte of the mod holds a token.
    let state = rig.state_bytes();
    let has = |needle: &[u8]| state.windows(needle.len()).any(|w| w == needle);
    assert!(has(b"pong"), "the search covers what the fixture recorded");
    for token in ["tok-1", "tok-2", "tok-3", "Bearer"] {
        assert!(!has(token.as_bytes()), "{token} reached the mod's state");
    }
}

#[test]
fn ac4_no_app_no_account_and_unavailable_end_the_stream_before_a_request() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let server = rig.serve("api.openai.com", answer_one("never"));
    let request = with(post(RESPONSES), "account", json!("openai"));
    *rig.app.tokens.lock().unwrap() = None;
    let s = rig.start_via(&request);
    rig.wait_ended(&s, "no-app");
    assert_eq!(
        rig.app.asked.lock().unwrap().len(),
        0,
        "nobody is asked without accounts"
    );
    *rig.app.tokens.lock().unwrap() = Some(Arc::clone(&rig.tokens));
    *rig.tokens.mode.lock().unwrap() = TokenMode::NoAccount;
    let s = rig.start_via(&request);
    rig.wait_ended(&s, "no-account");
    *rig.tokens.mode.lock().unwrap() = TokenMode::Unavailable;
    let s = rig.start_via(&request);
    rig.wait_ended(&s, "failed(account unavailable: offline)");
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
    assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), 2);
}

// ---- AC5: the account consent ----------------------------------------------------------------

const PENDING: &str = r#"{"type":"consent-pending","what":"account:local/https:openai"}"#;

#[test]
fn ac5_the_first_request_waits_for_the_consent_then_streams_and_a_second_asks_nothing() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", answer_one("pong"));
    let request = with(post(RESPONSES), "account", json!("openai"));
    let s = rig.start_via(&request);
    assert_eq!(rig.wait_items(&s, 1), [PENDING]);
    rig.wait_asked(1);
    assert_eq!(
        rig.app.asked.lock().unwrap().clone(),
        [ConsentRequest {
            origin: local("toolbox"),
            via: local("https"),
            consent: Consent::Account {
                via: "local/https".into(),
                account: "openai".into(),
            },
            profile: None,
            functions: vec![],
        }]
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        rig.tokens.calls.load(Ordering::SeqCst),
        0,
        "no token before the answer"
    );
    assert_eq!(
        server.connections.load(Ordering::SeqCst),
        0,
        "no byte before the answer"
    );
    rig.answer(0, ConsentAnswer::Granted);
    rig.wait_ended(&s, "closed");
    assert_eq!(
        rig.items(&s),
        [PENDING, SSE_HEAD, "pong"].map(str::to_owned)
    );
    assert_eq!(
        rig.consents_of("toolbox"),
        json!([{"kind": "account", "via": "local/https", "account": "openai"}])
    );
    let second = rig.start_via(&request);
    rig.wait_ended(&second, "closed");
    assert_eq!(rig.items(&second), [SSE_HEAD, "pong"].map(str::to_owned));
    assert_eq!(rig.app.asked.lock().unwrap().len(), 1);
    assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn ac5_a_refused_consent_ends_the_stream_consent_refused() {
    let mut rig = Rig::new(None);
    let server = rig.serve("api.openai.com", answer_one("never"));
    let s = rig.start_via(&with(post(RESPONSES), "account", json!("openai")));
    rig.wait_asked(1);
    rig.answer(0, ConsentAnswer::Refused);
    rig.wait_ended(&s, "consent-refused(account:local/https:openai)");
    assert_eq!(rig.items(&s), [PENDING]);
    assert_eq!(rig.consents_of("toolbox"), json!([]));
    assert_eq!(rig.tokens.calls.load(Ordering::SeqCst), 0);
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac5_a_preview_has_no_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
    crate::streams::install(&host, &["https", "toolbox"]);
    let server = Server::start(|c| async move { c.finish().await });
    host.set_test_net(TestNet {
        routes: vec![("api.openai.com".into(), server.addr)],
        root_pem: pki().root_pem.clone(),
    });
    let request = with(post(RESPONSES), "account", json!("openai"));
    let s = call_as(
        &host,
        Caller::Mod(local("toolbox")),
        "https",
        "api.start",
        json!({"request": request.to_string()}),
    )
    .unwrap();
    let s = s.as_str().unwrap().to_owned();
    assert!(hex32(&s), "{s}");
    wait("no-account", Duration::from_secs(20), || {
        call_as(&host, Caller::Page, "https", "api.ended", Value::Null).unwrap()
            == json!([format!("{s} no-account")])
    });
    assert_eq!(server.stats.connections.load(Ordering::SeqCst), 0);
}

#[cfg(all(unix, watane_dev_profile, debug_assertions))]
mod scripted_preview {
    use super::{
        Host, JudgeAccount, Limits, Mode, Pki, RESPONSES, SSE_HEAD, Server, TestNet, answer_one,
        get, make_pki, pki, post, with,
    };
    use crate::streams::{call_as, install, wait};
    use serde_json::{Value, json};
    use std::net::{SocketAddr, SocketAddrV4};
    use std::sync::Arc;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use watane_host::Caller;

    struct Preview {
        host: Arc<Host>,
        _dir: tempfile::TempDir,
    }

    impl Preview {
        fn new(addr: SocketAddr, pki: &Pki) -> Self {
            Self::with_judge(addr, pki, None)
        }

        fn with_judge(addr: SocketAddr, pki: &Pki, judge_addr: Option<SocketAddr>) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
            install(&host, &["https"]);
            let judge = judge_addr.map(|address| {
                (
                    v4(address),
                    JudgeAccount {
                        origin: "https://judge.test".into(),
                        key: "judge-fixture".into(),
                    },
                )
            });
            host.set_scripted_responses(v4(addr), &pki.root_pem, "tok".into(), judge)
                .unwrap();
            Self { host, _dir: dir }
        }

        fn test_route(&self, server: &Server) {
            self.host.set_test_net(TestNet {
                routes: vec![
                    ("api.openai.com".into(), server.addr),
                    ("judge.test".into(), server.addr),
                ],
                root_pem: pki().root_pem.clone(),
            });
        }

        fn call(&self, func: &str, args: Value) -> Value {
            call_as(&self.host, Caller::Page, "https", func, args).unwrap()
        }

        fn start(&self, request: Value) -> String {
            self.call("api.start", json!({"request": request.to_string()}))
                .as_str()
                .unwrap()
                .to_owned()
        }

        fn end(&self, stream: &str) -> String {
            let prefix = format!("{stream} ");
            let outcome = || {
                let lines = self.call("api.ended", Value::Null);
                lines.as_array().unwrap().iter().find_map(|line| {
                    line.as_str()
                        .unwrap()
                        .strip_prefix(&prefix)
                        .map(str::to_owned)
                })
            };
            wait("the HTTPS stream to end", Duration::from_secs(20), || {
                outcome().is_some()
            });
            outcome().unwrap()
        }
    }

    fn v4(addr: SocketAddr) -> SocketAddrV4 {
        let SocketAddr::V4(addr) = addr else {
            panic!("the fixture binds IPv4");
        };
        addr
    }

    fn openai(request: Value) -> Value {
        with(request, "account", json!("openai"))
    }

    #[test]
    fn rejects_an_empty_token_and_one_with_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        for token in ["", "two words"] {
            let error = host
                .set_scripted_responses(
                    "127.0.0.1:1".parse().unwrap(),
                    &pki().root_pem,
                    token.into(),
                    None,
                )
                .unwrap_err();
            assert_eq!(error.to_string(), "invalid scripted OpenAI token");
        }
    }

    #[test]
    fn reaches_the_loopback_fake_with_the_token_and_original_host() {
        let fake = Server::start(answer_one("scripted answer"));
        let preview = Preview::new(fake.addr, pki());
        let stream = preview.start(openai(post(RESPONSES)));
        assert_eq!(preview.end(&stream), "closed");
        assert_eq!(
            preview.call("api.items", json!({"s":stream})),
            json!([SSE_HEAD, "scripted answer"])
        );
        let requests = fake.stats.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].header("authorization"), ["Bearer tok"]);
        assert_eq!(requests[0].header("host"), ["api.openai.com"]);
    }

    #[test]
    fn rejects_the_same_trusted_root_with_the_wrong_tls_name() {
        let wrong_name = make_pki(&["second.test"]);
        let fake = Server::start_with_pki(&wrong_name, answer_one("wrong server"));
        let preview = Preview::new(fake.addr, &wrong_name);
        let stream = preview.start(openai(post(RESPONSES)));
        let end = preview.end(&stream);
        assert!(end.contains("scripted OpenAI TLS failed"), "{end}");
        assert_eq!(fake.stats.connections.load(Ordering::SeqCst), 1);
        assert!(fake.stats.requests().is_empty());
    }

    #[test]
    fn refuses_a_closed_fake_without_trying_the_normal_route() {
        // Reserve the port without listening, so another parallel fixture cannot take it.
        let closed = tokio::net::TcpSocket::new_v4().unwrap();
        closed.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let preview = Preview::new(closed.local_addr().unwrap(), pki());
        let ordinary = Server::start(answer_one("wrong route"));
        preview.test_route(&ordinary);
        let stream = preview.start(openai(post(RESPONSES)));
        let end = preview.end(&stream);
        assert!(end.contains("cannot connect to scripted OpenAI"), "{end}");
        assert_eq!(ordinary.stats.connections.load(Ordering::SeqCst), 0);
    }

    fn judge() -> Value {
        with(
            post("https://judge.test/verdict"),
            "account",
            json!("judge"),
        )
    }

    #[test]
    fn scripted_judge_reaches_the_fake_with_its_key_and_tls_name() {
        // The leaf cert is valid for judge.test alone, so this also pins the SNI name.
        let pki = make_pki(&["judge.test"]);
        let judge_server = Server::start_with_pki(&pki, answer_one("scripted verdict"));
        let responses = Server::start(answer_one("wrong account"));
        let preview = Preview::with_judge(responses.addr, &pki, Some(judge_server.addr));
        let stream = preview.start(judge());
        assert_eq!(preview.end(&stream), "closed");
        assert_eq!(
            preview.call("api.items", json!({"s":stream})),
            json!([SSE_HEAD, "scripted verdict"])
        );
        let requests = judge_server.stats.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].header("authorization"),
            ["Bearer judge-fixture"]
        );
        assert_eq!(requests[0].header("host"), ["judge.test"]);
        assert_eq!(responses.stats.connections.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn scripted_judge_socket_failure_never_tries_the_normal_route() {
        let closed = tokio::net::TcpSocket::new_v4().unwrap();
        closed.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let pki = make_pki(&["judge.test"]);
        let ordinary = Server::start_with_pki(&pki, answer_one("wrong route"));
        let preview = Preview::with_judge(ordinary.addr, &pki, Some(closed.local_addr().unwrap()));
        preview.test_route(&ordinary);
        let stream = preview.start(judge());
        let end = preview.end(&stream);
        assert!(end.contains("cannot connect to scripted judge"), "{end}");
        assert_eq!(ordinary.stats.connections.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn scripted_responses_without_a_judge_binding_has_no_judge_account() {
        let fake = Server::start(answer_one("wrong account"));
        let preview = Preview::new(fake.addr, pki());
        preview.test_route(&fake);
        let stream = preview.start(judge());
        assert_eq!(preview.end(&stream), "no-account");
        assert_eq!(fake.stats.connections.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn an_uncredentialed_request_uses_the_ordinary_route_without_authorization() {
        let fake = Server::start(answer_one("wrong route"));
        let preview = Preview::new(fake.addr, pki());
        let ordinary = Server::start(answer_one("ordinary answer"));
        preview.test_route(&ordinary);
        let stream = preview.start(post(RESPONSES));
        assert_eq!(preview.end(&stream), "closed");
        let requests = ordinary.stats.requests();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].header("authorization").is_empty());
        assert_eq!(fake.stats.connections.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn refuses_get_responses_before_contacting_the_fake() {
        let fake = Server::start(answer_one("wrong method"));
        let preview = Preview::new(fake.addr, pki());
        assert_eq!(preview.start(openai(get(RESPONSES))), "destination");
        assert_eq!(fake.stats.connections.load(Ordering::SeqCst), 0);
    }
}

fn live_pair(rig: &mut Rig) -> (Arc<Stats>, Arc<Stats>) {
    let openai = rig.serve("api.openai.com", hold);
    let other = rig.serve("second.test", hold);
    (openai, other)
}

#[test]
fn ac5_withdrawing_the_account_consent_ends_the_live_openai_streams_of_that_origin_client() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let (openai, other) = live_pair(&mut rig);
    let request = with(post(RESPONSES), "account", json!("openai"));
    let account = rig.start_via(&request);
    let plain = rig.start_via(&get("https://second.test/v1/x"));
    // Another origin client's stream through the same mod is not that consent's (the page's).
    let page = rig.start(&request);
    rig.wait_items(&account, 3);
    rig.wait_items(&plain, 2);
    rig.wait_items(&page, 3);
    let withdrawn = Instant::now();
    rig.host()
        .withdraw_consent(
            &local("toolbox"),
            &Consent::parse("account:local/https:openai").unwrap(),
        )
        .unwrap();
    rig.wait_ended(&account, "gone");
    assert!(
        withdrawn.elapsed() < Duration::from_secs(1),
        "{:?}",
        withdrawn.elapsed()
    );
    wait("the connection closed", Duration::from_secs(10), || {
        openai.closed.load(Ordering::SeqCst) == 1
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(rig.ended(), [format!("{account} gone")]);
    assert_eq!(openai.closed.load(Ordering::SeqCst), 1);
    assert_eq!(other.closed.load(Ordering::SeqCst), 0);
    assert_eq!(rig.items(&plain).len(), 2);
    assert_eq!(rig.items(&page).len(), 3);
}

#[test]
fn ac5_a_sign_out_ends_every_live_openai_stream() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let (openai, other) = live_pair(&mut rig);
    let request = with(post(RESPONSES), "account", json!("openai"));
    let (a, b) = (rig.start_via(&request), rig.start(&request));
    let plain = rig.start_via(&get("https://second.test/v1/x"));
    for s in [&a, &b] {
        rig.wait_items(s, 3);
    }
    rig.wait_items(&plain, 2);
    rig.host().account_changed(&Account::Judge {
        provider: "local/https".into(),
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        rig.ended(),
        Vec::<String>::new(),
        "changing a judge account does not end OpenAI streams"
    );
    rig.host().account_changed(&Account::OpenAi);
    rig.wait_ended(&a, "gone");
    rig.wait_ended(&b, "gone");
    wait("both connections closed", Duration::from_secs(10), || {
        openai.closed.load(Ordering::SeqCst) == 2
    });
    assert_eq!(other.closed.load(Ordering::SeqCst), 0);
    assert_eq!(rig.ended().len(), 2);
}

// ---- AC6: borrowed authority -----------------------------------------------------------------

/// A handle of `toolbox` for the toolset `h` (the tools of `https`), ready.
fn handle(rig: &Rig) -> String {
    rig.host()
        .set_toolset(
            &watane_host::ToolsetName::parse("h").unwrap(),
            vec!["https:api".to_owned()],
        )
        .unwrap();
    let opened = rig.call(
        Caller::Page,
        "toolbox",
        "api.open",
        json!({"toolsets": ["h"]}),
    );
    let h = opened["ok"].as_str().unwrap().to_owned();
    wait("a ready handle", Duration::from_secs(20), || {
        rig.call(
            Caller::Page,
            "toolbox",
            "api.describe",
            json!({"handle": h}),
        )["ok"]
            .is_array()
    });
    h
}
fn tool_args(request: &Value) -> String {
    json!({"request": request.to_string()}).to_string()
}

#[test]
fn ac6_a_job_that_starts_an_https_stream_is_not_granted() {
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let h = handle(&rig);
    let job = rig.call(
        Caller::Page,
        "toolbox",
        "api.call",
        json!({
            "handle": h, "key": "k", "tool": "https_api_start",
            "args": tool_args(&get("https://api.openai.com/v1/x")),
        }),
    )["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    wait("the job's item", Duration::from_secs(20), || {
        !rig.call(Caller::Page, "toolbox", "api.items", json!({"s": job}))
            .as_array()
            .unwrap()
            .is_empty()
    });
    let items = rig.call(Caller::Page, "toolbox", "api.items", json!({"s": job}));
    let item: Value = serde_json::from_str(items[0].as_str().unwrap()).unwrap();
    assert_eq!(item["err"]["code"], "not_granted", "{item}");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
    assert_eq!(rig.ended(), Vec::<String>::new());
}

/// The token of `/mcp/agent` reaches only a `claude` process on its fd 3 (19.3). Windows refuses
/// that launch (`claude::s110_refuses_claude_without_process_group_controls`), so no `tools/call`
/// exists there; the job half of AC6 above runs on every platform.
#[cfg(unix)] // Reads the MCP token from a Unix `claude` launch.
#[test]
fn ac6_a_tools_call_that_starts_an_https_stream_is_an_error_result() {
    use std::os::unix::fs::PermissionsExt;
    const MCP_INIT: &str = r#"{"type":"system","subtype":"init","tools":[],"mcp_servers":[{"name":"watane","status":"connected"}],"agents":[],"permissionMode":"default","plugins":[{"name":"watane-run"}]}"#;
    let mut rig = Rig::new(Some(ConsentAnswer::Granted));
    let server = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let fake = tempfile::tempdir().unwrap();
    let program = fake.path().join("claude");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude/claude"),
        &program,
    )
    .unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(fake.path().join("init"), format!("{MCP_INIT}\n")).unwrap();
    std::fs::write(fake.path().join("exit"), "0").unwrap();
    *rig.tokens.program.lock().unwrap() = Some(program);
    let served = rig
        .rt
        .block_on(rig.host().serve(0, axum::Router::new()))
        .unwrap();
    let h = handle(&rig);
    let launch = json!({"version": 1, "workspace": "scratch", "agent-handle": h});
    rig.call(
        Caller::Page,
        "toolbox",
        "api.start",
        json!({"launch": launch.to_string()}),
    );
    let file = fake.path().join("mcp.json");
    wait("the MCP config", Duration::from_secs(20), || file.exists());
    let config: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    let header = config["mcpServers"]["watane"]["headers"]["Authorization"]
        .as_str()
        .unwrap();
    let token = header.strip_prefix("Bearer ").unwrap().to_owned();
    let client = reqwest::Client::builder()
        .resolve("localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let body = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "https_api_start",
                   "arguments": {"request": get("https://api.openai.com/v1/x").to_string()}},
    });
    let (status, answer): (u16, Value) = rig.rt.block_on(async {
        let r = client
            .post(format!("http://localhost:{}/mcp/agent", served.port))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    });
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["result"]["isError"], true, "{answer}");
    let text = answer["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("not_granted:"), "{answer}");
    assert_eq!(server.connections.load(Ordering::SeqCst), 0);
}

// ---- AC7: budgets ----------------------------------------------------------------------------

/// A server that writes `count` events of about `size` bytes, `{"n":k,"t":<micros>,"pad":"..."}`,
/// `gap` apart, and counts each event it wrote (a write that returned).
fn writer(
    count: usize,
    size: usize,
    gap: Duration,
    stop: Option<Arc<AtomicBool>>,
) -> impl Fn(Conn) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    move |mut c| {
        let stop = stop.clone();
        Box::pin(async move {
            c.head("200 OK", SSE).await;
            let pad = "x".repeat(size);
            for n in 1..=count {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros();
                let event = format!("data: {{\"n\":{n},\"t\":{now},\"pad\":\"{pad}\"}}\n\n");
                if !c.write(event.as_bytes()).await {
                    return;
                }
                c.stats.written.fetch_add(1, Ordering::SeqCst);
                if stop
                    .as_ref()
                    .is_some_and(|stop| stop.load(Ordering::SeqCst))
                {
                    break;
                }
                if !gap.is_zero() {
                    tokio::time::sleep(gap).await;
                }
            }
            c.finish().await;
        })
    }
}

#[test]
fn ac7_a_consumer_that_defers_stops_the_host_reading_and_resume_loses_nothing() {
    const EVENTS: usize = 400;
    const SIZE: usize = 128 << 10;
    let mut rig = Rig::new(None);
    // The stream's lag and deferral deadlines are unrelated to the order tested here.
    rig.host().set_test_clock(TestClock::new());
    let stop = Arc::new(AtomicBool::new(false));
    let server = rig.serve(
        "api.openai.com",
        writer(EVENTS, SIZE, Duration::ZERO, Some(Arc::clone(&stop))),
    );
    rig.mode("defer");
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    wait(
        "the deferred stream to apply backpressure",
        Duration::from_secs(60),
        || rig.host().test_stream_deferred_and_full(&s),
    );
    // The 4 MiB host queue holds about 32 events. Allow another 4 MiB for
    // the writer, the socket buffers and the TLS/HTTP reader.
    let written = server.written.load(Ordering::SeqCst);
    assert!(
        written < (8 << 20) / SIZE,
        "the server wrote {written} events while the stream was deferred"
    );
    assert_eq!(
        rig.items(&s),
        Vec::<String>::new(),
        "a deferred batch records nothing"
    );
    // Let the writer finish its current event after resume. This keeps the amount drained
    // proportional to the bytes actually buffered, instead of sending another 50 MiB.
    stop.store(true, Ordering::SeqCst);
    rig.mode("quiet");
    assert_eq!(
        rig.call(Caller::Page, "https", "api.resume", json!({"s": s})),
        json!("ok")
    );
    let prefix = format!("{s} ");
    wait("the resumed stream to end", Duration::from_secs(60), || {
        rig.ended().iter().any(|line| line.starts_with(&prefix))
    });
    assert_eq!(rig.ended(), [format!("{s} closed")]);
    let tally = rig.tally(&s);
    let [n, bad, bytes] = ["n", "bad", "bytes"].map(|k| -> u64 {
        tally
            .split(';')
            .find_map(|p| p.strip_prefix(&format!("{k}=")))
            .unwrap()
            .parse()
            .unwrap()
    });
    // Every event, in order: a gap or a repeat would count as `bad`.
    let written = server.written.load(Ordering::SeqCst);
    assert!(written > 0);
    assert_eq!((n, bad), (written as u64, 0), "{tally}");
    assert!(bytes > (written * SIZE) as u64);
}

#[test]
#[ignore = "slow: stage D budget"]
fn ac7_stage_d_budget_p95_from_the_servers_write_to_the_items_call_is_at_most_50_ms() {
    const EVENTS: usize = 1000;
    let mut rig = Rig::new(None);
    rig.serve(
        "api.openai.com",
        writer(EVENTS, 200, Duration::from_millis(3), None),
    );
    rig.mode("stamp");
    let s = rig.start(&get("https://api.openai.com/v1/x"));
    rig.wait_ended(&s, "closed");
    let tally = rig.tally(&s);
    let lat = tally
        .split(';')
        .find_map(|p| p.strip_prefix("lat="))
        .unwrap();
    let mut micros: Vec<u64> = lat
        .split(',')
        .filter(|v| !v.is_empty())
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(micros.len(), EVENTS, "{tally}");
    micros.sort_unstable();
    let p95 = micros[micros.len() * 95 / 100];
    println!(
        "stage D: p50 {} us, p95 {p95} us, max {} us over {EVENTS} events",
        micros[micros.len() / 2],
        micros[micros.len() - 1]
    );
    assert!(p95 <= 50_000, "p95 {p95} us");
}

// ---- the harness ------------------------------------------------------------------------------

#[test]
fn the_harness_fakes_https_stream_start_and_records_the_request() {
    let h = watane_host::Harness::open(&fixture("https"), watane_host::Fakes::new()).unwrap();
    let out = h
        .call("api.start", json!({"request": "{\"url\":1}"}))
        .unwrap();
    assert_eq!(out.value, json!("s1"));
    assert_eq!(
        out.requests,
        vec![watane_host::RequestRecord {
            function: "https-stream.start".into(),
            args: json!({"request": "{\"url\":1}"}),
        }]
    );
}

// ---- AC8: `test-net` -------------------------------------------------------------------------

#[test]
#[ignore = "slow: only a cargo tree of the whole workspace shows that no normal build enables test-net"]
fn ac8_no_release_build_holds_the_feature_test_net() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let out = std::process::Command::new(env!("CARGO"))
        .args([
            "tree",
            "-e",
            "features",
            "-i",
            "watane-host",
            "--workspace",
            "--edges",
            "normal",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("watane-host v0.1.0"), "{text}");
    for feature in ["test-net", "test-streams"] {
        assert!(
            !text.contains(feature),
            "{feature} is in a normal build:\n{text}"
        );
    }
}

// ---- Stage A5: judge accounts (HOST-F-72) ----------------------------------------------------

fn judge_request() -> Value {
    with(
        post("https://SECOND.TEST:443/verdict"),
        "account",
        json!("judge"),
    )
}

fn enter_judge(rig: &Rig, identity: &str, origin: &str) {
    *rig.tokens.judge.lock().unwrap() = Some((
        identity.into(),
        JudgeAccount {
            key: "test-judge-key".into(),
            origin: origin.into(),
        },
    ));
}

#[test]
fn ac1_judge_post_carries_the_key_and_does_not_follow_a_redirect() {
    let mut rig = Rig::new(None);
    enter_judge(&rig, "local/https", "HTTPS://SECOND.TEST:443/");
    let judge = rig.serve("second.test", |mut c| async move {
        c.head(
            "307 Temporary Redirect",
            &[("location", RESPONSES), ("content-length", "0")],
        )
        .await;
        c.finish().await;
    });
    let other = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let id = rig.start(&judge_request());
    assert!(hex32(&id), "{id}");
    rig.wait_ended(&id, "closed");
    let requests = judge.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].target, "/verdict");
    assert_eq!(
        requests[0].header("authorization"),
        ["Bearer test-judge-key"]
    );
    assert_eq!(other.connections.load(Ordering::SeqCst), 0);
    assert!(rig.app.asked.lock().unwrap().is_empty());
}

#[test]
fn ac2_judge_requires_the_exact_provider_and_a_post_to_its_origin() {
    let mut rig = Rig::new(None);
    let judge = rig.serve("second.test", |c| async move { c.finish().await });
    let other = rig.serve("api.openai.com", |c| async move { c.finish().await });
    let missing = rig.start(&judge_request());
    rig.wait_ended(&missing, "no-account");
    enter_judge(&rig, "local/https-checked", "https://second.test");
    let different = rig.start(&judge_request());
    rig.wait_ended(&different, "no-account");
    enter_judge(&rig, "local/https", "https://second.test");
    let wrong_origin = with(post(RESPONSES), "account", json!("judge"));
    let wrong_method = with(judge_request(), "method", json!("GET"));
    assert_eq!(rig.start(&wrong_origin), "destination");
    assert_eq!(rig.start(&wrong_method), "destination");
    enter_judge(&rig, "local/https", "https://api.openai.com");
    assert_eq!(rig.start(&wrong_origin), "destination");
    enter_judge(&rig, "local/https", "https://second.test:8443");
    let ungranted = with(
        post("https://second.test:8443/verdict"),
        "account",
        json!("judge"),
    );
    assert_eq!(rig.start(&ungranted), "destination");
    assert_eq!(judge.connections.load(Ordering::SeqCst), 0);
    assert_eq!(other.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac6_a_nested_judge_call_uses_the_executing_providers_key() {
    let mut rig = Rig::new(None);
    let judge = rig.serve("second.test", answer_one("verdict"));
    enter_judge(&rig, "local/https", "https://second.test");

    let id = rig.start_via(&judge_request());
    rig.wait_ended(&id, "closed");
    let requests = judge.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        ["Bearer test-judge-key"]
    );
}

#[test]
fn ac2_judge_key_is_scoped_to_the_signed_publisher() {
    use watane_host::watane_log::Log;
    use watane_host::watane_pkg::{self as pkg, Mod, RELEASE_KIND, Release};
    use watane_host::{Requester, Source};

    let mut rig = Rig::new(None);
    let judge = rig.serve("second.test", answer_one("verdict"));
    let folder = fixture("https");
    let package = pkg::pack(&folder, false).unwrap();
    let body = Release::new(&Mod::read(&folder).unwrap(), &package).unwrap();
    let sources = ["first-publisher", "second-publisher"]
        .into_iter()
        .map(|name| {
            let log = Log::open(&rig.dir.path().join(name)).unwrap();
            let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
            Source::Release {
                package: package.clone(),
                entry,
            }
        })
        .collect();
    let plan = rig.host().plan(sources).unwrap();
    let mods = rig
        .rt
        .block_on(rig.host().install(plan, Requester::App))
        .unwrap();
    let [first, second] = mods.as_slice() else {
        panic!("expected two installed providers");
    };
    assert_eq!(first.slug, second.slug);
    assert_ne!(first.identity, second.identity);
    enter_judge(&rig, &first.identity, "https://second.test");

    let request = json!({"request": judge_request().to_string()});
    let start = |target| {
        rig.call(Caller::Page, target, "api.start", request.clone())
            .as_str()
            .unwrap()
            .to_owned()
    };
    let ends = |target, expected: String| {
        wait(&expected, Duration::from_secs(20), || {
            rig.call(Caller::Page, target, "api.ended", Value::Null) == json!([expected])
        });
    };
    let second_stream = start(second.local.as_ref());
    ends(second.local.as_ref(), format!("{second_stream} no-account"));
    assert_eq!(judge.connections.load(Ordering::SeqCst), 0);
    let first_stream = start(first.local.as_ref());
    ends(first.local.as_ref(), format!("{first_stream} closed"));
    let requests = judge.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].header("authorization"),
        ["Bearer test-judge-key"]
    );
}

#[test]
fn ac2_judge_without_an_app_account_source_is_no_app() {
    let mut rig = Rig::new(None);
    let judge = rig.serve("second.test", |c| async move { c.finish().await });
    *rig.app.tokens.lock().unwrap() = None;
    let id = rig.start(&judge_request());
    assert!(hex32(&id), "{id}");
    rig.wait_ended(&id, "no-app");
    assert_eq!(judge.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac2_a_preview_cannot_use_a_judge_account() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
    crate::streams::install(&host, &["https"]);
    let server = Server::start(|c| async move { c.finish().await });
    host.set_test_net(TestNet {
        routes: vec![("second.test".into(), server.addr)],
        root_pem: pki().root_pem.clone(),
    });
    let value = call_as(
        &host,
        Caller::Page,
        "https",
        "api.start",
        json!({"request": judge_request().to_string()}),
    )
    .unwrap();
    let id = value.as_str().unwrap();
    assert!(hex32(id), "{id}");
    wait("no-account", Duration::from_secs(20), || {
        call_as(&host, Caller::Page, "https", "api.ended", Value::Null).unwrap()
            == json!([format!("{id} no-account")])
    });
    assert_eq!(server.stats.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac4_judge_origin_is_checked_again_at_apply() {
    let mut rig = Rig::new(None);
    enter_judge(&rig, "local/https", "https://second.test");
    let judge = rig.serve("second.test", |c| async move { c.finish().await });
    // A fake source replaces the binding immediately after returning the first account.
    // The start check sees the old origin and the apply check must see the new one.
    *rig.tokens.judge_after_read.lock().unwrap() = Some(JudgeAccount {
        key: "replacement-judge-key".into(),
        origin: "https://api.openai.com".into(),
    });
    let id = rig.start(&judge_request());
    assert!(hex32(&id), "{id}");
    rig.wait_ended(&id, "gone");
    assert_eq!(judge.connections.load(Ordering::SeqCst), 0);
}

#[test]
fn ac4_removing_a_judge_key_ends_that_providers_live_stream() {
    let mut rig = Rig::new(None);
    enter_judge(&rig, "local/https", "https://second.test");
    let judge = rig.serve("second.test", hold);
    let account = rig.start(&judge_request());
    let plain = rig.start(&get("https://second.test/plain"));
    rig.wait_items(&account, 2);
    rig.wait_items(&plain, 2);
    rig.host().account_changed(&Account::Judge {
        provider: "local/another".into(),
    });
    assert!(rig.ended().is_empty());
    *rig.tokens.judge.lock().unwrap() = None;
    rig.host().account_changed(&Account::Judge {
        provider: "local/https".into(),
    });
    rig.wait_ended(&account, "gone");
    wait("judge connection closed", Duration::from_secs(10), || {
        judge.closed.load(Ordering::SeqCst) == 1
    });
    assert_eq!(rig.ended(), [format!("{account} gone")]);
}
