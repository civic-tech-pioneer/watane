use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_call_refuses_wrong_method_origin_type_size_and_body() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    installed(preview.install(&fixture("counter")).await);
    installed(preview.install(&fixture("hello")).await);
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    let url = format!("{origin}/call");
    let client = preview.client("counter.localhost", "127.0.0.1");
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(
        response
            .headers()
            .get("allow")
            .and_then(|v| v.to_str().ok()),
        Some("POST")
    );
    call_error(response, 405, "bad_request").await;
    for wrong_origin in [
        None,
        Some(format!("http://hello.localhost:{}", preview.ready.port)),
        Some(format!("http://localhost:{}", preview.ready.port)),
        Some(format!("{origin}/")),
        Some(origin.replace("http:", "https:")),
        Some("null".into()),
        Some(format!("{origin}.evil.example")),
    ] {
        let mut request = client.post(&url).json(&json!({"fn":"api.add","args":null}));
        if let Some(wrong_origin) = wrong_origin {
            request = request.header("Origin", wrong_origin);
        }
        call_error(request.send().await.unwrap(), 403, "forbidden").await;
    }
    for content_type in [None, Some("text/plain")] {
        let mut request = client
            .post(&url)
            .header("Origin", &origin)
            .body(r#"{"fn":"api.add","args":null}"#);
        if let Some(content_type) = content_type {
            request = request.header("Content-Type", content_type);
        }
        call_error(request.send().await.unwrap(), 415, "bad_request").await;
    }
    let response = client
        .post(&url)
        .header("Origin", &origin)
        .header(
            "Origin",
            format!("http://hello.localhost:{}", preview.ready.port),
        )
        .json(&json!({"fn":"api.add","args":null}))
        .send()
        .await
        .unwrap();
    call_error(response, 403, "forbidden").await;
    // The size refusal may close the connection before the body is written.
    let oversized = format!(
        "POST /call HTTP/1.1\r\nHost: counter.localhost:{}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: 2097152\r\nConnection: close\r\n\r\n{}",
        preview.ready.port,
        " ".repeat(2 << 20)
    );
    if let Some(reply) = raw(&preview, "127.0.0.1", oversized.as_bytes()) {
        assert_eq!(reply.header("connection"), Some("close"));
        reply.error(413, "bad_request");
    }
    for body in [
        r#"{"fn":"api.add","args":null,"caller":"agent:evil"}"#,
        r#"{"fn":42,"args":null}"#,
        r#"{"fn":"api.add","args":42}"#,
        r#"{"fn":"api.add","args":true}"#,
        r#"{"fn":"api.add","args":"bad"}"#,
        r#"{"fn":"api.add"}"#,
        r#"{"args":null}"#,
        "broken",
        "[]",
        "null",
        r#"["api.add",null]"#,
    ] {
        let response = client
            .post(&url)
            .header("Origin", &origin)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        call_error(response, 400, "bad_request").await;
    }
    // Refused requests must not run the function or change its state.
    let response = client
        .post(&url)
        .header("Origin", &origin)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(r#"{"fn":"api.get","args":null}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    no_cors(response.headers());
    pages::headers(&response);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"value":0}));
}

use std::{io::Write, net::TcpStream};

fn no_cors(headers: &reqwest::header::HeaderMap) {
    assert!(!headers.contains_key("access-control-allow-origin"));
}

fn installed(response: reqwest::Response) {
    no_cors(response.headers());
    assert_eq!(response.status(), 200);
}

async fn call_error(response: reqwest::Response, status: u16, code: &str) {
    no_cors(response.headers());
    pages::call_error(response, status, code).await;
}

fn setup() -> (TempDir, Preview) {
    let home = tempfile::tempdir().unwrap();
    let counter = fixture("counter");
    let hello = fixture("hello");
    let preview = Preview::start_with(
        home.path(),
        &[
            "--install".as_ref(),
            counter.as_os_str(),
            "--install".as_ref(),
            hello.as_os_str(),
        ],
    );
    (home, preview)
}

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn plain(&self, status: u16, body: &str) {
        assert_eq!(self.status, status);
        assert_eq!(
            self.header("content-type"),
            Some("text/plain; charset=utf-8")
        );
        assert_eq!(self.body, body.as_bytes());
    }

    fn error(&self, status: u16, code: &str) {
        assert_eq!(self.status, status);
        assert_eq!(self.header("content-type"), Some("application/json"));
        let body: Value = serde_json::from_slice(&self.body).unwrap();
        assert_eq!(body["error"]["code"], code);
    }
}

// HTTP/1.1 bytes bypass URL and header normalization. Connection: close gives
// a bounded response; SSE uses reqwest separately so its body stays open.
fn raw(preview: &Preview, ip: &str, request: &[u8]) -> Option<Reply> {
    exchange(preview, ip, request, false)
}

fn exchange(preview: &Preview, ip: &str, request: &[u8], end_body: bool) -> Option<Reply> {
    let mut stream = TcpStream::connect((ip, preview.ready.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    if let Err(error) = stream.write_all(request) {
        assert!(
            matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ),
            "{error}"
        );
    }
    if end_body {
        let _ = stream.shutdown(std::net::Shutdown::Write);
    }
    let mut bytes = Vec::new();
    if let Err(error) = stream.read_to_end(&mut bytes) {
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset, "{error}");
    }
    if bytes.is_empty() {
        return None;
    }
    let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = std::str::from_utf8(&bytes[..end]).unwrap();
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: Vec<_> = lines
        .map(|line| {
            let (name, value) = line.split_once(':').unwrap();
            (name.to_ascii_lowercase(), value.trim().to_owned())
        })
        .collect();
    assert!(
        !headers
            .iter()
            .any(|(key, _)| key == "access-control-allow-origin"),
        "{head}"
    );
    let mut body = bytes[end + 4..].to_vec();
    if headers
        .iter()
        .any(|(key, value)| key == "transfer-encoding" && value == "chunked")
    {
        let mut input = body.as_slice();
        let mut decoded = Vec::new();
        loop {
            let end = input.windows(2).position(|w| w == b"\r\n").unwrap();
            let size =
                usize::from_str_radix(std::str::from_utf8(&input[..end]).unwrap(), 16).unwrap();
            if size == 0 {
                break;
            }
            input = &input[end + 2..];
            decoded.extend_from_slice(&input[..size]);
            input = &input[size + 2..];
        }
        body = decoded;
    }
    Some(Reply {
        status,
        headers,
        body,
    })
}

fn request(
    preview: &Preview,
    method: &str,
    host: &str,
    path: &str,
    extra: &str,
    body: &str,
) -> Reply {
    raw(preview, "127.0.0.1", format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{extra}Content-Length: {}\r\n\r\n{body}", body.len()
    ).as_bytes()).expect("response")
}

fn origin(preview: &Preview, local: &str) -> String {
    format!("http://{local}.localhost:{}", preview.ready.port)
}

fn counter_call(preview: &Preview, function: &str, value: u32) {
    let reply = request(
        preview,
        "POST",
        &format!("counter.localhost:{}", preview.ready.port),
        "/call",
        &format!(
            "Origin: {}\r\nContent-Type: application/json\r\n",
            origin(preview, "counter")
        ),
        &format!(r#"{{"fn":"{function}","args":null}}"#),
    );
    assert_eq!(reply.status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&reply.body).unwrap(),
        json!({"value":value})
    );
}

#[test]
fn ac1_host_is_exact_on_both_loopbacks_and_unknown_mod_is_missing() {
    let (_home, preview) = setup();
    let port = preview.ready.port;
    for ip in ["127.0.0.1", "::1"] {
        for host in [
            format!("Host: COUNTER.localhost:{port}\r\n"),
            "Host: counter.localhost\r\n".into(),
            format!("Host: counter.localhost.:{port}\r\n"),
            format!("Host: counter.localhost:{port}.evil\r\n"),
            format!("Host: evil.example:{port}\r\n"),
            format!("Host: counter.localhost:{port}\r\nHost: hello.localhost:{port}\r\n"),
            String::new(),
        ] {
            let reply = raw(
                &preview,
                ip,
                format!("GET / HTTP/1.1\r\n{host}Connection: close\r\n\r\n").as_bytes(),
            )
            .unwrap();
            reply.plain(421, "unknown host");
        }
        for path in ["/", "/call", "/events"] {
            raw(&preview, ip, format!("GET {path} HTTP/1.1\r\nHost: ghost.localhost:{port}\r\nConnection: close\r\n\r\n").as_bytes()).unwrap().plain(404, "not found");
        }
    }
}

#[test]
fn ac2_cross_mod_calls_are_forbidden_and_leave_counter_unchanged() {
    let (_home, preview) = setup();
    for (target, source) in [("counter", "hello"), ("hello", "counter")] {
        request(
            &preview,
            "POST",
            &format!("{target}.localhost:{}", preview.ready.port),
            "/call",
            &format!(
                "Origin: {}\r\nContent-Type: application/json\r\n",
                origin(&preview, source)
            ),
            r#"{"fn":"api.add","args":null}"#,
        )
        .error(403, "forbidden");
        counter_call(&preview, "api.get", 0);
    }
}

#[test]
fn ac3_preflight_refuses_post_without_cors_headers() {
    let (_home, preview) = setup();
    let reply = request(
        &preview,
        "OPTIONS",
        &format!("counter.localhost:{}", preview.ready.port),
        "/call",
        &format!(
            "Origin: {}\r\nAccess-Control-Request-Method: POST\r\n",
            origin(&preview, "hello")
        ),
        "",
    );
    reply.error(405, "bad_request");
    assert_eq!(reply.header("allow"), Some("POST"));
    assert!(
        !reply
            .headers
            .iter()
            .any(|(key, _)| key.starts_with("access-control-allow-"))
    );
}

#[test]
fn ac4_chunked_and_declared_oversized_bodies_never_change_state() {
    let (_home, preview) = setup();
    let prefix = format!(
        "POST /call HTTP/1.1\r\nHost: counter.localhost:{}\r\nOrigin: {}\r\nContent-Type: application/json\r\nConnection: close\r\n",
        preview.ready.port,
        origin(&preview, "counter")
    );
    let mut chunked = format!("{prefix}Transfer-Encoding: chunked\r\n\r\n").into_bytes();
    // A valid mutating call padded to 2 MiB, split into 16 KiB chunks.
    let mut body = br#"{"fn":"api.add","args":null}"#.to_vec();
    body.resize(2 << 20, b' ');
    for chunk in body.chunks(16 << 10) {
        chunked.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
        chunked.extend_from_slice(chunk);
        chunked.extend_from_slice(b"\r\n");
    }
    chunked.extend_from_slice(b"0\r\n\r\n");
    raw(&preview, "127.0.0.1", &chunked)
        .expect("413 response")
        .error(413, "bad_request");
    counter_call(&preview, "api.get", 0);
    let short =
        format!("{prefix}Content-Length: 2097152\r\n\r\n{{\"fn\":\"api.add\",\"args\":null}}");
    if let Some(reply) = exchange(&preview, "127.0.0.1", short.as_bytes(), true) {
        reply.error(413, "bad_request");
    }
    counter_call(&preview, "api.get", 0);
}

#[test]
fn ac5_mod_and_api_routes_do_not_cross_origins() {
    let (home, preview) = setup();
    let catalog = fs::read(home.path().join("installed.json")).unwrap();
    let host = format!("counter.localhost:{}", preview.ready.port);
    for (method, path, headers, body) in [
        ("GET", "/api/status", String::new(), String::new()),
        (
            "POST",
            "/api/install",
            "Content-Type: application/json\r\n".into(),
            json!({"folders":[fixture("echo")]}).to_string(),
        ),
        (
            "POST",
            "/mcp/preview",
            format!(
                "Authorization: Bearer {}\r\nContent-Type: application/json\r\n",
                preview.ready.token.as_ref().unwrap()
            ),
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#.into(),
        ),
    ] {
        request(&preview, method, &host, path, &headers, &body).plain(404, "not found");
    }
    assert_eq!(
        fs::read(home.path().join("installed.json")).unwrap(),
        catalog
    );
    assert!(!home.path().join("mods/echo").exists());
    let api = format!("localhost:{}", preview.ready.port);
    for (method, path) in [("POST", "/call"), ("GET", "/events")] {
        request(
            &preview,
            method,
            &api,
            path,
            "Content-Type: application/json\r\n",
            r#"{"fn":"api.add","args":null}"#,
        )
        .error(404, "bad_request");
    }
    counter_call(&preview, "api.get", 0);
}

#[test]
fn ac6_raw_paths_cannot_escape_counter_page() {
    let (_home, preview) = setup();
    let host = format!("counter.localhost:{}", preview.ready.port);
    for path in [
        "/..%2fmods/hello/ui/index.html",
        "/%2e%2e%2f%2e%2e%2fhome.json",
        "/%5c..%5cinstalled.json",
        "/./index.html",
        "/index.html%00",
        "/%2fetc/passwd",
        "//etc/passwd",
        "/.%2e/lock",
    ] {
        request(&preview, "GET", &host, path, "", "").plain(404, "not found");
    }
    let reply = request(&preview, "GET", &host, "/", "", "");
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        fs::read(fixture("counter").join("ui/index.html")).unwrap()
    );
    assert_ne!(
        reply.body,
        fs::read(fixture("hello").join("ui/index.html")).unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac7_events_refuse_foreign_origins_and_do_not_leak_changes() {
    let (_home, preview) = setup();
    let counter = format!("counter.localhost:{}", preview.ready.port);
    for supplied in [origin(&preview, "hello"), "null".into()] {
        request(
            &preview,
            "GET",
            &counter,
            "/events",
            &format!("Origin: {supplied}\r\n"),
            "",
        )
        .error(403, "forbidden");
    }
    let hello = origin(&preview, "hello");
    let mut stream = preview
        .client("hello.localhost", "127.0.0.1")
        .get(format!("{hello}/events"))
        .header("Origin", &hello)
        .send()
        .await
        .unwrap();
    assert_eq!(stream.status(), 200);
    no_cors(stream.headers());
    assert_eq!(stream.headers()["content-type"], "text/event-stream");
    counter_call(&preview, "api.add", 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(500), stream.chunk())
            .await
            .is_err()
    );
}

#[test]
fn ac8_api_refuses_every_origin_before_any_route_runs() {
    let (home, preview) = setup();
    let catalog = fs::read(home.path().join("installed.json")).unwrap();
    for host in [
        format!("localhost:{}", preview.ready.port),
        format!("127.0.0.1:{}", preview.ready.port),
        format!("[::1]:{}", preview.ready.port),
    ] {
        for supplied in [
            origin(&preview, "counter"),
            origin(&preview, "hello"),
            format!("http://{host}"),
            "https://evil.example".into(),
            "null".into(),
            String::new(),
        ] {
            for (method, path, body) in [
                ("GET", "/api/status", String::new()),
                (
                    "POST",
                    "/api/install",
                    json!({"folders":[fixture("echo")]}).to_string(),
                ),
                ("GET", "/api/mods", String::new()),
                (
                    "POST",
                    "/api/plan",
                    json!({"folders":[fixture("echo")]}).to_string(),
                ),
                ("GET", "/api/missing", String::new()),
            ] {
                request(
                    &preview,
                    method,
                    &host,
                    path,
                    &format!("Origin: {supplied}\r\nContent-Type: application/json\r\n"),
                    &body,
                )
                .error(403, "forbidden");
            }
        }
    }
    assert_eq!(
        fs::read(home.path().join("installed.json")).unwrap(),
        catalog
    );
    counter_call(&preview, "api.get", 0);
}

#[test]
fn ac3_preview_token_cannot_authorize_another_toolset() {
    let (_home, preview) = setup();
    request(
        &preview,
        "POST",
        &format!("localhost:{}", preview.ready.port),
        "/mcp/other",
        &format!(
            "Authorization: Bearer {}\r\nContent-Type: application/json\r\n",
            preview.ready.token.as_ref().unwrap()
        ),
        r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
    )
    .error(403, "forbidden");
}

struct PageStream {
    response: reqwest::Response,
    buffered: Vec<u8>,
}

impl PageStream {
    async fn open(preview: &Preview) -> Self {
        let response = preview
            .client("counter.localhost", "127.0.0.1")
            .get(format!("{}/events", origin(preview, "counter")))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        no_cors(response.headers());
        pages::headers(&response);
        Self {
            response,
            buffered: Vec::new(),
        }
    }

    async fn frame(&mut self) -> String {
        loop {
            if let Some(end) = self.buffered.windows(2).position(|w| w == b"\n\n") {
                return String::from_utf8(self.buffered.drain(..end + 2).collect()).unwrap();
            }
            self.buffered
                .extend(self.response.chunk().await.unwrap().expect("stream ended"));
        }
    }

    async fn expect(&mut self, frame: &str) {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(3), self.frame())
                .await
                .unwrap(),
            frame
        );
    }

    async fn quiet(&mut self) {
        assert!(
            tokio::time::timeout(Duration::from_millis(500), self.frame())
                .await
                .is_err()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac7_s12_s13_program_serves_policy_and_changed_quiet_reload_events() {
    let (_home, preview) = setup();
    let response = preview
        .client("counter.localhost", "127.0.0.1")
        .get(format!("{}/", origin(&preview, "counter")))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    no_cors(response.headers());
    pages::headers(&response); // Includes the byte-for-byte CSP of S12.
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        fs::read(fixture("counter").join("ui/index.html")).unwrap()
    );
    let mut stream = PageStream::open(&preview).await;
    counter_call(&preview, "api.add", 1);
    let frame = tokio::time::timeout(Duration::from_secs(3), stream.frame())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(frame.strip_prefix("data: ").unwrap().trim_end()).unwrap(),
        json!({"type":"event","from":"counter","interface":"mods:counter/changes@0.1.0","function":"counted","args":{"count":1}})
    );
    stream.expect("data: {\"type\":\"changed\"}\n\n").await;
    counter_call(&preview, "api.get", 1);
    stream.quiet().await;
    let folder = tempfile::tempdir().unwrap();
    for file in [
        "mod.toml",
        "logic.wasm",
        "wit/world.wit",
        "wit/deps/watane-host/host.wit",
        "ui/index.html",
        "ui/app.js",
    ] {
        let dest = folder.path().join(file);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(fixture("counter").join(file), dest).unwrap();
    }
    fs::write(folder.path().join("ui/index.html"), "<p>S9 update</p>\n").unwrap();
    installed(preview.install(folder.path()).await);
    stream.expect("data: {\"type\":\"reload\"}\n\n").await;
    counter_call(&preview, "api.get", 1);
}

#[test]
fn ac9_test_plan_marks_subscription_and_revoke_as_tested() {
    let spec =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    let row = spec
        .lines()
        .find(|line| line.starts_with("| `tests/hostile.rs` |"))
        .unwrap();
    assert!(row.contains("`/call` with a subscription function → 404 (`ac12_s11_page_cannot_call_a_subscription_function`)"));
    assert!(row.contains(
        "`counter` without storage grant cannot read state that it wrote before the revoke (`ac8_s12a_storage_after_revoke_cannot_read_old_state`)"
    ));
}

#[test]
fn ac5_page_files_refuse_other_methods_only_when_they_exist() {
    let (_home, preview) = setup();
    let host = format!("counter.localhost:{}", preview.ready.port);
    for path in ["/", "/index.html", "/watane.js"] {
        let reply = request(&preview, "POST", &host, path, "", "");
        assert_eq!(reply.status, 405, "{path}");
        assert_eq!(reply.header("allow"), Some("GET, HEAD"), "{path}");
    }
    request(&preview, "POST", &host, "/missing.html", "", "").plain(404, "not found");
}

#[tokio::test]
async fn ac12_s11_page_cannot_call_a_subscription_function() {
    let home = tempfile::tempdir().unwrap();
    let host = watane_host::Host::open(
        home.path(),
        watane_host::Mode::Preview,
        watane_host::Limits::default(),
    )
    .unwrap();
    host.install_folders(&[fixture("recs"), fixture("shelf")], 0)
        .unwrap();
    let served = host.serve(0, axum::Router::new()).await.unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("recs.localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let origin = format!("http://recs.localhost:{}", served.port);
    let response = client
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"events.added","args":{"title":"forged"}}))
        .send()
        .await
        .unwrap();
    call_error(response, 404, "no_function").await;
    assert_eq!(
        host.call_async(
            watane_host::Caller::Page,
            watane_host::LocalName::parse("recs").unwrap(),
            "api.received".into(),
            Value::Null
        )
        .await
        .unwrap(),
        json!([])
    );
    served.shutdown().await;
}

#[tokio::test]
async fn ac8_s12a_storage_after_revoke_cannot_read_old_state() {
    use std::{future::Future, pin::Pin, sync::Arc};
    use watane_host::{
        Answer, Caller, Confirm, Host, InstallRequest, Limits, LocalName, Mode, Requester, Source,
    };
    struct App {
        requests: std::sync::Mutex<Vec<InstallRequest>>,
        answers: std::sync::Mutex<std::collections::VecDeque<Answer>>,
    }
    impl Confirm for App {
        fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
            self.requests.lock().unwrap().push(request);
            let answer = self
                .answers
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected confirmation");
            Box::pin(async move { answer })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let app = Arc::new(App {
        requests: std::sync::Mutex::new(vec![]),
        answers: std::sync::Mutex::new([Answer::Approve { withhold: vec![] }].into()),
    });
    let host = Host::open(
        dir.path(),
        Mode::Home {
            app: Some(app.clone()),
        },
        Limits::default(),
    )
    .unwrap();
    host.install(
        host.plan(vec![Source::Folder(fixture("counter"))]).unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    assert_eq!(app.requests.lock().unwrap().len(), 1);
    assert!(matches!(
        app.requests.lock().unwrap()[0].requester,
        Requester::App
    ));
    let local = LocalName::parse("counter").unwrap();
    for expected in 1..=3 {
        assert_eq!(
            host.call_async(Caller::Page, local.clone(), "api.add".into(), Value::Null)
                .await
                .unwrap(),
            expected
        );
    }
    host.revoke(&local, &watane_host::watane_pkg::Grant::Storage)
        .unwrap();
    let error = host
        .call_async(Caller::Page, local.clone(), "api.get".into(), Value::Null)
        .await
        .unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap()["code"], "not_granted");
    host.grant(&local, &watane_host::watane_pkg::Grant::Storage)
        .unwrap();
    assert_eq!(
        host.call_async(Caller::Page, local, "api.get".into(), Value::Null)
            .await
            .unwrap(),
        3
    );
}
