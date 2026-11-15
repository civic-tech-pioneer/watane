mod support;
use std::sync::Arc;
use support::RecordingConfirm;
use watane_dist::*;
#[test]
fn ac6_own_refuse_set_name() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Refuse]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let key = dist.identity().unwrap();
    assert!(matches!(dist.set_name("Alice"), Err(DistError::Refused)));
    assert_eq!(c.requests.lock().unwrap().len(), 1);
    drop(dist);
    let log = watane_log::Log::open(&home.path().join("log")).unwrap();
    assert_eq!(log.head(&key).unwrap(), None);
}

#[test]
fn ac7_own_request_set_name() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Approve]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let key = dist.identity().unwrap();
    let entry = dist.set_name("Alice").unwrap();
    let expected = vec![0x81, 0x65, 0x41, 0x6c, 0x69, 0x63, 0x65];
    assert_eq!(entry.body(), expected);
    assert_eq!(
        c.requests.lock().unwrap().as_slice(),
        &[SignRequest {
            channel: key,
            fingerprint: key.to_string()[..10].into(),
            statement: Statement::Profile(Profile {
                name: "Alice".into()
            })
        }]
    );
    assert_eq!(dist.own().unwrap().name.as_deref(), Some("Alice"));
    assert_eq!(dist.channel(&key).unwrap().head, Some(0));
    drop(dist);
    let log = watane_log::Log::open(&home.path().join("log")).unwrap();
    let stored = log.entries(&key, 0, 10).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].body(), expected);
}

#[test]
fn ac8_own_bad_name_before_confirm() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    assert!(matches!(
        dist.set_name("Al\u{202e}ice"),
        Err(DistError::BadName { .. })
    ));
    assert!(c.requests.lock().unwrap().is_empty());
    assert_eq!(dist.own().unwrap().head, None);
}
#[test]
fn ac9_corrupt_chain_refused_before_confirm() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Approve]));
    let dist = Dist::open(home.path(), c, Options::default()).unwrap();
    let entry = dist.set_name("Alice").unwrap();
    drop(dist);
    let db = rusqlite::Connection::open(home.path().join("log/channels.db")).unwrap();
    db.execute_batch("DROP TRIGGER entries_no_update").unwrap();
    let mut bytes = entry.bytes().to_vec();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    db.execute("UPDATE entries SET raw=?", [bytes]).unwrap();
    drop(db);
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    assert!(matches!(
        dist.set_name("Bob"),
        Err(DistError::Log(watane_log::Error::CorruptStore { .. }))
    ));
    assert!(c.requests.lock().unwrap().is_empty());
}

#[test]
fn ac10_own_refuse_unknown_withdraw() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let reference = ReleaseRef {
        publisher: PublisherKey::from_bytes(*dist.identity().unwrap().as_bytes()),
        slug: "reading-list".parse().unwrap(),
        version: "1.0.0".parse().unwrap(),
    };
    assert!(
        matches!(dist.withdraw(&reference,"bug"),Err(DistError::NotOwnRelease { release }) if release == reference)
    );
    let foreign = ReleaseRef {
        publisher: PublisherKey::from_bytes([0x11; 32]),
        ..reference
    };
    assert!(
        matches!(dist.withdraw(&foreign,"bug"),Err(DistError::NotOwnRelease { release }) if release == foreign)
    );
    assert!(c.requests.lock().unwrap().is_empty());
}
#[test]
fn ac11_error_text_escapes_controls() {
    let e = DistError::BadUrl {
        url: "https://example.org/\u{1b}".into(),
        cause: "invalid URL",
    };
    let text = e.to_string();
    assert!(text.contains("\\u{1b}"));
    assert!(!text.contains('\u{1b}'));
}

#[test]
fn ac1_own_refuse_publish() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Refuse]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let bytes = support::fixture("1.0.0", false);
    let id = blake3::hash(&bytes).to_hex().to_string();
    let key = dist.identity().unwrap();
    assert!(matches!(dist.publish(bytes), Err(DistError::Refused)));
    assert_eq!(c.requests.lock().unwrap().len(), 1);
    assert_eq!(std::fs::read_dir(dist.site_dir()).unwrap().count(), 0);
    assert!(!home.path().join("dist/packages").join(id).exists());
    drop(dist);
    assert_eq!(
        watane_log::Log::open(&home.path().join("log"))
            .unwrap()
            .head(&key)
            .unwrap(),
        None
    );
}
#[test]
fn ac2_own_request_publish() {
    for source in [false, true] {
        let home = tempfile::tempdir().unwrap();
        let c = Arc::new(RecordingConfirm::new([Decision::Approve]));
        let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
        let bytes = support::fixture("1.2.3", source);
        let size = bytes.len() as u64;
        let published = dist.publish(bytes).unwrap();
        let release = Release::decode(published.entry.body()).unwrap();
        assert_eq!(release.size, size);
        assert_eq!(release.source, source);
        let requests = c.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let Statement::Release {
            reference,
            release: recorded,
        } = &requests[0].statement
        else {
            panic!()
        };
        assert_eq!(recorded, &release);
        assert_eq!(
            reference.publisher.as_bytes(),
            dist.identity().unwrap().as_bytes()
        );
        assert_eq!(reference.slug.to_string(), "reading-list");
        assert_eq!(reference.version.to_string(), "1.2.3");
        drop(dist);
        let log = watane_log::Log::open(&home.path().join("log")).unwrap();
        assert_eq!(
            log.entries(&requests[0].channel, 0, 10).unwrap()[0].body(),
            published.entry.body()
        );
    }
}
#[test]
fn ac3_own_request_version_exists_before_confirm() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Approve]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    dist.publish(support::fixture("1.0.0", false)).unwrap();
    assert!(matches!(
        dist.publish(support::fixture("1.0.0", true)),
        Err(DistError::VersionExists { seq: 0, .. })
    ));
    assert_eq!(c.requests.lock().unwrap().len(), 1);
}
#[test]
fn ac4_own_request_unreleased_provider_before_confirm() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    support::copy_fixture("recommendations", tmp.path());
    let bytes = watane_pkg::pack(tmp.path(), false).unwrap().into_bytes();
    assert!(
        matches!(dist.publish(bytes), Err(DistError::UnreleasedProvider { provider }) if provider.to_string()=="reading-list")
    );
    assert!(c.requests.lock().unwrap().is_empty());
}
#[test]
fn ac5_own_request_package_too_large_before_parsing() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(
        home.path(),
        c.clone(),
        Options {
            max_package: 3,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(matches!(
        dist.publish(vec![0; 4]),
        Err(DistError::PackageTooLarge { size: 4, max: 3 })
    ));
    assert!(c.requests.lock().unwrap().is_empty());
}
