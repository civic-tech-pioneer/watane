#![allow(clippy::unwrap_used, reason = "tests may panic")]

use super::*;

// Python framed WATANEPK v1 records with big-endian path and content lengths;
// a standalone BLAKE3 invocation produced this literal.
const HELLO_PACKAGE: &str = "d049784754023367a3cf622c78271ff06ae34063f3da42ebf812c3076dc424b6";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_status_and_api_guards() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}status", preview.ready.api);
    let response = client.get(&url).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        response.json::<Value>().await.unwrap(),
        json!({
            "watane":1,"mode":"preview","home":home.path().canonicalize().unwrap(),
            "port":preview.ready.port,"app":false
        })
    );
    for origin in ["null", "", "http://hello.localhost", "http://localhost"] {
        let response = client
            .get(&url)
            .header("Origin", origin)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 403);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "forbidden"
        );
    }
    let port = preview.ready.port;
    for host in [
        "evil.example".to_owned(),
        format!("evil.example:{port}"),
        format!("hello.localhost.evil.example:{port}"),
        format!("x.hello.localhost:{port}"),
        format!("hello.localhost:{}", port.wrapping_add(1)),
        format!("localhost:{port}.evil.example"),
        "localhost".to_owned(),
        "localhost:1".to_owned(),
        "HELLO.localhost:1".to_owned(),
    ] {
        let response = client.get(&url).header("Host", &host).send().await.unwrap();
        assert_eq!(response.status(), 421);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
    }
    for ip in ["127.0.0.1", "[::1]"] {
        let response = client
            .get(format!("http://{ip}:{}/api/status", preview.ready.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let response = client
        .get(&url)
        .body("x".repeat(65_537))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_install_page_folder_and_reject_invalid_inputs() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let response = preview.install(&fixture("hello")).await;
    assert_eq!(response.status(), 200);
    let body = response.json::<Value>().await.unwrap();
    let installed = &body["installed"][0];
    assert_eq!(installed["local"], "hello");
    assert_eq!(installed["identity"], "local/hello");
    assert_eq!(installed["package"], HELLO_PACKAGE);
    assert_eq!(
        installed["page"],
        format!("http://hello.localhost:{}/", preview.ready.port)
    );
    assert_eq!(installed["has_logic"], false);
    assert_eq!(installed["enabled"], true);
    assert_eq!(installed["grants"], json!([]));
    assert_eq!(installed["links"], json!([]));
    serde_json::from_value::<watane_host::InstalledMod>(installed.clone()).unwrap();
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        assert_eq!(
            fs::read(home.path().join("mods/hello").join(file)).unwrap(),
            fs::read(fixture("hello").join(file)).unwrap()
        );
    }
    let catalog: Value =
        serde_json::from_slice(&fs::read(home.path().join("installed.json")).unwrap()).unwrap();
    assert_eq!(catalog["watane_installed"], 1);
    assert_eq!(catalog["generation"], 1);
    assert_eq!(
        catalog["mods"][0],
        json!({
            "local":"hello","identity":"local/hello","package":HELLO_PACKAGE,
            "release":null,"enabled":true,"grants":[],"links":[],"binds":[]
        })
    );
    let response = preview.install(Path::new("relative")).await;
    assert_eq!(response.status(), 400);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
    let response = preview.install(&fixture("empty")).await;
    assert_eq!(response.status(), 422);
    let error = response.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_mod");
    let pkg_text = watane_pkg::Mod::read(&fixture("empty"))
        .unwrap_err()
        .to_string();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&pkg_text)
    );

    let logic = copy_hello();
    fs::write(
        logic.path().join("wit/world.wit"),
        "package mods:hello@0.1.0;\ninterface api { ping: func(); }\nworld hello { export api; }\n",
    )
    .unwrap();
    fs::write(logic.path().join("logic.wasm"), b"unbuilt").unwrap();
    assert!(
        watane_pkg::Mod::read(logic.path())
            .unwrap()
            .logic()
            .is_some()
    );
    let response = preview.install(logic.path()).await;
    assert_eq!(response.status(), 422);
    let error = response.json::<Value>().await.unwrap();
    assert_eq!(error["error"]["code"], "invalid_mod");
    let pkg_text = watane_pkg::Mod::read(logic.path())
        .unwrap()
        .check_component()
        .unwrap_err()
        .to_string();
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains(&pkg_text)
    );
    assert_eq!(
        fs::read_dir(home.path().join("staging")).unwrap().count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_install_transport_and_symlink_guards() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}install", preview.ready.api);
    for (body, status) in [
        ("broken".to_owned(), 400),
        ("{}".to_owned(), 400),
        ("x".repeat(65_537), 413),
    ] {
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "bad_request"
        );
    }
    let response = client.post(&url).body("{}").send().await.unwrap();
    assert_eq!(response.status(), 415);
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "bad_request"
    );
    let response = client
        .post(&url)
        .header("Origin", "null")
        .json(&json!({"folders":[fixture("hello")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(!home.path().join("mods/hello").exists());

    let folder = copy_hello();
    std::os::unix::fs::symlink("/etc/passwd", folder.path().join("ui/leak.txt")).unwrap();
    let response = preview.install(folder.path()).await;
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_batch_install_orders_local_names_and_rejects_a_bad_batch() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    let zeta = copy_hello();
    let alpha = copy_hello();
    for (folder, slug) in [(&zeta, "zeta"), (&alpha, "alpha")] {
        fs::write(
            folder.path().join("wit/world.wit"),
            format!("package mods:{slug}@0.1.0;\nworld hello {{}}\n"),
        )
        .unwrap();
    }
    let client = preview.client("localhost", "127.0.0.1");
    let url = format!("{}install", preview.ready.api);
    let response = client
        .post(&url)
        .json(&json!({"folders":[zeta.path(),alpha.path()]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let body = response.json::<Value>().await.unwrap();
    let names: Vec<_> = body["installed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["local"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["alpha", "zeta"]);
    let response = client
        .post(&url)
        .json(&json!({"folders":[fixture("hello"),fixture("empty")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
    let response = client
        .post(&url)
        .json(&json!({"folders":[fixture("hello"),fixture("hello")]}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
    assert!(!home.path().join("mods/hello").exists());
    assert_eq!(
        fs::read_dir(home.path().join("staging")).unwrap().count(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_long_slugs_truncate_and_collisions_keep_distinct_origins() {
    let home = tempfile::tempdir().unwrap();
    let preview = Preview::start(home.path());
    for (slug, local) in [
        ("abcdefghijklmnopqrstu-one", "abcdefghijklmnopqrstu"),
        ("abcdefghijklmnopqrstu-two", "abcdefghijklmnopqrstu-2"),
        ("abcdefghijklmnopqrst-one", "abcdefghijklmnopqrst"),
    ] {
        let folder = copy_hello();
        fs::write(
            folder.path().join("wit/world.wit"),
            format!("package mods:{slug}@0.1.0;\nworld hello {{}}\n"),
        )
        .unwrap();
        let response = preview.install(folder.path()).await;
        assert_eq!(response.status(), 200);
        let body = response.json::<Value>().await.unwrap();
        assert_eq!(body["installed"][0]["local"], local);
        let response = preview
            .client(&format!("{local}.localhost"), "127.0.0.1")
            .get(format!("http://{local}.localhost:{}/", preview.ready.port))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac2_home_serves_existing_mods_and_ac3_keeps_quiet_updates() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("home.json"),
        br#"{"watane_home":1,"mode":"home"}"#,
    )
    .unwrap();
    let folder = copy_hello();
    let installed = dir.path().join("mods/hello");
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        fs::create_dir_all(installed.join(file).parent().unwrap()).unwrap();
        fs::copy(folder.path().join(file), installed.join(file)).unwrap();
    }
    fs::write(
        dir.path().join("installed.json"),
        serde_json::to_vec(&json!({
            "watane_installed":1,"generation":1,"mods":[{
                "local":"hello","identity":"local/hello","package":HELLO_PACKAGE,
                "release":null,"enabled":true,"grants":[],"links":[]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let mut home = Preview::home(dir.path());
    assert_eq!(
        serde_json::to_value(&home.ready.mods).unwrap(),
        json!([{
            "local":"hello","page":format!("http://hello.localhost:{}/", home.ready.port)
        }])
    );
    let url = format!("http://hello.localhost:{}/", home.ready.port);
    let response = home
        .client("hello.localhost", "127.0.0.1")
        .get(&url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        fs::read(fixture("hello").join("ui/index.html")).unwrap()
    );
    let before = fs::read(dir.path().join("installed.json")).unwrap();
    assert_eq!(home.install(folder.path()).await.status(), 200);
    assert_eq!(fs::read(dir.path().join("installed.json")).unwrap(), before);
    fs::write(folder.path().join("ui/index.html"), b"updated page").unwrap();
    assert_eq!(home.install(folder.path()).await.status(), 200);
    let response = home
        .client("hello.localhost", "127.0.0.1")
        .get(url)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"updated page");
    home.stop();
}

#[tokio::test]
async fn ac5_extra_api_receives_one_mib_unchanged_under_common_guards() {
    use axum::Router;
    use axum::body::Bytes;
    use axum::extract::DefaultBodyLimit;
    use axum::routing::post;
    let home = tempfile::tempdir().unwrap();
    let host = watane_host::Host::open(
        home.path(),
        watane_host::Mode::Home { app: None },
        watane_host::Limits::default(),
    )
    .unwrap();
    let extra = Router::new()
        .route("/api/publish", post(|body: Bytes| async move { body }))
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024));
    let served = host.serve(0, extra).await.unwrap();
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/api/publish", served.port);
    let body: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
    let response = client.post(&url).body(body.clone()).send().await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await.unwrap().as_ref(), body);
    let small = b"{}".to_vec();
    let response = client
        .post(&url)
        .header("Origin", "null")
        .body(small.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = client
        .post(&url)
        .header("Host", "evil.example")
        .body(small.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 421);
    let response = client
        .post(&url)
        .header("Host", format!("missing.localhost:{}", served.port))
        .body(small)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    // The 413 guard stops reading at the limit, so the peer may reset the connection
    // while we still write. A raw socket reads the status line whatever happens.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let over = vec![b'x'; 64 * 1024 + 1];
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", served.port))
        .await
        .unwrap();
    let head = format!(
        "POST /api/plan HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        served.port,
        over.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    let _ = stream.write_all(&over).await;
    let mut status = Vec::new();
    let mut buf = [0u8; 256];
    while !status.windows(2).any(|w| w == b"\r\n") {
        match stream.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => status.extend_from_slice(&buf[..n]),
        }
    }
    let line = String::from_utf8_lossy(&status);
    assert!(
        line.starts_with("HTTP/1.1 413"),
        "expected 413, got {line:?}"
    );
    served.shutdown().await;
}

#[test]
fn ac1_s84_installed_is_public_and_confirm_defaults_to_noop() {
    use watane_host::{Answer, Confirm, InstallRequest, Installed, Requester};
    struct EarlierConfirm;
    impl Confirm for EarlierConfirm {
        fn install(
            &self,
            _: InstallRequest,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Answer> + Send>> {
            Box::pin(async { Answer::Deny })
        }
    }
    let report = Installed {
        requester: Requester::App,
        new: vec![watane_host::LocalName::parse("hello").unwrap()],
        updated: vec![],
    };
    assert_eq!(report.clone(), report);
    assert_eq!(
        format!("{report:?}"),
        "Installed { requester: App, new: [LocalName(\"hello\")], updated: [] }"
    );
    EarlierConfirm.installed(&report);
}

mod s84 {
    use super::*;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, Weak};
    use watane_host::{
        Answer, Confirm, Host, InstallRequest, Installed, Limits, LocalName, Mode, Requester,
        Source,
    };

    #[derive(Default)]
    struct App {
        deny: Mutex<bool>,
        reports: Mutex<Vec<Installed>>,
        host: Mutex<Weak<Host>>,
    }
    impl Confirm for App {
        fn install(&self, _: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
            let deny = *self.deny.lock().unwrap();
            Box::pin(async move {
                if deny {
                    Answer::Deny
                } else {
                    Answer::Approve { withhold: vec![] }
                }
            })
        }
        fn installed(&self, report: &Installed) {
            let host = self.host.lock().unwrap().upgrade().unwrap();
            let disk: Value =
                serde_json::from_slice(&fs::read(host.home().join("installed.json")).unwrap())
                    .unwrap();
            // The callback sees both the durable catalog and the swapped registry.
            for name in report.new.iter().chain(&report.updated) {
                assert!(
                    disk["mods"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|m| m["local"] == name.as_ref())
                );
                assert!(host.get(name).is_some());
            }
            self.reports.lock().unwrap().push(report.clone());
        }
    }
    fn local(name: &str) -> LocalName {
        LocalName::parse(name).unwrap()
    }
    fn home() -> (TempDir, Arc<Host>, Arc<App>) {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(App::default());
        let host = Host::open(
            dir.path(),
            Mode::Home {
                app: Some(Arc::<App>::clone(&app)),
            },
            Limits::default(),
        )
        .unwrap();
        *app.host.lock().unwrap() = Arc::downgrade(&host);
        (dir, host, app)
    }

    #[tokio::test]
    async fn ac3_s84_w37_api_reports_new_in_install_order_and_skips_denied_and_bad_plans() {
        let (_dir, host, app) = home();
        let served = host.serve(0, axum::Router::new()).await.unwrap();
        let client = Client::builder().no_proxy().build().unwrap();
        let url = format!("http://127.0.0.1:{}/api/install", served.port);
        let response = client
            .post(&url)
            .json(&json!({"folders":[fixture("recs"), fixture("shelf")]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            *app.reports.lock().unwrap(),
            vec![Installed {
                requester: Requester::LocalApi,
                new: vec![local("shelf"), local("recs")],
                updated: vec![],
            }]
        );
        app.reports.lock().unwrap().clear();
        *app.deny.lock().unwrap() = true;
        assert_eq!(
            client
                .post(&url)
                .json(&json!({"folders":[fixture("hello")]}))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert!(app.reports.lock().unwrap().is_empty());
        *app.deny.lock().unwrap() = false;
        assert_eq!(
            client
                .post(&url)
                .json(&json!({"folders":[fixture("hello"), fixture("empty")]}))
                .send()
                .await
                .unwrap()
                .status(),
            422
        );
        assert!(app.reports.lock().unwrap().is_empty());
        served.shutdown().await;
    }

    #[tokio::test]
    async fn ac2_s84_app_and_manager_report_updates_and_omit_unchanged() {
        let (_dir, host, app) = home();
        let folder = copy_hello();
        let plan = host
            .plan(vec![
                Source::Folder(fixture("manager")),
                Source::Folder(folder.path().into()),
            ])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        assert_eq!(
            *app.reports.lock().unwrap(),
            vec![Installed {
                requester: Requester::App,
                new: vec![local("hello"), local("manager")],
                updated: vec![],
            }]
        );
        app.reports.lock().unwrap().clear();
        for requester in [Requester::App, Requester::Mod(local("manager"))] {
            fs::write(
                folder.path().join("ui/index.html"),
                format!("updated by {requester:?}"),
            )
            .unwrap();
            let plan = host
                .plan(vec![
                    Source::Folder(fixture("manager")),
                    Source::Folder(folder.path().into()),
                ])
                .unwrap();
            host.install(plan, requester.clone()).await.unwrap();
            assert_eq!(
                *app.reports.lock().unwrap(),
                vec![Installed {
                    requester: requester.clone(),
                    new: vec![],
                    updated: vec![local("hello")],
                }]
            );
            app.reports.lock().unwrap().clear();
            let before = fs::read(host.home().join("installed.json")).unwrap();
            let plan = host
                .plan(vec![Source::Folder(folder.path().into())])
                .unwrap();
            host.install(plan, requester).await.unwrap();
            assert_eq!(
                fs::read(host.home().join("installed.json")).unwrap(),
                before
            );
            assert!(app.reports.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn ac2_s84_no_report_for_missing_stale_or_failed_commit() {
        let (_dir, host, app) = home();
        let plan = host.plan(vec![Source::Folder(fixture("recs"))]).unwrap();
        assert_eq!(
            host.install(plan, Requester::App).await.unwrap_err().code(),
            "missing_providers"
        );
        assert!(app.reports.lock().unwrap().is_empty());
        let stale = host.plan(vec![Source::Folder(fixture("hello"))]).unwrap();
        let plan = host.plan(vec![Source::Folder(fixture("hello"))]).unwrap();
        host.install(plan, Requester::App).await.unwrap();
        app.reports.lock().unwrap().clear();
        assert_eq!(
            host.install(stale, Requester::App)
                .await
                .unwrap_err()
                .code(),
            "stale_plan"
        );
        assert!(app.reports.lock().unwrap().is_empty());
        let folder = copy_hello();
        fs::write(folder.path().join("ui/index.html"), "updated").unwrap();
        let plan = host
            .plan(vec![Source::Folder(folder.path().into())])
            .unwrap();
        let before = fs::read(host.home().join("installed.json")).unwrap();
        // A directory at the temporary file path makes the catalog write fail.
        fs::create_dir(host.home().join("installed.json.tmp")).unwrap();
        assert_eq!(
            host.install(plan, Requester::App).await.unwrap_err().code(),
            "internal"
        );
        assert!(app.reports.lock().unwrap().is_empty());
        assert_eq!(
            fs::read(host.home().join("installed.json")).unwrap(),
            before
        );
        assert_eq!(
            fs::read(host.home().join("mods/hello/ui/index.html")).unwrap(),
            fs::read(fixture("hello").join("ui/index.html")).unwrap()
        );
    }
}
