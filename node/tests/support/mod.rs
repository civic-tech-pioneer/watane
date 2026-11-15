//! Homes, signed fixture channels and a web host on loopback. Expectations come from
//! a keyed `Dist` of another home, never from `watane_node`.
#![allow(dead_code)]
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
use watane_dist::{Confirm, Decision, Dist, Entry, Link, PublicKey, SignRequest};

pub struct Approve(pub Mutex<usize>);
impl Approve {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(Mutex::new(0)))
    }
}
impl Confirm for Approve {
    fn confirm(&self, _: &SignRequest) -> Decision {
        *self.0.lock().unwrap() += 1;
        Decision::Approve
    }
}

pub fn fast() -> watane_dist::Options {
    watane_dist::Options {
        entry_timeout: Duration::from_secs(10),
        package_timeout: Duration::from_secs(10),
        ..Default::default()
    }
}

/// A packed mod of dist's own fixture, with the given version.
pub fn package(version: &str) -> Vec<u8> {
    fn copy(src: &Path, dst: &Path) {
        fs::create_dir_all(dst).unwrap();
        for e in fs::read_dir(src).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                copy(&e.path(), &dst.join(e.file_name()));
            } else {
                fs::copy(e.path(), dst.join(e.file_name())).unwrap();
            }
        }
    }
    let tmp = tempfile::tempdir().unwrap();
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/tests/fixtures/reading-list"),
        tmp.path(),
    );
    fs::write(
        tmp.path().join("wit/world.wit"),
        format!("package mods:reading-list@{version};\nworld reading-list {{}}\n"),
    )
    .unwrap();
    watane_pkg::pack(tmp.path(), false).unwrap().into_bytes()
}

pub struct Host {
    pub url: String,
    server: Arc<tiny_http::Server>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Host {
    pub fn serve(dir: &Path) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}/", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let (s, quit, dir) = (server.clone(), stop.clone(), dir.to_owned());
        let worker = std::thread::spawn(move || {
            while !quit.load(Ordering::SeqCst) {
                let Ok(Some(req)) = s.recv_timeout(Duration::from_millis(50)) else {
                    continue;
                };
                let path = req.url().trim_start_matches('/').to_owned();
                assert!(!path.contains(".."));
                let _ = match fs::read(dir.join(path)) {
                    Ok(bytes) => req.respond(tiny_http::Response::from_data(bytes)),
                    Err(_) => req.respond(tiny_http::Response::empty(404)),
                };
            }
        });
        Self {
            url,
            server,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.server.unblock();
        self.worker.take().unwrap().join().unwrap();
    }
}

/// Alice: a keyed home that signed a name and a release, served on loopback.
pub struct Alice {
    pub dir: tempfile::TempDir,
    pub dist: Dist,
    pub host: Host,
    pub key: PublicKey,
    pub published: watane_dist::Published,
}
impl Alice {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(dir.path(), Approve::new(), fast()).unwrap();
        dist.set_name("Alice").unwrap();
        let published = dist.publish(package("1.0.0")).unwrap();
        let host = Host::serve(&dist.site_dir());
        let key = dist.identity().unwrap();
        Self {
            dir,
            dist,
            host,
            key,
            published,
        }
    }
    pub fn link(&self) -> Link {
        Link::new(self.key, &self.host.url, None).unwrap()
    }
    /// The package bytes Alice's home stores.
    pub fn package_bytes(&self) -> Vec<u8> {
        let id = &self.published.info.release.package;
        fs::read(self.dist.package_file(id).unwrap()).unwrap()
    }
    pub fn entries(&self) -> Vec<Entry> {
        let head = self.dist.heads(None).unwrap()[0].1.unwrap().seq;
        (0..=head)
            .map(|n| {
                Entry::decode(&fs::read(self.dist.site_dir().join(format!("entries/{n}"))).unwrap())
                    .unwrap()
            })
            .collect()
    }
}

/// Sockets of this process; `/dev/fd` exists on Linux and macOS, and a stat of
/// `/dev/fd/<n>` reports the open file itself.
pub fn sockets() -> usize {
    use std::os::unix::fs::FileTypeExt;
    fs::read_dir("/dev/fd")
        .unwrap()
        .filter_map(|e| fs::metadata(e.ok()?.path()).ok())
        .filter(|m| m.file_type().is_socket())
        .count()
}
