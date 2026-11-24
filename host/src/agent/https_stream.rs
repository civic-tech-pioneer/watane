//! H3, HTTPS streams (TECHNICAL 19.4): `https-stream.start` at the call, the start that a
//! committed chain applies, one HTTPS connection per stream, its items, the idle clock, the
//! account `openai` with its consent, and the ends (withdrawal, sign-out, a lost grant).

use super::consent::Asked;
use super::streams::{Consumer, Hub, MAX_ITEM, MAX_OPEN, StreamError};
use super::{Effect, ModRef};
use crate::runtime::call::Invocation;
use crate::{Account, AccountError, CallError, Consent, ConsentRequest, Host, Mode};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderName, HeaderValue};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio_rustls::TlsConnector;
use url::Url;
use wasmtime::component::Val;

#[cfg(all(unix, debug_assertions))]
mod preview;

/// The largest request body (19.4).
const MAX_BODY: usize = 1 << 20;
/// The idle limit when the request names none, and the largest one (19.4).
const DEFAULT_IDLE: u64 = 90;
const MAX_IDLE: u64 = 300;
/// The size of an item of a body that is not SSE (19.4).
const CHUNK: usize = 64 << 10;
/// How often a push that found the stream full tries again.
const FULL_WAIT: Duration = Duration::from_millis(5);
/// The only host of the account `openai` (19.4).
const OPENAI_HOST: &str = "api.openai.com";

/// Headers that a mod never sets (19.4, S64), compared without case. `proxy-*` is a prefix.
const FORBIDDEN: [&str; 7] = [
    "authorization",
    "cookie",
    "host",
    "openai-organization",
    "openai-project",
    "chatgpt-account-id",
    // The host frames and encodes the request itself.
    "accept-encoding",
];
/// The framing headers of HTTP/1.1, which the host's client sets itself.
const FRAMING: [&str; 7] = [
    "content-length",
    "transfer-encoding",
    "connection",
    "keep-alive",
    "upgrade",
    "te",
    "trailer",
];

/// Why a request is refused at the call: a case of the WIT `start-error` (19.1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Refusal {
    BadLaunch { field: String, reason: String },
    Destination,
    ForbiddenHeader(String),
    TooLarge,
}
impl Refusal {
    /// The `result` of the call, in the JSON of 6.9.
    pub(crate) fn value(&self) -> Value {
        match self {
            Self::BadLaunch { field, reason } => {
                json!({"err": {"bad-launch": {"field": field, "reason": reason}}})
            }
            Self::Destination => json!({"err": "destination"}),
            Self::ForbiddenHeader(name) => json!({"err": {"forbidden-header": name}}),
            Self::TooLarge => json!({"err": "too-large"}),
        }
    }
}
fn bad(field: &str, reason: &str) -> Refusal {
    Refusal::BadLaunch {
        field: field.into(),
        reason: reason.into(),
    }
}

/// A checked request of `https-stream.start` (19.4).
#[derive(Debug)]
pub(crate) struct Request {
    /// The host as 6.8 reads it: lower case, one trailing dot dropped.
    pub name: String,
    pub port: u16,
    /// What the `Http` grants are compared with: `name`, or `name:port` for a port but 443.
    pub grant: String,
    /// The path and the query, as they are sent.
    target: String,
    method: hyper::Method,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
    /// `account: "openai"` (19.4).
    pub openai: bool,
    idle: Duration,
}

impl Request {
    /// Reads and checks the request text (19.4, Refused at the call). `grants` are the mod's
    /// granted `Http` hosts. Nothing leaves the host here.
    pub(crate) fn parse(text: &str, grants: &[String]) -> Result<Self, Refusal> {
        let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(text) else {
            return Err(bad("request", "the request is a JSON object"));
        };
        for key in fields.keys() {
            if !["url", "method", "headers", "body", "account", "idle-s"].contains(&key.as_str()) {
                return Err(bad(key, "unknown field"));
            }
        }
        // A field that is `null` is as good as missing.
        let field = |name: &str| fields.get(name).filter(|v| !v.is_null());
        let text_of = |name: &str| match field(name) {
            Some(Value::String(s)) => Ok(Some(s.as_str())),
            Some(_) => Err(bad(name, "a string")),
            None => Ok(None),
        };
        let url = text_of("url")?.ok_or_else(|| bad("url", "missing"))?;
        let method = match text_of("method")?.ok_or_else(|| bad("method", "missing"))? {
            m @ ("GET" | "POST" | "PUT" | "PATCH" | "DELETE") => {
                hyper::Method::from_bytes(m.as_bytes())
                    .map_err(|_| bad("method", "not a method"))?
            }
            _ => return Err(bad("method", "GET, POST, PUT, PATCH or DELETE")),
        };
        let idle = match field("idle-s") {
            None => DEFAULT_IDLE,
            Some(v) => v
                .as_u64()
                .filter(|s| (1..=MAX_IDLE).contains(s))
                .ok_or_else(|| bad("idle-s", "an integer from 1 to 300"))?,
        };
        let body = text_of("body")?.unwrap_or("");
        let account = text_of("account")?;
        let headers = match field("headers") {
            None => &Map::new(),
            Some(Value::Object(h)) => h,
            Some(_) => return Err(bad("headers", "an object of text values")),
        };
        let endpoint = Endpoint::parse(url, grants)?;
        let headers = headers_of(headers)?;
        if body.len() > MAX_BODY {
            return Err(Refusal::TooLarge);
        }
        // The stage-A4 rule: `openai` is the only account; `judge` is built in A5 (19.4).
        let openai = match account {
            None => false,
            Some("openai") => {
                let responses = method == hyper::Method::POST && endpoint.path == "/v1/responses";
                let models = method == hyper::Method::GET && endpoint.path == "/v1/models";
                if endpoint.grant != OPENAI_HOST || endpoint.query || !(responses || models) {
                    return Err(Refusal::Destination);
                }
                true
            }
            Some(_) => return Err(Refusal::Destination),
        };
        Ok(Self {
            name: endpoint.name,
            port: endpoint.port,
            grant: endpoint.grant,
            target: endpoint.target,
            method,
            headers,
            body: body.as_bytes().to_vec(),
            openai,
            idle: Duration::from_secs(idle),
        })
    }
}

/// The parts of a checked URL.
struct Endpoint {
    name: String,
    port: u16,
    grant: String,
    path: String,
    query: bool,
    target: String,
}
impl Endpoint {
    /// 19.4: `https`, no userinfo, a host that 6.8 matches (lower case, one trailing dot dropped)
    /// to one of the mod's `Http` grants. The URL crate drops the port 443.
    fn parse(text: &str, grants: &[String]) -> Result<Self, Refusal> {
        let url = Url::parse(text).map_err(|_| bad("url", "not a URL"))?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return Err(Refusal::Destination);
        }
        let host = match url.host() {
            Some(url::Host::Domain(d)) => d.to_owned(),
            Some(url::Host::Ipv4(a)) => a.to_string(),
            _ => return Err(Refusal::Destination),
        };
        let name = host.strip_suffix('.').unwrap_or(&host).to_owned();
        let grant = url
            .port()
            .map_or_else(|| name.clone(), |p| format!("{name}:{p}"));
        if name.is_empty() || !grants.contains(&grant) {
            return Err(Refusal::Destination);
        }
        let target = match url.query() {
            Some(q) => format!("{}?{q}", url.path()),
            None => url.path().to_owned(),
        };
        Ok(Self {
            port: url.port_or_known_default().unwrap_or(443),
            path: url.path().to_owned(),
            query: url.query().is_some(),
            name,
            grant,
            target,
        })
    }
}

/// The mod's headers: a token for a name, none of the names that 19.4 forbids, a value with no
/// control character.
fn headers_of(map: &Map<String, Value>) -> Result<Vec<(HeaderName, HeaderValue)>, Refusal> {
    let mut out = vec![];
    for (name, value) in map {
        let lower = name.to_ascii_lowercase();
        let token = HeaderName::from_bytes(name.as_bytes());
        let token = match token {
            Ok(t)
                if !FORBIDDEN.contains(&lower.as_str())
                    && !FRAMING.contains(&lower.as_str())
                    && !lower.starts_with("proxy-") =>
            {
                t
            }
            _ => return Err(Refusal::ForbiddenHeader(name.clone())),
        };
        let Value::String(value) = value else {
            return Err(bad("headers", "an object of text values"));
        };
        let value = HeaderValue::from_bytes(value.as_bytes()).map_err(|_| {
            bad(
                "headers",
                &format!("the value of {name:?} has a control character"),
            )
        })?;
        out.push((token, value));
    }
    Ok(out)
}

/// `https-stream.start` at the call (19.4): the checks that need nothing outside, then the request.
pub(crate) fn start(inv: &Invocation, args: &[Val]) -> Result<Value, CallError> {
    let Some(Val::String(text)) = args.first() else {
        return Err(CallError::internal("https-stream.start: bad argument"));
    };
    let host = &inv.chain.host;
    let grants = host.http_grants(&inv.local);
    let request = match Request::parse(text, &grants) {
        Ok(r) => r,
        Err(refusal) => return Ok(refusal.value()),
    };
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
    log.push(Effect::HttpsStart {
        id: id.clone(),
        consumer: me.clone(),
        consumer_gen: inv.package_of(&inv.local),
        origin: inv.origin.clone(),
        origin_gen: inv.origin_gen,
        request: Arc::new(request),
    });
    Ok(json!({"ok": id}))
}

/// Stops a stream's connection: set when the stream ends, whoever ends it.
#[derive(Debug)]
pub(crate) struct Control {
    stopped: AtomicBool,
    tx: tokio::sync::watch::Sender<bool>,
}
impl Control {
    fn new() -> Self {
        Self {
            stopped: AtomicBool::new(false),
            tx: tokio::sync::watch::channel(false).0,
        }
    }
    /// Runs under the hub's lock: takes no lock of the hub.
    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.tx.send_replace(true);
    }
    fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
    async fn stopped(&self) {
        let mut rx = self.tx.subscribe();
        let _ = rx.wait_for(|stopped| *stopped).await;
    }
}

/// One live stream of the registry.
#[derive(Debug)]
struct Entry {
    origin: String,
    consumer: ModRef,
    /// The grant that the stream depends on: the host (`name` or `name:port`).
    host: String,
    openai: bool,
    ctl: Arc<Control>,
}
/// The live HTTPS streams of a host (19.4): what a withdrawal, a sign-out or a lost grant ends.
#[derive(Debug, Default)]
pub(crate) struct Live(Mutex<HashMap<String, Entry>>);
impl Live {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.0
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
    }
    /// The ids of the live streams of `origin` through `via` that use the account `openai`.
    pub(crate) fn of_account(&self, origin: &str, via: &str) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|(_, e)| e.openai && e.origin == origin && e.consumer.identity == via)
            .map(|(id, _)| id.clone())
            .collect()
    }
    fn openai(&self) -> Vec<String> {
        self.lock()
            .iter()
            .filter(|(_, e)| e.openai)
            .map(|(id, _)| id.clone())
            .collect()
    }
    /// The host closes: every connection ends.
    pub(crate) fn stop_all(&self) {
        for entry in self.lock().values() {
            entry.ctl.stop();
        }
    }
}

/// Removes a stream from the registry when its thread ends.
struct Unregister<'a>(&'a Live, &'a str);
impl Drop for Unregister<'_> {
    fn drop(&mut self) {
        self.0.lock().remove(self.1);
    }
}

/// Test routes (19.9): a name that `https-stream` connects to a local address, and the root that
/// the TLS of those names chains to.
#[cfg(feature = "test-net")]
#[derive(Debug, Clone)]
pub struct TestNet {
    pub routes: Vec<(String, std::net::SocketAddr)>,
    pub root_pem: String,
}

/// Where a connection goes.
#[derive(Debug, PartialEq)]
enum Target {
    Addr(std::net::SocketAddr),
    Name(String, u16),
}

/// How a stream's work ends other than by the end of its body.
enum End {
    /// The stream ended elsewhere (a cancel, `gone`, the host closing): nothing to report.
    Stop,
    /// The producer's outcome (6.9).
    With(Value),
}
fn failed(message: impl std::fmt::Display) -> Value {
    json!({"failed": message.to_string()})
}

/// The thread of one stream (19.4).
#[derive(Debug)]
pub(crate) struct Job {
    id: String,
    hub: Arc<Hub>,
    weak: Weak<Host>,
    live: Arc<Live>,
    ctl: Arc<Control>,
    request: Arc<Request>,
    origin: String,
    origin_gen: Option<u64>,
    via: ModRef,
    mode: Mode,
    #[cfg(feature = "test-net")]
    net: Option<TestNet>,
}

impl Job {
    pub(crate) fn go(self) {
        let _registered = Unregister(&self.live, &self.id);
        #[cfg(all(unix, debug_assertions))]
        if preview::run(&self) {
            return;
        }
        let outcome = match self.consent() {
            Ok(()) => self.run(),
            Err(End::With(v)) => Some(v),
            Err(End::Stop) => None,
        };
        match outcome {
            Some(v) if v == json!("gone") => self.hub.end_gone(std::slice::from_ref(&self.id)),
            Some(v) => self.hub.finish(&self.id, v),
            None => {}
        }
    }

    fn run(&self) -> Option<Value> {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return Some(failed("cannot start a runtime for https-stream"));
        };
        match runtime.block_on(self.stream()) {
            Ok(outcome) | Err(End::With(outcome)) => Some(outcome),
            Err(End::Stop) => None,
        }
    }

    /// Whether this stream's work should stop.
    fn stopped(&self) -> bool {
        self.ctl.is_stopped() || self.weak.strong_count() == 0
    }

    /// The account consent of the origin client through `via`, before a token is asked (19.4,
    /// 19.7). A preview has no accounts; a home with no app or no accounts is `no-app`.
    fn consent(&self) -> Result<(), End> {
        if !self.request.openai {
            return Ok(());
        }
        let with = |v: Value| Err(End::With(v));
        let Mode::Home { app } = &self.mode else {
            return with(json!("no-account"));
        };
        if app.as_ref().is_none_or(|app| app.accounts().is_none()) {
            return with(json!("no-app"));
        }
        let consent = Consent::Account {
            via: self.via.identity.clone(),
            account: "openai".into(),
        };
        let host = self.weak.upgrade().ok_or(End::Stop)?;
        if host.consent_held(&self.origin, &consent) {
            return Ok(());
        }
        let what = consent.to_string();
        let origin = host
            .local_of(&self.origin)
            .ok_or(End::With(json!("gone")))?;
        self.push_sync(
            json!({"type": "consent-pending", "what": what})
                .to_string()
                .into_bytes(),
        )?;
        let request = ConsentRequest {
            origin,
            via: self.via.local.clone(),
            consent: consent.clone(),
            profile: None,
            functions: vec![],
        };
        drop(host);
        match super::consent::ask(&self.mode, &self.hub, || self.stopped(), request) {
            Asked::Granted => {
                let host = self.weak.upgrade().ok_or(End::Stop)?;
                host.store_consent(&self.origin, self.origin_gen, consent, None)
                    .map_err(|()| End::With(json!("gone")))?;
                if self.stopped() {
                    return Err(End::Stop);
                }
                Ok(())
            }
            Asked::Refused => with(json!({"consent-refused": what})),
            Asked::Stopped => with(json!("gone")),
        }
    }

    /// A fresh token of the account `openai` (19.4): the app's, asked once per request.
    async fn token(&self) -> Result<String, End> {
        let Mode::Home { app: Some(app) } = &self.mode else {
            return Err(End::With(json!("no-app")));
        };
        let accounts = app.accounts().ok_or(End::With(json!("no-app")))?;
        let token = tokio::select! {
            t = accounts.openai_token() => t,
            () = self.ctl.stopped() => return Err(End::Stop),
        };
        token.map_err(|e| {
            End::With(match e {
                AccountError::NoAccount => json!("no-account"),
                AccountError::Unavailable(m) => failed(format!("account unavailable: {m}")),
            })
        })
    }

    #[cfg(feature = "test-net")]
    fn route(&self, name: &str) -> Option<std::net::SocketAddr> {
        let net = self.net.as_ref()?;
        net.routes.iter().find(|(n, _)| n == name).map(|(_, a)| *a)
    }
    #[cfg(not(feature = "test-net"))]
    fn route(&self, _: &str) -> Option<std::net::SocketAddr> {
        None
    }
    /// The default resolver connects to the real name; a test route (19.9) names an address.
    fn target(&self) -> Target {
        match self.route(&self.request.name) {
            Some(addr) => Target::Addr(addr),
            None => Target::Name(self.request.name.clone(), self.request.port),
        }
    }

    fn connector(&self) -> Result<TlsConnector, String> {
        use rustls::{ClientConfig, RootCertStore, crypto};
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        #[cfg(feature = "test-net")]
        if let Some(net) = &self.net {
            use rustls::pki_types::CertificateDer;
            use rustls::pki_types::pem::PemObject;
            for cert in CertificateDer::pem_slice_iter(net.root_pem.as_bytes()) {
                let cert = cert.map_err(|e| format!("the test root: {e}"))?;
                roots.add(cert).map_err(|e| format!("the test root: {e}"))?;
            }
        }
        let config =
            ClientConfig::builder_with_provider(Arc::new(crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
        Ok(TlsConnector::from(Arc::new(config)))
    }

    /// Awaits network work: it ends when the stream does, and with `idle-timeout` when it takes
    /// longer than `idle-s` (19.4, Idle).
    async fn within<T>(&self, work: impl Future<Output = T>) -> Result<T, End> {
        tokio::select! {
            r = tokio::time::timeout(self.request.idle, work) => {
                r.map_err(|_| End::With(json!("idle-timeout")))
            }
            () = self.ctl.stopped() => Err(End::Stop),
        }
    }

    /// The connection, the request and the items.
    async fn stream(&self) -> Result<Value, End> {
        let request = &self.request;
        // A stream that ended while the consent waited sends nothing (19.4, Ends).
        if self.stopped() {
            return Err(End::Stop);
        }
        let token = if request.openai {
            Some(self.token().await?)
        } else {
            None
        };
        let connector = self.connector().map_err(|m| End::With(failed(m)))?;
        let name = ServerName::try_from(request.name.clone())
            .map_err(|_| End::With(failed("the host is not a valid TLS name")))?;
        let connect = async {
            let tcp = match self.target() {
                Target::Addr(a) => tokio::net::TcpStream::connect(a).await,
                Target::Name(n, p) => tokio::net::TcpStream::connect((n.as_str(), p)).await,
            }
            .map_err(|e| format!("cannot connect to {}: {e}", request.name))?;
            let _ = tcp.set_nodelay(true);
            connector
                .connect(name, tcp)
                .await
                .map_err(|e| format!("TLS with {} failed: {e}", request.name))
        };
        let tls = self
            .within(connect)
            .await?
            .map_err(|m| End::With(failed(m)))?;
        let (mut sender, connection) = self
            .within(hyper::client::conn::http1::handshake::<_, Full<Bytes>>(
                TokioIo::new(tls),
            ))
            .await?
            .map_err(|e| End::With(failed(format!("HTTP: {e}"))))?;
        let driver = tokio::spawn(async move {
            let _ = connection.await;
        });
        let result = self.exchange(&mut sender, token).await;
        driver.abort();
        result
    }

    async fn exchange(
        &self,
        sender: &mut hyper::client::conn::http1::SendRequest<Full<Bytes>>,
        token: Option<String>,
    ) -> Result<Value, End> {
        let request = &self.request;
        if self.stopped() {
            return Err(End::Stop);
        }
        let authority = if request.grant == request.name {
            request.name.clone()
        } else {
            request.grant.clone()
        };
        let mut built = hyper::Request::builder()
            .method(request.method.clone())
            .uri(request.target.as_str())
            .header(hyper::header::HOST, authority)
            .header(hyper::header::ACCEPT_ENCODING, "identity");
        for (name, value) in &request.headers {
            built = built.header(name, value);
        }
        if let Some(token) = token {
            // The token is in this header and nowhere else (19.4).
            let mut value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
                End::With(failed(
                    "account unavailable: the token is not a header value",
                ))
            })?;
            value.set_sensitive(true);
            built = built.header(hyper::header::AUTHORIZATION, value);
        }
        let built = built
            .body(Full::new(Bytes::from(request.body.clone())))
            .map_err(|e| End::With(failed(format!("request: {e}"))))?;
        let response = self
            .within(sender.send_request(built))
            .await?
            .map_err(|e| End::With(failed(format!("HTTP: {e}"))))?;
        let (parts, mut body) = response.into_parts();
        self.push(head_item(&parts).into_bytes()).await?;
        let sse = parts
            .headers
            .get(hyper::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|t| t.trim().eq_ignore_ascii_case("text/event-stream"))
            });
        let mut framer = if sse {
            Framer::Sse(Sse::default())
        } else {
            Framer::Chunks(Vec::new())
        };
        loop {
            let frame = match self.within(body.frame()).await {
                Ok(f) => f,
                Err(End::With(outcome)) => {
                    self.send_all(framer.finish()).await?;
                    return Ok(outcome);
                }
                Err(End::Stop) => return Err(End::Stop),
            };
            match frame {
                None => {
                    self.send_all(framer.finish()).await?;
                    return Ok(json!("closed"));
                }
                Some(Err(e)) => {
                    self.send_all(framer.finish()).await?;
                    return Ok(failed(format!("the response ended with an error: {e}")));
                }
                Some(Ok(frame)) => {
                    let Ok(data) = frame.into_data() else {
                        continue;
                    };
                    match framer.feed(&data) {
                        Ok(items) => self.send_all(items).await?,
                        Err(m) => return Ok(failed(m)),
                    }
                }
            }
        }
    }

    async fn send_all(&self, items: Vec<Vec<u8>>) -> Result<(), End> {
        for item in items {
            self.push(item).await?;
        }
        Ok(())
    }
    /// Pushes one item and waits while the stream is full: the connection is not read meanwhile
    /// (19.2, In flight). The idle clock does not run.
    async fn push(&self, body: Vec<u8>) -> Result<(), End> {
        loop {
            match self.hub.host_push(&self.id, &body) {
                Ok(()) => return Ok(()),
                Err(StreamError::Full) => {
                    tokio::select! {
                        () = tokio::time::sleep(FULL_WAIT) => {}
                        () = self.ctl.stopped() => return Err(End::Stop),
                    }
                }
                Err(StreamError::TooLarge) => {
                    return Err(End::With(failed("an item is over 4 MiB")));
                }
                Err(_) => return Err(End::Stop),
            }
        }
    }
    /// `push` for the thread before the runtime.
    fn push_sync(&self, body: Vec<u8>) -> Result<(), End> {
        loop {
            match self.hub.host_push(&self.id, &body) {
                Ok(()) => return Ok(()),
                Err(StreamError::Full) if !self.stopped() => std::thread::sleep(FULL_WAIT * 2),
                Err(_) => return Err(End::Stop),
            }
        }
    }
}

/// The first item (19.4): `{"status":200,"headers":{"name":"value",...}}`. Names are lower case;
/// a repeated header is one entry with its values joined by `, `, in the order sent.
fn head_item(parts: &hyper::http::response::Parts) -> String {
    let mut pairs = vec![];
    for name in parts.headers.keys() {
        let values: Vec<String> = parts
            .headers
            .get_all(name)
            .iter()
            .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
            .collect();
        pairs.push(format!(
            "{}:{}",
            Value::String(name.as_str().to_owned()),
            Value::String(values.join(", "))
        ));
    }
    format!(
        "{{\"status\":{},\"headers\":{{{}}}}}",
        parts.status.as_u16(),
        pairs.join(",")
    )
}

/// How a body becomes items (19.4).
enum Framer {
    Sse(Sse),
    /// A body that is not SSE: items of 64 KiB, the last one shorter.
    Chunks(Vec<u8>),
}
impl Framer {
    /// The items that `bytes` completes; `Err` is the text of a `failed` outcome.
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
        match self {
            Self::Sse(sse) => sse.feed(bytes),
            Self::Chunks(buf) => {
                buf.extend_from_slice(bytes);
                let mut out = vec![];
                while buf.len() >= CHUNK {
                    out.push(buf.drain(..CHUNK).collect());
                }
                Ok(out)
            }
        }
    }
    /// What is left at the end of the body or of the stream: a short chunk. An event that was not
    /// finished by an empty line is dropped, as the SSE rules say.
    fn finish(&mut self) -> Vec<Vec<u8>> {
        match self {
            Self::Sse(_) => vec![],
            Self::Chunks(buf) if buf.is_empty() => vec![],
            Self::Chunks(buf) => vec![std::mem::take(buf)],
        }
    }
}

/// The server-sent events parser (WHATWG HTML, 9.2.6), reduced to the `data` of each event.
#[derive(Debug, Default)]
struct Sse {
    line: Vec<u8>,
    data: Vec<u8>,
    has_data: bool,
    /// The last byte was a CR: a following LF belongs to the same line end.
    after_cr: bool,
    started: bool,
}
impl Sse {
    fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, &'static str> {
        let mut out = vec![];
        for &b in bytes {
            if std::mem::take(&mut self.after_cr) && b == b'\n' {
                continue;
            }
            match b {
                b'\r' => {
                    self.after_cr = true;
                    self.end_line(&mut out);
                }
                b'\n' => self.end_line(&mut out),
                _ => {
                    self.line.push(b);
                    if self.line.len() + self.data.len() > MAX_ITEM {
                        return Err("a server-sent event is over 4 MiB");
                    }
                }
            }
        }
        Ok(out)
    }
    fn end_line(&mut self, out: &mut Vec<Vec<u8>>) {
        let mut line = std::mem::take(&mut self.line);
        if !std::mem::replace(&mut self.started, true) && line.starts_with(&[0xef, 0xbb, 0xbf]) {
            line.drain(..3);
        }
        if line.is_empty() {
            if std::mem::take(&mut self.has_data) {
                let mut data = std::mem::take(&mut self.data);
                data.pop();
                out.push(data);
            }
            return;
        }
        if line[0] == b':' {
            return;
        }
        let (field, value) = match line.iter().position(|&b| b == b':') {
            Some(i) => {
                let value = &line[i + 1..];
                (&line[..i], value.strip_prefix(b" ").unwrap_or(value))
            }
            None => (&*line, &[][..]),
        };
        if field == b"data" {
            self.data.extend_from_slice(value);
            self.data.push(b'\n');
            self.has_data = true;
        }
    }
}

impl Host {
    /// The mod's granted `Http` hosts (6.8), as the grants name them.
    pub(crate) fn http_grants(&self, local: &crate::LocalName) -> Vec<String> {
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .filter(|m| &m.local == local)
            .flat_map(|m| m.grants.iter())
            .filter(|g| g.granted)
            .filter_map(|g| match &g.grant {
                watane_pkg::Grant::Http { host } => Some(host.as_str().to_owned()),
                _ => None,
            })
            .collect()
    }

    /// Applies a `start` of a committed chain: the stream, and the thread with the connection
    /// (19.6, At apply). The catalog lock is held by the caller, which passes `valid`.
    pub(crate) fn https_start(
        &self,
        (id, consumer): (String, ModRef),
        (origin, origin_gen): (String, Option<u64>),
        request: Arc<Request>,
        valid: bool,
    ) -> Option<Job> {
        let ctl = Arc::new(Control::new());
        let hook = Arc::clone(&ctl);
        self.hub.open_host(
            id.clone(),
            consumer.clone(),
            (origin.clone(), origin_gen),
            Box::new(move || hook.stop()),
        );
        if !valid {
            self.hub.end_gone(&[id]);
            return None;
        }
        self.https.lock().insert(
            id.clone(),
            Entry {
                origin: origin.clone(),
                consumer: consumer.clone(),
                host: request.grant.clone(),
                openai: request.openai,
                ctl: Arc::clone(&ctl),
            },
        );
        Some(Job {
            id,
            hub: Arc::clone(&self.hub),
            weak: Weak::clone(&self.weak),
            live: Arc::clone(&self.https),
            ctl,
            request,
            origin,
            origin_gen,
            via: consumer,
            mode: self.home.mode.clone(),
            #[cfg(feature = "test-net")]
            net: self
                .test_net
                .lock()
                .expect("lock poisoned: another thread panicked while holding it")
                .clone(),
        })
    }

    /// Starts the thread of a stream, after every effect of the chain is applied, so that a
    /// `cancel` of the same chain ends the stream before a byte leaves (19.3, At apply).
    pub(crate) fn https_run(&self, job: Job) {
        let id = job.id.clone();
        let spawned = std::thread::Builder::new()
            .name("watane-https".into())
            .spawn(move || job.go());
        if spawned.is_err() {
            self.https.lock().remove(&id);
            self.hub
                .finish(&id, failed("cannot start a thread for https-stream"));
        }
    }

    /// An account entry changed (19.4, Lifetime): the live streams that used it end `gone`. In
    /// stage A4 only `OpenAi` has streams.
    pub fn account_changed(&self, account: &Account) {
        if let Account::OpenAi = account {
            self.hub.end_gone(&self.https.openai());
        }
    }

    /// Ends the streams whose consumer lost the `Http` grant of their host (19.2, Ends). Called
    /// after every catalog write.
    pub(crate) fn end_lost_https(&self) {
        let live: Vec<(String, ModRef, String)> = self
            .https
            .lock()
            .iter()
            .map(|(id, e)| (id.clone(), e.consumer.clone(), e.host.clone()))
            .collect();
        if live.is_empty() {
            return;
        }
        let lost: Vec<String> = {
            let catalog = self
                .catalog
                .lock()
                .expect("lock poisoned: another thread panicked while holding it");
            live.into_iter()
                .filter(|(_, consumer, host)| {
                    !catalog.mods.iter().any(|m| {
                        m.local == consumer.local
                            && m.identity == consumer.identity
                            && m.enabled
                            && m.grants.iter().any(|g| {
                                g.granted
                                    && matches!(&g.grant,
                                        watane_pkg::Grant::Http { host: h } if h.as_str() == host)
                            })
                    })
                })
                .map(|(id, ..)| id)
                .collect()
        };
        self.hub.end_gone(&lost);
    }

    /// Tests only (19.9): names that `https-stream` connects to a local address, and a root.
    #[cfg(feature = "test-net")]
    pub fn set_test_net(&self, net: TestNet) {
        *self
            .test_net
            .lock()
            .expect("lock poisoned: another thread panicked while holding it") = Some(net);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grants() -> Vec<String> {
        vec!["api.openai.com".into(), "other.test:8443".into()]
    }
    fn parse(text: &str) -> Result<Request, Refusal> {
        Request::parse(text, &grants())
    }

    #[test]
    fn ac8_without_a_test_route_a_name_connects_to_the_real_name() {
        let request =
            parse(r#"{"url":"https://api.openai.com/v1/models","method":"GET"}"#).unwrap();
        let job = Job {
            id: "i".into(),
            hub: Hub::new(),
            weak: Weak::new(),
            live: Arc::default(),
            ctl: Arc::new(Control::new()),
            request: Arc::new(request),
            origin: "o".into(),
            origin_gen: None,
            via: ModRef {
                local: crate::LocalName::parse("m").unwrap(),
                identity: "local/m".into(),
            },
            mode: Mode::Preview,
            #[cfg(feature = "test-net")]
            net: None,
        };
        assert_eq!(job.target(), Target::Name("api.openai.com".into(), 443));
    }

    #[test]
    fn ac1_sse_events_are_the_data_text_of_each_event() {
        let mut sse = Sse::default();
        let wire = b"\xEF\xBB\xBF: hi\r\nevent: e\r\ndata: one\r\n\r\ndata:two\ndata: three\n\ndata\n\nid: 1\n\ndata: x\n";
        // Split in the middle of a CRLF and of a field.
        let (a, b) = wire.split_at(20);
        let mut items = sse.feed(a).unwrap();
        items.extend(sse.feed(b).unwrap());
        assert_eq!(
            items,
            vec![b"one".to_vec(), b"two\nthree".to_vec(), Vec::new()]
        );
    }

    #[test]
    fn ac1_a_body_that_is_not_sse_is_cut_in_chunks_of_64_kib() {
        let mut framer = Framer::Chunks(vec![]);
        let body: Vec<u8> = (0..150 * 1024).map(|i| (i % 251) as u8).collect();
        let mut items = vec![];
        for part in body.chunks(10_000) {
            items.extend(framer.feed(part).unwrap());
        }
        items.extend(framer.finish());
        assert_eq!(
            items.iter().map(Vec::len).collect::<Vec<_>>(),
            [65536, 65536, 22528]
        );
        assert_eq!(items.concat(), body);
    }

    #[test]
    fn ac2_the_host_is_read_as_6_8_reads_it() {
        let url = |u: &str| format!(r#"{{"url":"{u}","method":"GET"}}"#);
        parse(&url("https://API.OpenAI.com/x")).unwrap();
        parse(&url("https://api.openai.com./x")).unwrap();
        parse(&url("https://api.openai.com:443/x")).unwrap();
        parse(&url("https://other.test:8443/x")).unwrap();
        for refused in [
            "https://api.openai.com../x",
            "https://other.test/x",
            "https://user@api.openai.com/x",
            "https://:p@api.openai.com/x",
            "https://api.openai.com:8443/x",
            "http://api.openai.com/x",
            "https://[::1]/x",
        ] {
            assert_eq!(
                parse(&url(refused)).unwrap_err(),
                Refusal::Destination,
                "{refused}"
            );
        }
    }

    #[test]
    fn ac2_the_names_the_host_sets_itself_are_forbidden_and_a_bad_value_is_bad_launch() {
        let with = |headers: &str| {
            parse(&format!(
                r#"{{"url":"https://api.openai.com/x","method":"GET","headers":{headers}}}"#
            ))
        };
        for name in [
            "Content-Length",
            "transfer-encoding",
            "Connection",
            "Keep-Alive",
            "UPGRADE",
            "te",
            "Trailer",
            "Accept-Encoding",
            "Proxy-",
        ] {
            assert_eq!(
                with(&format!(r#"{{"{name}":"v"}}"#)).unwrap_err(),
                Refusal::ForbiddenHeader(name.into()),
                "{name}"
            );
        }
        with(r#"{"x-te":"v","accept":"v"}"#).unwrap();
        assert!(matches!(
            with(r#"{"x-a":"a\u0001b"}"#),
            Err(Refusal::BadLaunch { ref field, .. }) if field == "headers"
        ));
        assert!(matches!(
            parse(r#"{"url":"https://api.openai.com/x","method":"GET","extra":1}"#),
            Err(Refusal::BadLaunch { ref field, ref reason }) if field == "extra" && reason == "unknown field"
        ));
        let defaulted =
            parse(r#"{"url":"https://api.openai.com/x","method":"GET","idle-s":null}"#).unwrap();
        assert_eq!(defaulted.idle, Duration::from_secs(DEFAULT_IDLE));
    }

    #[test]
    fn ac1_idle_s_is_from_1_to_300() {
        let with = |idle: &str| {
            parse(&format!(
                r#"{{"url":"https://api.openai.com/x","method":"GET","idle-s":{idle}}}"#
            ))
        };
        with("1").unwrap();
        with("300").unwrap();
        for idle in ["0", "301", "-1", "1.5", r#""9""#] {
            assert!(
                matches!(with(idle), Err(Refusal::BadLaunch { ref field, .. }) if field == "idle-s"),
                "{idle}"
            );
        }
    }
}
