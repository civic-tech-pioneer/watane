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
