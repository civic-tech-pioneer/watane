mod support;
use support::*;
use watane_dist::*;
fn entries(d: &Dist, seq: u64) -> Vec<Entry> {
    (0..=seq)
        .map(|n| {
            Entry::decode(&std::fs::read(d.site_dir().join(format!("entries/{n}"))).unwrap())
                .unwrap()
        })
        .collect()
}
#[test]
fn ac3_holder_stored_bytes_and_kept_record_survive() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.set_name("Alice").unwrap();
    let published = alice.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.identity().unwrap();
    let expected = entries(&alice, 1);
    let host = TestHost::serve(&alice.site_dir());
    let b = tempfile::tempdir().unwrap();
    let bob = open(b.path());
    bob.follow(&Link::new(key, &host.url, None).unwrap())
        .unwrap();
    assert_eq!(
        bob.heads(None)
            .unwrap()
            .iter()
            .find(|(k, _)| *k == key)
            .unwrap()
            .1
            .unwrap()
            .id,
        expected[1].id()
    );
    assert_eq!(bob.entries(&key, 0, 10, usize::MAX).unwrap(), expected);
    assert!(bob.fork(&key).unwrap().is_none());
    assert_eq!(bob.entries(&key, 0, 10, 1).unwrap(), expected[..1]);
    assert!(bob.entries(&key, 0, 0, 1).unwrap().is_empty());
    let h = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
    assert!(matches!(
        d.ingest(&key, expected.clone(), Reason::Kept),
        Ingested::Stored(1)
    ));
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h.path().join("dist/follows.json")).unwrap())
            .unwrap();
    let r = &record["channels"][key.to_string()];
    assert_eq!(r["kept"], true);
    assert_eq!(r["followed"], false);
    assert_eq!(r["index"], false);
    assert_eq!(r["base"], "");
    assert_eq!(r["bases"], serde_json::json!([]));
    d.refresh();
    assert!(d.channel(&key).unwrap().kept);
    d.follow(&Link::new(key, &host.url, None).unwrap()).unwrap();
    d.unfollow(&key).unwrap();
    assert!(d.channel(&key).unwrap().kept);
    let mut changed = expected[0].bytes().to_vec();
    changed[10] ^= 1;
    assert!(Entry::decode(&changed).is_err());
    bob.fetch(&published.info.reference).unwrap();
    assert!(bob.package_file(&published.info.release.package).is_some());
    alice.withdraw(&published.info.reference, "ended").unwrap();
    bob.ingest(&key, entries(&alice, 2), Reason::Asked);
    assert!(bob.package_file(&published.info.release.package).is_none());
    drop(d);
    let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
    assert!(d.channel(&key).unwrap().kept);
}
#[test]
fn ac4_holder_cap_and_asked_channels() {
    let h = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(
        h.path(),
        Options {
            max_kept: 2,
            ..fast_options()
        },
    )
    .unwrap();
    let mut homes = Vec::new();
    let mut channels = Vec::new();
    for name in ["one", "two", "three"] {
        let h = tempfile::tempdir().unwrap();
        let a = open(h.path());
        let e = a.set_name(name).unwrap();
        channels.push((a.identity().unwrap(), e));
        homes.push((h, a));
    }
    for (k, e) in &channels[..2] {
        assert!(matches!(
            d.ingest(k, vec![e.clone()], Reason::Kept),
            Ingested::Stored(0)
        ));
    }
    let (k, e) = &channels[2];
    assert!(matches!(
        d.ingest(k, vec![e.clone()], Reason::Kept),
        Ingested::NotWanted
    ));
    assert!(d.entries(k, 0, 10, 10000).unwrap().is_empty());
    assert!(matches!(
        d.keep(k),
        Err(DistError::NotFollowed {
            local_only: false,
            ..
        })
    ));
    assert!(matches!(
        d.ingest(k, vec![e.clone()], Reason::Asked),
        Ingested::Stored(0)
    ));
    assert!(!d.heads(None).unwrap().iter().any(|(key, _)| key == k));
    assert!(matches!(d.keep(k), Err(DistError::TooManyKept { max: 2 })));
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h.path().join("dist/follows.json")).unwrap())
            .unwrap();
    assert!(record["channels"].get(k.to_string()).is_none());
}
#[test]
fn ac5_holder_packages_verify_and_fetch_without_host() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let published = alice.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.identity().unwrap();
    for reason in [Reason::Kept, Reason::Asked] {
        let h = tempfile::tempdir().unwrap();
        let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
        assert!(matches!(
            d.ingest(&key, entries(&alice, 0), reason),
            Ingested::Stored(0)
        ));
        let id = &published.info.release.package;
        if reason == Reason::Kept {
            assert_eq!(
                d.missing_packages().unwrap(),
                vec![(id.clone(), published.info.release.size)]
            );
        } else {
            assert!(d.missing_packages().unwrap().is_empty());
        }
        let file = alice.site_dir().join(format!("packages/{id}"));
        let mut bad = std::fs::read(&file).unwrap();
        bad[10] ^= 1;
        let changed = a.path().join("changed");
        std::fs::write(&changed, &bad).unwrap();
        assert!(matches!(
            d.store_package(id, &changed),
            Err(DistError::HashMismatch { .. })
        ));
        assert!(!h.path().join(format!("dist/packages/{id}")).exists());
        let other = PackageId::of(&bad);
        assert!(matches!(
            d.store_package(&other, &changed),
            Err(DistError::NotNamed { .. })
        ));
        assert!(matches!(
            d.fetch(&published.info.reference),
            Err(DistError::NotFollowed {
                local_only: false,
                ..
            })
        ));
        d.store_package(id, &file).unwrap();
        let fetched = d.fetch(&published.info.reference).unwrap();
        assert!(fetched.link.is_none());
        assert_eq!(fetched.package.as_bytes(), std::fs::read(&file).unwrap());
        assert_eq!(
            d.package_file(id),
            Some(h.path().join(format!("dist/packages/{id}")))
        );
        assert!(d.missing_packages().unwrap().is_empty());
    }
}
#[test]
fn ac7_holder_confirm_exact_restored_head() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let backup = alice.backup("passphrase").unwrap();
    for name in ["Alice", "Alice B", "Alice C"] {
        alice.set_name(name).unwrap();
    }
    let key = alice.identity().unwrap();
    let expected = entries(&alice, 2);
    let h = tempfile::tempdir().unwrap();
    let d = open(h.path());
    d.restore(&backup, "passphrase").unwrap();
    assert!(matches!(
        d.ingest(&key, expected.clone(), Reason::Asked),
        Ingested::Stored(2)
    ));
    assert!(matches!(
        d.confirm_own_head(2, &expected[0].id()),
        Err(DistError::HeadNotStored { seq: 2 })
    ));
    assert!(matches!(
        d.confirm_own_head(1, &expected[1].id()),
        Err(DistError::HeadNotStored { seq: 1 })
    ));
    assert!(matches!(
        d.set_name("No"),
        Err(DistError::OwnHeadUnconfirmed)
    ));
    d.confirm_own_head(2, &expected[2].id()).unwrap();
    assert_eq!(d.set_name("Next").unwrap().seq(), 3);
}
#[test]
fn ac8_holder_search_stored_and_kept_refresh() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.identity().unwrap();
    let h = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
    d.ingest(&key, entries(&alice, 0), Reason::Asked);
    let hits = d.search_stored(&Query::default(), &[key]).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].via.is_empty());
    assert!(hits[0].link.is_none());
    assert!(d.search(&Query::default()).unwrap().is_empty());
    d.keep(&key).unwrap();
    let before = std::fs::read(h.path().join("dist/follows.json")).unwrap();
    assert!(matches!(
        d.refresh_channel(&key).unwrap().outcome,
        Outcome::Current { head: 0 }
    ));
    assert!(d.refresh().is_empty());
    assert_eq!(
        std::fs::read(h.path().join("dist/follows.json")).unwrap(),
        before
    );
    let hits = d.search(&Query::default()).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].via, vec![Via::Kept]);
    assert!(hits[0].link.is_none());
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(key, &host.url, None).unwrap();
    d.follow(&link).unwrap();
    d.choose_index(&link).unwrap();
    let hits = d.search(&Query::default()).unwrap();
    assert_eq!(
        hits[0].via,
        vec![
            Via::Followed,
            Via::Index {
                index: key,
                through: None,
                note: String::new()
            },
            Via::Kept
        ]
    );
    assert_eq!(
        d.search_stored(&Query::default(), &[key]).unwrap()[0]
            .link
            .as_ref()
            .unwrap()
            .base(),
        host.url
    );
    assert!(
        d.search_stored(&Query::default(), &[key]).unwrap()[0]
            .via
            .is_empty()
    );
    d.unfollow(&key).unwrap();
    d.drop_index(&key).unwrap();
    assert!(d.channel(&key).unwrap().kept);
    assert!(d.channel(&key).unwrap().base.is_none());
}
#[test]
fn ac6_holder_returns_while_refresh_waits_on_host() {
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.set_name("Alice").unwrap();
    let key = alice.identity().unwrap();
    let first = entries(&alice, 0);
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", server.server_addr());
    let h = tempfile::tempdir().unwrap();
    let d = Arc::new(
        Dist::open_keyless(
            h.path(),
            Options {
                entry_timeout: Duration::from_secs(30),
                ..fast_options()
            },
        )
        .unwrap(),
    );
    // The initial follow serves head and entries/0 normally; the refresh really answers after 20 s.
    let bytes = first[0].bytes().to_vec();
    let (blocked_tx, blocked_rx) = mpsc::channel();
    let server_thread = std::thread::spawn(move || {
        for _ in 0..2 {
            server
                .recv()
                .unwrap()
                .respond(tiny_http::Response::from_data(bytes.clone()))
                .unwrap();
        }
        let request = server.recv().unwrap();
        blocked_tx.send(()).unwrap();
        std::thread::sleep(Duration::from_secs(20));
        let _ = request.respond(tiny_http::Response::from_data(bytes));
    });
    d.follow(&Link::new(key, &url, None).unwrap()).unwrap();
    let reader = d.clone();
    let refresh = std::thread::spawn(move || reader.refresh());
    blocked_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (tx, rx) = mpsc::channel();
    let holder = d.clone();
    let holder_thread = std::thread::spawn(move || {
        let start = std::time::Instant::now();
        holder.heads(None).unwrap();
        tx.send(start.elapsed()).unwrap();
        let start = std::time::Instant::now();
        holder.entries(&key, 0, 10, 10000).unwrap();
        tx.send(start.elapsed()).unwrap();
        let start = std::time::Instant::now();
        assert!(matches!(
            holder.ingest(&key, first, Reason::Kept),
            Ingested::Stored(0)
        ));
        tx.send(start.elapsed()).unwrap();
    });
    for _ in 0..3 {
        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap() < Duration::from_secs(1));
    }
    holder_thread.join().unwrap();
    refresh.join().unwrap();
    server_thread.join().unwrap();
    assert!(d.channel(&key).unwrap().kept);
}

#[test]
fn ac3_holder_fork_proof_and_rejected_batch() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let backup = alice.backup("passphrase").unwrap();
    let zero = alice.set_name("Alice").unwrap();
    let one = alice.set_name("Alice 2").unwrap();
    let key = alice.identity().unwrap();
    let f = tempfile::tempdir().unwrap();
    let fork = open(f.path());
    fork.restore(&backup, "passphrase").unwrap();
    fork.ingest(&key, vec![zero.clone()], Reason::Asked);
    fork.confirm_own_head(0, &zero.id()).unwrap();
    let other = fork.set_name("Other").unwrap();
    let h = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
    let wrong = PublicKey::from_bytes(&[1; 32]).unwrap();
    assert!(matches!(
        d.ingest(&wrong, vec![zero.clone()], Reason::Kept),
        Ingested::Failed(DistError::Log(watane_log::Error::AuthorMismatch { .. }))
    ));
    assert!(d.heads(None).unwrap().is_empty());
    assert!(!h.path().join("dist/follows.json").exists());
    let mut bad = one.bytes().to_vec();
    *bad.last_mut().unwrap() ^= 1;
    assert!(Entry::decode(&bad).is_err());
    assert!(d.entries(&key, 0, 10, 10000).unwrap().is_empty());
    assert!(matches!(
        d.ingest(&key, vec![zero, one.clone()], Reason::Kept),
        Ingested::Stored(1)
    ));
    assert!(matches!(
        d.ingest(&key, vec![other.clone()], Reason::Asked),
        Ingested::Compromised { seq: 1 }
    ));
    assert_eq!(d.fork(&key).unwrap(), Some((one, other)));
    assert!(d.heads(None).unwrap()[0].1.unwrap().forked);
    assert!(matches!(
        d.ingest(&key, vec![], Reason::Asked),
        Ingested::Compromised { seq: 1 }
    ));
}

#[test]
fn ac5_holder_concurrent_package_writes() {
    use std::sync::{Arc, Barrier};
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let p = alice.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.identity().unwrap();
    let h = tempfile::tempdir().unwrap();
    let d = Arc::new(Dist::open_keyless(h.path(), fast_options()).unwrap());
    d.ingest(&key, entries(&alice, 0), Reason::Kept);
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let d = d.clone();
            let barrier = barrier.clone();
            let id = p.info.release.package.clone();
            let file = alice.site_dir().join(format!("packages/{id}"));
            std::thread::spawn(move || {
                barrier.wait();
                d.store_package(&id, &file)
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert_eq!(
        std::fs::read(d.package_file(&p.info.release.package).unwrap()).unwrap(),
        fixture("1.0.0", false)
    );
}
