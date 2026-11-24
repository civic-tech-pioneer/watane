#![allow(clippy::unwrap_used, reason = "tests may panic")]

use super::*;
#[cfg(unix)]
use std::io::Write;

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac7_call_distinguishes_missing_origin_from_wrong_origin() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(&fixture("counter")).await.status(), 200);
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    let client = preview.client("counter.localhost", "127.0.0.1");
    let response = client
        .post(format!("{origin}/call"))
        .json(&json!({"fn":"api.add","args":{}}))
        .send()
        .await
        .unwrap();
    let body = call_error(response, 403, "forbidden").await;
    assert_eq!(
        body["error"]["message"],
        format!("the Origin header is missing; it must be {origin}")
    );
    let response = client
        .post(format!("{origin}/call"))
        .header(
            "Origin",
            format!("http://echo.localhost:{}", preview.ready.port),
        )
        .json(&json!({"fn":"api.add","args":{}}))
        .send()
        .await
        .unwrap();
    let body = call_error(response, 403, "forbidden").await;
    assert_eq!(body["error"]["message"], "Origin must match the mod origin");
    let response = client
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"api.get","args":{}}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"value":0}));
}

pub(super) fn headers(response: &reqwest::Response) {
    for (name, value) in [
        ("content-security-policy", CSP),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cache-control", "no-store"),
    ] {
        let values: Vec<_> = response.headers().get_all(name).iter().collect();
        assert_eq!(values, [value], "{name}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_page_calls_its_counter_as_page() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(&fixture("counter")).await.status(), 200);
    let origin = format!("http://counter.localhost:{}", preview.ready.port);
    let client = preview.client("counter.localhost", "127.0.0.1");
    for (body, expected) in [
        (json!({"fn":"api.add","args":null}), "{\"value\":1}"),
        (json!({"fn":"api.add","args":null}), "{\"value\":2}"),
        (json!({"fn":"api.get","args":{}}), "{\"value\":2}"),
        (json!({"fn":"api.get","args":[]}), "{\"value\":2}"),
        (json!({"fn":"api.who","args":null}), "{\"value\":\"page\"}"),
    ] {
        let response = client
            .post(format!("{origin}/call"))
            .header("Origin", &origin)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(response.text().await.unwrap(), expected);
    }
}

pub(super) async fn call_error(response: reqwest::Response, status: u16, code: &str) -> Value {
    assert_eq!(response.status(), status);
    headers(&response);
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = response.json::<Value>().await.unwrap();
    assert_eq!(body["error"]["code"], code);
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    );
    assert!(body.get("value").is_none());
    if code != "limit" {
        assert!(body["error"].get("limit").is_none());
    }
    body
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac4_call_errors_keep_codes_and_limits_allow_recovery() {
    use watane_host::{Host, Limits};
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(
        home.path(),
        watane_host::Mode::Preview,
        Limits {
            time: Duration::from_millis(200),
            fuel: u64::MAX,
            ..Limits::default()
        },
    )
    .unwrap();
    let served = host.serve(0, axum::Router::new()).await.unwrap();
    host.install_folders(
        &[fixture("counter"), fixture("hello"), fixture("hog")],
        served.port,
    )
    .unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("counter.localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("hello.localhost", ([127, 0, 0, 1], served.port).into())
        .resolve("hog.localhost", ([127, 0, 0, 1], served.port).into())
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    for (local, function, status, code) in [
        ("counter", "api.unknown", 404, "no_function"),
        ("counter", "api.add-then-fail", 422, "trap"),
        ("hello", "api.add", 404, "no_function"),
    ] {
        let origin = format!("http://{local}.localhost:{}", served.port);
        let response = client
            .post(format!("{origin}/call"))
            .header("Origin", &origin)
            .json(&json!({"fn":function,"args":null}))
            .send()
            .await
            .unwrap();
        call_error(response, status, code).await;
    }
    let origin = format!("http://counter.localhost:{}", served.port);
    let response = client
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"api.get","args":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.json::<Value>().await.unwrap(), json!({"value":0}));
    let origin = format!("http://hog.localhost:{}", served.port);
    let start = Instant::now();
    let response = client
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"api.spin","args":null}))
        .send()
        .await
        .unwrap();
    let body = call_error(response, 422, "limit").await;
    assert_eq!(body["error"]["limit"], "time");
    assert!(start.elapsed() >= Duration::from_millis(200));
    assert!(start.elapsed() < Duration::from_secs(2));
    let response = client
        .post(format!("{origin}/call"))
        .header("Origin", &origin)
        .json(&json!({"fn":"api.probe","args":null}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    headers(&response);
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({"value":["no-dir","no-env","no-socket"]})
    );
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac1_watane_js_is_the_host_library_with_page_headers() {
    let home = tempfile::tempdir().unwrap();
    let folder = copy_hello();
    for file in ["watane.js", "call", "events"] {
        fs::write(folder.path().join("ui").join(file), "mod override").unwrap();
    }
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(folder.path()).await.status(), 200);
    let url = format!("http://hello.localhost:{}/watane.js", preview.ready.port);
    for ip in ["127.0.0.1"]
        .into_iter()
        .chain(super::has_v6().then_some("::1"))
    {
        let response = preview
            .client("hello.localhost", ip)
            .get(&url)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(
            response.headers()["content-type"],
            "text/javascript; charset=utf-8"
        );
        let source = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/watane.js")).unwrap();
        assert_eq!(watane_host::WATANE_JS.as_bytes(), source);
        assert_eq!(response.bytes().await.unwrap().as_ref(), source);
        let spec =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
        let source_in_spec = spec
            .split("### 8.5 `watane.js`")
            .nth(1)
            .unwrap()
            .split("```js\n")
            .nth(1)
            .unwrap()
            .split("```\n")
            .next()
            .unwrap();
        assert_eq!(source, source_in_spec.as_bytes());
        let response = preview
            .client("hello.localhost", ip)
            .head(&url)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(
            response.headers()["content-type"],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            response.headers()["content-length"],
            source.len().to_string()
        );
        assert!(response.bytes().await.unwrap().is_empty());
    }
    let client = preview.client("hello.localhost", "127.0.0.1");
    let origin = format!("http://hello.localhost:{}", preview.ready.port);
    let response = client.get(format!("{origin}/call")).send().await.unwrap();
    call_error(response, 405, "bad_request").await;
    let response = client.get(format!("{origin}/events")).send().await.unwrap();
    assert_eq!(response.status(), 200);
    headers(&response);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
}

#[cfg(unix)] // Uses a Unix symlink fixture.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac4_page_bytes_headers_paths_and_both_loopbacks() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(&fixture("hello")).await.status(), 200);
    let url = format!("http://hello.localhost:{}/", preview.ready.port);
    for ip in ["127.0.0.1"]
        .into_iter()
        .chain(super::has_v6().then_some("::1"))
    {
        let client = preview.client("hello.localhost", ip);
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            fs::read(fixture("hello").join("ui/index.html")).unwrap()
        );
        let response = client.head(&url).send().await.unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(
            response.headers()["content-type"],
            "text/html; charset=utf-8"
        );
        assert!(response.bytes().await.unwrap().is_empty());
        let response = client.get(format!("{url}app.js")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        headers(&response);
        assert_eq!(
            response.headers()["content-type"],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            fs::read(fixture("hello").join("ui/app.js")).unwrap()
        );
    }
    // URL parsers normalize dot segments and backslashes. Send these paths verbatim.
    for path in [
        "/../x",
        "/a//b",
        "/.env",
        "/%2e%2e/x",
        "/a\\b",
        "/%00",
        "/%ff",
        "/%",
        "/app.js/",
        "/call",
        "/watane.js",
        "/api/status",
    ] {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", preview.ready.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: hello.localhost:{}\r\nConnection: close\r\n\r\n",
            preview.ready.port
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (status, content_type) = match path {
            "/watane.js" => (200, "text/javascript; charset=utf-8"),
            "/call" => (405, "application/json"),
            _ => (404, "text/plain; charset=utf-8"),
        };
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "{path}: {response}"
        );
        assert!(response.contains(&format!("content-type: {content_type}\r\n")));
        assert!(response.contains(&format!("content-security-policy: {CSP}\r\n")));
    }
    let response = preview
        .client("absent.localhost", "127.0.0.1")
        .get(format!("http://absent.localhost:{}/", preview.ready.port))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    headers(&response);
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    std::os::unix::fs::symlink("/etc/passwd", home.path().join("mods/hello/ui/leak.txt")).unwrap();
    let response = preview
        .client("hello.localhost", "127.0.0.1")
        .get(format!("{url}leak.txt"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_reinstall_updates_in_place_and_survives_restart() {
    let home = tempfile::tempdir().unwrap();
    let folder = copy_hello();
    let mut preview = Preview::start(home.path());
    assert_eq!(preview.install(folder.path()).await.status(), 200);
    let before: Value =
        serde_json::from_slice(&fs::read(home.path().join("installed.json")).unwrap()).unwrap();
    // Equal input is Unchanged: no catalog write.
    assert_eq!(preview.install(folder.path()).await.status(), 200);
    let unchanged: Value =
        serde_json::from_slice(&fs::read(home.path().join("installed.json")).unwrap()).unwrap();
    assert_eq!(unchanged, before);
    fs::write(folder.path().join("ui/index.html"), "<p>updated</p>\n").unwrap();
    let response = preview.install(folder.path()).await;
    assert_eq!(response.status(), 200);
    let installed = response.json::<Value>().await.unwrap();
    assert_eq!(installed["installed"][0]["local"], "hello");
    let url = format!("http://hello.localhost:{}/", preview.ready.port);
    let response = preview
        .client("hello.localhost", "127.0.0.1")
        .get(&url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "<p>updated</p>\n");
    let catalog: Value =
        serde_json::from_slice(&fs::read(home.path().join("installed.json")).unwrap()).unwrap();
    assert_eq!(catalog["generation"], 2);
    assert_eq!(
        catalog["mods"][0]["package"],
        "46ef634169247ba5adfe4d2645269389c65d3707a4bb1b3c04ba08ef50e41406"
    );
    assert_ne!(catalog["mods"][0]["package"], before["mods"][0]["package"]);
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 1);
    preview.stop();
    let mut preview = Preview::start(home.path());
    assert_eq!(preview.ready.mods.len(), 1);
    assert_eq!(preview.ready.mods[0].local.to_string(), "hello");
    let url = format!("http://hello.localhost:{}/", preview.ready.port);
    assert_eq!(preview.ready.mods[0].page.as_deref(), Some(url.as_str()));
    let response = preview
        .client("hello.localhost", "127.0.0.1")
        .get(url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.text().await.unwrap(), "<p>updated</p>\n");
    preview.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_failed_update_preserves_previous_page() {
    let home = tempfile::tempdir().unwrap();
    let folder = copy_hello();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(folder.path()).await.status(), 200);
    let before = fs::read(home.path().join("installed.json")).unwrap();
    fs::write(folder.path().join("ui/index.html"), "<p>not committed</p>").unwrap();
    // Force the catalog write to fail after the mod-directory swap.
    fs::create_dir(home.path().join("installed.json.tmp")).unwrap();
    let response = preview.install(folder.path()).await;
    assert_eq!(response.status(), 500);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "internal"
    );
    assert_eq!(
        fs::read(home.path().join("installed.json")).unwrap(),
        before
    );
    let response = preview
        .client("hello.localhost", "127.0.0.1")
        .get(format!("http://hello.localhost:{}/", preview.ready.port))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        fs::read(fixture("hello").join("ui/index.html")).unwrap()
    );
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 1);
    assert_eq!(
        fs::read_dir(home.path().join("staging")).unwrap().count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_restart_restores_committed_page_after_interrupted_update() {
    for phase in 0..3 {
        let home = tempfile::tempdir().unwrap();
        let mut preview = Preview::start(home.path());
        assert_eq!(preview.install(&fixture("hello")).await.status(), 200);
        preview.stop();
        let mods = home.path().join("mods");
        let replacement = mods.join(if phase == 2 { "hello" } else { "hello.new" });
        if phase > 0 {
            fs::rename(mods.join("hello"), mods.join("hello.old")).unwrap();
        }
        for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
            fs::create_dir_all(replacement.join(file).parent().unwrap()).unwrap();
            fs::copy(fixture("hello").join(file), replacement.join(file)).unwrap();
        }
        fs::write(replacement.join("ui/index.html"), "<p>not committed</p>").unwrap();
        let mut preview = Preview::start(home.path());
        let response = preview
            .client("hello.localhost", "127.0.0.1")
            .get(format!("http://hello.localhost:{}/", preview.ready.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            fs::read(fixture("hello").join("ui/index.html")).unwrap()
        );
        assert_eq!(fs::read_dir(mods).unwrap().count(), 1);
        preview.stop();
    }
}
