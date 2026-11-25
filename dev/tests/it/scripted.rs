//! Tests of `ScriptedResponses` and the program `scripted-responses` (TECHNICAL 18.3).
#![cfg(feature = "fake-claude")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::Duration;
use watane_dev::scripted::{Play, Reply, ScriptedResponses, SseEvent};

fn client(s: &ScriptedResponses) -> reqwest::blocking::Client {
    client_for(
        &s.ca_pem(),
        &s.responses_addr().to_string(),
        &s.judge_addr().to_string(),
    )
}

fn client_for(ca: &str, raddr: &str, jaddr: &str) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(ca.as_bytes()).unwrap())
        .resolve("api.openai.com", raddr.parse().unwrap())
        .resolve("judge.test", jaddr.parse().unwrap())
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

const R: &str = "https://api.openai.com/v1/responses";

fn post(c: &reqwest::blocking::Client, body: &str) -> reqwest::blocking::Response {
    c.post(R).body(body.to_string()).send().unwrap()
}

#[test]
fn ac1_two_origins_and_one_authority() {
    let s = ScriptedResponses::start().unwrap();
    assert_ne!(s.responses_addr(), s.judge_addr());
    assert_eq!(s.responses_addr().ip().to_string(), "127.0.0.1");
    assert_eq!(s.judge_addr().ip().to_string(), "127.0.0.1");
    assert_eq!(s.judge_origin(), "https://judge.test");
    assert_eq!(s.ca_pem().matches("-----BEGIN CERTIFICATE-----").count(), 1);
    let c = client(&s);
    assert_eq!(post(&c, "{}").status(), 200);
    assert_eq!(c.get("https://judge.test/x").send().unwrap().status(), 200);
    // DEV-S-61: a client that trusts only the system roots is refused at the handshake
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    assert!(!roots.is_empty(), "this machine has system roots");
    let cfg = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    let conn = rustls::ClientConnection::new(
        std::sync::Arc::new(cfg),
        "api.openai.com".try_into().unwrap(),
    )
    .unwrap();
    let tcp = std::net::TcpStream::connect(s.responses_addr()).unwrap();
    let mut tls = rustls::StreamOwned::new(conn, tcp);
    let err = std::io::Write::write_all(&mut tls, b"GET /v1/models HTTP/1.1\r\n\r\n")
        .and_then(|()| std::io::Write::flush(&mut tls))
        .unwrap_err();
    assert!(err.to_string().contains("UnknownIssuer"), "{err}");
}

#[test]
fn ac2_default_script_models_and_404() {
    let s = ScriptedResponses::start().unwrap();
    s.models(&["gpt-x", "gpt-y"]);
    let c = client(&s);
    let r = post(&c, "{}");
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "text/event-stream");
    let body = r.text().unwrap();
    let parts: Vec<&str> = body.split("\n\n").collect();
    assert_eq!(parts.len(), 3);
    assert!(parts[0].starts_with("data: {"));
    assert!(
        parts[0].contains(r#""type":"response.output_text.delta""#)
            && parts[0].contains(r#""delta":"ok""#)
    );
    assert!(
        parts[1].contains(r#""type":"response.completed""#) && parts[1].contains(r#""usage":{"#)
    );
    assert_eq!(parts[2], "");

    let m: serde_json::Value = c
        .get("https://api.openai.com/v1/models")
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(
        m,
        serde_json::json!({"object":"list","data":[
            {"id":"gpt-x","object":"model","created":0,"owned_by":"openai"},
            {"id":"gpt-y","object":"model","created":0,"owned_by":"openai"}]})
    );
    assert_json_error(
        c.get("https://api.openai.com/v1/other").send().unwrap(),
        404,
        "not found",
    );
    assert_json_error(c.put(R).send().unwrap(), 404, "not found");
    let reqs = s.requests();
    assert_eq!(reqs.len(), 4);
    assert_eq!(
        (reqs[2].method.as_str(), reqs[2].path.as_str()),
        ("GET", "/v1/other")
    );
    assert_eq!(
        (reqs[3].method.as_str(), reqs[3].path.as_str()),
        ("PUT", "/v1/responses")
    );
}

#[test]
fn ac2_script_in_order_last_repeats() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![
        Reply::Status {
            code: 503,
            body: "a".into(),
        },
        Reply::Status {
            code: 500,
            body: "b".into(),
        },
    ]);
    let c = client(&s);
    let codes: Vec<u16> = (0..4).map(|_| post(&c, "{}").status().as_u16()).collect();
    assert_eq!(codes, [503, 500, 500, 500]);
}

#[test]
fn ac3_sse_events_each_one_write() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![Reply::Sse(vec![
        SseEvent::Delta("a".into()),
        SseEvent::ToolCalls(3),
        SseEvent::Completed {
            input_tokens: 3,
            output_tokens: 1,
        },
        SseEvent::Failed {
            code: "boom".into(),
        },
        SseEvent::Raw("data: {oops\n\n".into()),
        SseEvent::Cut,
        SseEvent::Delta("never".into()),
    ])]);
    let body = post(&client(&s), "{}").text().unwrap();
    let events: Vec<&str> = body.split("\n\n").filter(|e| !e.is_empty()).collect();
    assert_eq!(events.len(), 1 + 3 + 1 + 1 + 1);
    assert_eq!(
        events[0],
        r#"data: {"content_index":0,"delta":"a","item_id":"msg_1","output_index":0,"type":"response.output_text.delta"}"#
    );
    let ids: Vec<String> = events[1..4]
        .iter()
        .map(|e| {
            let v: serde_json::Value =
                serde_json::from_str(e.strip_prefix("data: ").unwrap()).unwrap();
            assert_eq!(v["item"]["type"], "function_call");
            v["item"]["call_id"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(ids, ["call_1", "call_2", "call_3"]);
    assert_eq!(
        events[4],
        r#"data: {"response":{"id":"resp_1","object":"response","status":"completed","usage":{"input_tokens":3,"output_tokens":1,"total_tokens":4}},"type":"response.completed"}"#
    );
    assert!(
        events[5].contains(r#""type":"response.failed""#) && events[5].contains(r#""code":"boom""#)
    );
    assert_eq!(events[6], "data: {oops");
    assert!(!body.contains("never"));
}

#[test]
fn ac3_cut_has_no_completed() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![Reply::Sse(vec![
        SseEvent::Delta("a".into()),
        SseEvent::Cut,
    ])]);
    let body = post(&client(&s), "{}").text().unwrap();
    assert!(body.contains("response.output_text.delta") && !body.contains("response.completed"));
}

#[test]
fn ac3_status_usage_limit_redirect_raw() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![
        Reply::Status {
            code: 503,
            body: "down".into(),
        },
        Reply::UsageLimit { resets_in_s: 3600 },
        Reply::Redirect("https://other.test/x".into()),
        Reply::Raw {
            status: 418,
            headers: vec![("x-a".into(), "b".into())],
            body: b"tea".to_vec(),
        },
    ]);
    let c = client(&s);
    let r = post(&c, "{}");
    assert_eq!(
        (r.status().as_u16(), r.text().unwrap().as_str()),
        (503, "down")
    );
    let r = post(&c, "{}");
    assert_eq!(r.status().as_u16(), 429);
    let v: serde_json::Value = r.json().unwrap();
    assert_eq!(v["error"]["code"], "usage_limit_reached");
    assert_eq!(v["error"]["resets_in_seconds"], 3600);
    let r = post(&c, "{}");
    assert_eq!(r.status().as_u16(), 307);
    assert_eq!(r.headers()["location"], "https://other.test/x");
    let r = post(&c, "{}");
    assert_eq!(r.status().as_u16(), 418);
    assert_eq!(r.headers()["x-a"], "b");
    assert_eq!(r.text().unwrap(), "tea");
}

#[test]
fn ac3_reset_and_stall() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![Reply::Reset, Reply::Stall]);
    let c = client(&s);
    c.post(R).body("{}").send().unwrap_err();
    let short = reqwest::blocking::Client::builder()
        .no_proxy()
        .add_root_certificate(reqwest::Certificate::from_pem(s.ca_pem().as_bytes()).unwrap())
        .resolve("api.openai.com", s.responses_addr())
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let err = short.post(R).body("{}").send().unwrap_err();
    assert!(err.is_timeout(), "{err}");
    assert_eq!(s.requests().len(), 2);
}

#[test]
fn ac4_token_and_whole_requests() {
    let s = ScriptedResponses::start().unwrap();
    s.expect_token("t");
    let c = client(&s);
    assert_json_error(post(&c, "{}"), 401, "unauthorized");
    assert_eq!(
        c.post(R)
            .header("authorization", "Bearer u")
            .body("{}")
            .send()
            .unwrap()
            .status(),
        401
    );
    let big = vec![b'x'; 2 * 1024 * 1024];
    let r = c
        .post(R)
        .header("authorization", "Bearer t")
        .header("x-mine", "1")
        .body(big.clone())
        .send()
        .unwrap();
    assert_eq!(r.status(), 200);
    let reqs = s.requests();
    assert_eq!(reqs.len(), 3);
    assert_eq!(reqs[0].body, b"{}");
    assert_eq!(reqs[2].body, big);
    assert_eq!(
        (reqs[2].method.as_str(), reqs[2].path.as_str()),
        ("POST", "/v1/responses")
    );
    let h = |k: &str| {
        reqs[2]
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.clone())
    };
    assert_eq!(h("x-mine").as_deref(), Some("1"));
    assert_eq!(h("authorization").as_deref(), Some("Bearer t"));
    assert_eq!(h("host").as_deref(), Some("api.openai.com"));
}

#[test]
fn ac4_chunked_body_is_recorded_whole() {
    let s = ScriptedResponses::start().unwrap();
    let body = vec![b'x'; 32 * 1024];
    let response = client(&s)
        .post(R)
        .body(reqwest::blocking::Body::new(std::io::Cursor::new(
            body.clone(),
        )))
        .send()
        .unwrap();
    assert_eq!(response.status(), 200);
    let requests = s.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("transfer-encoding") && value == "chunked"
    }));
    assert_eq!(requests[0].body, body);
}

#[test]
fn ac5_judge_plays() {
    let s = ScriptedResponses::start().unwrap();
    let j = s.judge();
    j.script(vec![
        Play::Safe(vec![]),
        Play::Unsafe(vec!["a".into(), "b".into()]),
        Play::Unsure(vec!["c".into()]),
        Play::Down,
        Play::Slow {
            ms: 300,
            then: Box::new(Play::Safe(vec![])),
        },
        Play::Unreadable,
        Play::Redirect("https://api.openai.com/v1/responses".into()),
        Play::Raw {
            status: 202,
            headers: vec![],
            body: b"raw".to_vec(),
        },
    ]);
    let c = client(&s);
    let get = || c.post("https://judge.test/v1/judge").body("q").send();
    let v = |r: reqwest::blocking::Response| r.json::<Value>().unwrap();
    assert_eq!(v(get().unwrap()), json!({"reasons":[],"verdict":"safe"}));
    assert_eq!(
        v(get().unwrap()),
        json!({"reasons":["a","b"],"verdict":"unsafe"})
    );
    assert_eq!(
        v(get().unwrap()),
        json!({"reasons":["c"],"verdict":"unsure"})
    );
    get().unwrap_err();
    let t = std::time::Instant::now();
    assert_eq!(v(get().unwrap()), json!({"reasons":[],"verdict":"safe"}));
    assert!(t.elapsed() >= Duration::from_millis(300));
    let r = get().unwrap();
    assert_eq!(r.status(), 200);
    r.json::<Value>().unwrap_err();
    let r = get().unwrap();
    assert_eq!(r.status(), 307);
    let r = get().unwrap();
    assert_eq!(
        (r.status().as_u16(), r.text().unwrap().as_str()),
        (202, "raw")
    );
    assert_eq!(j.count(), 8);
    assert_eq!(j.requests().len(), 8);
    assert_eq!(j.requests()[0].body, b"q");
    assert_eq!(s.requests().len(), 0);
}

#[test]
fn ac5_judge_key_and_repeat() {
    let s = ScriptedResponses::start().unwrap();
    s.judge().expect_key("k");
    s.judge().script(vec![Play::Unsafe(vec![])]);
    let c = client(&s);
    assert_json_error(
        c.post("https://judge.test/").send().unwrap(),
        401,
        "unauthorized",
    );
    assert_eq!(
        c.post("https://judge.test/")
            .header("authorization", "Bearer z")
            .send()
            .unwrap()
            .status(),
        401
    );
    for _ in 0..2 {
        let r = c
            .post("https://judge.test/")
            .header("authorization", "Bearer k")
            .send()
            .unwrap();
        assert_eq!(
            r.json::<Value>().unwrap(),
            json!({"reasons":[],"verdict":"unsafe"})
        );
    }
    assert_eq!(s.judge().count(), 4);
}

struct Program(std::process::Child);
impl Drop for Program {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_program(args: &[&str]) -> (Program, serde_json::Value) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_scripted-responses"))
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    (Program(child), serde_json::from_str(&line).unwrap())
}

fn program_client(v: &serde_json::Value) -> reqwest::blocking::Client {
    client_for(
        v["ca_pem"].as_str().unwrap(),
        v["responses_addr"].as_str().unwrap(),
        v["judge_addr"].as_str().unwrap(),
    )
}

#[test]
fn ac6_program_default_and_log() {
    let dir = tempfile::tempdir().unwrap();
    let (_p, v) = start_program(&["--log", dir.path().to_str().unwrap()]);
    let mut keys: Vec<_> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "ca_pem",
            "judge_addr",
            "judge_origin",
            "responses_addr",
            "scripted"
        ]
    );
    assert_eq!(v["scripted"], 1);
    assert_eq!(v["judge_origin"], "https://judge.test");
    assert!(
        v["responses_addr"]
            .as_str()
            .unwrap()
            .starts_with("127.0.0.1:")
    );
    assert!(v["judge_addr"].as_str().unwrap().starts_with("127.0.0.1:"));
    let c = program_client(&v);
    let body = post(&c, "hello").text().unwrap();
    assert!(body.contains(r#""delta":"ok""#));
    assert_eq!(
        c.post("https://judge.test/j")
            .body("q")
            .send()
            .unwrap()
            .json::<Value>()
            .unwrap(),
        json!({"reasons":[],"verdict":"safe"})
    );
    let one: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("responses").join("1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(one["method"], "POST");
    assert_eq!(one["path"], "/v1/responses");
    assert_eq!(one["body_b64"], "aGVsbG8=");
    assert!(
        one["headers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h[0] == "host" && h[1] == "api.openai.com")
    );
    let j: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("judge").join("1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(j["body_b64"], "cQ==");
}

#[test]
fn ac6_program_script_file() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("s.json");
    std::fs::write(
        &f,
        r#"{"token":"t","models":["m1"],"replies":[{"usage-limit":{"resets_in_s":60}}],"judge_key":"k","plays":[{"unsure":["r"]}]}"#,
    )
    .unwrap();
    let (_p, v) = start_program(&["--script", f.to_str().unwrap()]);
    let c = program_client(&v);
    assert_json_error(post(&c, "{}"), 401, "unauthorized");
    let r = c
        .post(R)
        .header("authorization", "Bearer t")
        .body("{}")
        .send()
        .unwrap();
    assert_eq!(r.status(), 429);
    assert_eq!(
        r.json::<serde_json::Value>().unwrap()["error"]["resets_in_seconds"],
        60
    );
    let r = c
        .get("https://api.openai.com/v1/models")
        .header("authorization", "Bearer t")
        .send()
        .unwrap();
    assert!(r.text().unwrap().contains(r#""id":"m1""#));
    assert_json_error(
        c.post("https://judge.test/").send().unwrap(),
        401,
        "unauthorized",
    );
    let r = c
        .post("https://judge.test/")
        .header("authorization", "Bearer k")
        .send()
        .unwrap();
    assert_eq!(
        r.json::<Value>().unwrap(),
        json!({"reasons":["r"],"verdict":"unsure"})
    );
}

fn assert_json_error(response: reqwest::blocking::Response, status: u16, message: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(
        response.json::<Value>().unwrap(),
        json!({"error":{"message":message}})
    );
}

#[test]
fn ac2_empty_scripts_return_json_errors() {
    let s = ScriptedResponses::start().unwrap();
    s.script(vec![]);
    s.judge().script(vec![]);
    let c = client(&s);
    assert_json_error(post(&c, "{}"), 500, "no script");
    assert_json_error(
        c.post("https://judge.test/").send().unwrap(),
        500,
        "no script",
    );
}

#[test]
fn ac7_dropping_the_server_closes_both_listeners() {
    use std::io::ErrorKind;
    use std::net::TcpStream;
    use std::time::Instant;

    let s = ScriptedResponses::start().unwrap();
    let addrs = [s.responses_addr(), s.judge_addr()];
    for addr in addrs {
        TcpStream::connect_timeout(&addr, Duration::from_secs(1)).unwrap();
    }
    drop(s);

    let deadline = Instant::now() + Duration::from_secs(5);
    for addr in addrs {
        loop {
            match TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
                Err(e) if e.kind() == ErrorKind::ConnectionRefused => break,
                result => assert!(
                    Instant::now() < deadline,
                    "listener {addr} after drop: {result:?}"
                ),
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn ac6_program_refuses_unknown_script_fields() {
    use std::io::Read;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("script.json");
    for (script, field) in [
        (r#"{"replise":[]}"#, "replise"),
        (
            r#"{"replies":[{"usage-limit":{"resets_in_s":60,"extra":0}}]}"#,
            "extra",
        ),
        (
            r#"{"replies":[{"sse":[{"completed":{"input_tokens":1,"output_tokens":1,"extra":0}}]}]}"#,
            "extra",
        ),
        (
            r#"{"plays":[{"slow":{"ms":1,"then":"down","extra":0}}]}"#,
            "extra",
        ),
    ] {
        std::fs::write(&file, script).unwrap();
        let mut process = Program(
            Command::new(env!("CARGO_BIN_EXE_scripted-responses"))
                .arg("--script")
                .arg(&file)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut line = String::new();
        assert_eq!(
            BufReader::new(process.0.stdout.take().unwrap())
                .read_line(&mut line)
                .unwrap(),
            0,
            "{line}"
        );
        let mut error = String::new();
        process
            .0
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut error)
            .unwrap();
        assert_eq!(process.0.wait().unwrap().code(), Some(1), "{error}");
        assert!(
            error.contains(&format!("unknown field `{field}`")),
            "{error}"
        );
    }
}

#[cfg(unix)]
#[test]
fn ac6_program_ends_on_sigterm() {
    use std::os::unix::process::ExitStatusExt;
    use std::time::Instant;

    let (mut program, _) = start_program(&[]);
    let signal = Command::new("kill")
        .args(["-TERM", &program.0.id().to_string()])
        .status()
        .unwrap();
    assert!(signal.success(), "{signal}");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = program.0.try_wait().unwrap() {
            assert_eq!(status.signal(), Some(15), "{status}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "scripted-responses ignored SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
