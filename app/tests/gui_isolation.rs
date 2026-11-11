#![cfg(feature = "driver")]
mod support;

use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use support::{
    driver::App,
    pages::{Listener, Pages},
};

static GUI: std::sync::Mutex<()> = std::sync::Mutex::new(());
const TIMEOUT: Duration = Duration::from_secs(30);
const MOD: &str = "mod-hostile";

fn until(app: &App, label: &str, js: &str) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let value = app.eval(label, js);
        if value == json!(true) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition timed out: {js}: {value}"
        );
        std::thread::yield_now();
    }
}

fn start(home: &Path, pages: &Pages, listener: &Listener) -> App {
    let page = format!(
        "{}?listener={}&api={}",
        pages.url("hostile"),
        listener.port,
        pages.port
    );
    let fake = json!({"mods":[{
        "local":"hostile", "name":"Hostile", "slug":"hostile", "version":"0.1.0",
        "release":null, "publisher":null, "enabled":true, "page":page,
        "package":"hostile-fixture", "derivedFrom":null, "error":null, "grants":[]
    }]})
    .to_string();
    let app = App::start(home, &[("WATANE_APP_FAKE", &fake)]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    open(&app);
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
    let pages = Pages::start(fixtures());
    let listener = Listener::start();
    let app = start(home.path(), &pages, &listener);
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

// Since the baseline has no entries, any added descendant must also add an
// immediate child. Comparing these lists proves the whole tree stayed empty.
fn mods_entries(home: &Path) -> Option<Vec<std::ffi::OsString>> {
    match std::fs::read_dir(home.join("mods")) {
        Ok(entries) => {
            let mut entries: Vec<_> = entries.map(|e| e.unwrap().file_name()).collect();
            entries.sort();
            Some(entries)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("read mods tree: {e}"),
    }
}

#[test]
fn ac2_hostile_attacks_are_refused_without_side_effects() {
    let _gui = GUI.lock().unwrap_or_else(|e| e.into_inner());
    let home = tempfile::tempdir().unwrap();
    let before = mods_entries(home.path());
    assert!(before.as_ref().is_none_or(Vec::is_empty));
    let pages = Pages::start(fixtures());
    let listener = Listener::start();
    let app = start(home.path(), &pages, &listener);
    let report = report(&app);
    let attacks = report["attacks"].as_array().unwrap();
    assert_eq!(attacks.len(), 109, "incomplete hostile report: {report}");
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
        format!("http://hostile.localhost:{}", pages.port)
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
    let pages = Pages::start(fixtures());
    let listener = Listener::start();
    let app = start(home.path(), &pages, &listener);
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
    let pages = Pages::start(fixtures());
    let listener = Listener::start();
    let app = start(home.path(), &pages, &listener);
    report(&app);
    assert!(app.windows().iter().any(|w| w["label"] == MOD));
    app.eval(
        "main",
        "document.querySelector('[data-local=hostile] button.disable').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('[data-local=hostile] button.disable')?.disabled === true",
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
