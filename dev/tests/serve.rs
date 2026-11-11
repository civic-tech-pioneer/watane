use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::Command,
    time::Duration,
};
struct Mod {
    temp: tempfile::TempDir,
    dir: PathBuf,
    host: PathBuf,
}
impl Mod {
    fn new(real: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("hello");
        let host = if real {
            std::env::var_os("WATANE_BIN")
                .expect("set WATANE_BIN")
                .into()
        } else {
            temp.path().join("missing")
        };
        let m = Self { temp, dir, host };
        assert_eq!(m.run(&["new", "hello", "--json"]).0, 0);
        m
    }
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let o = self.output(args);
        (
            o.status.code().unwrap(),
            serde_json::from_slice(&o.stdout).unwrap(),
        )
    }
    fn output(&self, args: &[&str]) -> std::process::Output {
        let cwd = if self.dir.exists() {
            &self.dir
        } else {
            self.temp.path()
        };
        Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(cwd)
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env("WATANE_BIN", &self.host)
            .args(args)
            .output()
            .unwrap()
    }
    fn serve(&self) -> Value {
        let (c, v) = self.run(&["serve", "--json"]);
        assert_eq!(c, 0, "{v}");
        v["result"].clone()
    }
}
impl Drop for Mod {
    fn drop(&mut self) {
        if self.dir.exists() {
            let _ = self.run(&["serve", "--stop", "--json"]);
        }
    }
}
fn port(v: &Value) -> u16 {
    v["api"]
        .as_str()
        .unwrap()
        .split(':')
        .nth(2)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn get(v: &Value, path: &str) -> (String, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port(v))).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    write!(
        s,
        "GET /{path} HTTP/1.1\r\nHost: hello.localhost:{}\r\nConnection: close\r\n\r\n",
        port(v)
    )
    .unwrap();
    let mut bytes = Vec::new();
    s.read_to_end(&mut bytes).unwrap();
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8(bytes[..split].to_vec()).unwrap(),
        bytes[split + 4..].to_vec(),
    )
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac4_real_preview_serves_page_and_csp() {
    let m = Mod::new(true);
    let v = m.serve();
    assert_eq!(v["reused"], false);
    assert_eq!(
        v["mcp"]["url"],
        format!("http://localhost:{}/mcp/preview", port(&v))
    );
    assert_eq!(v["mcp"]["toolset"], "preview");
    assert!(v["mcp"]["token"].as_str().unwrap().starts_with("wt_"));
    let hash = blake3::hash(m.dir.canonicalize().unwrap().to_str().unwrap().as_bytes()).to_hex();
    let folder = m
        .temp
        .path()
        .canonicalize()
        .unwrap()
        .join(format!("cache/previews/hello-{}/", &hash[..8]));
    assert_eq!(v["home"], folder.join("home").to_str().unwrap());
    assert_eq!(v["log"], folder.join("watane.log").to_str().unwrap());
    assert_eq!(
        v["pages"][0]["url"],
        format!("http://hello.localhost:{}/", port(&v))
    );
    let (headers, body) = get(&v, "");
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert_eq!(body, fs::read(m.dir.join("ui/index.html")).unwrap());
    let csp = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'";
    assert!(
        headers
            .to_lowercase()
            .contains(&format!("content-security-policy: {csp}"))
    );
    let html = String::from_utf8(body).unwrap();
    assert!(html.contains("<script src=\"app.js\"></script>"));
    assert!(html.contains("<script src=\"/watane.js\"></script>"));
    assert_eq!(html.matches("<script").count(), 2);
    assert_eq!(
        get(&v, "app.js").1,
        fs::read(m.dir.join("ui/app.js")).unwrap()
    );
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac5_serve_reuses_preview_and_updates_page() {
    let m = Mod::new(true);
    let first = m.serve();
    fs::write(
        m.dir.join("ui/index.html"),
        b"<!doctype html><h1>Edited</h1>",
    )
    .unwrap();
    let second = m.serve();
    assert_eq!(second["reused"], true);
    assert_eq!(port(&first), port(&second));
    assert_eq!(get(&second, "").1, b"<!doctype html><h1>Edited</h1>");
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac6_stop_closes_port_and_removes_preview() {
    let m = Mod::new(true);
    let v = m.serve();
    let (c, stopped) = m.run(&["serve", "--stop", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(
        stopped["result"],
        serde_json::json!({"stopped":true,"home":v["home"]})
    );
    assert!(TcpStream::connect(("127.0.0.1", port(&v))).is_err());
    assert!(
        !PathBuf::from(v["home"].as_str().unwrap())
            .parent()
            .unwrap()
            .exists()
    );
    let (c, v) = m.run(&["serve", "--stop", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(v["result"]["stopped"], false);
}
#[test]
fn ac7_missing_host_has_environment_error_and_fix() {
    let m = Mod::new(false);
    let (c, v) = m.run(&["serve", "--json"]);
    assert_eq!(c, 3);
    assert_eq!(v["errors"][0]["code"], "env.watane_missing");
    assert_eq!(
        v["errors"][0]["fix"],
        "Install watane, or set WATANE_BIN to the program"
    );
    // The tests passed, so `next` must not send the agent back to them.
    assert!(v["next"].is_null(), "{v}");
}
#[test]
fn stop_with_unreadable_run_json_is_not_running() {
    let m = Mod::new(false);
    let hash = blake3::hash(m.dir.canonicalize().unwrap().to_str().unwrap().as_bytes()).to_hex();
    let folder = m
        .temp
        .path()
        .canonicalize()
        .unwrap()
        .join(format!("cache/previews/hello-{}", &hash[..8]));
    fs::create_dir_all(folder.join("home")).unwrap();
    fs::write(folder.join("home/run.json"), "not json").unwrap();
    let (c, v) = m.run(&["serve", "--stop", "--json"]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["result"]["stopped"], false);
    assert!(!folder.exists());
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac6_stop_works_after_mod_files_are_broken() {
    for file in ["mod.toml", "wit/world.wit"] {
        let m = Mod::new(true);
        let preview = m.serve();
        let original = fs::read(m.dir.join(file)).unwrap();
        fs::write(m.dir.join(file), "broken [").unwrap();
        let (code, value) = m.run(&["serve", "--stop", "--json"]);
        // Restore for cleanup even if this regression assertion fails.
        fs::write(m.dir.join(file), original).unwrap();
        assert_eq!(code, 0, "{value}");
        assert_eq!(
            value["result"],
            serde_json::json!({"stopped":true,"home":preview["home"]})
        );
        assert!(TcpStream::connect(("127.0.0.1", port(&preview))).is_err());
        assert!(
            !PathBuf::from(preview["home"].as_str().unwrap())
                .parent()
                .unwrap()
                .exists()
        );
    }
}

#[test]
fn ac6_stop_finishes_when_process_exits_with_run_json_left() {
    use std::{net::TcpListener, process::Stdio, time::Instant};
    let m = Mod::new(false);
    let hash = blake3::hash(m.dir.canonicalize().unwrap().as_os_str().as_encoded_bytes()).to_hex();
    let folder = m
        .temp
        .path()
        .canonicalize()
        .unwrap()
        .join(format!("cache/previews/hello-{}", &hash[..8]));
    let home = folder.join("home");
    fs::create_dir_all(&home).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    fs::write(
        home.join("run.json"),
        serde_json::to_vec(
            &serde_json::json!({"watane":1,"pid":child.id(),"port":port,"mode":"preview"}),
        )
        .unwrap(),
    )
    .unwrap();
    let reaper = std::thread::spawn(move || child.wait().unwrap());
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        assert!(request.starts_with(b"GET /api/status "));
        let body =
            serde_json::json!({"watane":1,"mode":"preview","home":home,"port":port,"app":false})
                .to_string();
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let started = Instant::now();
    let (code, value) = m.run(&["serve", "--stop", "--json"]);
    server.join().unwrap();
    reaper.join().unwrap();
    assert_eq!(code, 0, "{value}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(value["result"]["stopped"], true);
    assert!(!folder.exists());
}

#[test]
fn stop_works_after_mod_folder_is_deleted() {
    let m = Mod::new(false);
    let dir = m.dir.canonicalize().unwrap();
    let hash = blake3::hash(dir.as_os_str().as_encoded_bytes()).to_hex();
    let folder = m
        .temp
        .path()
        .canonicalize()
        .unwrap()
        .join(format!("cache/previews/hello-{}", &hash[..8]));
    fs::create_dir_all(folder.join("home")).unwrap();
    fs::remove_dir_all(&dir).unwrap();
    let (code, value) = m.run(&["--dir", dir.to_str().unwrap(), "serve", "--stop", "--json"]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["result"]["stopped"], false);
    assert!(!folder.exists());
}

fn call(v: &Value, function: &str) -> Value {
    let page = v["pages"][0]["url"].as_str().unwrap();
    let origin = page.trim_end_matches('/');
    let authority = origin.strip_prefix("http://").unwrap();
    let body = serde_json::json!({"fn": function, "args": null}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port(v))).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream, "POST /call HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let headers = String::from_utf8_lossy(&bytes[..split]);
    assert!(
        headers.starts_with("HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes[split + 4..]).unwrap()
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac9_real_counter_page_calls_logic() {
    let m = Mod::new(true);
    let first = m.serve();
    let html = String::from_utf8(get(&first, "").1).unwrap();
    assert!(html.contains("<script src=\"/watane.js\"></script>"));
    assert!(html.contains("<script src=\"app.js\"></script>"));
    assert_eq!(call(&first, "api.add"), serde_json::json!({"value":1}));
    assert_eq!(call(&first, "api.add"), serde_json::json!({"value":2}));
    assert_eq!(call(&first, "api.get"), serde_json::json!({"value":2}));
    let second = m.serve();
    assert_eq!(second["reused"], true);
    assert_eq!(call(&second, "api.get"), serde_json::json!({"value":2}));
}

fn edited_counter() -> (Mod, Value, Vec<u8>) {
    let m = Mod::new(true);
    let first = m.serve();
    assert_eq!(call(&first, "api.add"), serde_json::json!({"value": 1}));
    assert_eq!(call(&first, "api.add"), serde_json::json!({"value": 2}));

    let logic = m.dir.join("logic/src/lib.rs");
    let source = fs::read_to_string(&logic).unwrap();
    assert_eq!(source.matches("count.saturating_add(1)").count(), 1);
    fs::write(
        &logic,
        source.replace("count.saturating_add(1)", "count.saturating_add(10)"),
    )
    .unwrap();
    // As a maker would, update the unit and contract tests to the new logic.
    let source = fs::read_to_string(&logic).unwrap();
    assert_eq!(source.matches("assert_eq!(next(0), 1);").count(), 1);
    fs::write(
        &logic,
        source.replace("assert_eq!(next(0), 1);", "assert_eq!(next(0), 10);"),
    )
    .unwrap();
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.get","returns":0},{"call":"api.add","returns":10},{"call":"api.add","returns":20}]}"#,
    )
    .unwrap();
    let html = fs::read_to_string(m.dir.join("ui/index.html")).unwrap();
    assert!(html.contains("<h1>Hello</h1>"));
    let edited = html
        .replace("<h1>Hello</h1>", "<h1>Add ten</h1>")
        .into_bytes();
    fs::write(m.dir.join("ui/index.html"), &edited).unwrap();

    let second = m.serve();
    assert_eq!(second["reused"], true);
    assert_eq!(second["api"], first["api"]);
    assert_eq!(second["home"], first["home"]);
    assert_eq!(second["pages"][0]["local"], first["pages"][0]["local"]);
    let (headers, body) = get(&second, "");
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert_eq!(body, edited);
    assert_eq!(call(&second, "api.get"), serde_json::json!({"value": 2}));
    assert_eq!(call(&second, "api.add"), serde_json::json!({"value": 12}));
    (m, second, edited)
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac2_edit_keeps_data_and_updates_logic_and_page() {
    let (_m, _, _) = edited_counter();
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac3_broken_build_leaves_running_revision_and_count() {
    let (m, preview, previous_html) = edited_counter();
    fs::write(m.dir.join("logic/src/lib.rs"), "fn broken( {").unwrap();
    fs::write(m.dir.join("ui/index.html"), "<h1>Unbuilt revision</h1>").unwrap();
    let (code, value) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{value}");
    assert_eq!(value["ok"], false);
    assert_eq!(value["errors"][0]["code"], "build.compile");
    let (headers, body) = get(&preview, "");
    assert!(headers.starts_with("HTTP/1.1 200"));
    assert_eq!(body, previous_html);
    assert_eq!(call(&preview, "api.get"), serde_json::json!({"value": 12}));
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac8_failed_contract_keeps_running_preview_bytes() {
    let m = Mod::new(true);
    let preview = m.serve();
    let old_bytes = fs::read(m.dir.join("ui/index.html")).unwrap();
    fs::write(m.dir.join("ui/index.html"), "<h1>Rejected revision</h1>").unwrap();
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.add","returns":2}]}"#,
    )
    .unwrap();
    let (code, v) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert_eq!(get(&preview, "").1, old_bytes);
    fs::remove_file(m.dir.join("tests/counter.json")).unwrap();
    let (code, v) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "mod.no_contract_tests");
    assert_eq!(get(&preview, "").1, old_bytes);
}

#[test]
fn ac3_failed_contract_reports_test_next() {
    let m = Mod::new(false);
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.add","returns":2}]}"#,
    )
    .unwrap();
    let (code, v) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert_eq!(v["next"], "watane-dev test --json");
}

fn mcp_request(v: &Value, method: &str, params: Value) -> Value {
    let url = v["mcp"]["url"].as_str().unwrap();
    assert_eq!(url, format!("http://localhost:{}/mcp/preview", port(v)));
    let token = v["mcp"]["token"].as_str().unwrap();
    let body =
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", port(v))).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    write!(stream, "POST /mcp/preview HTTP/1.1\r\nHost: localhost:{}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", port(v), body.len()).unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).unwrap();
    let split = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    assert!(
        bytes.starts_with(b"HTTP/1.1 200"),
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let response: Value = serde_json::from_slice(&bytes[split + 4..]).unwrap();
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac1_preview_mcp_lists_and_calls_counter() {
    let m = Mod::new(true);
    let v = m.serve();
    assert_eq!(
        v["mcp"]["url"],
        format!("http://localhost:{}/mcp/preview", port(&v))
    );
    assert_eq!(v["mcp"]["toolset"], "preview");
    assert!(v["mcp"]["token"].as_str().unwrap().starts_with("wt_"));
    let slug = "hello";
    let add = format!("{slug}_api_add");
    let get = format!("{slug}_api_get");
    let listed = mcp_request(&v, "tools/list", serde_json::json!({}));
    let names: Vec<_> = listed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&add.as_str()), "{listed}");
    assert!(names.contains(&get.as_str()), "{listed}");
    let called = mcp_request(
        &v,
        "tools/call",
        serde_json::json!({"name":add,"arguments":{}}),
    );
    assert_eq!(
        called,
        serde_json::json!({"content":[{"type":"text","text":"1"}],"isError":false})
    );
    assert_eq!(call(&v, "api.get"), serde_json::json!({"value":1}));
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac2_reused_preview_keeps_mcp() {
    let m = Mod::new(true);
    let first = m.serve();
    assert!(first["mcp"].is_object());
    let second = m.serve();
    assert_eq!(second["reused"], true);
    assert_eq!(second["mcp"], first["mcp"]);
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac3_refused_serve_warns_about_previous_preview() {
    let m = Mod::new(true);
    let preview = m.serve();
    let old_bytes = fs::read(m.dir.join("ui/index.html")).unwrap();
    fs::write(m.dir.join("ui/index.html"), "<h1>Rejected revision</h1>").unwrap();
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.add","returns":2}]}"#,
    )
    .unwrap();
    let (code, v) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["ok"], false);
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert_eq!(v["next"], "watane-dev test --json");
    assert_eq!(
        v["warnings"],
        serde_json::json!([{
            "code": "preview.previous",
            "message": format!("the preview at {} still serves the previous build", preview["api"].as_str().unwrap()),
            "file": null, "line": null, "column": null,
            "fix": "Fix the errors, then run: watane-dev serve --json"
        }])
    );
    assert_eq!(get(&preview, "").1, old_bytes);
}

#[test]
fn ac4_refused_serve_without_preview_has_no_previous_warning() {
    let m = Mod::new(false);
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.add","returns":2}]}"#,
    )
    .unwrap();
    let (code, v) = m.run(&["serve", "--json"]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert!(
        v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|w| w["code"] != "preview.previous")
    );
}

#[test]
#[ignore = "needs WATANE_BIN"]
fn ac5_refused_serve_prints_previous_preview_warning() {
    let m = Mod::new(true);
    let preview = m.serve();
    fs::write(
        m.dir.join("tests/counter.json"),
        r#"{"steps":[{"call":"api.add","returns":2}]}"#,
    )
    .unwrap();
    let output = m.output(&["serve"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.lines().any(|line| line
            == format!(
                "warning[preview.previous]: the preview at {} still serves the previous build",
                preview["api"].as_str().unwrap()
            )),
        "{text}"
    );
    assert!(
        text.contains("  fix: Fix the errors, then run: watane-dev serve --json"),
        "{text}"
    );
}
