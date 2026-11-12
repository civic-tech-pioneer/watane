mod support;
use std::sync::Arc;
use support::RecordingConfirm;
use watane_dist::*;
#[test]
fn ac6_own_refuse_set_name() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Refuse]));
    let dist = Dist::open(home.path(), c.clone(), Options::default()).unwrap();
    let key = dist.identity();
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
    let key = dist.identity();
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
        publisher: PublisherKey::from_bytes(*dist.identity().as_bytes()),
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
