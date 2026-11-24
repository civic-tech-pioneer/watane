#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! The workspace consent (TECHNICAL 19.3 Picked folders, 19.7; FUNCTIONAL S58, S61, S62, S66):
//! a home with an app whose fake `Confirm` holds each answer until the test gives it, the
//! `claude` fixture, and the installed mods `claude`, `claude-b` and `claude-evt`.

use crate::claude::{INIT, Rig};
use crate::streams::{call_as, fixture, local, wait};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use watane_host::{
    AccountError, AccountSource, Caller, Confirm, Consent, ConsentAnswer, ConsentRequest, Host,
    JudgeAccount, Limits, Mode, TestClock,
};

type Held = tokio::sync::oneshot::Sender<ConsentAnswer>;

/// The app: approves installs, names the fake `claude`, and holds each consent question.
#[derive(Default)]
struct App {
    program: Mutex<Option<PathBuf>>,
    asked: Mutex<Vec<ConsentRequest>>,
    held: Mutex<Vec<Option<Held>>>,
    dropped: AtomicUsize,
    /// `false`: the default `Confirm::consent` body.
    asks: bool,
}
struct Drop1(Arc<App>);
impl Drop for Drop1 {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
struct Plain(Arc<App>);
struct Accounts(PathBuf);
impl AccountSource for Accounts {
    fn openai_token(
        &self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<String, AccountError>> + Send>> {
        Box::pin(async { Err(AccountError::NoAccount) })
    }
    fn judge(&self, _: &str) -> Result<JudgeAccount, AccountError> {
        Err(AccountError::NoAccount)
    }
    fn claude_binary(&self) -> Result<PathBuf, AccountError> {
        Ok(self.0.clone())
    }
}
fn install(
    _: watane_host::InstallRequest,
) -> Pin<Box<dyn std::future::Future<Output = watane_host::Answer> + Send>> {
    Box::pin(async { watane_host::Answer::Approve { withhold: vec![] } })
}
fn accounts(app: &App) -> Option<Arc<dyn AccountSource>> {
    let program = app.program.lock().unwrap().clone()?;
    Some(Arc::new(Accounts(program)))
}
struct Asking(Arc<App>);
impl Confirm for Asking {
    fn install(
        &self,
        request: watane_host::InstallRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = watane_host::Answer> + Send>> {
        install(request)
    }
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
        accounts(&self.0)
    }
    fn consent(
        &self,
        request: ConsentRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = ConsentAnswer> + Send>> {
        let app = &self.0;
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.asked.lock().unwrap().push(request);
        app.held.lock().unwrap().push(Some(tx));
        let guard = Drop1(Arc::clone(app));
        Box::pin(async move {
            let _guard = guard;
            rx.await.unwrap_or(ConsentAnswer::Refused)
        })
    }
}
impl Confirm for Plain {
    fn install(
        &self,
        request: watane_host::InstallRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = watane_host::Answer> + Send>> {
        install(request)
    }
    fn accounts(&self) -> Option<Arc<dyn AccountSource>> {
        accounts(&self.0)
    }
}

struct Home {
    dir: TempDir,
    fake: TempDir,
    app: Arc<App>,
    host: Option<Arc<Host>>,
    _rt: tokio::runtime::Runtime,
}
impl Home {
    fn new(files: &[(&str, &[u8])]) -> Self {
        Self::with(files, true)
    }
    fn with(files: &[(&str, &[u8])], asks: bool) -> Self {
        let fake = tempfile::tempdir().unwrap();
        let program = fake.path().join("claude");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude/claude"),
            &program,
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(fake.path().join("init"), format!("{INIT}\n")).unwrap();
        for (name, body) in files {
            std::fs::write(fake.path().join(name), body).unwrap();
        }
        let app = Arc::new(App {
            asks,
            ..App::default()
        });
        *app.program.lock().unwrap() = Some(program);
        let mut home = Self {
            dir: tempfile::tempdir().unwrap(),
            fake,
            app,
            host: None,
            _rt: tokio::runtime::Runtime::new().unwrap(),
        };
        home.open();
        let names = ["claude", "claude-b", "claude-evt", "shelf"];
        let plan = home
            .host()
            .plan(
                names
                    .iter()
                    .map(|n| watane_host::Source::Folder(fixture(n)))
                    .collect(),
            )
            .unwrap();
        let host = Arc::clone(home.host());
        home._rt
            .block_on(host.install(plan, watane_host::Requester::App))
            .unwrap();
        home
    }
    fn open(&mut self) {
        let app: Arc<dyn Confirm> = if self.app.asks {
            Arc::new(Asking(Arc::clone(&self.app)))
        } else {
            Arc::new(Plain(Arc::clone(&self.app)))
        };
        self.host = Some(
            Host::open(
                self.dir.path(),
                Mode::Home { app: Some(app) },
                Limits::default(),
            )
            .unwrap(),
        );
    }
    fn host(&self) -> &Arc<Host> {
        self.host.as_ref().unwrap()
    }
    /// Closes the host and waits until the state database is free.
    fn close(&mut self) {
        self.host = None;
        let path = self.dir.path().join("state/state.redb");
        wait("the state database", Duration::from_secs(10), || {
            redb::Database::open(&path).is_ok()
        });
    }
    fn call(&self, caller: Caller, target: &str, function: &str, args: Value) -> Value {
        call_as(self.host(), caller, target, function, args).unwrap()
    }
    fn page(&self, target: &str, function: &str, args: Value) -> Value {
        self.call(Caller::Page, target, function, args)
    }
    /// A start through `target` as the page; the result text.
    fn start_in(&self, target: &str, launch: &Value) -> String {
        self.page(target, "api.start", json!({"launch": launch.to_string()}))
            .as_str()
            .unwrap()
            .to_owned()
    }
    fn start(&self, launch: &Value) -> String {
        let s = self.start_in("claude", launch);
        assert_eq!(s.len(), 32, "no stream id: {s}");
        s
    }
    fn items(&self, target: &str, s: &str) -> Vec<String> {
        serde_json::from_value(self.page(target, "api.items", json!({"s": s}))).unwrap()
    }
    fn wait_items(&self, s: &str, n: usize) -> Vec<String> {
        wait("items", Duration::from_secs(20), || {
            self.items("claude", s).len() >= n
        });
        self.items("claude", s)
    }
    fn ended(&self, target: &str) -> Vec<String> {
        serde_json::from_value(self.page(target, "api.ended", Value::Null)).unwrap()
    }
    fn wait_ended(&self, n: usize) -> Vec<String> {
        wait("ended", Duration::from_secs(20), || {
            self.ended("claude").len() >= n
        });
        self.ended("claude")
    }
    fn asked(&self) -> Vec<ConsentRequest> {
        self.app.asked.lock().unwrap().clone()
    }
    fn wait_asked(&self, n: usize) {
        wait("a question", Duration::from_secs(20), || {
            self.app.asked.lock().unwrap().len() >= n
        });
    }
    /// Gives the answer to the `i`th question.
    fn answer(&self, i: usize, answer: ConsentAnswer) {
        let tx = self.app.held.lock().unwrap()[i].take().unwrap();
        let _ = tx.send(answer);
    }
    fn log(&self) -> Vec<String> {
        std::fs::read_to_string(self.fake.path().join("claude.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
    fn entries(&self, kind: &str) -> Vec<String> {
        let prefix = format!("{kind} ");
        self.log()
            .iter()
            .filter_map(|l| l.strip_prefix(&prefix).map(str::to_owned))
            .collect()
    }
    fn installed(&self) -> Value {
        let text = std::fs::read_to_string(self.dir.path().join("installed.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }
    /// The `consents` of the installed mod `local`, from `installed.json`.
    fn consents_of(&self, local: &str) -> Value {
        let all = self.installed();
        let m = all["mods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == local)
            .unwrap();
        m.get("consents").cloned().unwrap_or_else(|| json!([]))
    }
}

/// A canonical empty folder.
fn folder() -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().display().to_string();
    (dir, path)
}
fn launch(workspace: &str) -> Value {
    json!({"version": 1, "workspace": workspace, "profile": "coder"})
}
fn what(via: &str, folder: &str) -> String {
    format!("workspace:{via}:{folder}")
}
fn pending(via: &str, folder: &str) -> String {
    json!({"type": "consent-pending", "what": what(via, folder)}).to_string()
}
fn alive(pid: &str) -> bool {
    let out = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .unwrap();
    out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().starts_with('Z')
}

// ---- AC2 -----------------------------------------------------------------------------------

#[test]
fn ac2_a_picked_folder_is_resolved_at_the_call_and_the_refusals_are_bad_launch() {
    let rig = Rig::new(&[]);
    let (_dir, path) = folder();
    let ok = |field: &str, value: &str| {
        let doc = if field == "workspace" {
            json!({"version": 1, "workspace": value})
        } else {
            json!({"version": 1, "workspace": "scratch", "extra-workspaces": [value]})
        };
        rig.call("api.start", json!({"launch": doc.to_string()}))
            .as_str()
            .unwrap()
            .to_owned()
    };
    let user = std::env::var("HOME").unwrap();
    let watane = rig.host().home().canonicalize().unwrap();
    let file = watane.join("installed.json");
    let tmp = tempfile::tempdir().unwrap();
    let nfd = tmp.path().canonicalize().unwrap().join("e\u{301}");
    std::fs::create_dir(&nfd).unwrap();
    let plain_file = tmp.path().join("file");
    std::fs::write(&plain_file, "x").unwrap();
    let mut refused: Vec<String> = vec![
        "relative/dir".into(),
        "./x".into(),
        "/no/such/folder/at/all".into(),
        plain_file.display().to_string(),
        "/".into(),
        user,
        watane.display().to_string(),
        watane.parent().unwrap().display().to_string(),
        watane.join("mods").display().to_string(),
        watane.join("scratch").display().to_string(),
        file.display().to_string(),
        nfd.display().to_string(),
    ];
    refused.dedup();
    for field in ["workspace", "extra-workspaces"] {
        for value in &refused {
            let r = ok(field, value);
            assert!(
                r.starts_with(&format!("bad-launch:{field}:")),
                "{field} {value}: {r}"
            );
            assert!(!r.contains("stage A3"), "{r}");
        }
        // A folder passes, and a link to one resolves to its target.
        let r = ok(field, &path);
        assert_eq!(r.len(), 32, "{field}: {r}");
        let link = tmp.path().join("link");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&path, &link).unwrap();
        let r = ok(field, &link.display().to_string());
        assert_eq!(r.len(), 32, "{field} link: {r}");
    }
}

// ---- AC3, AC4 ------------------------------------------------------------------------------

#[test]
fn ac3_a_new_folder_asks_once_holds_the_process_and_keeps_the_grant() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    assert_eq!(
        home.asked(),
        vec![ConsentRequest {
            origin: local("claude"),
            via: local("claude"),
            consent: Consent::Workspace {
                via: "local/claude".into(),
                folder: f.clone()
            },
            profile: Some("coder".into()),
            functions: vec![],
        }]
    );
    let items = home.wait_items(&s, 1);
    assert_eq!(items, [pending("local/claude", &f)]);
    std::thread::sleep(Duration::from_millis(400));
    assert!(home.log().is_empty(), "claude started before the answer");
    assert_eq!(home.items("claude", &s).len(), 1);
    home.answer(0, ConsentAnswer::Granted);
    let items = home.wait_items(&s, 3);
    assert_eq!(items[0], pending("local/claude", &f));
    assert_eq!(
        items[1],
        json!({"type": "initialized", "workspace": f}).to_string()
    );
    assert_eq!(
        items
            .iter()
            .filter(|i| i.contains("consent-pending"))
            .count(),
        1
    );
    assert_eq!(
        home.consents_of("claude"),
        json!([{"kind": "workspace", "via": "local/claude", "folder": f}])
    );
    assert_eq!(home.entries("CWD"), std::slice::from_ref(&f));
    // A second start in the same folder asks nothing and pushes no `consent-pending`.
    let s2 = home.start(&launch(&f));
    let items = home.wait_items(&s2, 2);
    assert!(items[0].contains("initialized"), "{items:?}");
    assert!(!items.iter().any(|i| i.contains("consent-pending")));
    assert_eq!(home.asked().len(), 1);
}

#[test]
fn ac4_refused_ends_consent_refused_stores_nothing_and_the_next_start_asks_again() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Refused);
    let w = what("local/claude", &f);
    assert_eq!(home.wait_ended(1), [format!("{s} consent-refused({w})")]);
    assert_eq!(home.consents_of("claude"), json!([]));
    assert!(home.log().is_empty());
    assert_eq!(home.items("claude", &s), [pending("local/claude", &f)]);
    home.start(&launch(&f));
    home.wait_asked(2);
}

/// A folder that became a link while the answer was held ends `gone`: no process starts in it.
#[test]
fn ac4_a_folder_swapped_for_a_link_while_the_answer_is_held_starts_nothing() {
    let home = Home::new(&[]);
    let (dir, f) = folder();
    let (_other, elsewhere) = folder();
    let picked = dir.path().canonicalize().unwrap().join("work");
    std::fs::create_dir(&picked).unwrap();
    let p = picked.display().to_string();
    let s = home.start(&launch(&p));
    home.wait_asked(1);
    std::fs::rename(&picked, Path::new(&f).join("moved")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &picked).unwrap();
    home.answer(0, ConsentAnswer::Granted);
    assert_eq!(home.wait_ended(1), [format!("{s} gone")]);
    assert!(
        home.log().is_empty(),
        "claude started in the swapped folder"
    );
}

#[test]
fn ac4_a_dropped_answer_ends_consent_refused() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    // The app drops its sender: no answer will come.
    drop(home.app.held.lock().unwrap()[0].take());
    let w = what("local/claude", &f);
    assert_eq!(home.wait_ended(1), [format!("{s} consent-refused({w})")]);
    assert!(home.log().is_empty());
}

#[test]
fn ac4_no_answer_in_600_s_on_the_test_clock_ends_consent_refused() {
    let home = Home::new(&[]);
    let clock = TestClock::new();
    home.host().set_test_clock(clock.clone());
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    clock.advance(Duration::from_secs(599));
    std::thread::sleep(Duration::from_millis(300));
    assert!(home.ended("claude").is_empty());
    clock.advance(Duration::from_secs(2));
    let w = what("local/claude", &f);
    assert_eq!(home.wait_ended(1), [format!("{s} consent-refused({w})")]);
    assert!(home.log().is_empty());
    wait("the drop", Duration::from_secs(5), || {
        home.app.dropped.load(Ordering::SeqCst) == 1
    });
}

#[test]
fn ac4_a_confirm_with_the_default_body_refuses_at_once() {
    let home = Home::with(&[], false);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    let w = what("local/claude", &f);
    assert_eq!(home.wait_ended(1), [format!("{s} consent-refused({w})")]);
    assert!(home.log().is_empty());
    assert_eq!(home.consents_of("claude"), json!([]));
}

#[test]
fn ac4_a_cancel_while_the_answer_is_held_drops_the_future_and_starts_nothing() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    home.page("claude", "api.cancel", json!({"s": s}));
    wait("the drop", Duration::from_secs(5), || {
        home.app.dropped.load(Ordering::SeqCst) == 1
    });
    assert_eq!(home.wait_ended(1), [format!("{s} cancelled")]);
    std::thread::sleep(Duration::from_millis(300));
    assert!(home.log().is_empty());
    assert_eq!(home.consents_of("claude"), json!([]));
}

// ---- AC5 -----------------------------------------------------------------------------------

#[test]
fn ac5_a_consent_given_through_one_provider_is_not_used_through_another() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let doc = launch(&f).to_string();
    // The origin client is claude-b; the provider is `claude`.
    let s = home.call(
        Caller::Mod(local("claude-b")),
        "claude",
        "api.start",
        json!({"launch": doc}),
    );
    assert_eq!(s.as_str().unwrap().len(), 32);
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Granted);
    home.wait_items(s.as_str().unwrap(), 2);
    // The same origin client through the provider `claude-b` asks again, with its own `via`.
    let s2 = home.start_in("claude-b", &launch(&f));
    assert_eq!(s2.len(), 32);
    home.wait_asked(2);
    let asked = home.asked();
    assert_eq!(asked[0].origin, local("claude-b"));
    assert_eq!(asked[0].via, local("claude"));
    assert_eq!(
        asked[1].consent,
        Consent::Workspace {
            via: "local/claude-b".into(),
            folder: f.clone()
        }
    );
    assert_eq!(asked[1].origin, local("claude-b"));
    assert_eq!(asked[1].via, local("claude-b"));
    // Another origin client through the same provider `claude` asks again: the consent is
    // claude-b's.
    let s3 = home.start(&launch(&f));
    home.wait_asked(3);
    assert_eq!(home.asked()[2].origin, local("claude"));
    assert_eq!(home.asked()[2].consent, asked[0].consent);
    assert_eq!(home.items("claude", &s3), [pending("local/claude", &f)]);
}

#[test]
fn ac5_an_event_subscriber_is_its_own_origin_client() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    // The page of `shelf` adds an item; `claude-evt` subscribes and starts claude from it.
    home.page(
        "shelf",
        "items.add",
        json!({"title": launch(&f).to_string()}),
    );
    home.wait_asked(1);
    let asked = home.asked();
    assert_eq!(asked[0].origin, local("claude-evt"));
    assert_eq!(asked[0].via, local("claude-evt"));
}

#[test]
fn ac5_a_timer_fires_with_its_own_mod_once_its_origin_client_is_removed() {
    let home = Home::new(&[]);
    let (_dir, f) = folder();
    let doc = launch(&f).to_string();
    let arm = |ms: u64| {
        home.call(
            Caller::Mod(local("claude-b")),
            "claude-evt",
            "api.arm",
            json!({"launch": doc, "ms": ms}),
        );
    };
    // The origin client `claude-b` is still there: it is asked, and its consent is kept.
    arm(100);
    home.wait_asked(1);
    assert_eq!(home.asked()[0].origin, local("claude-b"));
    home.answer(0, ConsentAnswer::Granted);
    wait("the consent", Duration::from_secs(10), || {
        home.consents_of("claude-b") != json!([])
    });
    // It is removed before the timer fires: the timer fires as `claude-evt`, which holds nothing.
    arm(1500);
    home.host().remove(&local("claude-b"), false).unwrap();
    home.wait_asked(2);
    assert_eq!(home.asked()[1].origin, local("claude-evt"));
    assert_eq!(
        home.asked()[1].consent,
        Consent::Workspace {
            via: "local/claude-evt".into(),
            folder: f
        }
    );
}

// ---- AC6 -----------------------------------------------------------------------------------

#[test]
fn ac6_withdrawal_ends_the_dependent_streams_at_once_and_only_those() {
    // A long turn: the fake runs a turn of 60 s after the first `say`.
    let home = Home::new(&[("rate", b"1 0 10 60")]);
    let (_a, fa) = folder();
    let (_b, fb) = folder();
    let grant = |n: usize| {
        home.wait_asked(n);
        home.answer(n - 1, ConsentAnswer::Granted);
    };
    // S1 works in A; S2 in B with A as an extra folder; S3 only in B.
    let s1 = home.start(&launch(&fa));
    grant(1);
    home.wait_items(&s1, 3);
    let mut two = launch(&fb);
    two["extra-workspaces"] = json!([fa]);
    let s2 = home.start(&two);
    grant(2);
    home.wait_items(&s2, 3);
    let s3 = home.start(&launch(&fb));
    home.wait_items(&s3, 2);
    assert_eq!(home.asked().len(), 2);
    for s in [&s1, &s2, &s3] {
        home.page("claude", "api.say", json!({"s": s, "text": "go"}));
    }
    wait("three runs", Duration::from_secs(10), || {
        home.entries("PID").len() == 3
    });
    let pids = home.entries("PID");
    let held = home.consents_of("claude");
    assert_eq!(held.as_array().unwrap().len(), 2);
    // Not held: `bad_request`.
    let missing = Consent::parse("workspace:local/claude:/no/such").unwrap();
    let e = home
        .host()
        .withdraw_consent(&local("claude"), &missing)
        .unwrap_err();
    assert_eq!(e.code(), "bad_request");
    let e = home
        .host()
        .withdraw_consent(&local("nobody"), &missing)
        .unwrap_err();
    assert_eq!(e.code(), "no_mod");
    let started = std::time::Instant::now();
    home.host()
        .withdraw_consent(
            &local("claude"),
            &Consent::parse(&what("local/claude", &fa)).unwrap(),
        )
        .unwrap();
    let ended = home.wait_ended(2);
    assert!(started.elapsed() < Duration::from_secs(1), "too slow");
    assert!(ended.contains(&format!("{s1} gone")), "{ended:?}");
    assert!(ended.contains(&format!("{s2} gone")), "{ended:?}");
    assert_eq!(
        home.consents_of("claude"),
        json!([{"kind": "workspace", "via": "local/claude", "folder": fb}])
    );
    wait("two groups gone", Duration::from_secs(5), || {
        pids.iter().filter(|p| alive(p)).count() == 1
    });
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(home.ended("claude").len(), 2, "the stream in B went on");
    // After the withdrawal a start in A asks again.
    home.start(&launch(&fa));
    home.wait_asked(3);
}

#[test]
fn ac6_disable_keeps_the_consents_and_remove_deletes_them() {
    let mut home = Home::new(&[]);
    let (_dir, f) = folder();
    let s = home.start(&launch(&f));
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Granted);
    home.wait_items(&s, 2);
    let expected = json!([{"kind": "workspace", "via": "local/claude", "folder": f}]);
    home.host().disable(&local("claude")).unwrap();
    assert_eq!(home.consents_of("claude"), expected);
    home.host().enable(&local("claude")).unwrap();
    assert_eq!(home.consents_of("claude"), expected);
    // An update keeps them: the fixture with another summary is another package.
    let copy = tempfile::tempdir().unwrap();
    let to = copy.path().join("claude");
    std::fs::create_dir(&to).unwrap();
    let from = fixture("claude");
    std::fs::copy(from.join("logic.wasm"), to.join("logic.wasm")).unwrap();
    let wit = std::process::Command::new("cp")
        .args([
            "-R",
            &from.join("wit").display().to_string(),
            &to.display().to_string(),
        ])
        .status()
        .unwrap();
    assert!(wit.success());
    std::fs::write(
        to.join("mod.toml"),
        "name = \"claude\"\nsummary = \"S103 update.\"\n",
    )
    .unwrap();
    let plan = home
        .host()
        .plan(vec![watane_host::Source::Folder(to)])
        .unwrap();
    assert!(
        matches!(plan.entries()[0].change, watane_host::Change::Update { .. }),
        "{:?}",
        plan.entries()[0].change
    );
    let host = Arc::clone(home.host());
    home._rt
        .block_on(host.install(plan, watane_host::Requester::App))
        .unwrap();
    drop(host);
    assert_eq!(
        home.host().get(&local("claude")).unwrap().summary,
        "S103 update."
    );
    assert_eq!(home.consents_of("claude"), expected);
    // It survives a restart.
    home.close();
    home.open();
    assert_eq!(home.consents_of("claude"), expected);
    home.host().remove(&local("claude"), false).unwrap();
    let all = home.installed();
    assert!(
        !all["mods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["local"] == "claude")
    );
}

// ---- AC7 -----------------------------------------------------------------------------------

fn deny_of(home: &Home) -> Vec<String> {
    let args = home.entries("ARG");
    let settings = args
        .iter()
        .find_map(|a| a.strip_prefix("--settings="))
        .unwrap();
    let v: Value = serde_json::from_str(settings).unwrap();
    v["permissions"]["deny"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn ac7_the_protected_paths_are_written_once_per_workspace_root() {
    let home = Home::new(&[]);
    let (_a, w) = folder();
    let (_b, x) = folder();
    let mut doc = launch(&w);
    doc["extra-workspaces"] = json!([x]);
    let s = home.start(&doc);
    home.wait_asked(1);
    home.answer(0, ConsentAnswer::Granted);
    home.wait_asked(2);
    home.answer(1, ConsentAnswer::Granted);
    home.wait_items(&s, 4);
    let deny = deny_of(&home);
    for tool in ["Edit", "Write", "NotebookEdit"] {
        for root in [&w, &x] {
            for rule in [
                format!("{tool}(/{root}/**/.git/**)"),
                format!("{tool}(/{root}/.git/**)"),
                format!("{tool}(/{root}/**/.claude/**)"),
                format!("{tool}(/{root}/.claude/**)"),
                format!("{tool}(/{root}/**/.agents/**)"),
                format!("{tool}(/{root}/**/.codex/**)"),
                format!("{tool}(/{root}/**/.husky/**)"),
                format!("{tool}(/{root}/**/CLAUDE.md)"),
                format!("{tool}(/{root}/Makefile)"),
                format!("{tool}(/{root}/package.json)"),
                format!("{tool}(/{root}/.mcp.json)"),
            ] {
                assert!(deny.contains(&rule), "{rule}");
            }
        }
    }
    assert_eq!(
        home.entries("ARG")
            .iter()
            .filter(|a| **a == format!("--add-dir={x}"))
            .count(),
        1
    );
}

#[test]
fn ac7_a_scratch_launch_holds_its_rules_for_its_own_root() {
    let home = Home::new(&[]);
    let s = home.start(&json!({"version": 1, "workspace": "scratch"}));
    let items = home.wait_items(&s, 2);
    let id = crate::claude::workspace_of(&items[0]);
    let root = home
        .host()
        .home()
        .join("scratch")
        .join(id.strip_prefix("scratch:").unwrap())
        .canonicalize()
        .unwrap();
    let deny = deny_of(&home);
    assert!(deny.contains(&format!("Edit(/{}/**/.git/**)", root.display())));
    assert!(deny.contains(&format!("Write(/{}/.claude/**)", root.display())));
}

// ---- AC8 -----------------------------------------------------------------------------------

const SESSION: &str = "7b1f1b7e-8c4a-4b9e-9d0a-2f5d6c1a3e4f";
const OTHER: &str = "11111111-2222-4333-8444-555555555555";

fn sessions_table(home: &Home) -> Vec<(String, String)> {
    let db = redb::Database::open(home.dir.path().join("state/state.redb")).unwrap();
    let read = redb::ReadableDatabase::begin_read(&db).unwrap();
    let Ok(table) = read.open_table(redb::TableDefinition::<&str, &str>::new("sessions")) else {
        return vec![];
    };
    redb::ReadableTable::iter(&table)
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (k.value().to_owned(), v.value().to_owned())
        })
        .collect()
}

#[test]
fn ac8_sessions_belong_to_the_origin_client_that_ran_them() {
    let init = INIT.replace(
        r#""subtype":"init""#,
        &format!(r#""subtype":"init","session_id":"{SESSION}""#),
    );
    let mut home = Home::new(&[("init", format!("{init}\n").as_bytes())]);
    let scratch = json!({"version": 1, "workspace": "scratch"});
    let s = home.start(&scratch);
    home.wait_items(&s, 2);
    // The owner's own resume passes; another origin client's resume, fork and new are refused.
    let with = |session: Value| {
        let mut doc = scratch.clone();
        doc["session"] = session;
        doc
    };
    let own = home.start_in("claude", &with(json!({"resume": SESSION})));
    assert_eq!(own.len(), 32, "{own}");
    for session in [
        json!({"resume": SESSION}),
        json!({"fork": SESSION}),
        json!({"new": SESSION}),
    ] {
        let r = home.start_in("claude-b", &with(session.clone()));
        assert!(r.starts_with("bad-launch:session:"), "{session}: {r}");
    }
    // An id nobody holds cannot be resumed; a new one can be named.
    let r = home.start_in("claude", &with(json!({"resume": OTHER})));
    assert!(r.starts_with("bad-launch:session:"), "{r}");
    let r = home.start_in("claude", &with(json!({"new": OTHER})));
    assert_eq!(r.len(), 32, "{r}");
    home.close();
    assert_eq!(
        sessions_table(&home),
        [(SESSION.to_owned(), "local/claude".to_owned())]
    );
    home.open();
    home.host().remove(&local("claude"), false).unwrap();
    home.close();
    assert!(sessions_table(&home).is_empty());
}
