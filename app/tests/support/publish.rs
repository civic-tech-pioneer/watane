#![allow(clippy::unwrap_used, reason = "tests may panic")]

use super::driver::App;
use super::journey::*;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct Approve;
impl watane_dist::Confirm for Approve {
    fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
        watane_dist::Decision::Approve
    }
}

pub fn dist(home: &std::path::Path) -> watane_dist::Dist {
    watane_dist::Dist::open(home, std::sync::Arc::new(Approve), Default::default()).unwrap()
}

pub fn choose(app: &App, link: &str) {
    until(app, "main", "document.body.dataset.ready === 'true'");
    app.eval("main", &format!("document.querySelector('#index-link').value = {}; document.querySelector('#add-index').click(); null", json!(link)));
    until(
        app,
        "main",
        &format!(
            "Array.from(document.querySelectorAll('#indexes [data-index]')).some(e => e.dataset.index === {})",
            json!(
                link.split('@')
                    .next()
                    .unwrap()
                    .trim_start_matches("watane:")
            )
        ),
    );
}

pub fn search_reading(app: &App) {
    app.eval("main", "document.querySelector('#search-text').value = 'reading'; document.querySelector('#search').click(); null");
    until(
        app,
        "main",
        "document.querySelectorAll('#results [data-release]').length === 1",
    );
}

pub fn refresh_find(app: &App) {
    // Callers wait for the stale/compromised DOM condition after this click.
    app.eval(
        "main",
        "document.querySelector('#refresh-channels').click(); null",
    );
}
pub fn http(port: u16, content: &str, bytes: Vec<u8>) -> (u16, Value) {
    let r = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .no_proxy()
        .build()
        .unwrap()
        .post(format!("http://localhost:{port}/api/publish"))
        .header("content-type", content)
        .body(bytes)
        .send()
        .unwrap();
    (r.status().as_u16(), r.json().unwrap())
}
pub fn publish(app: &App, bytes: Vec<u8>) -> std::thread::JoinHandle<(u16, Value)> {
    let port = app.port;
    std::thread::spawn(move || http(port, "application/octet-stream", bytes))
}
pub fn consent(app: &App) {
    let deadline = std::time::Instant::now() + TIMEOUT;
    while !app.wait("consent", std::time::Duration::ZERO) {
        // `begin` runs in main; with main closed no command of it can be pending.
        let command = if app.wait("main", std::time::Duration::ZERO) {
            app.eval("main", "window.__stageC ?? null")
        } else {
            Value::Null
        };
        if let Some(error) = command.get("error") {
            let text = error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| error.to_string());
            panic!("command failed before consent: {text}");
        }
        assert!(
            std::time::Instant::now() < deadline,
            "consent window timed out"
        );
        std::thread::yield_now();
    }
    until(app, "consent", "document.body.dataset.ready === 'true'");
}
pub fn begin(app: &App, cmd: &str, args: Value) {
    app.eval("main",&format!("window.__stageC = null; window.__TAURI_INTERNALS__.invoke({},{}).then(ok => window.__stageC = {{ok}}, error => window.__stageC = {{error}}); null",json!(cmd),args));
}
pub fn finish(app: &App) -> Value {
    until(app, "main", "window.__stageC !== null");
    app.eval("main", "window.__stageC")
}
pub struct Site {
    pub base: String,
    requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    gate: tokio::sync::watch::Sender<bool>,
    held_requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    pub fn requests(&self) -> usize {
        self.requests.load(std::sync::atomic::Ordering::SeqCst)
    }
    /// Hold real HTTP responses until release, returning the current held count.
    pub fn hold(&self) -> usize {
        let before = self.held_requests.load(std::sync::atomic::Ordering::SeqCst);
        self.gate.send_replace(true);
        before
    }
    pub fn wait_held(&self, before: usize) {
        let deadline = std::time::Instant::now() + TIMEOUT;
        while self.held_requests.load(std::sync::atomic::Ordering::SeqCst) <= before {
            assert!(
                std::time::Instant::now() < deadline,
                "no HTTP read reached the held site"
            );
            std::thread::yield_now();
        }
    }
    pub fn release(&self) {
        self.gate.send_replace(false);
    }
    pub fn serve(dir: std::path::PathBuf) -> Self {
        let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&requests);
        let (gate, _) = tokio::sync::watch::channel(false);
        let request_gate = gate.clone();
        let held_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let waiting = Arc::clone(&held_requests);
        let (stop, rx) = tokio::sync::oneshot::channel();
        let (tx, ready) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    tx.send(format!("http://{}/", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(
                        listener,
                        axum::Router::new()
                            .fallback_service(tower_http::services::ServeDir::new(dir))
                            .layer(axum::middleware::from_fn(move |request: axum::extract::Request, next: axum::middleware::Next| {
                                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                let mut gate = request_gate.subscribe();
                                let waiting = Arc::clone(&waiting);
                                async move {
                                    if *gate.borrow() {
                                        waiting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    loop {
                                        let held = *gate.borrow_and_update();
                                        if !held || gate.changed().await.is_err() { break; }
                                    }
                                    next.run(request).await
                                }
                            })),
                    )
                    .with_graceful_shutdown(async move {
                        let _ = rx.await;
                    })
                    .await
                    .unwrap();
                });
        });
        Self {
            base: ready.recv_timeout(TIMEOUT).unwrap(),
            requests,
            gate,
            held_requests,
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        // A failed assertion must not leave graceful shutdown waiting on a hold.
        self.release();
        let _ = self.stop.take().unwrap().send(());
        self.thread.take().unwrap().join().unwrap();
    }
}

// Reuse the committed component. Equal-length interface version edits only
// rename its existing imports/exports; neither its code nor its state changes.
pub fn update_package(version: &str, network: bool) -> Vec<u8> {
    let temp = tempfile::tempdir().unwrap();
    copy_dir(&fixture("reading-list"), temp.path());
    let world = temp.path().join("wit/world.wit");
    let mut wit = std::fs::read_to_string(&world).unwrap().replace(
        "mods:reading-list@0.1.0",
        &format!("mods:reading-list@{version}"),
    );
    let wasm = temp.path().join("logic.wasm");
    let mut bytes = std::fs::read(&wasm).unwrap();
    for interface in ["books", "events"] {
        let old = format!("mods:reading-list/{interface}@0.1.0");
        let new = format!("mods:reading-list/{interface}@{version}");
        assert_eq!(old.len(), new.len());
        let positions: Vec<_> = bytes
            .windows(old.len())
            .enumerate()
            .filter_map(|(i, b)| (b == old.as_bytes()).then_some(i))
            .collect();
        for i in positions {
            bytes[i..i + old.len()].copy_from_slice(new.as_bytes());
        }
    }
    std::fs::write(wasm, bytes).unwrap();
    if network {
        let dep = temp.path().join("wit/deps/http");
        std::fs::create_dir_all(&dep).unwrap();
        std::fs::write(
            dep.join("http.wit"),
            "package wasi:http@0.2.0; interface outgoing-handler {} ",
        )
        .unwrap();
        wit = wit.replace(
            "world reading-list {",
            "world reading-list { import wasi:http/outgoing-handler@0.2.0;",
        );
        let meta = temp.path().join("mod.toml");
        let mut text = std::fs::read_to_string(&meta).unwrap();
        text.push_str("\n[http]\nhosts = [\"example.org\"]\n");
        std::fs::write(meta, text).unwrap();
    }
    std::fs::write(world, wit).unwrap();
    watane_host::watane_pkg::pack(temp.path(), false)
        .unwrap()
        .as_bytes()
        .to_vec()
}
