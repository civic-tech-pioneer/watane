use std::{collections::VecDeque, sync::Mutex};
use watane_dist::{Confirm, Decision, SignRequest};
#[derive(Default)]
pub struct RecordingConfirm {
    pub decisions: Mutex<VecDeque<Decision>>,
    pub requests: Mutex<Vec<SignRequest>>,
}
impl RecordingConfirm {
    pub fn new(decisions: impl IntoIterator<Item = Decision>) -> Self {
        Self {
            decisions: Mutex::new(decisions.into_iter().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}
impl Confirm for RecordingConfirm {
    fn confirm(&self, request: &SignRequest) -> Decision {
        self.requests.lock().unwrap().push(request.clone());
        self.decisions
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected confirmation")
    }
}

#[allow(dead_code)]
pub fn fixture(version: &str, source: bool) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("reading-list", tmp.path());
    std::fs::write(
        tmp.path().join("wit/world.wit"),
        format!("package mods:reading-list@{version};\nworld reading-list {{}}\n"),
    )
    .unwrap();
    watane_pkg::pack(tmp.path(), source).unwrap().into_bytes()
}
#[allow(dead_code)]
pub fn copy_fixture(name: &str, dest: &std::path::Path) {
    fn copy(src: &std::path::Path, dst: &std::path::Path) {
        std::fs::create_dir_all(dst).unwrap();
        for e in std::fs::read_dir(src).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                copy(&e.path(), &dst.join(e.file_name()));
            } else {
                std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
            }
        }
    }
    copy(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
        dest,
    );
}

#[allow(dead_code)]
pub fn fast_options() -> watane_dist::Options {
    watane_dist::Options {
        entry_timeout: std::time::Duration::from_millis(300),
        package_timeout: std::time::Duration::from_secs(1),
        max_package: 4096,
        ..Default::default()
    }
}
#[allow(dead_code)]
pub fn open(home: &std::path::Path) -> watane_dist::Dist {
    watane_dist::Dist::open(
        home,
        std::sync::Arc::new(RecordingConfirm::new(std::iter::repeat_n(
            Decision::Approve,
            20,
        ))),
        fast_options(),
    )
    .unwrap()
}
#[allow(dead_code)]
#[derive(Clone)]
pub enum Mode {
    Files,
    Stall(std::time::Duration),
    Oversize(usize),
}
#[allow(dead_code)]
pub struct TestHost {
    pub url: String,
    pub mode: std::sync::Arc<Mutex<Mode>>,
    pub requests: std::sync::Arc<Mutex<Vec<String>>>,
    pub request_times: std::sync::Arc<Mutex<Vec<std::time::Instant>>>,
    server: std::sync::Arc<tiny_http::Server>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[allow(dead_code)]
impl TestHost {
    pub fn serve(dir: &std::path::Path) -> Self {
        let server = std::sync::Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}/", server.server_addr());
        let mode = std::sync::Arc::new(Mutex::new(Mode::Files));
        let requests = std::sync::Arc::new(Mutex::new(Vec::new()));
        let request_times = std::sync::Arc::new(Mutex::new(Vec::new()));
        let times = request_times.clone();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (s, m, r, quit) = (server.clone(), mode.clone(), requests.clone(), stop.clone());
        let dir = dir.to_owned();
        let worker = std::thread::spawn(move || {
            while !quit.load(std::sync::atomic::Ordering::SeqCst) {
                let Ok(Some(req)) = s.recv_timeout(std::time::Duration::from_millis(50)) else {
                    continue;
                };
                r.lock().unwrap().push(req.url().into());
                times.lock().unwrap().push(std::time::Instant::now());
                let mode = m.lock().unwrap().clone();
                match mode {
                    Mode::Files => {
                        let path = req.url().trim_start_matches('/');
                        // Only the paths dist constructs are served by this test fixture.
                        assert!(!path.contains(".."));
                        match std::fs::read(dir.join(path)) {
                            Ok(bytes) => {
                                let _ = req.respond(tiny_http::Response::from_data(bytes));
                            }
                            Err(_) => {
                                let _ = req.respond(tiny_http::Response::empty(404));
                            }
                        }
                    }
                    Mode::Oversize(n) => {
                        let _ = req.respond(tiny_http::Response::from_data(vec![0; n]));
                    }
                    Mode::Stall(duration) => {
                        std::thread::sleep(duration);
                        drop(req);
                    }
                }
            }
        });
        Self {
            url,
            mode,
            requests,
            request_times,
            server,
            stop,
            worker: Some(worker),
        }
    }
    pub fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
impl Drop for TestHost {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.server.unblock();
        self.worker.take().unwrap().join().unwrap();
    }
}

/// A separate home and static host for search journeys.
#[allow(dead_code)]
pub struct SearchHome {
    pub dist: watane_dist::Dist,
    pub host: Option<TestHost>,
    pub dir: tempfile::TempDir,
    pub url: String,
}
#[allow(dead_code)]
impl SearchHome {
    pub fn new(name: &str) -> Self {
        let mut home = Self::offline(name);
        let host = TestHost::serve(&home.dist.site_dir());
        home.url = host.url.clone();
        home.host = Some(host);
        home
    }
    pub fn offline(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let dist = watane_dist::Dist::open(
            dir.path(),
            std::sync::Arc::new(RecordingConfirm::new(std::iter::repeat_n(
                Decision::Approve,
                300,
            ))),
            watane_dist::Options {
                listed_local: true,
                entry_timeout: std::time::Duration::from_secs(3),
                ..fast_options()
            },
        )
        .unwrap();
        dist.set_name(name).unwrap();
        Self {
            dist,
            host: None,
            dir,
            url: String::new(),
        }
    }
    pub fn link(&self, slug: Option<&str>) -> watane_dist::Link {
        watane_dist::Link::new(
            self.dist.identity(),
            &self.url,
            slug.map(|s| s.parse().unwrap()).as_ref(),
        )
        .unwrap()
    }
}

#[allow(dead_code)]
pub fn named_fixture(slug: &str, name: &str) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("reading-list", tmp.path());
    std::fs::write(
        tmp.path().join("wit/world.wit"),
        format!("package mods:{slug}@0.1.0; world app {{}}"),
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("mod.toml"),
        format!("name = {name:?}\nsummary = \"Other mod.\"\n"),
    )
    .unwrap();
    watane_pkg::pack(tmp.path(), false).unwrap().into_bytes()
}

#[allow(dead_code)]
pub fn interface_fixture(kind: &str, publisher: watane_dist::PublicKey) -> Vec<u8> {
    let (slug, items) = match kind {
        "offer" => ("reading-list", "export books;"),
        "call" => (
            "caller",
            "import mods:reading-list/books@0.1.0; export api;",
        ),
        "subscribe" => ("subscriber", "export mods:reading-list/books@0.1.0;"),
        _ => panic!("unknown variant"),
    };
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("reading-list", tmp.path());
    std::fs::write(tmp.path().join("wit/world.wit"), format!("package mods:{slug}@0.1.0;\ninterface books {{ count: func() -> u32; }}\ninterface api {{ ping: func(); }}\nworld app {{ {items} }}\n")).unwrap();
    if kind != "offer" {
        let deps = tmp.path().join("wit/deps/reading-list");
        std::fs::create_dir_all(&deps).unwrap();
        std::fs::write(
            deps.join("world.wit"),
            if kind == "subscribe" { "package mods:reading-list@0.1.0; interface books { count: func() -> u32; } world reading-list { import books; }" } else { "package mods:reading-list@0.1.0; interface books { count: func() -> u32; } world reading-list { export books; }" },
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("deps.json"),
            format!(r#"{{"mods:reading-list":"{publisher}"}}"#),
        )
        .unwrap();
    }
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/search/{kind}.wasm")),
        tmp.path().join("logic.wasm"),
    )
    .unwrap();
    watane_pkg::pack(tmp.path(), false).unwrap().into_bytes()
}

#[allow(dead_code)]
pub fn origin_fixture(
    slug: &str,
    version: &str,
    origin: Option<&watane_dist::ReleaseRef>,
) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    copy_fixture("reading-list", tmp.path());
    std::fs::write(
        tmp.path().join("wit/world.wit"),
        format!("package mods:{slug}@{version}; world app {{}}"),
    )
    .unwrap();
    let mut meta = format!("name = {slug:?}\nsummary = \"Derived mod.\"\n");
    if let Some(origin) = origin {
        meta.push_str(&format!("[origin]\nrelease = \"{origin}\"\n"));
    }
    std::fs::write(tmp.path().join("mod.toml"), meta).unwrap();
    watane_pkg::pack(tmp.path(), false).unwrap().into_bytes()
}
