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
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (s, m, r, quit) = (server.clone(), mode.clone(), requests.clone(), stop.clone());
        let dir = dir.to_owned();
        let worker = std::thread::spawn(move || {
            while !quit.load(std::sync::atomic::Ordering::SeqCst) {
                let Ok(Some(req)) = s.recv_timeout(std::time::Duration::from_millis(50)) else {
                    continue;
                };
                r.lock().unwrap().push(req.url().into());
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
