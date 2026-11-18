//! Homes, signed fixture channels and a web host on loopback. Expectations come from
//! a keyed `Dist` of another home, never from `watane_node`.
#![allow(
    dead_code,
    reason = "shared by several test binaries, each using only part of it"
)]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
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
        let (s, quit, dir) = (Arc::clone(&server), Arc::clone(&stop), dir.to_owned());
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

// ---- homes on loopback with net on (S4 to S9) ----

use watane_net::{DeviceList, Event, NodeKey, Role};
use watane_node::{Node, Options};

pub fn net_opts() -> watane_net::Options {
    watane_net::Options {
        on: true,
        bind: Some("127.0.0.1:0".parse().unwrap()),
        ..Default::default()
    }
}

pub fn node_opts(dist: watane_dist::Options) -> Options {
    Options {
        dist,
        net: net_opts(),
    }
}

/// An app home (anchor unless it joins), net on, loopback, mDNS off.
pub fn app(dir: &Path) -> Node {
    app_with(dir, fast())
}
pub fn app_with(dir: &Path, dist: watane_dist::Options) -> Node {
    let node = Node::open(dir, Approve::new(), node_opts(dist)).unwrap();
    node.net().set_options(net_opts()).unwrap();
    node
}

/// A node home (`watane-node`'s open), net on.
pub fn keyless(dir: &Path) -> Node {
    let node = Node::open_keyless(dir, node_opts(fast())).unwrap();
    node.net().set_options(net_opts()).unwrap();
    node
}

/// Each knows where the other is.
pub fn introduce(a: &Node, b: &Node) {
    a.net().add_addrs(&b.net().node_key(), &b.net().addrs());
    b.net().add_addrs(&a.net().node_key(), &a.net().addrs());
}

/// `joiner` joins `anchor` as `role`, the anchor says yes; returns the joiner's list
/// once both machines see each other.
pub fn pair(anchor: &Node, joiner: &Node, role: Role, replace: bool) -> DeviceList {
    let (ea, ej) = (anchor.net().events(), joiner.net().events());
    let code = anchor.net().pair_code(role).unwrap();
    let list = std::thread::scope(|s| {
        let joined = s.spawn(|| joiner.join(&code, replace));
        loop {
            if let Event::JoinAsked { node, .. } = ea_recv(&ea) {
                assert_eq!(node, joiner.net().node_key());
                anchor.net().answer_join(&node, true);
                break;
            }
        }
        joined.join().unwrap().unwrap()
    });
    introduce(anchor, joiner);
    for (events, peer) in [(ea, joiner.net().node_key()), (ej, anchor.net().node_key())] {
        loop {
            if matches!(ea_recv(&events), Event::PeerUp(k) if k == peer) {
                break;
            }
        }
    }
    list
}

fn ea_recv(events: &std::sync::mpsc::Receiver<Event>) -> Event {
    events.recv_timeout(Duration::from_secs(15)).unwrap()
}

pub fn wait_for(secs: u64, mut f: impl FnMut() -> bool) -> bool {
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    while std::time::Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    f()
}

pub fn key_of(node: &Node) -> NodeKey {
    node.net().node_key()
}

/// The stored head of `channel` in `node`'s dist: `(seq, id)`.
pub fn head(node: &Node, channel: &PublicKey) -> Option<(u64, watane_dist::EntryId)> {
    node.dist()
        .heads(Some(&[*channel]))
        .unwrap()
        .into_iter()
        .find_map(|(_, h)| h.map(|h| (h.seq, h.id)))
}

/// Runs the test `name` (its full path in the `it` binary) alone in a child process and returns
/// true; in that child it returns false and the test goes on. For a test that counts the
/// process's sockets, which every test of the binary shares.
pub fn rerun_alone(name: &str) -> bool {
    if std::env::var_os("WATANE_RERUN_ALONE").is_some() {
        return false;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--test-threads=1"])
        .env("WATANE_RERUN_ALONE", "1")
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && text.contains("1 passed"), "{text}");
    true
}
