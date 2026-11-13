use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use watane_host::watane_log::Log;
use watane_host::watane_pkg::{self as pkg, Mod, Provider, PublisherKey, RELEASE_KIND, Release};
use watane_host::{
    Answer, Bind, Caller, Change, Confirm, Host, InstallRequest, Limits, LocalName, Mode,
    Requester, Source,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
fn local(name: &str) -> LocalName {
    LocalName::parse(name).unwrap()
}
#[derive(Default)]
struct ConfirmFake(Mutex<Vec<InstallRequest>>);
impl Confirm for ConfirmFake {
    fn install(
        &self,
        request: InstallRequest,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Answer> + Send>> {
        self.0.lock().unwrap().push(request);
        Box::pin(async { Answer::Approve { withhold: vec![] } })
    }
}
struct Releases {
    _dir: tempfile::TempDir,
    k1: Log,
    k2: Log,
    k3: Log,
    recs: PathBuf,
}
impl Releases {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let k1 = Log::open(&dir.path().join("k1")).unwrap();
        let k2 = Log::open(&dir.path().join("k2")).unwrap();
        let k3 = Log::open(&dir.path().join("k3")).unwrap();
        let recs = dir.path().join("recs");
        pkg::pack(&fixture("recs"), false)
            .unwrap()
            .unpack(&recs)
            .unwrap();
        fs::write(
            recs.join("deps.json"),
            json!({"mods:shelf": k1.identity().to_string()}).to_string(),
        )
        .unwrap();
        Self {
            _dir: dir,
            k1,
            k2,
            k3,
            recs,
        }
    }
    fn source(&self, log: &Log, dir: &Path) -> Source {
        let package = pkg::pack(dir, false).unwrap();
        let body = Release::new(&Mod::read(dir).unwrap(), &package).unwrap();
        let entry = log.append(RELEASE_KIND, &body.encode()).unwrap();
        Source::Release { package, entry }
    }
    fn recs(&self) -> Source {
        self.source(&self.k2, &self.recs)
    }
    fn shelf(&self) -> Source {
        self.source(&self.k3, &fixture("shelf"))
    }
    fn bind(&self) -> Bind {
        Bind {
            consumer: format!("{}/recs", self.k2.identity()),
            provider: Provider {
                slug: "shelf".parse().unwrap(),
                publisher: Some(PublisherKey::from_bytes(*self.k1.identity().as_bytes())),
            },
            to: format!("{}/shelf", self.k3.identity()),
        }
    }
}
fn home(path: &Path, fake: Arc<ConfirmFake>) -> Arc<Host> {
    Host::open(path, Mode::Home { app: Some(fake) }, Limits::default()).unwrap()
}
fn disk(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path.join("installed.json")).unwrap()).unwrap()
}
fn rec(value: &Value) -> &Value {
    value["mods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["local"] == "recs")
        .unwrap()
}
async fn who(host: &Arc<Host>) {
    assert_eq!(
        host.call_async(
            Caller::Page,
            local("recs"),
            "api.who-of-shelf".into(),
            Value::Null
        )
        .await
        .unwrap(),
        "mod:recs"
    );
}

#[tokio::test]
async fn ac1_picked_release_links_and_calls() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let host = home(dir.path(), Arc::default());
    let plan = host.plan(vec![r.recs(), r.shelf()]).unwrap();
    assert_eq!(plan.missing()[0].provider, r.bind().provider);
    assert_eq!(plan.missing()[0].needed_by, r.bind().consumer);
    let plan = host
        .plan_with(vec![r.recs(), r.shelf()], vec![r.bind()])
        .unwrap();
    assert!(plan.missing().is_empty());
    assert_eq!(
        plan.entries()
            .iter()
            .find(|e| e.slug.as_str() == "recs")
            .unwrap()
            .binds,
        vec![r.bind()]
    );
    host.install(plan, Requester::App).await.unwrap();
    who(&host).await;
    let call = host
        .get(&local("recs"))
        .unwrap()
        .grants
        .into_iter()
        .find(|g| matches!(g.grant, pkg::Grant::Call { .. }))
        .unwrap()
        .grant;
    assert!(matches!(&call, pkg::Grant::Call { provider, .. } if provider == &r.bind().provider));
    host.revoke(&local("recs"), &call).unwrap();
    let error = host
        .call_async(
            Caller::Page,
            local("recs"),
            "api.who-of-shelf".into(),
            Value::Null,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code, watane_host::CallCode::NotGranted);
}
#[tokio::test]
async fn ac2_binds_persist_and_step4_catalog_opens() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let host = home(dir.path(), Arc::default());
    host.install(
        host.plan_with(vec![r.recs(), r.shelf()], vec![r.bind()])
            .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    drop(host);
    let host = home(dir.path(), Arc::default());
    assert_eq!(host.get(&local("recs")).unwrap().binds, vec![r.bind()]);
    assert_eq!(
        host.get(&local("recs")).unwrap().links[0].provider,
        Some(local("shelf"))
    );
    who(&host).await;
    let mut value = disk(dir.path());
    assert_eq!(
        rec(&value)["binds"],
        json!([{"consumer":r.bind().consumer,"provider":{"slug":"shelf","publisher":r.k1.identity().to_string()},"to":r.bind().to}])
    );
    drop(host);
    for m in value["mods"].as_array_mut().unwrap() {
        m.as_object_mut().unwrap().remove("binds");
    }
    fs::write(dir.path().join("installed.json"), value.to_string()).unwrap();
    let host = home(dir.path(), Arc::default());
    assert!(host.get(&local("recs")).unwrap().binds.is_empty());
}
#[tokio::test]
async fn ac3_bind_only_update_confirms_and_keeps_directory_and_pick() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(ConfirmFake::default());
    let host = home(dir.path(), fake.clone());
    host.install(
        host.plan(vec![
            r.recs(),
            r.shelf(),
            r.source(&r.k1, &fixture("shelf")),
        ])
        .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    fake.0.lock().unwrap().clear();
    fs::write(dir.path().join("mods/recs/marker"), "survives").unwrap();
    let plan = host.plan_with(vec![r.recs()], vec![r.bind()]).unwrap();
    assert!(
        matches!(&plan.entries()[0].change, Change::Update { from_version } if from_version.to_string()=="0.1.0")
    );
    assert!(plan.needs_confirmation());
    host.install(plan, Requester::App).await.unwrap();
    assert_eq!(fake.0.lock().unwrap().len(), 1);
    assert_eq!(fake.0.lock().unwrap()[0].entries[0].binds, vec![r.bind()]);
    assert_eq!(
        fs::read_to_string(dir.path().join("mods/recs/marker")).unwrap(),
        "survives"
    );
    assert_eq!(
        host.plan(vec![r.recs()]).unwrap().entries()[0].binds,
        vec![r.bind()]
    );
    // A real package update also keeps the pick.
    fs::write(r.recs.join("ui/index.html"), "updated").unwrap();
    let plan = host.plan(vec![r.recs()]).unwrap();
    assert!(!plan.needs_confirmation());
    host.install(plan, Requester::App).await.unwrap();
    assert_eq!(host.get(&local("recs")).unwrap().binds, vec![r.bind()]);
}
#[tokio::test]
async fn ac4_remove_deletes_pick_and_reinstall_does_not_restore_it() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let host = home(dir.path(), Arc::default());
    host.install(
        host.plan_with(vec![r.recs(), r.shelf()], vec![r.bind()])
            .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    host.remove(&local("shelf"), false).unwrap();
    assert!(host.get(&local("recs")).unwrap().binds.is_empty());
    assert_eq!(rec(&disk(dir.path()))["binds"], json!([]));
    assert_eq!(disk(dir.path())["generation"], 2);
    host.install(host.plan(vec![r.shelf()]).unwrap(), Requester::App)
        .await
        .unwrap();
    assert!(
        host.get(&local("recs"))
            .unwrap()
            .links
            .iter()
            .all(|l| l.provider.is_none())
    );
}
#[test]
fn ac5_incompatible_pick_stays_missing_and_invalid_binds_name_the_bind() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let host = home(dir.path(), Arc::default());
    let plan = host
        .plan_with(
            vec![r.recs(), r.source(&r.k3, &fixture("shelf-no-items"))],
            vec![r.bind()],
        )
        .unwrap();
    assert_eq!(plan.missing()[0].provider, r.bind().provider);
    for bad in [
        Bind {
            to: "local/counter".into(),
            ..r.bind()
        },
        Bind {
            consumer: "local/absent".into(),
            ..r.bind()
        },
        Bind {
            provider: Provider {
                slug: "shelf".parse().unwrap(),
                publisher: None,
            },
            ..r.bind()
        },
        Bind {
            to: r.bind().provider.to_string(),
            ..r.bind()
        },
    ] {
        let err = host
            .plan_with(vec![r.recs(), r.shelf()], vec![bad.clone()])
            .unwrap_err();
        assert_eq!(err.code(), "bad_request");
        assert_eq!(err.status(), 400);
        let message = err.to_string();
        assert!(
            message.contains(&bad.consumer)
                && message.contains(&bad.to)
                && message.contains(&bad.provider.to_string()),
            "{message}"
        );
    }
}
#[tokio::test]
async fn ac6_preview_matches_slug_and_interface_participant_keeps_pin() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
    host.install(
        host.plan(vec![
            Source::Folder(r.recs.clone()),
            Source::Folder(fixture("shelf")),
        ])
        .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    who(&host).await;
    let dir = tempfile::tempdir().unwrap();
    let host = home(dir.path(), Arc::default());
    let plan = host
        .plan(vec![
            Source::Folder(r.recs.clone()),
            Source::Folder(fixture("shelf")),
        ])
        .unwrap();
    assert_eq!(plan.missing()[0].provider, r.bind().provider);
}
#[tokio::test]
async fn ac7_local_api_rejects_binds_and_lists_empty_binds() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("shelf")], 0).unwrap();
    let served = host.serve(0, axum::Router::new()).await.unwrap();
    let client = reqwest::Client::new();
    for route in ["plan", "install"] {
        let response = client
            .post(format!("http://127.0.0.1:{}/api/{route}", served.port))
            .json(&json!({"folders":[fixture("shelf")],"binds":[]}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
        assert_eq!(
            response.json::<Value>().await.unwrap()["error"]["code"],
            "bad_request"
        );
    }
    let value = client
        .get(format!("http://127.0.0.1:{}/api/mods", served.port))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(value["mods"][0]["binds"], json!([]));
    served.shutdown().await;
}

#[tokio::test]
async fn ac3_pick_for_installed_consumer_without_source_is_confirmed() {
    let r = Releases::new();
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(ConfirmFake::default());
    let host = home(dir.path(), fake.clone());
    host.install(
        host.plan(vec![r.recs(), r.source(&r.k1, &fixture("shelf"))])
            .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    fake.0.lock().unwrap().clear();
    let plan = host.plan_with(vec![r.shelf()], vec![r.bind()]).unwrap();
    assert!(
        plan.entries()
            .iter()
            .any(|e| e.identity == r.bind().consumer
                && matches!(e.change, Change::Update { .. })
                && e.binds == vec![r.bind()])
    );
    host.install(plan, Requester::App).await.unwrap();
    assert_eq!(fake.0.lock().unwrap().len(), 1);
    who(&host).await;
    host.remove(&local("shelf-2"), false).unwrap();
    assert!(host.get(&local("recs")).unwrap().binds.is_empty());
    assert_eq!(
        host.get(&local("recs")).unwrap().links[0].provider,
        Some(local("shelf"))
    );
    who(&host).await;
}
