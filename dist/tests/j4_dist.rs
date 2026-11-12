mod support;
use support::*;
use watane_dist::*;

#[test]
fn ac8_j4_dist() {
    let alice = SearchHome::new("Alice");
    let ivy = SearchHome::new("Ivy");
    let carol = SearchHome::new("Carol");
    let bytes = fixture("0.1.0", false);
    alice.dist.publish(bytes.clone()).unwrap();
    ivy.dist
        .list(&alice.link(Some("reading-list")), "good for books")
        .unwrap();
    carol.dist.choose_index(&ivy.link(None)).unwrap();
    let hit = carol
        .dist
        .search(&Query {
            text: "reading".into(),
            ..Default::default()
        })
        .unwrap()
        .remove(0);
    assert_eq!(
        hit.info.reference.to_string(),
        format!("{}/reading-list@0.1.0", alice.dist.identity())
    );
    carol.dist.follow(hit.link.as_ref().unwrap()).unwrap();
    let fetched = carol.dist.fetch(&hit.info.reference).unwrap();
    assert_eq!(fetched.package.as_bytes(), bytes);
    assert_eq!(
        fetched.package.id().to_string(),
        blake3::hash(&bytes).to_hex().to_string()
    );
    alice.dist.publish(fixture("0.2.0", false)).unwrap();
    let newer = format!("{}/reading-list@0.2.0", alice.dist.identity());
    let reports = carol.dist.refresh();
    let report = reports
        .iter()
        .find(|r| r.channel == alice.dist.identity())
        .unwrap();
    assert_eq!(report.changes.len(), 1);
    assert!(matches!(&report.changes[0], Change::Release(r) if r.reference.to_string()==newer));
    alice.dist.withdraw(&newer.parse().unwrap(), "bug").unwrap();
    let reports = carol.dist.refresh();
    let report = reports
        .iter()
        .find(|r| r.channel == alice.dist.identity())
        .unwrap();
    assert_eq!(report.changes.len(), 1);
    assert!(
        matches!(&report.changes[0], Change::Withdrawn { release, .. } if release.reference.to_string()==newer && release.withdrawn.as_ref().unwrap().reason=="bug")
    );
    // Independent A15 fork producer, required by the task and TECHNICAL section 7.
    let tmp = tempfile::tempdir().unwrap();
    let fork = watane_log::Log::open(&tmp.path().join("log")).unwrap();
    fork.restore_identity(&alice.dist.backup("passphrase").unwrap(), "passphrase")
        .unwrap();
    let zero =
        Entry::decode(&std::fs::read(alice.dist.site_dir().join("entries/0")).unwrap()).unwrap();
    fork.ingest(&alice.dist.identity(), &[zero]).unwrap();
    fork.confirm_own_head().unwrap();
    let other = fork.append("profile", b"\x81\x65Other").unwrap();
    std::fs::write(alice.dist.site_dir().join("head"), other.bytes()).unwrap();
    std::fs::write(alice.dist.site_dir().join("entries/1"), other.bytes()).unwrap();
    assert!(
        carol
            .dist
            .refresh()
            .iter()
            .any(|r| r.channel == alice.dist.identity()
                && matches!(r.outcome, Outcome::Compromised { seq: 1 }))
    );
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].info.reference.to_string(),
        format!("{}/reading-list@0.1.0", alice.dist.identity())
    );
    assert_eq!(hits[0].status, Status::Compromised { seq: 1 });
    assert!(matches!(
        carol.dist.fetch(&hits[0].info.reference),
        Err(DistError::Compromised { seq: 1, .. })
    ));
}
