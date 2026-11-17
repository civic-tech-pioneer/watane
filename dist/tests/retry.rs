#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use support::*;
use watane_dist::*;

#[derive(Clone, Copy)]
enum Behaviour {
    /// 503 to the first GET of each path, the file afterwards.
    FailFirst,
    Always(u16),
    Stall(Duration),
}

struct Host {
    url: String,
    gets: Arc<Mutex<Vec<String>>>,
    server: Arc<tiny_http::Server>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Host {
    fn serve(dir: &std::path::Path, behaviour: Behaviour) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}/", server.server_addr());
        let gets: Arc<Mutex<Vec<String>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (s, g, quit, dir) = (
            Arc::clone(&server),
            Arc::clone(&gets),
            Arc::clone(&stop),
            dir.to_owned(),
        );
        let worker = std::thread::spawn(move || {
            while !quit.load(Ordering::SeqCst) {
                let Ok(Some(req)) = s.recv_timeout(Duration::from_millis(20)) else {
                    continue;
                };
                let path = req.url().to_owned();
                let seen = g.lock().unwrap().iter().filter(|p| **p == path).count();
                g.lock().unwrap().push(path.clone());
                match behaviour {
                    Behaviour::Always(status) => {
                        let _ = req.respond(tiny_http::Response::empty(status));
                    }
                    Behaviour::Stall(wait) => {
                        std::thread::sleep(wait);
                        drop(req);
                    }
                    Behaviour::FailFirst if seen == 0 => {
                        let _ = req.respond(tiny_http::Response::empty(503));
                    }
                    Behaviour::FailFirst => {
                        match std::fs::read(dir.join(path.trim_start_matches('/'))) {
                            Ok(bytes) => {
                                let _ = req.respond(tiny_http::Response::from_data(bytes));
                            }
                            Err(_) => {
                                let _ = req.respond(tiny_http::Response::empty(404));
                            }
                        }
                    }
                }
            }
        });
        Self {
            url,
            gets,
            server,
            stop,
            worker: Some(worker),
        }
    }
    fn count(&self, path: &str) -> usize {
        self.gets
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == path)
            .count()
    }
    fn total(&self) -> usize {
        self.gets.lock().unwrap().len()
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.server.unblock();
        self.worker.take().unwrap().join().unwrap();
    }
}

fn opened(home: &std::path::Path, entry_timeout: Duration) -> Dist {
    let mut options = fast_options();
    options.entry_timeout = entry_timeout;
    Dist::open(
        home,
        Arc::new(RecordingConfirm::new(std::iter::repeat_n(
            Decision::Approve,
            20,
        ))),
        options,
    )
    .unwrap()
}

fn alice_with_release() -> (tempfile::TempDir, Dist) {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.set_name("Alice").unwrap();
    alice.publish(fixture("1.0.0", false)).unwrap();
    (a, alice)
}

#[test]
fn ac1_follow_and_open_link_ride_out_a_503() {
    let (_a, alice) = alice_with_release();
    let key = alice.identity().unwrap();
    let b = tempfile::tempdir().unwrap();
    let bob = opened(b.path(), Duration::from_secs(10));
    let host = Host::serve(&alice.site_dir(), Behaviour::FailFirst);
    let link = Link::new(key, &host.url, None).unwrap();
    assert!(matches!(
        bob.follow(&link).unwrap().outcome,
        Outcome::Current { head: 1 }
    ));
    assert_eq!(host.count("/head"), 2);

    let c = tempfile::tempdir().unwrap();
    let carol = opened(c.path(), Duration::from_secs(10));
    let host = Host::serve(&alice.site_dir(), Behaviour::FailFirst);
    let slug = alice.channel(&key).unwrap().releases[0]
        .release
        .slug
        .clone();
    let link = Link::new(key, &host.url, Some(&slug)).unwrap();
    let fetched = carol.open_link(&link).unwrap();
    assert_eq!(fetched.package.as_bytes(), fixture("1.0.0", false));
}

#[test]
fn ac2_a_host_that_keeps_failing_gives_http_after_three_gets() {
    let (_a, alice) = alice_with_release();
    let b = tempfile::tempdir().unwrap();
    let bob = opened(b.path(), Duration::from_secs(3));
    let host = Host::serve(&alice.site_dir(), Behaviour::Always(503));
    let link = Link::new(alice.identity().unwrap(), &host.url, None).unwrap();
    let start = Instant::now();
    let error = bob.follow(&link).unwrap_err();
    assert!(
        start.elapsed() < Duration::from_millis(3500),
        "{:?}",
        start.elapsed()
    );
    assert!(
        matches!(&error, DistError::Http { url, status: 503 } if *url == format!("{}head", host.url)),
        "{error:?}"
    );
    assert_eq!(host.count("/head"), 3);
    assert!(bob.followed().unwrap().is_empty());
}

#[test]
fn ac3_no_retry_starts_after_the_deadline() {
    let (_a, alice) = alice_with_release();
    let b = tempfile::tempdir().unwrap();
    let bob = opened(b.path(), Duration::from_millis(300));
    let host = Host::serve(&alice.site_dir(), Behaviour::Always(503));
    let link = Link::new(alice.identity().unwrap(), &host.url, None).unwrap();
    let start = Instant::now();
    bob.follow(&link).unwrap_err();
    assert!(
        start.elapsed() < Duration::from_millis(700),
        "{:?}",
        start.elapsed()
    );
    assert!(host.total() < 3, "{}", host.total());
    let seen = host.total();
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(host.total(), seen);
}

#[test]
fn ac4_only_a_5xx_is_retried() {
    let (_a, alice) = alice_with_release();
    let key = alice.identity().unwrap();
    let b = tempfile::tempdir().unwrap();
    let bob = opened(b.path(), Duration::from_secs(3));
    let host = Host::serve(&alice.site_dir(), Behaviour::Always(404));
    let link = Link::new(key, &host.url, None).unwrap();
    assert!(matches!(bob.follow(&link), Err(DistError::NotFound { .. })));
    assert_eq!(host.total(), 1);
    let host = Host::serve(&alice.site_dir(), Behaviour::Always(403));
    let link = Link::new(key, &host.url, None).unwrap();
    assert!(matches!(
        bob.follow(&link),
        Err(DistError::Http { status: 403, .. })
    ));
    assert_eq!(host.total(), 1);
    let host = Host::serve(&alice.site_dir(), Behaviour::Stall(Duration::from_secs(2)));
    let link = Link::new(key, &host.url, None).unwrap();
    let q = tempfile::tempdir().unwrap();
    let quick = opened(q.path(), Duration::from_millis(300));
    assert!(matches!(
        quick.follow(&link),
        Err(DistError::Timeout { .. })
    ));
    assert_eq!(host.total(), 1);
    // A refused connection is a network error, tried once.
    let dead = {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        format!("http://{}/", server.server_addr())
    };
    let link = Link::new(key, &dead, None).unwrap();
    let start = Instant::now();
    assert!(matches!(bob.follow(&link), Err(DistError::Network { .. })));
    assert!(start.elapsed() < Duration::from_millis(200));
}
