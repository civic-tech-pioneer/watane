mod support;
use support::*;
use watane_dist::*;
#[test]
fn ac11_restore_confirms_published_head_before_signing() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let r = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let carol = open(c.path());
    alice.set_name("Alice").unwrap();
    let p = alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let link = Link::new(alice.identity(), &host.url, None).unwrap();
    bob.follow(&link).unwrap();
    let confirm = std::sync::Arc::new(RecordingConfirm::new([Decision::Approve]));
    let restored = Dist::open(r.path(), confirm.clone(), fast_options()).unwrap();
    assert_eq!(
        restored
            .restore(&alice.backup("secret").unwrap(), "secret")
            .unwrap(),
        alice.identity()
    );
    assert!(!restored.own_confirmed().unwrap());
    assert!(matches!(
        restored.publish(fixture("1.1.0", false)),
        Err(DistError::OwnHeadUnconfirmed)
    ));
    assert!(confirm.requests.lock().unwrap().is_empty());
    assert!(matches!(
        restored.confirm_own(Some(&Link::new(carol.identity(), &host.url, None).unwrap())),
        Err(DistError::WrongChannel { .. })
    ));
    assert!(!restored.own_confirmed().unwrap());
    // Alice's key, but the host serves Carol's channel: the read fails, so no confirmation.
    carol.set_name("Carol").unwrap();
    let carol_host = TestHost::serve(&carol.site_dir());
    assert!(matches!(
        restored.confirm_own(Some(
            &Link::new(alice.identity(), &carol_host.url, None).unwrap()
        )),
        Err(DistError::WrongChannel { .. })
    ));
    assert!(!restored.own_confirmed().unwrap());
    restored.confirm_own(Some(&link)).unwrap();
    assert!(restored.own_confirmed().unwrap());
    let next = restored.publish(fixture("1.1.0", false)).unwrap();
    assert_eq!(next.entry.seq(), p.entry.seq() + 1);
    assert_eq!(confirm.requests.lock().unwrap().len(), 1);
    restored.export(&alice.site_dir()).unwrap();
    assert!(matches!(
        bob.refresh()[0].outcome,
        Outcome::Current { head: 2 }
    ));
    assert_eq!(bob.channel(&alice.identity()).unwrap().status, Status::Ok);
    let log = watane_log::Log::open(&r.path().join("log")).unwrap();
    assert!(log.own_head_confirmed().unwrap());
    assert_eq!(log.head(&alice.identity()).unwrap().unwrap().seq, 2);
}
