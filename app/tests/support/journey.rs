use super::driver::App;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub const TIMEOUT: Duration = Duration::from_secs(30);
pub fn until(app: &App, label: &str, js: &str) {
    let end = Instant::now() + TIMEOUT;
    loop {
        if app.eval(label, js) == true {
            return;
        }
        assert!(Instant::now() < end, "DOM timed out: {label}: {js}");
        std::thread::yield_now();
    }
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
    app.eval("consent", "document.querySelector('#approve').disabled = false; document.querySelector('#approve').click(); null");
    consent_closed(app);
}
pub fn deny(app: &App) {
    app.eval("consent", "document.querySelector('#deny').click(); null");
    consent_closed(app);
}
// The answered window closes asynchronously and keeps the label "consent"
// until Destroyed. Wait for it, or the next request's driver calls reach it.
fn consent_closed(app: &App) {
    let end = Instant::now() + TIMEOUT;
    while app.windows().iter().any(|w| w["label"] == "consent") {
        assert!(Instant::now() < end);
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
