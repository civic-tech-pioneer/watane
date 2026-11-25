//! S118 (TECHNICAL 4.2 step 6): `kept_state` on a plan entry after a remove that kept the data.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::json;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use watane_host::{
    Answer, Caller, Change, Confirm, Host, InstallRequest, Limits, LocalName, ManageOutcome,
    ManageRequest, Mode, PlanEntry, Requester, Source,
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
struct App {
    requests: Mutex<Vec<Vec<PlanEntry>>>,
}
struct AppRef(Arc<App>);
impl Confirm for AppRef {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        self.0.requests.lock().unwrap().push(request.entries);
        Box::pin(async { Answer::Approve { withhold: vec![] } })
    }
    fn manage(
        &self,
        _request: ManageRequest,
    ) -> Pin<Box<dyn Future<Output = ManageOutcome> + Send>> {
        Box::pin(async { ManageOutcome::Denied })
    }
}

struct World {
    _dir: tempfile::TempDir,
    host: Arc<Host>,
    app: Arc<App>,
    runtime: tokio::runtime::Runtime,
}
impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = Arc::new(App::default());
        let host = Host::open(
            dir.path(),
            Mode::Home {
                app: Some(Arc::new(AppRef(Arc::clone(&app)))),
            },
            Limits::default(),
        )
        .unwrap();
        Self {
            _dir: dir,
            host,
            app,
            runtime: tokio::runtime::Runtime::new().unwrap(),
        }
    }
    fn plan(&self, name: &str) -> watane_host::Plan {
        self.host.plan(vec![Source::Folder(fixture(name))]).unwrap()
    }
    fn install(&self, name: &str) {
        let plan = self.plan(name);
        self.runtime
            .block_on(self.host.install(plan, Requester::App))
            .unwrap();
    }
    fn call(&self, function: &str) -> Result<serde_json::Value, watane_host::CallError> {
        let host = Arc::clone(&self.host);
        let function = function.to_owned();
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || host.call(&Caller::Page, &local("counter"), &function, json!(null)))
            .unwrap()
            .join()
            .unwrap()
    }
    fn kept(&self, name: &str) -> bool {
        let plan = self.plan(name);
        assert_eq!(plan.entries().len(), 1);
        plan.entries()[0].kept_state
    }
}

/// Installs the counter and writes one count.
fn written() -> World {
    let w = World::new();
    w.install("counter");
    w.call("api.add").unwrap();
    w
}

#[test]
fn ac1_a_reinstall_after_keep_data_is_new_with_kept_state() {
    let w = written();
    w.host.remove(&local("counter"), false).unwrap();
    let plan = w.plan("counter");
    let entry = &plan.entries()[0];
    assert!(matches!(entry.change, Change::New));
    assert!(entry.kept_state);
    // Grants are untouched: Storage is still new.
    assert!(
        entry
            .new_grants
            .iter()
            .any(|g| matches!(g, watane_host::watane_pkg::Grant::Storage))
    );
    let text = serde_json::to_string(&plan).unwrap();
    assert!(text.contains("\"kept_state\":true"), "{text}");
}

#[test]
fn ac2_a_reinstall_after_delete_data_has_no_kept_state() {
    let w = written();
    w.host.remove(&local("counter"), true).unwrap();
    let plan = w.plan("counter");
    assert!(matches!(plan.entries()[0].change, Change::New));
    assert!(!plan.entries()[0].kept_state);
    assert!(
        serde_json::to_string(&plan)
            .unwrap()
            .contains("\"kept_state\":false")
    );
}

#[test]
fn ac3_new_update_and_unchanged_entries_have_no_kept_state() {
    let w = World::new();
    assert!(!w.kept("counter"));
    w.install("counter");
    w.call("api.add").unwrap();
    // Unchanged: the same package, the state is there.
    let plan = w.plan("counter");
    assert!(matches!(plan.entries()[0].change, Change::Unchanged));
    assert!(!plan.entries()[0].kept_state);
    // Update: a changed copy.
    let copy = tempfile::tempdir().unwrap();
    copy_dir(&fixture("counter"), copy.path());
    std::fs::write(copy.path().join("ui/app.js"), "// S118 change\n").unwrap();
    let plan = w
        .host
        .plan(vec![Source::Folder(copy.path().to_owned())])
        .unwrap();
    assert!(matches!(plan.entries()[0].change, Change::Update { .. }));
    assert!(!plan.entries()[0].kept_state);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}

#[test]
fn ac4_the_reinstall_commits_reads_old_state_and_the_app_is_told() {
    let w = written();
    w.host.remove(&local("counter"), false).unwrap();
    w.app.requests.lock().unwrap().clear();
    w.install("counter");
    let requests = w.app.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].len(), 1);
    assert!(requests[0][0].kept_state);
    assert_eq!(w.call("api.get").unwrap(), json!(1));
}

/// AC1 (F1.2): a plan made after the removal's catalog commit never says `kept_state` for data
/// that the same removal deletes. Without a seam: a planner thread plans in a tight loop while
/// `remove(local, true)` runs. A plan with `kept_state == true` has no mod of the name, so it was
/// made after the commit and `install` would not refuse it as `stale_plan`.
#[test]
fn ac1_a_plan_during_a_delete_data_removal_never_keeps_state_that_goes() {
    for round in 0..6 {
        let w = written();
        let host = Arc::clone(&w.host);
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let planned = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let planner = {
            let (done, planned) = (Arc::clone(&done), Arc::clone(&planned));
            std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn(move || {
                    let mut kept = vec![];
                    while !done.load(std::sync::atomic::Ordering::SeqCst) {
                        let plan = host.plan(vec![Source::Folder(fixture("counter"))]).unwrap();
                        planned.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if plan.entries()[0].kept_state {
                            kept.push(plan);
                        }
                    }
                    kept
                })
                .unwrap()
        };
        while planned.load(std::sync::atomic::Ordering::SeqCst) < 2 {
            std::thread::yield_now();
        }
        w.host.remove(&local("counter"), true).unwrap();
        done.store(true, std::sync::atomic::Ordering::SeqCst);
        let kept = planner.join().unwrap();
        for plan in kept {
            // Made after the commit: installing it is not refused as stale.
            let result = w.runtime.block_on(w.host.install(plan, Requester::App));
            assert!(
                matches!(result, Err(watane_host::HostError::StalePlan)),
                "round {round}: a live plan claims kept_state for deleted data"
            );
        }
    }
}

/// AC3 (P1.1): a table with no record is no kept data.
#[test]
fn ac3_a_table_emptied_before_a_keep_data_removal_is_not_kept_state() {
    let w = World::new();
    w.install("echo");
    let call = |function: &str, args: serde_json::Value| {
        let host = Arc::clone(&w.host);
        let function = function.to_owned();
        std::thread::Builder::new()
            .stack_size(16 << 20)
            .spawn(move || host.call(&Caller::Page, &local("echo"), &function, args))
            .unwrap()
            .join()
            .unwrap()
            .unwrap()
    };
    call("api.put", json!(["k", [1]]));
    call("api.delete", json!(["k"]));
    w.host.remove(&local("echo"), false).unwrap();
    assert!(!w.kept("echo"));
}
