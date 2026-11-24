//! `ScriptedResponses`: a local https server that stands in for OpenAI's Responses API and for a judge, in tests (TECHNICAL 18.3).

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use rcgen::{
    BasicConstraints, CertificateParams, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};

const RESPONSES_NAME: &str = "api.openai.com";
const JUDGE_NAME: &str = "judge.test";

/// One request as a server received it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What the Responses origin answers to `POST /v1/responses`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reply {
    Sse(Vec<SseEvent>),
    Status {
        code: u16,
        body: String,
    },
    UsageLimit {
        resets_in_s: u64,
    },
    Redirect(String),
    Reset,
    Stall,
    Raw {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

/// One event of an event stream, sent as one write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SseEvent {
    Delta(String),
    ToolCalls(usize),
    Completed {
        input_tokens: u64,
        output_tokens: u64,
    },
    Failed {
        code: String,
    },
    Raw(String),
    Cut,
}

/// What the judge origin does with a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Play {
    Safe(Vec<String>),
    Unsafe(Vec<String>),
    Unsure(Vec<String>),
    Down,
    Slow {
        ms: u64,
        then: Box<Play>,
    },
    Unreadable,
    Redirect(String),
    Raw {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

/// What a connection does after its request is read.
enum Out {
    /// Write each chunk (flushed one by one), then close.
    Write(Vec<Vec<u8>>),
    Reset,
    Stall,
    Delay(u64, Box<Out>),
}

/// Script, secret and record of one origin.
struct Side<T> {
    script: Mutex<(Vec<T>, usize)>,
    secret: Mutex<Option<String>>,
    recorded: Mutex<Vec<Recorded>>,
    log: Mutex<Option<PathBuf>>,
    log_sub: &'static str,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().expect("a scripted-server mutex is never poisoned")
}

impl<T: Clone> Side<T> {
    fn new(default: T, log_sub: &'static str) -> Side<T> {
        Side {
            script: Mutex::new((vec![default], 0)),
            secret: Mutex::new(None),
            recorded: Mutex::new(Vec::new()),
            log: Mutex::new(None),
            log_sub,
        }
    }

    fn set_script(&self, items: Vec<T>) {
        *lock(&self.script) = (items, 0);
    }

    fn next(&self) -> Option<T> {
        let mut s = lock(&self.script);
        let i = s.1.min(s.0.len().saturating_sub(1));
        let item = s.0.get(i).cloned();
        s.1 += 1;
        item
    }

    fn record(&self, r: &Recorded) {
        let n = {
            let mut v = lock(&self.recorded);
            v.push(r.clone());
            v.len()
        };
        if let Some(dir) = lock(&self.log).clone() {
            let headers: Vec<[&str; 2]> = r
                .headers
                .iter()
                .map(|(k, v)| [k.as_str(), v.as_str()])
                .collect();
            let doc = serde_json::json!({
                "method": r.method,
                "path": r.path,
                "headers": headers,
                "body_b64": base64::engine::general_purpose::STANDARD.encode(&r.body),
            });
            let dir = dir.join(self.log_sub);
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join(format!("{n}.json")), doc.to_string());
        }
    }

    fn authorized(&self, r: &Recorded) -> bool {
        match lock(&self.secret).as_deref() {
            None => true,
            Some(want) => {
                let want = format!("Bearer {want}");
                r.headers
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case("authorization") && *v == want)
            }
        }
    }
}

/// The judge origin of a [`ScriptedResponses`].
pub struct Judge {
    side: Arc<Side<Play>>,
}

impl std::fmt::Debug for Judge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Judge")
            .field("count", &self.count())
            .finish()
    }
}

impl std::fmt::Debug for ScriptedResponses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptedResponses")
            .field("responses_addr", &self.responses_addr)
            .field("judge_addr", &self.judge_addr)
            .finish()
    }
}

impl Judge {
    /// Plays consumed in order; the last repeats.
    pub fn script(&self, plays: Vec<Play>) {
        self.side.set_script(plays);
    }

    /// The bearer key a request must carry.
    pub fn expect_key(&self, key: &str) {
        *lock(&self.side.secret) = Some(key.to_string());
    }

    /// Every request received, whatever the play.
    pub fn count(&self) -> usize {
        lock(&self.side.recorded).len()
    }

    pub fn requests(&self) -> Vec<Recorded> {
        lock(&self.side.recorded).clone()
    }
}

/// Both origins: Responses (`api.openai.com`) and judge (`judge.test`), on `127.0.0.1`.
pub struct ScriptedResponses {
    responses: Arc<Side<Reply>>,
    models: Arc<Mutex<Vec<String>>>,
    judge: Judge,
    responses_addr: SocketAddr,
    judge_addr: SocketAddr,
    ca_pem: String,
    stop: Arc<AtomicBool>,
}

impl ScriptedResponses {
    /// Binds a free port on `127.0.0.1` for each origin and serves.
    pub fn start() -> io::Result<ScriptedResponses> {
        let (ca_pem, issuer_key, issuer_params) = make_ca()?;
        let issuer = Issuer::new(issuer_params, issuer_key);
        let stop = Arc::new(AtomicBool::new(false));
        let call_ids = Arc::new(AtomicU64::new(0));

        let default_reply = Reply::Sse(vec![
            SseEvent::Delta("ok".into()),
            SseEvent::Completed {
                input_tokens: 1,
                output_tokens: 1,
            },
        ]);
        let responses = Arc::new(Side::new(default_reply, "responses"));
        let models = Arc::new(Mutex::new(Vec::<String>::new()));
        let judge_side = Arc::new(Side::new(Play::Safe(Vec::new()), "judge"));

        let (rl, responses_addr) = bind()?;
        let (jl, judge_addr) = bind()?;
        let rcfg = tls_config(RESPONSES_NAME, &issuer)?;
        let jcfg = tls_config(JUDGE_NAME, &issuer)?;

        let (side, models_for_route, ids) = (Arc::clone(&responses), Arc::clone(&models), call_ids);
        accept_loop(rl, rcfg, Arc::clone(&stop), Arc::clone(&side), move |r| {
            if !side.authorized(r) {
                return status_out(401, "unauthorized");
            }
            let path = r.path.split('?').next().unwrap_or("");
            match (r.method.as_str(), path) {
                ("POST", "/v1/responses") => side
                    .next()
                    .map_or_else(|| status_out(500, "no script"), |rep| reply_out(&rep, &ids)),
                ("GET", "/v1/models") => {
                    let data: Vec<_> = lock(&models_for_route)
                            .iter()
                            .map(|s| serde_json::json!({"id": s, "object": "model", "created": 0, "owned_by": "openai"}))
                            .collect();
                    json_out(200, &serde_json::json!({"object": "list", "data": data}))
                }
                _ => status_out(404, "not found"),
            }
        });
        let jside = Arc::clone(&judge_side);
        accept_loop(jl, jcfg, Arc::clone(&stop), Arc::clone(&jside), move |r| {
            if !jside.authorized(r) {
                return status_out(401, "unauthorized");
            }
            jside
                .next()
                .map_or_else(|| status_out(500, "no script"), |p| play_out(&p))
        });

        Ok(ScriptedResponses {
            responses,
            models,
            judge: Judge { side: judge_side },
            responses_addr,
            judge_addr,
            ca_pem,
            stop,
        })
    }

    pub fn responses_addr(&self) -> SocketAddr {
        self.responses_addr
    }

    pub fn judge_addr(&self) -> SocketAddr {
        self.judge_addr
    }

    /// Always `https://judge.test`.
    pub fn judge_origin(&self) -> String {
        format!("https://{JUDGE_NAME}")
    }

    /// The test authority's certificate: the only root a client needs.
    pub fn ca_pem(&self) -> String {
        self.ca_pem.clone()
    }

    /// The bearer token a Responses request must carry.
    pub fn expect_token(&self, token: &str) {
        *lock(&self.responses.secret) = Some(token.to_string());
    }

    /// Replies consumed in order; the last repeats.
    pub fn script(&self, replies: Vec<Reply>) {
        self.responses.set_script(replies);
    }

    /// The answer of `GET /v1/models`.
    pub fn models(&self, slugs: &[&str]) {
        *lock(&self.models) = slugs.iter().map(|s| s.to_string()).collect();
    }

    /// Responses requests in arrival order.
    pub fn requests(&self) -> Vec<Recorded> {
        lock(&self.responses.recorded).clone()
    }

    pub fn judge(&self) -> &Judge {
        &self.judge
    }

    /// Writes each request received from now on as `<dir>/responses/<n>.json` or `<dir>/judge/<n>.json`.
    pub fn log_to(&self, dir: &Path) {
        *lock(&self.responses.log) = Some(dir.to_path_buf());
        *lock(&self.judge.side.log) = Some(dir.to_path_buf());
    }
}

impl Drop for ScriptedResponses {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        for addr in [self.responses_addr, self.judge_addr] {
            let _ = TcpStream::connect_timeout(&addr, Duration::from_secs(1));
        }
    }
}

fn bind() -> io::Result<(TcpListener, SocketAddr)> {
    let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let a = l.local_addr()?;
    Ok((l, a))
}

fn other(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

fn make_ca() -> io::Result<(String, KeyPair, CertificateParams)> {
    let mut params = CertificateParams::new(Vec::<String>::new()).map_err(other)?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "watane scripted test authority");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let key = KeyPair::generate().map_err(other)?;
    let cert = params.self_signed(&key).map_err(other)?;
    Ok((cert.pem(), key, params))
}

fn tls_config(name: &str, issuer: &Issuer<'_, KeyPair>) -> io::Result<Arc<rustls::ServerConfig>> {
    let mut params = CertificateParams::new(vec![name.to_string()]).map_err(other)?;
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let key = KeyPair::generate().map_err(other)?;
    let cert = params.signed_by(&key, issuer).map_err(other)?;
    let der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
    let cfg = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(other)?
    .with_no_client_auth()
    .with_single_cert(vec![CertificateDer::from(cert.der().to_vec())], der)
    .map_err(other)?;
    Ok(Arc::new(cfg))
}

fn accept_loop<T: Clone + Send + Sync + 'static>(
    listener: TcpListener,
    cfg: Arc<rustls::ServerConfig>,
    stop: Arc<AtomicBool>,
    side: Arc<Side<T>>,
    route: impl Fn(&Recorded) -> Out + Send + Sync + 'static,
) {
    let route = Arc::new(route);
    thread::spawn(move || {
        for tcp in listener.incoming() {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            let Ok(tcp) = tcp else { continue };
            let (cfg, stop, side, route) = (
                Arc::clone(&cfg),
                Arc::clone(&stop),
                Arc::clone(&side),
                Arc::clone(&route),
            );
            thread::spawn(move || {
                let _ = serve_one(tcp, cfg, &stop, &side, &*route);
            });
        }
    });
}

fn serve_one<T: Clone>(
    tcp: TcpStream,
    cfg: Arc<rustls::ServerConfig>,
    stop: &AtomicBool,
    side: &Side<T>,
    route: &dyn Fn(&Recorded) -> Out,
) -> io::Result<()> {
    let conn = rustls::ServerConnection::new(cfg).map_err(other)?;
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let req = read_request(&mut tls)?;
    side.record(&req);
    let mut out = route(&req);
    loop {
        match out {
            Out::Delay(ms, then) => {
                sleep_unless_stopped(ms, stop);
                out = *then;
            }
            Out::Reset => return Ok(()),
            Out::Stall => {
                while !stop.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(50));
                }
                return Ok(());
            }
            Out::Write(chunks) => {
                for c in chunks {
                    tls.write_all(&c)?;
                    tls.flush()?;
                }
                tls.conn.send_close_notify();
                tls.flush()?;
                return Ok(());
            }
        }
    }
}

fn sleep_unless_stopped(ms: u64, stop: &AtomicBool) {
    let mut left = ms;
    while left > 0 && !stop.load(Ordering::SeqCst) {
        let step = left.min(50);
        thread::sleep(Duration::from_millis(step));
        left -= step;
    }
}

fn read_request(r: &mut impl Read) -> io::Result<Recorded> {
    let mut buf = Vec::new();
    let head_end = loop {
        if let Some(i) = find(&buf, b"\r\n\r\n") {
            break i;
        }
        fill(r, &mut buf)?;
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut rest = buf.split_off(head_end + 4);
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let method = first.next().unwrap_or("").to_string();
    let path = first.next().unwrap_or("").to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let header = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };
    let body = if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        let mut body = Vec::new();
        loop {
            let i = loop {
                if let Some(i) = find(&rest, b"\r\n") {
                    break i;
                }
                fill(r, &mut rest)?;
            };
            let size_text = String::from_utf8_lossy(&rest[..i]).into_owned();
            let size = usize::from_str_radix(size_text.split(';').next().unwrap_or("").trim(), 16)
                .map_err(other)?;
            rest.drain(..i + 2);
            while rest.len() < size + 2 {
                fill(r, &mut rest)?;
            }
            body.extend_from_slice(&rest[..size]);
            rest.drain(..size + 2);
            if size == 0 {
                break body;
            }
        }
    } else {
        let len = header("content-length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        while rest.len() < len {
            fill(r, &mut rest)?;
        }
        rest.truncate(len);
        rest
    };
    Ok(Recorded {
        method,
        path,
        headers,
        body,
    })
}

fn fill(r: &mut impl Read, buf: &mut Vec<u8>) -> io::Result<()> {
    let mut chunk = [0u8; 16384];
    let n = r.read(&mut chunk)?;
    if n == 0 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    buf.extend_from_slice(&chunk[..n]);
    Ok(())
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn reason(code: u16) -> &'static str {
    match code {
        200 => "OK",
        307 => "Temporary Redirect",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn head(code: u16, headers: &[(&str, String)]) -> Vec<u8> {
    let mut s = format!("HTTP/1.1 {code} {}\r\nconnection: close\r\n", reason(code));
    for (k, v) in headers {
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    s.push_str("\r\n");
    s.into_bytes()
}

fn full(code: u16, content_type: &str, extra: &[(&str, String)], body: &[u8]) -> Out {
    let mut hs = vec![
        ("content-type", content_type.to_string()),
        ("content-length", body.len().to_string()),
    ];
    hs.extend(extra.iter().cloned());
    let mut bytes = head(code, &hs);
    bytes.extend_from_slice(body);
    Out::Write(vec![bytes])
}

fn status_out(code: u16, body: &str) -> Out {
    full(code, "application/json", &[], body.as_bytes())
}

fn json_out(code: u16, v: &serde_json::Value) -> Out {
    full(code, "application/json", &[], v.to_string().as_bytes())
}

fn redirect_out(url: &str) -> Out {
    full(307, "text/plain", &[("location", url.to_string())], b"")
}

fn raw_out(status: u16, headers: &[(String, String)], body: &[u8]) -> Out {
    let mut s = format!("HTTP/1.1 {status} {}\r\n", reason(status));
    for (k, v) in headers {
        s.push_str(&format!("{k}: {v}\r\n"));
    }
    s.push_str("\r\n");
    let mut bytes = s.into_bytes();
    bytes.extend_from_slice(body);
    Out::Write(vec![bytes])
}

fn event(v: serde_json::Value) -> Vec<u8> {
    format!("data: {v}\n\n").into_bytes()
}

fn reply_out(reply: &Reply, ids: &AtomicU64) -> Out {
    match reply {
        Reply::Sse(events) => {
            let mut chunks = vec![head(
                200,
                &[
                    ("content-type", "text/event-stream".to_string()),
                    ("cache-control", "no-cache".to_string()),
                ],
            )];
            for e in events {
                match e {
                    SseEvent::Cut => break,
                    SseEvent::Delta(d) => chunks.push(event(serde_json::json!({
                        "type": "response.output_text.delta", "item_id": "msg_1", "output_index": 0, "content_index": 0, "delta": d
                    }))),
                    SseEvent::ToolCalls(n) => {
                        let mut one = Vec::new();
                        for i in 0..*n {
                            let id = ids.fetch_add(1, Ordering::SeqCst) + 1;
                            one.extend(event(serde_json::json!({
                                "type": "response.output_item.done", "output_index": i,
                                "item": {"type": "function_call", "id": format!("fc_{id}"), "call_id": format!("call_{id}"),
                                         "name": "tool", "arguments": "{}", "status": "completed"}
                            })));
                        }
                        chunks.push(one);
                    }
                    SseEvent::Completed { input_tokens, output_tokens } => chunks.push(event(serde_json::json!({
                        "type": "response.completed",
                        "response": {"id": "resp_1", "object": "response", "status": "completed",
                                     "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens,
                                               "total_tokens": input_tokens + output_tokens}}
                    }))),
                    SseEvent::Failed { code } => chunks.push(event(serde_json::json!({
                        "type": "response.failed",
                        "response": {"id": "resp_1", "object": "response", "status": "failed",
                                     "error": {"code": code, "message": code}}
                    }))),
                    SseEvent::Raw(line) => chunks.push(line.clone().into_bytes()),
                }
            }
            Out::Write(chunks)
        }
        Reply::Status { code, body } => full(*code, "application/json", &[], body.as_bytes()),
        Reply::UsageLimit { resets_in_s } => json_out(
            429,
            &serde_json::json!({"error": {
                "type": "usage_limit_reached", "code": "usage_limit_reached",
                "message": "The usage limit has been reached", "plan_type": "plus", "resets_in_seconds": resets_in_s
            }}),
        ),
        Reply::Redirect(url) => redirect_out(url),
        Reply::Reset => Out::Reset,
        Reply::Stall => Out::Stall,
        Reply::Raw {
            status,
            headers,
            body,
        } => raw_out(*status, headers, body),
    }
}

fn play_out(play: &Play) -> Out {
    let verdict = |v: &str, reasons: &[String]| {
        json_out(200, &serde_json::json!({"verdict": v, "reasons": reasons}))
    };
    match play {
        Play::Safe(r) => verdict("safe", r),
        Play::Unsafe(r) => verdict("unsafe", r),
        Play::Unsure(r) => verdict("unsure", r),
        Play::Down => Out::Reset,
        Play::Slow { ms, then } => Out::Delay(*ms, Box::new(play_out(then))),
        Play::Unreadable => full(200, "text/plain", &[], b"not a verdict"),
        Play::Redirect(url) => redirect_out(url),
        Play::Raw {
            status,
            headers,
            body,
        } => raw_out(*status, headers, body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac7_listens_on_loopback_only() {
        let s = ScriptedResponses::start().unwrap();
        assert!(s.responses_addr().ip().is_loopback() && s.judge_addr().ip().is_loopback());
        assert_eq!(s.responses_addr().ip(), Ipv4Addr::LOCALHOST);
        assert_eq!(s.judge_addr().ip(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn ac3_each_event_is_one_write_and_cut_ends_it() {
        let ids = AtomicU64::new(0);
        let reply = Reply::Sse(vec![
            SseEvent::Delta("a".into()),
            SseEvent::ToolCalls(2),
            SseEvent::Raw("x".into()),
            SseEvent::Cut,
            SseEvent::Delta("never".into()),
        ]);
        let Out::Write(chunks) = reply_out(&reply, &ids) else {
            panic!("an Sse reply writes")
        };
        // the head, then one write per event up to the cut
        assert_eq!(chunks.len(), 1 + 3);
        assert_eq!(chunks[2].iter().filter(|&&b| b == b'\n').count(), 4);
        assert_eq!(chunks[3], b"x");
    }

    #[test]
    fn ac_serde_forms() {
        let r: Vec<Reply> = serde_json::from_str(
            r#"[{"sse":[{"delta":"a"},{"tool-calls":2},{"completed":{"input_tokens":1,"output_tokens":2}},{"failed":{"code":"c"}},{"raw":"x"},"cut"]},"reset","stall",{"redirect":"u"},{"usage-limit":{"resets_in_s":60}}]"#,
        )
        .unwrap();
        assert_eq!(
            r,
            [
                Reply::Sse(vec![
                    SseEvent::Delta("a".into()),
                    SseEvent::ToolCalls(2),
                    SseEvent::Completed {
                        input_tokens: 1,
                        output_tokens: 2
                    },
                    SseEvent::Failed { code: "c".into() },
                    SseEvent::Raw("x".into()),
                    SseEvent::Cut,
                ]),
                Reply::Reset,
                Reply::Stall,
                Reply::Redirect("u".into()),
                Reply::UsageLimit { resets_in_s: 60 },
            ]
        );
        assert_eq!(
            serde_json::to_string(&Play::Slow {
                ms: 1,
                then: Box::new(Play::Down)
            })
            .unwrap(),
            r#"{"slow":{"ms":1,"then":"down"}}"#
        );
    }

    #[test]
    fn public_types_are_send_and_sync() {
        fn both<T: Send + Sync>() {}
        both::<ScriptedResponses>();
        both::<Judge>();
        both::<Recorded>();
        both::<Reply>();
        both::<Play>();
    }

    #[test]
    fn side_script_repeats_last() {
        let s = Side::new(1, "x");
        s.set_script(vec![1, 2]);
        assert_eq!([s.next(), s.next(), s.next()], [Some(1), Some(2), Some(2)]);
    }
}
