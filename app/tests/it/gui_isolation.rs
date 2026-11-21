#![cfg(feature = "driver")]
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

use crate::support;

use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};
use support::driver::App;
use support::journey::{approve, begin_install, copy_dir, installed};
use support::pages::Listener;

use crate::support::GUI;
const TIMEOUT: Duration = Duration::from_secs(30);
const MOD: &str = "mod-hostile";

fn until(app: &App, label: &str, js: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if app.is_true(label, js) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition timed out: {label}: {js}; last result: {}",
            app.eval_raw(label, js)
        );
        std::thread::yield_now();
    }
}

fn install_hostile(app: &App, listener: &Listener) {
    let copied = tempfile::tempdir().unwrap();
    copy_dir(&fixtures().join("hostile"), copied.path());
    std::fs::write(
        copied.path().join("ui/config.js"),
        format!("window.fixtureListener = {};", listener.port),
    )
    .unwrap();
    begin_install(app, &[copied.path().to_path_buf()]);
    approve(app);
    installed(app, "hostile");
}
fn start(home: &Path, listener: &Listener) -> App {
    let app = App::start(home, &[]);
    install_hostile(&app, listener);
    app
}

fn open(app: &App) {
    app.eval(
        "main",
        "document.querySelector('[data-local=hostile] button.open').click(); null",
    );
    assert!(
        app.wait(MOD, TIMEOUT),
        "hostile page did not finish loading"
    );
}

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

#[test]
#[ignore = "front: takes the screen"]
fn ac1_consent_stays_above_hostile_and_takes_focus() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    // The driver has no field for all Spaces; this part is a source check.
    let views =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/views.rs")).unwrap();
    assert!(
        views
            .split("fn sync_consent")
            .nth(1)
            .unwrap()
            .contains(".visible_on_all_workspaces(true)")
    );
    let home = tempfile::tempdir().unwrap();
    let listener = Listener::start();
    let Some(app) = App::front(home.path(), &[]) else {
        return;
    };
    install_hostile(&app, &listener);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    open(&app);
    let ask = app.ask(json!({"kind":"install", "by_program":false, "mods":[]}));
    assert!(app.wait("consent", TIMEOUT));
    until(
        &app,
        "consent",
        "document.body.dataset.ready === 'true' && document.hasFocus()",
    );
    let windows = app.windows();
    let consent = windows.iter().find(|w| w["label"] == "consent").unwrap();
    assert_eq!(consent["alwaysOnTop"], true);
    assert_eq!(consent["visible"], true);
    assert_eq!(consent["focused"], true);
    assert_eq!(
        windows.iter().find(|w| w["label"] == MOD).unwrap()["alwaysOnTop"],
        false
    );
    assert!(ask.is_open());
    // Reopening an existing page must preserve the consent window's focus.
    support::journey::invoke(&app, "mod_open", json!({"local":"hostile"}));
    assert_eq!(
        app.windows()
            .iter()
            .find(|w| w["label"] == "consent")
            .unwrap()["focused"],
        consent["focused"]
    );
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(app.quit().success());
}

fn report(app: &App) -> Value {
    until(
        app,
        MOD,
        "window.__report?.done === true && window.__report?.navTried === true",
    );
    app.eval(MOD, "window.__report")
}

fn mods_entries(home: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &Path,
        path: &Path,
        tree: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, tree);
            } else {
                tree.insert(
                    path.strip_prefix(root).unwrap().into(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut tree = std::collections::BTreeMap::new();
    walk(&home.join("mods"), &home.join("mods"), &mut tree);
    tree
}

#[test]
fn ac2_hostile_attacks_are_refused_without_side_effects() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let listener = Listener::start();
    let app = start(home.path(), &listener);
    let before = mods_entries(home.path());
    open(&app);
    let report = report(&app);
    let attacks = report["attacks"].as_array().unwrap();
    assert_eq!(attacks.len(), 132, "incomplete hostile report: {report}");
    let names: std::collections::HashSet<_> = attacks
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), attacks.len(), "duplicate attack names");
    for attack in attacks {
        assert_eq!(attack["outcome"], "refused", "attack succeeded: {attack}");
    }
    let log = app.log();
    assert!(
        log["commands"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| !c["label"].as_str().unwrap().starts_with("mod-")),
        "mod reached a command: {log}"
    );
    let blocked = log["blocked"].as_array().unwrap();
    for kind in ["navigation", "new_window"] {
        assert!(
            blocked
                .iter()
                .any(|b| b["label"] == MOD && b["kind"] == kind),
            "missing {kind} refusal: {log}"
        );
    }
    let downloads: Vec<_> = blocked.iter().filter(|b| b["kind"] == "download").collect();
    if downloads.is_empty() {
        eprintln!("S6: WebKit did not reach the native download hook for blob/data attempts");
    } else {
        assert!(
            downloads.iter().any(|b| b["label"] == MOD),
            "missing hostile download refusal: {log}"
        );
        eprintln!("S6: WebKit reached the native download hook: {downloads:?}");
    }
    let windows = app.windows();
    let mut labels: Vec<_> = windows
        .iter()
        .map(|w| w["label"].as_str().unwrap())
        .collect();
    labels.sort();
    assert_eq!(labels, ["main", MOD]);
    let url = windows.iter().find(|w| w["label"] == MOD).unwrap()["url"]
        .as_str()
        .unwrap();
    assert_eq!(
        tauri::Url::parse(url)
            .unwrap()
            .origin()
            .ascii_serialization(),
        format!("http://hostile.localhost:{}", app.port)
    );
    assert_eq!(
        listener.count(),
        0,
        "hostile page connected to the listener"
    );
    assert_eq!(
        mods_entries(&app.home),
        before,
        "hostile page changed the mods tree"
    );
    assert!(app.quit().success());
}

#[test]
fn ac3_reopened_hostile_window_has_fresh_storage() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let listener = Listener::start();
    let app = start(home.path(), &listener);
    open(&app);
    assert_eq!(report(&app)["storageBefore"], Value::Null);
    assert_eq!(
        app.eval(MOD, "localStorage.getItem('hostile') !== null"),
        true,
        "fixture must write storage before closing"
    );
    app.close(MOD);
    assert!(!app.windows().iter().any(|w| w["label"] == MOD));
    open(&app);
    assert_eq!(report(&app)["storageBefore"], Value::Null);
    assert_eq!(
        app.eval(MOD, "localStorage.getItem('hostile') !== null"),
        true
    );
    assert!(app.quit().success());
}

#[test]
fn ac4_disabling_hostile_closes_window_and_disables_catalog_entry() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let listener = Listener::start();
    let app = start(home.path(), &listener);
    open(&app);
    report(&app);
    assert!(app.windows().iter().any(|w| w["label"] == MOD));
    app.eval(
        "main",
        "document.querySelector('[data-local=hostile] button.disable').click(); null",
    );
    until(
        &app,
        "main",
        "!!document.querySelector('[data-local=hostile] button.enable') && !document.querySelector('[data-local=hostile] button.disable')",
    );
    let deadline = Instant::now() + TIMEOUT;
    while app.windows().iter().any(|w| w["label"] == MOD) {
        assert!(Instant::now() < deadline, "disabled mod window stayed open");
        std::thread::yield_now();
    }
    // Native eval does not await promises; collect mods_list asynchronously and
    // poll its result through the trusted launcher's existing IPC interface.
    app.eval("main", "window.__TAURI_INTERNALS__.invoke('mods_list').then(mods => { window.__isolationMods = mods; }); null");
    until(&app, "main", "Array.isArray(window.__isolationMods)");
    let mods = app.eval("main", "window.__isolationMods");
    assert_eq!(
        mods.as_array()
            .unwrap()
            .iter()
            .find(|m| m["local"] == "hostile")
            .unwrap()["enabled"],
        false
    );
    assert!(app.quit().success());
}

#[test]
fn ac1_hidden_consent_retains_topmost_configuration() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let views =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/views.rs")).unwrap();
    assert!(
        views
            .split("fn sync_consent")
            .nth(1)
            .unwrap()
            .contains(".visible_on_all_workspaces(true)")
    );
    let home = tempfile::tempdir().unwrap();
    let listener = Listener::start();
    let app = start(home.path(), &listener);
    open(&app);
    let ask = app.ask(json!({"kind":"install", "by_program":false, "mods":[]}));
    assert!(app.wait("consent", TIMEOUT));
    until(&app, "consent", "document.body.dataset.ready === 'true'");
    let windows = app.windows();
    assert_eq!(
        windows.iter().find(|w| w["label"] == "consent").unwrap()["alwaysOnTop"],
        true
    );
    assert_eq!(
        windows.iter().find(|w| w["label"] == MOD).unwrap()["alwaysOnTop"],
        false
    );
    assert!(ask.is_open());
    app.close("consent");
    assert_eq!(ask.wait(TIMEOUT), Some(json!({"answer":"deny"})));
    assert!(app.quit().success());
}
