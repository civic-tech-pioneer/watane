//! Step 10 (stage 11): S40 to S47 with the fixture `manager` (TECHNICAL 13.2).
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use watane_host::watane_pkg::Grant;
use watane_host::{
    Answer, Ask, CallCode, Caller, Confirm, Fakes, Harness, Host, InstallRequest, Limits,
    LocalName, ManageOutcome, ManageRecord, ManageRequest, Mode, Outcome, Reduction, Requester,
    Source,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
fn local(name: &str) -> LocalName {
    LocalName::parse(name).unwrap()
}

/// The app: records what it is asked and answers `manage` from a gate.
#[derive(Default)]
struct App {
    withhold: Mutex<Vec<(LocalName, Grant)>>,
    installs: Mutex<Vec<Requester>>,
    reduced: Mutex<Vec<(LocalName, Reduction)>>,
    requests: Mutex<Vec<ManageRequest>>,
    gates: Mutex<VecDeque<oneshot::Receiver<ManageOutcome>>>,
    dropped: AtomicUsize,
}
struct Guard(Arc<App>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
struct AppRef(Arc<App>);
impl App {
    /// The answer of the next request.
    fn gate(&self) -> oneshot::Sender<ManageOutcome> {
        let (tx, rx) = oneshot::channel();
        self.gates.lock().unwrap().push_back(rx);
        tx
    }
}
impl Confirm for AppRef {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        self.0.installs.lock().unwrap().push(request.requester);
        let withhold = self.0.withhold.lock().unwrap().clone();
        Box::pin(async move { Answer::Approve { withhold } })
    }
    fn manage(
        &self,
        request: ManageRequest,
    ) -> Pin<Box<dyn Future<Output = ManageOutcome> + Send>> {
        self.0.requests.lock().unwrap().push(request);
        let gate = self.0.gates.lock().unwrap().pop_front();
        let guard = Guard(Arc::clone(&self.0));
        Box::pin(async move {
            let _guard = guard;
            match gate {
                Some(rx) => rx.await.unwrap_or(ManageOutcome::Denied),
                None => std::future::pending().await,
            }
        })
    }
    fn reduced(&self, by: &LocalName, reduction: &Reduction) {
        self.0
            .reduced
            .lock()
            .unwrap()
            .push((by.clone(), reduction.clone()));
    }
}

struct World {
    _dir: tempfile::TempDir,
    host: Arc<Host>,
    app: Arc<App>,
    runtime: tokio::runtime::Runtime,
}
impl World {
    fn new(names: &[&str], withhold: Vec<(LocalName, Grant)>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(App::default());
        *app.withhold.lock().unwrap() = withhold;
        let host = Host::open(
            dir.path(),
            Mode::Home {
                app: Some(Arc::new(AppRef(Arc::clone(&app)))),
            },
            Limits::default(),
        )
        .unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let plan = host
            .plan(names.iter().map(|n| Source::Folder(fixture(n))).collect())
            .unwrap();
        runtime
            .block_on(host.install(plan, Requester::App))
            .unwrap();
        app.installs.lock().unwrap().clear();
        Self {
            _dir: dir,
            host,
            app,
            runtime,
        }
    }
    fn standard() -> Self {
        Self::new(&["counter", "manager", "recs", "shelf"], vec![])
    }
    fn call(
        &self,
        who: &str,
        function: &str,
        args: Value,
    ) -> Result<Value, watane_host::CallError> {
        let host = Arc::clone(&self.host);
        let who = local(who);
        let function = function.to_owned();
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || host.call(&Caller::Page, &who, &function, args))
            .unwrap()
            .join()
            .unwrap()
    }
    fn manager(&self, function: &str, args: Value) -> Value {
        self.call("manager", function, args).unwrap()
    }
    fn code(&self, who: &str, function: &str) -> CallCode {
        self.call(who, function, json!({})).unwrap_err().code
    }
    fn recs_call_grant(&self) -> Value {
        self.manager("api.mods", json!({}))[2]["grants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["grant"].get("call").is_some())
            .unwrap()["grant"]
            .clone()
    }
    fn state(&self, id: &str) -> Value {
        self.manager("api.request", json!({ "id": id }))
    }
    fn wait_state(&self, id: &str, want: Value) {
        let end = Instant::now() + Duration::from_secs(5);
        while self.state(id) != want {
            assert!(Instant::now() < end, "request {id} never became {want}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn is_id(v: &Value) -> bool {
    let s = v.as_str().unwrap();
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn ac1_s40_mods_lists_every_mod_in_order_with_literal_json() {
    let w = World::standard();
    let mods = w.manager("api.mods", json!({}));
    let names: Vec<_> = mods
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["local"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["counter", "manager", "recs", "shelf"]);
    let recs = &mods[2];
    assert_eq!(recs["identity"], "local/recs");
    assert_eq!(recs["enabled"], true);
    assert_eq!(recs["has-page"], true);
    assert_eq!(recs["has-logic"], true);
    assert_eq!(
        recs["links"],
        json!([
            {"kind":"call","interface":"mods:shelf/items@0.1.0","provider":"shelf","provider-interface":"mods:shelf/items@0.1.0"},
            {"kind":"subscribe","interface":"mods:shelf/events@0.1.0","provider":"shelf","provider-interface":"mods:shelf/events@0.1.0"},
        ])
    );
    assert_eq!(
        recs["grants"],
        json!([
            {"grant":"storage","granted":true},
            {"grant":{"call":{"interface":"mods:shelf/items@0.1.0","provider":"local/shelf"}},"granted":true},
            {"grant":{"subscribe":{"interface":"mods:shelf/events@0.1.0","provider":"local/shelf"}},"granted":true},
        ])
    );
    assert_eq!(mods[1]["name"], "Desk");
    assert!(
        mods[1]["grants"]
            .as_array()
            .unwrap()
            .contains(&json!({"grant":"manage","granted":true}))
    );
}

#[test]
fn ac1_s40_without_the_grant_mods_fails_not_granted() {
    let w = World::new(
        &["manager", "counter"],
        vec![(local("manager"), Grant::Manage)],
    );
    assert_eq!(w.code("manager", "api.mods"), CallCode::NotGranted);
}

#[test]
fn ac2_s41_reductions_tell_the_app_then_act() {
    let w = World::standard();
    assert_eq!(
        w.manager("api.disable", json!({"local":"counter"})),
        json!({"ok":null})
    );
    assert_eq!(
        *w.app.reduced.lock().unwrap(),
        vec![(local("manager"), Reduction::Disable(local("counter")))]
    );
    assert_eq!(w.code("counter", "api.get"), CallCode::Disabled);

    let grant = w.recs_call_grant();
    assert_eq!(
        w.manager("api.withdraw", json!({"local":"recs","grant":grant})),
        json!({"ok":null})
    );
    assert_eq!(w.code("recs", "api.suggest"), CallCode::NotGranted);

    let before = fs::read(w.host.home().join("installed.json")).unwrap();
    assert_eq!(
        w.manager("api.close-page", json!({"local":"shelf"})),
        json!({"ok":null})
    );
    assert_eq!(
        w.app.reduced.lock().unwrap().last().unwrap().1,
        Reduction::ClosePage(local("shelf"))
    );
    assert_eq!(
        fs::read(w.host.home().join("installed.json")).unwrap(),
        before
    );

    let nope = w.manager("api.disable", json!({"local":"nope"}));
    assert_eq!(nope["err"]["code"], "no_mod");
    assert_eq!(w.app.reduced.lock().unwrap().len(), 3);
}

#[test]
fn ac2_s82_open_page_reaches_the_app_on_a_served_host_in_chain_order() {
    let w = World::standard();
    let served = w
        .runtime
        .block_on(w.host.serve(0, axum::Router::new()))
        .unwrap();
    let before = fs::read(w.host.home().join("installed.json")).unwrap();
    assert_eq!(
        w.manager("api.open-page", json!({"local":"shelf"})),
        json!({"ok":null})
    );
    assert_eq!(
        *w.app.reduced.lock().unwrap(),
        vec![(local("manager"), Reduction::OpenPage(local("shelf")))]
    );
    assert_eq!(
        fs::read(w.host.home().join("installed.json")).unwrap(),
        before
    );
    w.app.reduced.lock().unwrap().clear();
    assert_eq!(
        w.manager("api.close-open-close", json!({"local":"shelf"})),
        Value::Null
    );
    assert_eq!(
        *w.app.reduced.lock().unwrap(),
        vec![
            (local("manager"), Reduction::ClosePage(local("shelf"))),
            (local("manager"), Reduction::OpenPage(local("shelf"))),
            (local("manager"), Reduction::ClosePage(local("shelf"))),
        ]
    );
    w.runtime.block_on(served.shutdown());
}

#[test]
fn ac3_s82_open_page_refuses_invalid_targets_without_effects() {
    let w = World::new(&["manager", "counter", "hog"], vec![]);
    w.host.disable(&local("counter")).unwrap();
    for (name, code) in [
        ("nope", "no_mod"),
        ("counter", "bad_request"),
        ("hog", "bad_request"),
        ("Bad!", "bad_request"),
    ] {
        let result = w.manager("api.open-page", json!({"local":name}));
        assert_eq!(result["err"]["code"], code, "{name}: {result}");
        assert!(w.app.reduced.lock().unwrap().is_empty());
    }
}

#[test]
fn ac3_s82_open_page_requires_manage_and_discards_a_failed_call() {
    let w = World::standard();
    // Page presence comes from ui/, even before the host has a page URL.
    assert_eq!(
        w.manager("api.open-page", json!({"local":"shelf"})),
        json!({"ok":null})
    );
    assert_eq!(
        *w.app.reduced.lock().unwrap(),
        vec![(local("manager"), Reduction::OpenPage(local("shelf")))]
    );
    w.app.reduced.lock().unwrap().clear();
    assert_eq!(
        w.call(
            "manager",
            "api.open-page-then-fail",
            json!({"local":"shelf"})
        )
        .unwrap_err()
        .code,
        CallCode::Trap
    );
    assert!(w.app.reduced.lock().unwrap().is_empty());
    w.host.revoke(&local("manager"), &Grant::Manage).unwrap();
    assert_eq!(
        w.call("manager", "api.open-page", json!({"local":"shelf"}))
            .unwrap_err()
            .code,
        CallCode::NotGranted
    );
    assert!(w.app.reduced.lock().unwrap().is_empty());
}

#[test]
fn ac4_s82_harness_fakes_and_records_open_page() {
    let h = harness_fakes();
    assert_eq!(
        h.call("api.open-page", json!({"local":"uninstalled"}))
            .unwrap(),
        Outcome {
            value: json!({"ok":null}),
            events: vec![],
            manages: vec![ManageRecord {
                function: "open-page".into(),
                args: json!({"local":"uninstalled"})
            }],
        }
    );
}

#[test]
fn ac5_s82_manager_component_exports_open_page() {
    let w = World::new(&["manager"], vec![]);
    let function = w
        .host
        .functions(&local("manager"))
        .unwrap()
        .into_iter()
        .find(|f| f.name == "open-page")
        .expect("manager exports open-page");
    assert_eq!(function.params, vec![("local".into(), "string".into())]);
    assert_eq!(
        function.result.as_deref(),
        Some("result<_, record { code: string, message: string }>")
    );
}

#[test]
fn ac3_s42_a_request_is_pending_then_decided_by_the_app() {
    let w = World::standard();
    let grant = w.recs_call_grant();
    w.manager("api.withdraw", json!({"local":"recs","grant":grant}));
    let gate = w.app.gate();
    let result = w.manager("api.request-grant", json!({"local":"recs","grant":grant}));
    let id = result["ok"].as_str().unwrap().to_owned();
    assert!(is_id(&result["ok"]), "{result}");
    {
        let requests = w.app.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id.as_str(), id);
        assert_eq!(requests[0].by, local("manager"));
        assert!(
            matches!(&requests[0].ask, Ask::Grant { local: l, grant: Grant::Call { .. } } if l == &local("recs"))
        )
    };
    assert_eq!(w.state(&id), json!("pending"));
    gate.send(ManageOutcome::Approved).unwrap();
    w.wait_state(&id, json!("approved"));
    // The app acts, not the host: the grant is still withdrawn.
    assert_eq!(w.code("recs", "api.suggest"), CallCode::NotGranted);
    assert_eq!(
        w.manager("api.request-enable", json!({"local":"counter"}))["err"]["code"],
        "bad_request"
    );
    assert_eq!(w.state("0123456789abcdef0123456789abcdef"), Value::Null);
}

fn bump(dir: &Path, text: &str) {
    fs::write(dir.join("ui/app.js"), text).unwrap();
}
fn counter_copy() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    copy(&fixture("counter"), dir.path());
    dir
}

#[test]
fn ac4_s43_an_install_that_a_manager_asked_is_always_asked() {
    let w = World::standard();
    let copy = counter_copy();
    bump(copy.path(), "// S43 first change\n");
    let plan = w
        .host
        .plan(vec![Source::Folder(copy.path().to_owned())])
        .unwrap();
    assert!(!plan.needs_confirmation());
    w.runtime
        .block_on(w.host.install(plan, Requester::Mod(local("manager"))))
        .unwrap();
    assert_eq!(
        *w.app.installs.lock().unwrap(),
        vec![Requester::Mod(local("manager"))]
    );

    bump(copy.path(), "// S43 second change\n");
    let plan = w
        .host
        .plan(vec![Source::Folder(copy.path().to_owned())])
        .unwrap();
    w.runtime
        .block_on(w.host.install(plan, Requester::App))
        .unwrap();
    assert_eq!(w.app.installs.lock().unwrap().len(), 1);

    w.host.revoke(&local("manager"), &Grant::Manage).unwrap();
    bump(copy.path(), "// S43 third change\n");
    let plan = w
        .host
        .plan(vec![Source::Folder(copy.path().to_owned())])
        .unwrap();
    let error = w
        .runtime
        .block_on(w.host.install(plan, Requester::Mod(local("manager"))))
        .unwrap_err();
    assert_eq!(error.code(), "refused");
    assert_eq!(w.app.installs.lock().unwrap().len(), 1);
}

#[test]
fn ac5_s44_one_request_at_a_time() {
    let w = World::standard();
    let gate = w.app.gate();
    let first = w.manager(
        "api.request-remove",
        json!({"local":"shelf","delete-data":true}),
    );
    let id = first["ok"].as_str().unwrap().to_owned();
    assert!(matches!(
        &w.app.requests.lock().unwrap()[0].ask,
        Ask::Remove {
            delete_data: true,
            ..
        }
    ));
    for (function, args) in [
        ("api.request-update", json!({"local":"counter"})),
        (
            "api.request-remove",
            json!({"local":"counter","delete-data":false}),
        ),
    ] {
        let second = w.manager(function, args);
        assert_eq!(second["err"]["code"], "request_open");
        assert!(second["err"]["message"].as_str().unwrap().contains(&id));
    }
    assert_eq!(w.app.requests.lock().unwrap().len(), 1);
    gate.send(ManageOutcome::Failed("no disk".into())).unwrap();
    w.wait_state(&id, json!({"failed": "no disk"}));
    let third = w.manager("api.request-update", json!({"local":"counter"}));
    assert!(is_id(&third["ok"]), "{third}");
    assert!(matches!(
        &w.app.requests.lock().unwrap()[1].ask,
        Ask::Update(l) if l == &local("counter")
    ));
}

#[test]
fn ac6_s45_losing_the_grant_denies_a_pending_request_and_drops_its_future() {
    let w = World::standard();
    let id = w.manager("api.request-enable", json!({"local":"counter"}));
    assert_eq!(id["err"]["code"], "bad_request");
    w.manager("api.disable", json!({"local":"counter"}));
    let _gate = w.app.gate();
    let id = w.manager("api.request-enable", json!({"local":"counter"}))["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(w.app.dropped.load(Ordering::SeqCst), 0);
    w.host.revoke(&local("manager"), &Grant::Manage).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    while w.app.dropped.load(Ordering::SeqCst) == 0 {
        assert!(Instant::now() < end, "the future was not dropped");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(w.code("manager", "api.mods"), CallCode::NotGranted);
    w.host.grant(&local("manager"), &Grant::Manage).unwrap();
    assert_eq!(w.state(&id), json!("denied"));
}

#[test]
fn ac7_s46_a_failed_call_reduces_nothing() {
    let w = World::standard();
    assert_eq!(
        w.call(
            "manager",
            "api.disable-then-fail",
            json!({"local":"counter"})
        )
        .unwrap_err()
        .code,
        CallCode::Trap
    );
    assert!(w.host.get(&local("counter")).unwrap().enabled);
    assert!(w.app.reduced.lock().unwrap().is_empty());
    assert_eq!(
        w.manager("api.disable-then-read", json!({"local":"counter"})),
        json!(true)
    );
    assert!(!w.host.get(&local("counter")).unwrap().enabled);
    assert_eq!(w.app.reduced.lock().unwrap().len(), 1);
}

#[test]
fn ac8_s47_a_preview_refuses_requests_but_reduces() {
    let dir = tempfile::tempdir().unwrap();
    let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
    host.install_folders(&[fixture("manager"), fixture("counter")], 0)
        .unwrap();
    let call = |function: &'static str, args: Value| {
        let host = Arc::clone(&host);
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || host.call(&Caller::Page, &local("manager"), function, args))
            .unwrap()
            .join()
            .unwrap()
            .unwrap()
    };
    let refused = call("api.request-enable", json!({"local":"counter"}));
    assert_eq!(refused["err"]["code"], "refused");
    assert_eq!(
        call("api.disable", json!({"local":"counter"})),
        json!({"ok":null})
    );
    assert!(!host.get(&local("counter")).unwrap().enabled);
}

fn harness_fakes() -> Harness {
    Harness::open(&fixture("manager"), Fakes::new()).unwrap()
}
#[test]
fn ac8_s47_harness_fakes_and_records() {
    let h = harness_fakes();
    assert_eq!(
        h.call("api.disable", json!({"local":"x"})).unwrap(),
        Outcome {
            value: json!({"ok":null}),
            events: vec![],
            manages: vec![ManageRecord {
                function: "disable".into(),
                args: json!({"local":"x"}),
            }],
        }
    );
    let out = h
        .call("api.withdraw", json!({"local":"x","grant":"storage"}))
        .unwrap();
    assert_eq!(
        out.manages,
        vec![ManageRecord {
            function: "withdraw".into(),
            args: json!({"local":"x","grant":"storage"}),
        }]
    );
    let out = h.call("api.mods", json!({})).unwrap();
    assert_eq!((out.value, out.manages), (json!([]), vec![]));
    let one = h
        .call(
            "api.request-remove",
            json!({"local":"x","delete-data":true}),
        )
        .unwrap();
    assert_eq!(one.value, json!({"ok":"r1"}));
    assert_eq!(
        one.manages,
        vec![ManageRecord {
            function: "request-remove".into(),
            args: json!({"local":"x","delete-data":true}),
        }]
    );
    assert_eq!(
        h.call("api.request-install", json!({"release":"x","link":null}))
            .unwrap()
            .value,
        json!({"ok":"r2"})
    );
    // A failed call records nothing and takes no number.
    h.call("api.disable-then-fail", json!({"local":"x"}))
        .unwrap_err();
    assert_eq!(
        h.call("api.request-update", json!({"local":"x"}))
            .unwrap()
            .value,
        json!({"ok":"r3"})
    );
    assert_eq!(
        h.call("api.request", json!({"id":"r1"})).unwrap().value,
        json!("pending")
    );
}

#[test]
fn ac8_s47_harness_returns_the_fake_list() {
    let info = json!({
        "local":"a","identity":"local/a","slug":"a","name":"A","summary":"s","version":"0.1.0",
        "publisher":null,"release":null,"origin":null,"enabled":true,"has-page":false,
        "has-logic":true,"grants":[{"grant":"storage","granted":false}],"links":[],"binds":[],
        "error":null
    });
    let mut fakes = Fakes::new();
    fakes.insert(
        "watane:host/manage".into(),
        [("mods".to_owned(), json!([info]))].into(),
    );
    let h = Harness::open(&fixture("manager"), fakes).unwrap();
    assert_eq!(h.call("api.mods", json!({})).unwrap().value, json!([info]));
}

#[test]
fn ac8_s47_a_served_manager_hears_catalog_after_a_disable() {
    let w = World::standard();
    let served = w
        .runtime
        .block_on(w.host.serve(free_port(), axum::Router::new()))
        .unwrap();
    let port = served.port;
    let client = reqwest::Client::builder()
        .resolve("manager.localhost", ([127, 0, 0, 1], port).into())
        .build()
        .unwrap();
    let message = w.runtime.block_on(async {
        let mut response = client
            .get(format!("http://manager.localhost:{port}/events"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let host = Arc::clone(&w.host);
        let disabler = tokio::task::spawn_blocking(move || {
            std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn(move || {
                    host.call(
                        &Caller::Page,
                        &local("manager"),
                        "api.disable",
                        json!({"local":"counter"}),
                    )
                })
                .unwrap()
                .join()
                .unwrap()
        });
        let mut seen = String::new();
        let found = tokio::time::timeout(Duration::from_secs(5), async {
            while let Some(chunk) = response.chunk().await.unwrap() {
                seen.push_str(&String::from_utf8_lossy(&chunk));
                if seen.contains(r#"data: {"type":"catalog"}"#) {
                    return true;
                }
            }
            false
        })
        .await
        .unwrap_or(false);
        disabler.await.unwrap().unwrap();
        (found, seen)
    });
    w.runtime.block_on(served.shutdown());
    assert!(message.0, "no catalog message in {:?}", message.1);
}

#[test]
fn ac9_s5_3_a_component_that_imports_manage_without_the_world_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }
    let manager = dir.path().join("manager");
    copy(&fixture("manager"), &manager);
    fs::write(
        manager.join("wit/world.wit"),
        "package mods:manager@0.1.0;\ninterface api { who: func() -> string; }\nworld manager { import watane:host/caller@0.1.0; export api; }\n",
    )
    .unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Mode::Preview, Limits::default()).unwrap();
    let error = host.install_folders(&[manager], 0).unwrap_err();
    assert_eq!(error.code(), "invalid_mod");
    assert!(error.to_string().contains("watane:host/manage"), "{error}");
}

/// Withdraws the manager's `Manage` while the participant decides.
struct Revoker(std::sync::OnceLock<Arc<Host>>);
impl Confirm for Revoker {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        if let Requester::Mod(by) = &request.requester {
            self.0.get().unwrap().revoke(by, &Grant::Manage).unwrap();
        }
        Box::pin(async { Answer::Approve { withhold: vec![] } })
    }
}

#[test]
fn ac4_s43_a_manager_install_is_checked_again_before_the_commit() {
    let dir = tempfile::tempdir().unwrap();
    let app = Arc::new(Revoker(std::sync::OnceLock::new()));
    let host = Host::open(
        dir.path(),
        Mode::Home {
            app: Some(Arc::<Revoker>::clone(&app)),
        },
        Limits::default(),
    )
    .unwrap();
    app.0.set(Arc::clone(&host)).ok().unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let plan = host
        .plan(vec![
            Source::Folder(fixture("manager")),
            Source::Folder(fixture("counter")),
        ])
        .unwrap();
    runtime
        .block_on(host.install(plan, Requester::App))
        .unwrap();
    let copy = counter_copy();
    bump(copy.path(), "// S43 changed while asked\n");
    let plan = host
        .plan(vec![Source::Folder(copy.path().to_owned())])
        .unwrap();
    let error = runtime
        .block_on(host.install(plan, Requester::Mod(local("manager"))))
        .unwrap_err();
    assert_eq!(error.code(), "refused");
    let installed = fs::read(dir.path().join("mods/counter/ui/app.js")).unwrap();
    assert_eq!(
        installed,
        fs::read(fixture("counter").join("ui/app.js")).unwrap()
    );
}
