#![allow(clippy::unwrap_used, reason = "tests may panic")]

use super::driver::App;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
pub const TIMEOUT: Duration = Duration::from_secs(30);
pub fn until(app: &App, label: &str, js: &str) {
    let end = Instant::now() + TIMEOUT;
    loop {
        if app.is_true(label, js) {
            return;
        }
        assert!(
            Instant::now() < end,
            "DOM timed out: {label}: {js}; last result: {}",
            app.eval_raw(label, js)
        );
        std::thread::yield_now();
    }
}
pub fn area(app: &App, hash: &str) {
    let selector = match hash {
        "#mods" => "#library",
        "#settings" => "#settings",
        "#find" => "#find",
        "#updates" => "#updates",
        _ => panic!("unknown app area: {hash}"),
    };
    let parent = if hash == "#settings" {
        "#settings"
    } else {
        "#mods"
    };
    app.eval("main", &format!("location.hash = {}; null", json!(hash)));
    until(
        app,
        "main",
        &format!(
            "document.body.dataset.ready === 'true' && location.hash === {} && !document.querySelector({}).hidden && !document.querySelector({}).hidden",
            json!(hash),
            json!(selector),
            json!(parent)
        ),
    );
}
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
pub fn invoke(app: &App, cmd: &str, args: Value) -> Value {
    app.eval("main", &format!("window.__result = null; window.__TAURI_INTERNALS__.invoke({}, {}).then(value => window.__result = {{ok:value}}, error => window.__result = {{error}}); null", json!(cmd), args));
    until(app, "main", "window.__result !== null");
    let result = app.eval("main", "window.__result");
    assert!(result.get("error").is_none(), "{cmd}: {result}");
    result["ok"].clone()
}
pub fn begin_install(app: &App, paths: &[PathBuf]) {
    assert!(app.wait("main", TIMEOUT));
    until(app, "main", "document.body.dataset.ready === 'true'");
    app.pick(paths);
    app.eval("main", "document.querySelector('#install').click(); null");
    assert!(app.wait("consent", TIMEOUT));
    until(app, "consent", "document.body.dataset.ready === 'true'");
}
pub fn approve(app: &App) {
    // Field and integration tests run hidden. Front tests prove the focus gate.
    answer(
        app,
        "document.querySelector('#approve').disabled = false; document.querySelector('#approve').click();",
    );
}
pub fn deny(app: &App) {
    answer(app, "document.querySelector('#deny').click();");
}
const ANSWERED: &str = "#answered";
fn answer(app: &App, click: &str) {
    // Mark this window instance by its URL, before it is answered: a later
    // consent window has the same label but opens at the plain URL.
    app.eval(
        "consent",
        &format!("location.hash = {ANSWERED:?}; {click} null"),
    );
    consent_closed(app);
}
// The answered window closes asynchronously and keeps the label "consent"
// until Destroyed. Wait for that instance, or the next request's driver calls
// reach it. Not for "no window labelled consent": when the answer lets the
// next confirmation of the same flow be asked before the old window is
// destroyed, the successor is opened by the old window's Destroyed handler
// (views.rs `sync_consent`) under the same label, and a label poll waits on
// that one. Poll every 10 ms, not back to back: each `windows` call reads
// title, url, focus and visibility of every window (driver.rs `windows`), and
// those getters are main-thread round trips, the thread that must also close
// the window (#106: on the macOS runner the answered window stayed listed for
// 30 s; no Mac here, so a miss now prints the window list).
fn consent_closed(app: &App) {
    let end = Instant::now() + TIMEOUT;
    loop {
        let windows = app.windows();
        if !windows.iter().any(|w| {
            w["label"] == "consent" && w["url"].as_str().is_some_and(|url| url.ends_with(ANSWERED))
        }) {
            return;
        }
        assert!(
            Instant::now() < end,
            "the answered consent window was not destroyed in {TIMEOUT:?}; windows: {}",
            Value::Array(windows)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub fn installed(app: &App, local: &str) {
    until(
        app,
        "main",
        &format!("document.querySelector('section[data-local={local}]') !== null"),
    );
    assert!(app.home.join("mods").join(local).is_dir());
}
pub fn open(app: &App, local: &str) {
    app.eval(
        "main",
        &format!("document.querySelector('section[data-local={local}] button.open').click(); null"),
    );
    assert!(app.wait(&format!("mod-{local}"), TIMEOUT));
}
