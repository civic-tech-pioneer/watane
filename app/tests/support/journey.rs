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
    app.answer(
        "consent",
        "document.querySelector('#approve').disabled = false; document.querySelector('#approve').click();",
    );
}
pub fn deny(app: &App) {
    app.answer("consent", "document.querySelector('#deny').click();");
}
// The answered window closes asynchronously and keeps the label "consent"
// until Destroyed, and a successor can open under that label as the old one
// goes (#106: `sync_consent`, views.rs:177, called from the old window's
// Destroyed handler, views.rs:217-232; on macOS the install consent comes
// before that Destroyed). So the driver's `answer` waits for the Destroyed of
// the instance it answered (app TECHNICAL 10), never for "no window labelled
// consent". It asks for no eval result: the window can go before one comes
// back, which a driver eval reports as "channel is empty and sending half is
// closed" (S167 r1: the URL-hash marker of round 0 delayed that result on
// macOS until the window had gone).
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
