use super::{driver::App, journey::*};
use serde_json::{Value, json};
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
    assert!(app.wait("consent", TIMEOUT));
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
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Site {
    pub fn serve(dir: std::path::PathBuf) -> Self {
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
                            .fallback_service(tower_http::services::ServeDir::new(dir)),
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
            stop: Some(stop),
            thread: Some(thread),
        }
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        let _ = self.stop.take().unwrap().send(());
        self.thread.take().unwrap().join().unwrap();
    }
}
