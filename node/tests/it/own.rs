//! S4 and S5: the laptop publishes and the server keeps (node 2, stage 2).
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use support::*;
use watane_dist::{Link, Reason};
use watane_net::Role;

#[test]
fn s4_the_server_keeps_what_alice_publishes() {
    let (l_dir, s_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (laptop, server) = (app(l_dir.path()), keyless(s_dir.path()));
    pair(&laptop, &server, Role::Full, false);

    let published = laptop.publish(package("1.0.0")).unwrap();
    let alice = laptop.dist().identity().unwrap();
    let (seq, id) = (published.entry.seq(), published.entry.id());
    assert!(
        wait_for(5, || head(&server, &alice) == Some((seq, id))
            && server
                .dist()
                .package_file(&published.info.release.package)
                .is_some()),
        "the server holds the entry and the package within 5 s"
    );
    let stored = fs::read(
        server
            .dist()
            .package_file(&published.info.release.package)
            .unwrap(),
    )
    .unwrap();
    let want = fs::read(
        laptop
            .dist()
            .package_file(&published.info.release.package)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stored, want);
    let record = |dir: &std::path::Path, key: &watane_dist::PublicKey| {
        // no records at all: no file
        let Ok(bytes) = fs::read(dir.join("dist/follows.json")) else {
            return serde_json::Value::Null;
        };
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["channels"][key.to_string()].clone()
    };
    let rec = record(s_dir.path(), &alice);
    assert_eq!(rec["kept"], true);
    assert_eq!(rec["followed"], false);
    assert_eq!(rec["index"], false);
    assert!(rec["base"].as_str().unwrap_or("").is_empty(), "{rec}");

    // A channel that only the server holds, pulled by the laptop: no record there (NO-8).
    let third = Alice::new();
    let entries = third.entries();
    let last = entries.last().unwrap().seq();
    assert!(matches!(
        server.dist().ingest(&third.key, entries, Reason::Kept),
        watane_dist::Ingested::Stored(_)
    ));
    laptop.net().need(&third.key).unwrap();
    assert_eq!(head(&laptop, &third.key).map(|h| h.0), Some(last));
    assert!(record(l_dir.path(), &third.key).is_null());
}

/// AC7 at the level that shows it: the laptop (anchor, not light) never asks the server
/// with `need` before a read, and tells the server a head it read from a web host.
#[test]
fn s4_the_laptop_tells_what_it_read_and_never_needs() {
    let (l_dir, s_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (laptop, server) = (app(l_dir.path()), keyless(s_dir.path()));
    pair(&laptop, &server, Role::Full, false);

    // a channel only the server holds: a `need` before the read would bring it here
    let other = Alice::new();
    assert!(matches!(
        server
            .dist()
            .ingest(&other.key, other.entries(), Reason::Kept),
        watane_dist::Ingested::Stored(_)
    ));
    let _ = laptop.refresh_channel(&other.key);
    let _ = laptop.fetch(&other.published.info.reference);
    assert_eq!(
        head(&laptop, &other.key),
        None,
        "no need on a non-light machine"
    );

    // a head read from a web host is told: the server pulls it within seconds
    let alice = Alice::new();
    let want = alice.dist.heads(None).unwrap()[0].1.map(|h| (h.seq, h.id));
    laptop.follow(&alice.link()).unwrap();
    assert!(
        wait_for(5, || head(&server, &alice.key) == want),
        "the server holds the head the laptop read"
    );
}

/// A web host that answers each request after `delay`.
struct SlowHost {
    url: String,
    server: Arc<tiny_http::Server>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl SlowHost {
    fn new(delay: Duration) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let url = format!("http://{}/", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let (s, quit) = (Arc::clone(&server), Arc::clone(&stop));
        let worker = std::thread::spawn(move || {
            while !quit.load(Ordering::SeqCst) {
                if let Ok(Some(req)) = s.recv_timeout(Duration::from_millis(50)) {
                    std::thread::sleep(delay);
                    let _ = req.respond(tiny_http::Response::empty(404));
                }
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
impl Drop for SlowHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.server.unblock();
        let _ = self.worker.take().unwrap().join();
    }
}

#[test]
fn s5_the_server_reads_heads_while_the_laptop_waits_for_a_web_host() {
    // TECHNICAL 10 says a host that answers after 20 s; here 4 s, with dist's entry
    // timeout at 3 s, so that the laptop's read is under way for the whole check.
    let dist = watane_dist::Options {
        entry_timeout: Duration::from_secs(3),
        connect_timeout: Duration::from_secs(3),
        ..fast()
    };
    let (l_dir, s_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let laptop = app_with(l_dir.path(), dist);
    let server = keyless(s_dir.path());
    pair(&laptop, &server, Role::Full, false);
    let host = SlowHost::new(Duration::from_secs(4));
    let link = Link::new(Alice::new().key, &host.url, None).unwrap();
    std::thread::scope(|s| {
        let reading = s.spawn(|| laptop.follow(&link));
        std::thread::sleep(Duration::from_millis(500));
        let started = Instant::now();
        let heads = server
            .net()
            .heads_of(&key_of(&laptop), &[])
            .expect("the laptop answers while its dist reads");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "{:?}",
            started.elapsed()
        );
        assert!(
            heads
                .iter()
                .any(|(k, _)| *k == laptop.dist().identity().unwrap())
        );
        assert!(!reading.is_finished(), "the read was still under way");
        let _ = reading.join().unwrap();
    });
}
