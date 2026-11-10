use super::*;
use std::io::Write;

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'";

fn headers(response: &reqwest::Response) {
    for (name, value) in [
        ("content-security-policy", CSP),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cache-control", "no-store"),
    ] {
        assert_eq!(response.headers()[name], value, "{name}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac4_page_bytes_headers_paths_and_both_loopbacks() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    assert_eq!(preview.install(&fixture("hello")).await.status(), 200);
    let url = format!("http://hello.localhost:{}/", preview.ready.port);
    for ip in ["127.0.0.1", "::1"] {
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
        "/events",
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
        assert!(response.starts_with("HTTP/1.1 404"), "{path}: {response}");
        assert!(response.contains("content-type: text/plain; charset=utf-8\r\n"));
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
        "4db0f3a20b5d44b521ceddd0f3ad3a3ad0cdfe604bbf8914986fe75cd7a39b30"
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
