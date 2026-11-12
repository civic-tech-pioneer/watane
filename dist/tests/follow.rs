mod support;
use support::*;
use watane_dist::*;

#[test]
fn ac1_follow_open_link_verified() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    alice.set_name("Alice").unwrap();
    let bytes = fixture("1.0.0", false);
    let published = alice.publish(bytes.clone()).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(
        alice.identity(),
        &host.url,
        Some(&"reading-list".parse().unwrap()),
    )
    .unwrap();
    let fetched = bob.open_link(&link).unwrap();
    assert_eq!(fetched.package.as_bytes(), bytes);
    let log = watane_log::Log::open(&a.path().join("log")).unwrap();
    let entry = log
        .entries(&alice.identity(), published.entry.seq(), 1)
        .unwrap()
        .remove(0);
    assert_eq!(fetched.entry.bytes(), entry.bytes());
    assert!(bob.channel(&alice.identity()).unwrap().followed);
    assert_eq!(fetched.publisher_name.as_deref(), Some("Alice"));
    assert!(fetched.checked_at.is_some());
    drop(bob);
    let bob = open(b.path());
    assert_eq!(bob.followed().unwrap().len(), 1);
    bob.unfollow(&alice.identity()).unwrap();
    assert!(bob.followed().unwrap().is_empty());
}

#[test]
fn ac3_follow_tampered_entry_keeps_only_verified_prefix() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    alice.set_name("Alice").unwrap();
    alice.publish(fixture("1.0.0", false)).unwrap();
    alice.set_name("Alice again").unwrap();
    let path = alice.site_dir().join("entries/1");
    let mut bytes = std::fs::read(&path).unwrap();
    let n = bytes.len() - 1;
    bytes[n] ^= 1;
    std::fs::write(path, bytes).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(alice.identity(), &host.url, None).unwrap();
    assert!(
        matches!(bob.follow(&link), Err(DistError::Invalid {url,..}) if url == format!("{}entries/1", host.url))
    );
    assert!(bob.followed().unwrap().is_empty());
    assert_eq!(bob.channel(&alice.identity()).unwrap().head, Some(0));
}

#[test]
fn ac4_follow_wrong_channel() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let carol = open(c.path());
    carol.set_name("Carol").unwrap();
    let host = TestHost::serve(&carol.site_dir());
    assert!(matches!(
        bob.follow(&Link::new(alice.identity(), &host.url, None).unwrap()),
        Err(DistError::WrongChannel { .. })
    ));
    assert!(bob.followed().unwrap().is_empty());
}
#[test]
fn ac5_follow_own_channel() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    assert!(matches!(
        alice.follow(&Link::new(alice.identity(), "http://127.0.0.1:1/", None).unwrap()),
        Err(DistError::OwnChannel)
    ));
}

#[test]
fn ac6_refresh_releases_and_withdrawals_in_order() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let old = alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity(), &host.url, None).unwrap())
        .unwrap();
    alice.publish(fixture("1.1.0", false)).unwrap();
    alice.withdraw(&old.info.reference, "bug").unwrap();
    let reports = bob.refresh();
    assert_eq!(reports.len(), 1);
    let changes = &reports[0].changes;
    assert_eq!(changes.len(), 2);
    assert!(matches!(&changes[0], Change::Release(r) if r.release.version.to_string() == "1.1.0"));
    assert!(
        matches!(&changes[1], Change::Withdrawn {release,..} if release.reference == old.info.reference && release.withdrawn.as_ref().unwrap().reason == "bug")
    );
    assert!(bob.refresh()[0].changes.is_empty());
}

#[test]
fn ac7_refresh_fork_stops_fetch_and_requests() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let f = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let zero = alice.set_name("Alice").unwrap();
    let release = alice.publish(fixture("1.0.0", false)).unwrap();
    alice.set_name("Alice two").unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity(), &host.url, None).unwrap())
        .unwrap();
    // The task explicitly requires a fork made with log alone.
    let fork = watane_log::Log::open(&f.path().join("log")).unwrap();
    fork.restore_identity(&alice.backup("passphrase").unwrap(), "passphrase")
        .unwrap();
    fork.ingest(&alice.identity(), &[zero]).unwrap();
    fork.confirm_own_head().unwrap();
    let other = fork.append("profile", b"\x81\x65Other").unwrap();
    std::fs::write(alice.site_dir().join("head"), other.bytes()).unwrap();
    std::fs::write(alice.site_dir().join("entries/1"), other.bytes()).unwrap();
    assert!(matches!(
        bob.refresh()[0].outcome,
        Outcome::Compromised { seq: 1 }
    ));
    assert!(matches!(
        bob.fetch(&release.info.reference),
        Err(DistError::Compromised { seq: 1, .. })
    ));
    let count = host.count();
    assert!(matches!(
        bob.refresh()[0].outcome,
        Outcome::Compromised { seq: 1 }
    ));
    assert_eq!(host.count(), count);
}
#[test]
fn ac8_refresh_behind_keeps_newer_head() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    alice.set_name("Alice").unwrap();
    let old = alice.publish(fixture("1.0.0", false)).unwrap();
    alice.set_name("New Alice").unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity(), &host.url, None).unwrap())
        .unwrap();
    std::fs::write(alice.site_dir().join("head"), old.entry.bytes()).unwrap();
    assert!(matches!(
        bob.refresh()[0].outcome,
        Outcome::Behind {
            served: 1,
            stored: 2
        }
    ));
    assert_eq!(bob.channel(&alice.identity()).unwrap().head, Some(2));
}

#[test]
fn ac3_follow_wrong_seq_and_read_limits() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let zero = alice.set_name("Alice").unwrap();
    alice.publish(fixture("1.0.0", false)).unwrap();
    alice.set_name("New Alice").unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(alice.identity(), &host.url, None).unwrap();
    std::fs::write(alice.site_dir().join("entries/1"), zero.bytes()).unwrap();
    let bob = open(b.path());
    assert!(
        matches!(bob.follow(&link),Err(DistError::WrongSeq {expected:1,found:0,url}) if url==format!("{}entries/1",host.url))
    );
    assert_eq!(bob.channel(&alice.identity()).unwrap().head, Some(0));
    assert!(bob.followed().unwrap().is_empty());
    let c = tempfile::tempdir().unwrap();
    let mut options = fast_options();
    options.max_entry_file = 1;
    let limited = Dist::open(
        c.path(),
        std::sync::Arc::new(RecordingConfirm::new([])),
        options,
    )
    .unwrap();
    assert!(
        matches!(limited.follow(&link),Err(DistError::TooLarge {limit:1,url}) if url==format!("{}head",host.url))
    );
}
#[test]
fn ac6_refresh_failure_records_attempt_and_keeps_success() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    alice.set_name("Alice").unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(alice.identity(), &host.url, None).unwrap();
    bob.follow(&link).unwrap();
    let metadata = b.path().join("dist/follows.json");
    let mut file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
    file["channels"][alice.identity().to_string()]["last_attempt"] = 0.into();
    std::fs::write(&metadata, serde_json::to_vec(&file).unwrap()).unwrap();
    drop(bob);
    let bob = open(b.path());
    std::fs::remove_file(alice.site_dir().join("head")).unwrap();
    assert!(matches!(bob.follow(&link), Err(DistError::NotFound { .. })));
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&metadata).unwrap()).unwrap();
    let record = &after["channels"][alice.identity().to_string()];
    assert!(record["last_attempt"].as_u64().unwrap() > 0);
    assert_eq!(
        record["last_success"],
        file["channels"][alice.identity().to_string()]["last_success"]
    );
    assert!(bob.channel(&alice.identity()).unwrap().followed);
}
