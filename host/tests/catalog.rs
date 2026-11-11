use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
use watane_host::{Host, Limits};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}

fn page() -> tempfile::TempDir {
    let folder = tempfile::tempdir().unwrap();
    for file in ["mod.toml", "wit/world.wit", "ui/index.html", "ui/app.js"] {
        let dest = folder.path().join(file);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        fs::copy(fixture("hello").join(file), dest).unwrap();
    }
    folder
}

fn catalog(home: &Path) -> Value {
    serde_json::from_slice(&fs::read(home.join("installed.json")).unwrap()).unwrap()
}

#[test]
fn ac1_installed_and_persisted_ids_match_pack_for_both_parts() {
    for name in ["counter", "hello"] {
        let folder = fixture(name);
        let expected = watane_pkg::pack(&folder, false).unwrap().id();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let installed = host.install_folders(&[folder], 0).unwrap();
        assert_eq!(installed[0].package, expected);
        assert_eq!(
            catalog(home.path())["mods"][0]["package"],
            expected.to_string()
        );
        drop(host);
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let served = host.install_folders(&[fixture(name)], 0).unwrap();
        assert_eq!(served[0].package, expected);
        assert_eq!(catalog(home.path())["generation"], 1);
    }
}

#[test]
fn ac2_pack_skipped_files_are_absent_from_install() {
    let folder = page();
    for file in [
        ".git/x",
        ".hidden",
        "ui/.hidden",
        "logic/target/x",
        "logic/src/lib.rs",
    ] {
        let path = folder.path().join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "excluded").unwrap();
    }
    let expected = watane_pkg::pack(folder.path(), false).unwrap().id();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let installed = host.install_folders(&[folder.path().into()], 0).unwrap();
    for file in [".git", ".hidden", "ui/.hidden", "logic"] {
        assert!(
            !home.path().join("mods/hello").join(file).exists(),
            "{file}"
        );
    }
    assert_eq!(installed[0].package, expected);
}

#[test]
fn ac3_unchanged_keeps_generation_one_byte_updates_package() {
    let folder = page();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let folders = [folder.path().into()];
    let first = host.install_folders(&folders, 0).unwrap();
    let before = fs::read(home.path().join("installed.json")).unwrap();
    let same = host.install_folders(&folders, 0).unwrap();
    assert_eq!(same[0].package, first[0].package);
    assert_eq!(
        fs::read(home.path().join("installed.json")).unwrap(),
        before
    );
    let path = folder.path().join("ui/index.html");
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(path, bytes).unwrap();
    let updated = host.install_folders(&folders, 0).unwrap();
    assert_ne!(updated[0].package, first[0].package);
    assert_eq!(
        updated[0].package,
        watane_pkg::pack(folder.path(), false).unwrap().id()
    );
    assert_eq!(catalog(home.path())["generation"], 2);
}

#[test]
fn ac4_package_errors_name_input_and_leave_no_staging() {
    for symlink in [true, false] {
        let folder = page();
        if symlink {
            std::os::unix::fs::symlink("index.html", folder.path().join("ui/link")).unwrap();
        } else {
            fs::File::create(folder.path().join("ui/large"))
                .unwrap()
                .set_len(watane_pkg::MAX_PACKAGE_BYTES + 1)
                .unwrap();
        }
        let expected = watane_pkg::pack(folder.path(), false)
            .unwrap_err()
            .to_string();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let error = host
            .install_folders(&[fixture("hello"), folder.path().into()], 0)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_mod");
        match error {
            watane_host::HostError::InvalidMod { input, reason } => {
                assert_eq!(input, folder.path().display().to_string());
                assert_eq!(reason, expected);
            }
            other => panic!("{other}"),
        }
        assert_eq!(
            fs::read_dir(home.path().join("staging")).unwrap().count(),
            0
        );
        assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
    }
}

#[test]
fn ac1_s11_providers_install_first_and_links_are_literal_json() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("recs"), fixture("shelf")], 0)
        .unwrap();
    let disk = catalog(home.path());
    assert_eq!(
        disk["mods"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["local"].clone())
            .collect::<Vec<_>>(),
        vec![serde_json::json!("shelf"), serde_json::json!("recs")]
    );
    assert_eq!(
        disk["mods"][1]["links"],
        serde_json::json!([
            {"kind":"call","interface":"mods:shelf/items@0.1.0","provider":"shelf","provider_interface":"mods:shelf/items@0.1.0"},
            {"kind":"subscribe","interface":"mods:shelf/events@0.1.0","provider":"shelf","provider_interface":"mods:shelf/events@0.1.0"}
        ])
    );
}

#[test]
fn ac2_s11_missing_provider_refuses_without_installing_anything() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let error = host.install_folders(&[fixture("recs")], 0).unwrap_err();
    assert_eq!(error.code(), "missing_providers");
    assert!(error.to_string().contains("shelf"));
    assert!(error.to_string().contains("0.1.0"));
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
}
#[test]
fn ac3_s11_call_cycle_names_closed_path() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let error = host
        .install_folders(&[fixture("cyc-a"), fixture("cyc-b")], 0)
        .unwrap_err();
    assert_eq!(error.code(), "cycle");
    assert!(
        error.to_string().contains("cyc-a -> cyc-b -> cyc-a"),
        "{error}"
    );
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
}

#[test]
fn ac1_s11_consumer_can_link_to_an_already_installed_provider() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("shelf")], 0).unwrap();
    host.install_folders(&[fixture("recs")], 0).unwrap();
    assert_eq!(
        catalog(home.path())["mods"][1]["links"][0]["provider"],
        "shelf"
    );
}
#[test]
fn ac2_s11_pinned_publisher_never_links_to_a_local_provider() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("shelf")], 0).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let dir = folder.path().join("recs");
    watane_pkg::pack(&fixture("recs"), false)
        .unwrap()
        .unpack(&dir)
        .unwrap();
    fs::write(
        dir.join("deps.json"),
        r#"{"mods:shelf":"yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo"}"#,
    )
    .unwrap();
    let error = host
        .install_folders(&[fixture("counter"), dir], 0)
        .unwrap_err();
    assert_eq!(error.code(), "missing_providers");
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 1);
    assert!(!home.path().join("mods/counter").exists());
    assert_eq!(catalog(home.path())["generation"], 1);
}

mod participant {
    use super::*;
    use serde_json::json;
    use std::{
        collections::VecDeque,
        future::Future,
        pin::Pin,
        sync::{Arc, Mutex},
    };
    use watane_host::{
        Answer, Caller, Change, Confirm, InstallRequest, LocalName, Mode, Requester, Source,
    };
    use watane_pkg::Grant;

    #[derive(Default)]
    struct Fake {
        requests: Mutex<Vec<InstallRequest>>,
        answers: Mutex<VecDeque<Answer>>,
    }
    impl Fake {
        fn new(answers: Vec<Answer>) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(answers.into()),
                ..Self::default()
            })
        }
        fn count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }
    impl Confirm for Fake {
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
    fn approve() -> Answer {
        Answer::Approve { withhold: vec![] }
    }
    fn local(s: &str) -> LocalName {
        LocalName::parse(s).unwrap()
    }
    fn home(fake: Arc<Fake>) -> (tempfile::TempDir, Arc<Host>) {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(
            dir.path(),
            Mode::Home { app: Some(fake) },
            Limits::default(),
        )
        .unwrap();
        (dir, host)
    }
    fn plan(host: &Host, names: &[&str]) -> watane_host::Plan {
        host.plan(names.iter().map(|n| Source::Folder(fixture(n))).collect())
            .unwrap()
    }
    async fn install(host: &Host, names: &[&str]) {
        host.install(plan(host, names), Requester::App)
            .await
            .unwrap();
    }
    async fn call(host: &Arc<Host>, name: &str, function: &str, args: Value) -> Value {
        host.call_async(Caller::Page, local(name), function.into(), args)
            .await
            .unwrap()
    }
    async fn denied(host: &Arc<Host>, name: &str, function: &str) {
        let e = host
            .call_async(Caller::Page, local(name), function.into(), Value::Null)
            .await
            .unwrap_err();
        assert_eq!(serde_json::to_value(e).unwrap()["code"], "not_granted");
    }

    #[test]
    fn ac1_s12a_plan_is_provider_first_and_serializes() {
        let (_dir, host) = home(Fake::default().into());
        let p = plan(&host, &["recs", "shelf"]);
        assert!(p.needs_confirmation());
        assert_eq!(
            p.entries()
                .iter()
                .map(|e| e.local.to_string())
                .collect::<Vec<_>>(),
            ["shelf", "recs"]
        );
        for e in p.entries() {
            assert!(e.publisher.is_none());
            assert!(matches!(e.change, Change::New));
        }
        assert_eq!(p.entries()[1].providers, [local("shelf")]);
        let value = serde_json::to_value(&p).unwrap();
        // All plan fields are checked against a literal; package bytes have an
        // independent Python canonical framing and BLAKE3 oracle.
        assert_eq!(value, expected_plan());
        drop(p);
        assert_eq!(
            fs::read_dir(host.home().join("staging")).unwrap().count(),
            0
        );
    }
    fn expected_plan() -> Value {
        json!({"entries":[
            {"local":"shelf","identity":"local/shelf","slug":"shelf","name":"shelf","summary":"S11 shelf fixture.","version":"0.1.0","publisher":null,"package":"74e8e328e2b0e5d6090674ce1d2dbb4f1a2db094762fb62bd3089936faed9176","release":null,"origin":null,"change":{"kind":"new"},"grants":[{"kind":"storage"}],"new_grants":[{"kind":"storage"}],"providers":[]},
            {"local":"recs","identity":"local/recs","slug":"recs","name":"recs","summary":"S11 recs fixture.","version":"0.1.0","publisher":null,"package":"07cb4a2e9c962cd1119c5947521c4f261619d2b5e84f1dcca4bc95713b3fb7b9","release":null,"origin":null,"change":{"kind":"new"},"grants":[{"kind":"storage"},{"kind":"call","interface":"mods:shelf/items@0.1.0","provider":{"slug":"shelf","publisher":null}},{"kind":"subscribe","interface":"mods:shelf/events@0.1.0","provider":{"slug":"shelf","publisher":null}}],"new_grants":[{"kind":"storage"},{"kind":"call","interface":"mods:shelf/items@0.1.0","provider":{"slug":"shelf","publisher":null}},{"kind":"subscribe","interface":"mods:shelf/events@0.1.0","provider":{"slug":"shelf","publisher":null}}],"providers":["shelf"]}],"missing":[]})
    }
    #[tokio::test]
    async fn ac2_s12a_missing_provider_never_asks_or_installs() {
        let fake = Fake::new(vec![]);
        let (dir, host) = home(fake.clone());
        let p = plan(&host, &["recs"]);
        assert_eq!(
            serde_json::to_value(p.missing()).unwrap(),
            json!([{"provider":{"slug":"shelf","publisher":null},"version":"0.1.0","release":null,"needed_by":"local/recs"}])
        );
        assert_eq!(
            host.install(p, Requester::App).await.unwrap_err().code(),
            "missing_providers"
        );
        assert_eq!(fake.count(), 0);
        assert_eq!(fs::read_dir(dir.path().join("mods")).unwrap().count(), 0);
    }
    #[test]
    fn ac3_s12a_plan_rejects_cycle_and_empty() {
        let (_dir, host) = home(Fake::default().into());
        let e = host
            .plan(vec![
                Source::Folder(fixture("cyc-a")),
                Source::Folder(fixture("cyc-b")),
            ])
            .unwrap_err();
        assert_eq!(e.code(), "cycle");
        assert!(e.to_string().contains("cyc-a -> cyc-b -> cyc-a"));
        assert_eq!(
            host.plan(vec![Source::Folder(fixture("empty"))])
                .unwrap_err()
                .code(),
            "invalid_mod"
        );
    }
    #[tokio::test]
    async fn ac4_s12a_confirmation_denies_and_withholds_storage() {
        let fake = Fake::new(vec![
            Answer::Deny,
            Answer::Approve {
                withhold: vec![(local("counter"), Grant::Storage)],
            },
        ]);
        let (dir, host) = home(fake.clone());
        assert_eq!(
            host.install(plan(&host, &["counter"]), Requester::App)
                .await
                .unwrap_err()
                .code(),
            "refused"
        );
        assert_eq!(fake.count(), 1);
        assert!(!dir.path().join("mods/counter").exists());
        install(&host, &["counter"]).await;
        assert!(!host.get(&local("counter")).unwrap().grants[0].granted);
        assert_eq!(
            catalog(dir.path())["mods"][0]["grants"],
            json!([{"grant":{"kind":"storage"},"granted":false}])
        );
        let requests = fake.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(matches!(requests[1].requester, Requester::App));
        assert_eq!(requests[1].entries.len(), 1);
        assert_eq!(requests[1].entries[0].local, local("counter"));
    }
    #[tokio::test]
    async fn ac5_s12a_local_api_reaches_confirm() {
        let fake = Fake::new(vec![approve()]);
        let (_dir, host) = home(fake.clone());
        let served = host.serve(0, axum::Router::new()).await.unwrap();
        let r = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/api/install", served.port))
            .json(&json!({"folders":[fixture("counter")]}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(
            r.json::<Value>().await.unwrap()["installed"][0]["local"],
            "counter"
        );
        assert!(matches!(
            fake.requests.lock().unwrap()[0].requester,
            Requester::LocalApi
        ));
        served.shutdown().await;
    }
    #[tokio::test]
    async fn ac6_s12a_update_keeps_state_and_withdrawn_grants_without_asking() {
        let fake = Fake::new(vec![approve()]);
        let (_dir, host) = home(fake.clone());
        install(&host, &["counter"]).await;
        call(&host, "counter", "api.add", Value::Null).await;
        call(&host, "counter", "api.add", Value::Null).await;
        let copy = tempfile::tempdir().unwrap();
        let folder = copy.path().join("counter");
        watane_pkg::pack(&fixture("counter"), false)
            .unwrap()
            .unpack(&folder)
            .unwrap();
        let ui = folder.join("ui/index.html");
        let mut bytes = fs::read(&ui).unwrap();
        bytes[0] ^= 1;
        fs::write(&ui, bytes).unwrap();
        let p = host.plan(vec![Source::Folder(folder.clone())]).unwrap();
        assert!(!p.needs_confirmation());
        assert_eq!(
            serde_json::to_value(&p.entries()[0].change).unwrap(),
            json!({"kind":"update","from_version":"0.1.0"})
        );
        host.install(p, Requester::App).await.unwrap();
        assert_eq!(call(&host, "counter", "api.get", Value::Null).await, 2);
        host.revoke(&local("counter"), &Grant::Storage).unwrap();
        fs::write(ui, b"another build").unwrap();
        host.install(
            host.plan(vec![Source::Folder(folder)]).unwrap(),
            Requester::App,
        )
        .await
        .unwrap();
        assert!(!host.get(&local("counter")).unwrap().grants[0].granted);
        assert_eq!(fake.count(), 1);
    }
    #[tokio::test]
    async fn ac7_s12a_call_and_subscription_grants_stop_and_resume() {
        let (_dir, host) = home(Fake::new(vec![approve()]));
        install(&host, &["shelf", "recs"]).await;
        call(&host, "shelf", "items.add", json!({"title":"Dune"})).await;
        let grants = host.get(&local("recs")).unwrap().grants;
        let call_grant = &grants
            .iter()
            .find(|g| matches!(g.grant, Grant::Call { .. }))
            .unwrap()
            .grant;
        let sub = &grants
            .iter()
            .find(|g| matches!(g.grant, Grant::Subscribe { .. }))
            .unwrap()
            .grant;
        host.revoke(&local("recs"), call_grant).unwrap();
        denied(&host, "recs", "api.suggest").await;
        assert_eq!(call(&host, "recs", "api.ping", Value::Null).await, "pong");
        host.grant(&local("recs"), call_grant).unwrap();
        assert_eq!(
            call(&host, "recs", "api.suggest", Value::Null).await,
            "Dune"
        );
        host.revoke(&local("recs"), sub).unwrap();
        call(&host, "shelf", "items.add", json!({"title":"X"})).await;
        assert_eq!(
            call(&host, "recs", "api.received", Value::Null).await,
            json!(["Dune"])
        );
        host.grant(&local("recs"), sub).unwrap();
        call(&host, "shelf", "items.add", json!({"title":"Y"})).await;
        assert_eq!(
            call(&host, "recs", "api.received", Value::Null).await,
            json!(["Dune", "Y"])
        );
        for result in [
            host.grant(&local("shelf"), call_grant),
            host.revoke(&local("shelf"), sub),
        ] {
            assert_eq!(result.unwrap_err().code(), "not_declared");
        }
    }
    #[tokio::test]
    async fn ac9_s12a_remove_keeps_or_deletes_state_and_unbinds_links() {
        let (_dir, host) = home(Fake::new(vec![approve(), approve(), approve(), approve()]));
        install(&host, &["counter"]).await;
        call(&host, "counter", "api.add", Value::Null).await;
        host.remove(&local("counter"), false).unwrap();
        assert!(host.get(&local("counter")).is_none());
        assert!(!host.home().join("mods/counter").exists());
        install(&host, &["counter"]).await;
        assert_eq!(call(&host, "counter", "api.get", Value::Null).await, 1);
        host.remove(&local("counter"), true).unwrap();
        install(&host, &["counter"]).await;
        assert_eq!(call(&host, "counter", "api.get", Value::Null).await, 0);
        install(&host, &["recs", "shelf"]).await;
        host.remove(&local("shelf"), false).unwrap();
        assert!(
            host.get(&local("recs"))
                .unwrap()
                .links
                .iter()
                .all(|l| l.provider.is_none())
        );
        denied(&host, "recs", "api.suggest").await;
    }
    #[tokio::test]
    async fn ac10_s12a_disable_enable_and_restart() {
        let fake = Fake::new(vec![approve()]);
        let (dir, mut host) = home(fake.clone());
        install(&host, &["shelf", "recs"]).await;
        call(&host, "shelf", "items.add", json!({"title":"Dune"})).await;
        for enabled in [false, true] {
            if enabled {
                host.enable(&local("shelf")).unwrap();
            } else {
                host.disable(&local("shelf")).unwrap();
            }
            drop(host);
            host = Host::open(
                dir.path(),
                Mode::Home {
                    app: Some(fake.clone()),
                },
                Limits::default(),
            )
            .unwrap();
            assert_eq!(host.get(&local("shelf")).unwrap().enabled, enabled);
            assert_eq!(
                host.mods()
                    .iter()
                    .map(|m| m.local.to_string())
                    .collect::<Vec<_>>(),
                ["recs", "shelf"]
            );
            let served = host.serve(0, axum::Router::new()).await.unwrap();
            let client = reqwest::Client::new();
            let url = format!("http://127.0.0.1:{}", served.port);
            let header = format!("shelf.localhost:{}", served.port);
            let r = client
                .post(format!("{url}/call"))
                .header("Host", &header)
                .header("Origin", format!("http://{header}"))
                .json(&json!({"fn":"items.all","args":null}))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), if enabled { 200 } else { 409 });
            if !enabled {
                assert_eq!(
                    r.json::<Value>().await.unwrap()["error"]["code"],
                    "disabled"
                );
            }
            assert_eq!(
                client
                    .get(format!("{url}/"))
                    .header("Host", header)
                    .send()
                    .await
                    .unwrap()
                    .status(),
                if enabled { 200 } else { 404 }
            );
            if enabled {
                assert_eq!(
                    call(&host, "recs", "api.suggest", Value::Null).await,
                    "Dune"
                );
            } else {
                denied(&host, "recs", "api.suggest").await;
            }
            served.shutdown().await;
        }
    }
    #[tokio::test]
    async fn ac11_s12a_extra_routes_have_guards_and_their_own_body_limit() {
        use axum::{
            Router,
            extract::DefaultBodyLimit,
            routing::{get, post},
        };
        let (_dir, host) = home(Fake::default().into());
        let extra = Router::new()
            .route("/api/hello", get(|| async { "hello" }))
            .route(
                "/api/upload",
                post(|bytes: axum::body::Bytes| async move { bytes.len().to_string() }),
            )
            .layer(DefaultBodyLimit::max(2 << 20));
        let served = host.serve(0, extra).await.unwrap();
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}", served.port);
        assert_eq!(
            client
                .get(format!("{url}/api/hello"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "hello"
        );
        assert_eq!(
            client
                .get(format!("{url}/api/hello"))
                .header("Origin", "null")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            client
                .get(format!("{url}/api/hello"))
                .header("Host", format!("counter.localhost:{}", served.port))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .post(format!("{url}/api/upload"))
                .body(vec![b'x'; 100_000])
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "100000"
        );
        served.shutdown().await;
    }
    #[tokio::test]
    async fn ac4_s12a_stale_plan_never_asks_or_overwrites_catalog() {
        let fake = Fake::new(vec![approve()]);
        let (_dir, host) = home(fake.clone());
        let pending = plan(&host, &["counter"]);
        install(&host, &["shelf"]).await;
        assert_eq!(
            host.install(pending, Requester::App)
                .await
                .unwrap_err()
                .code(),
            "stale_plan"
        );
        assert_eq!(fake.count(), 1);
        assert!(host.get(&local("counter")).is_none());
        assert!(host.get(&local("shelf")).is_some());
        assert_eq!(
            fs::read_dir(host.home().join("staging")).unwrap().count(),
            0
        );
    }

    #[tokio::test]
    async fn ac5_s12a_api_plan_and_mods_are_read_only() {
        let fake = Fake::new(vec![]);
        let (_dir, host) = home(fake.clone());
        let served = host.serve(0, axum::Router::new()).await.unwrap();
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}", served.port);
        let response = client
            .post(format!("{url}/api/plan"))
            .json(&json!({"folders":[fixture("recs"),fixture("shelf")]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.json::<Value>().await.unwrap(), expected_plan());
        assert_eq!(
            client
                .get(format!("{url}/api/mods"))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap(),
            json!({"mods":[]})
        );
        assert_eq!(
            client
                .get(format!("{url}/api/status"))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()["app"],
            true
        );
        assert_eq!(fake.count(), 0);
        assert_eq!(
            fs::read_dir(host.home().join("staging")).unwrap().count(),
            0
        );
        assert_eq!(fs::read_dir(host.home().join("mods")).unwrap().count(), 0);
        served.shutdown().await;
    }

    #[tokio::test]
    async fn ac12_s12a_toolsets_and_live_tokens_are_manageable_without_secrets() {
        use watane_host::{Handle, ToolsetName};
        let (dir, mut host) = home(Fake::default().into());
        let t = ToolsetName::parse("t").unwrap();
        host.set_toolset(&t, vec!["counter:api".into()]).unwrap();
        drop(host);
        host = Host::open(dir.path(), Mode::Home { app: None }, Limits::default()).unwrap();
        assert_eq!(
            serde_json::to_value(host.toolsets()).unwrap(),
            json!({"t":["counter:api"]})
        );
        let token = host
            .issue_token(&t, &Handle::parse("alice").unwrap())
            .unwrap();
        let v = serde_json::to_value(host.tokens()).unwrap();
        assert_eq!(v, json!([{"id":token.id(),"toolset":"t","handle":"alice"}]));
        assert!(!v.to_string().contains(token.as_str()));
        assert!(host.end_token(&token.id()));
        assert!(host.tokens().is_empty());
        let served = host.serve(0, axum::Router::new()).await.unwrap();
        let r = reqwest::Client::new()
            .post(format!("http://127.0.0.1:{}/mcp/t", served.port))
            .bearer_auth(token.as_str())
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
        host.remove_toolset(&t).unwrap();
        assert!(host.toolsets().is_empty());
        assert_eq!(host.remove_toolset(&t).unwrap_err().code(), "no_toolset");
        served.shutdown().await;
    }
}

#[tokio::test]
#[should_panic(expected = "Overlapping method route")]
async fn ac11_s12a_extra_router_cannot_replace_builtin_api() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let extra =
        axum::Router::new().route("/api/install", axum::routing::post(|| async { "bypass" }));
    let _ = host.serve(0, extra).await;
}
