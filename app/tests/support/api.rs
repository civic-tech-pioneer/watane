use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
pub struct Recording {
    pub requests: Mutex<Vec<watane_dist::SignRequest>>,
    pub approve_list: bool,
}
impl Recording {
    pub fn new(approve_list: bool) -> Arc<Self> {
        Arc::new(Self {
            requests: Mutex::new(vec![]),
            approve_list,
        })
    }
}
impl watane_dist::Confirm for Recording {
    fn confirm(&self, request: &watane_dist::SignRequest) -> watane_dist::Decision {
        self.requests.lock().unwrap().push(request.clone());
        if request.statement.kind() == "list" && !self.approve_list {
            watane_dist::Decision::Refuse
        } else {
            watane_dist::Decision::Approve
        }
    }
}
pub fn open(
    home: &std::path::Path,
    confirm: Arc<Recording>,
    local: bool,
) -> Arc<watane_dist::Dist> {
    Arc::new(
        watane_dist::Dist::open(
            home,
            confirm,
            watane_dist::Options {
                listed_local: local,
                ..Default::default()
            },
        )
        .unwrap(),
    )
}
pub struct Site {
    pub base: String,
    pub hits: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}
impl Site {
    pub async fn serve(dir: std::path::PathBuf) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let hits = Arc::new(AtomicUsize::new(0));
        let count = hits.clone();
        let router = axum::Router::new()
            .fallback_service(tower_http::services::ServeDir::new(dir))
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    count.fetch_add(1, Ordering::SeqCst);
                    async move { next.run(request).await }
                },
            ));
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { base, hits, task }
    }
}
impl Drop for Site {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub fn consumer(key: &watane_dist::PublicKey) -> Vec<u8> {
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy(&entry.path(), &dest);
            } else {
                std::fs::copy(entry.path(), dest).unwrap();
            }
        }
    }
    let temp = tempfile::tempdir().unwrap();
    copy(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recommendations"),
        temp.path(),
    );
    std::fs::write(
        temp.path().join("deps.json"),
        serde_json::to_vec(&serde_json::json!({"mods:reading-list":key.to_string()})).unwrap(),
    )
    .unwrap();
    watane_host::watane_pkg::pack(temp.path(), false)
        .unwrap()
        .into_bytes()
}
