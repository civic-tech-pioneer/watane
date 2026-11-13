mod support;
use std::sync::Arc;
use support::*;
use watane_dist::*;

fn listing_body(entry: &Entry, link: &Link, note: &str) {
    // Independently decode the signed body, rather than using dist's codec.
    let mut d = minicbor::Decoder::new(entry.body());
    assert_eq!(d.array().unwrap(), Some(2));
    assert_eq!(d.str().unwrap(), link.to_string());
    assert_eq!(d.str().unwrap(), note);
    assert_eq!(d.position(), entry.body().len());
}

#[test]
fn ac4_own_listing_refuse_and_requests() {
    let a = tempfile::tempdir().unwrap();
    let i = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let slug = "reading-list".parse().unwrap();
    let link = Link::new(alice.identity(), &host.url, Some(&slug)).unwrap();
    let channel = Link::new(alice.identity(), &host.url, None).unwrap();
    let confirm = Arc::new(RecordingConfirm::new([
        Decision::Refuse,
        Decision::Approve,
        Decision::Approve,
        Decision::Approve,
        Decision::Approve,
    ]));
    let ivy = Dist::open(i.path(), confirm.clone(), fast_options()).unwrap();
    assert!(matches!(ivy.list(&link, "books"), Err(DistError::Refused)));
    assert_eq!(ivy.own().unwrap().head, None);
    assert_eq!(confirm.requests.lock().unwrap().len(), 1);
    assert!(host.count() > 0);
    assert!(!i.path().join("dist/follows.json").exists());
    host.requests.lock().unwrap().clear();
    let listed = ivy.list(&link, "books").unwrap();
    assert_eq!(listed.kind(), "list");
    listing_body(&listed, &link, "books");
    let included = ivy.include(&channel, "").unwrap();
    assert_eq!(included.kind(), "include");
    listing_body(&included, &channel, "");
    let unlisted = ivy.unlist(&listed.id()).unwrap();
    assert_eq!(unlisted.kind(), "unlist");
    let mut d = minicbor::Decoder::new(unlisted.body());
    assert_eq!(d.array().unwrap(), Some(1));
    assert_eq!(d.bytes().unwrap(), listed.id().as_bytes());
    assert_eq!(d.position(), unlisted.body().len());
    assert_eq!(ivy.own().unwrap().head, Some(2));
    let requests = confirm.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        requests[1].statement,
        Statement::List(Listing {
            link: link.clone(),
            note: "books".into()
        })
    );
    assert_eq!(
        requests[2].statement,
        Statement::Include(Listing {
            link: channel.clone(),
            note: "".into()
        })
    );
    assert_eq!(
        requests[3].statement,
        Statement::Unlist {
            entry: listed.id(),
            listing: Listing {
                link: link.clone(),
                note: "books".into()
            },
            include: false
        }
    );
    drop(requests);
    // A stored channel is not fetched again; relisting an ended mod is allowed.
    ivy.list(&link, "again").unwrap();
    assert_eq!(host.count(), 0);
    assert!(ivy.followed().unwrap().is_empty());
    // Include also reads an unstored channel without making a follow record.
    let j = tempfile::tempdir().unwrap();
    let jay = open(j.path());
    host.requests.lock().unwrap().clear();
    jay.include(&channel, "").unwrap();
    assert!(host.count() > 0);
    assert!(!j.path().join("dist/follows.json").exists());
}

#[test]
fn ac4_own_listing_invalid_before_confirm() {
    let a = tempfile::tempdir().unwrap();
    let i = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let release = alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let confirm = Arc::new(RecordingConfirm::new([
        Decision::Approve,
        Decision::Approve,
        Decision::Approve,
        Decision::Approve,
    ]));
    let ivy = Dist::open(i.path(), confirm.clone(), fast_options()).unwrap();
    let slug = "reading-list".parse().unwrap();
    let link = Link::new(alice.identity(), &host.url, Some(&slug)).unwrap();
    let channel = Link::new(alice.identity(), &host.url, None).unwrap();
    let own = Link::new(ivy.identity(), &host.url, Some(&slug)).unwrap();
    assert!(matches!(ivy.list(&own, ""), Err(DistError::OwnListing)));
    assert!(matches!(
        ivy.include(&Link::new(ivy.identity(), &host.url, None).unwrap(), ""),
        Err(DistError::OwnListing)
    ));
    assert_eq!(host.count(), 0);
    assert!(matches!(
        ivy.list(&channel, ""),
        Err(DistError::BadLink { .. })
    ));
    assert!(matches!(
        ivy.include(&link, ""),
        Err(DistError::BadLink { .. })
    ));
    assert!(matches!(
        ivy.list(&link, "a\u{202e}b"),
        Err(DistError::BadNote { .. })
    ));
    assert!(matches!(
        ivy.include(&channel, "a\u{202e}b"),
        Err(DistError::BadNote { .. })
    ));
    let missing = Link::new(
        alice.identity(),
        &host.url,
        Some(&"missing".parse().unwrap()),
    )
    .unwrap();
    assert!(matches!(
        ivy.list(&missing, ""),
        Err(DistError::NoRelease { .. })
    ));
    let unknown = EntryId::from_bytes([0x22; 32]);
    assert!(matches!(ivy.unlist(&unknown),Err(DistError::NotListed {entry}) if entry==unknown));
    assert!(confirm.requests.lock().unwrap().is_empty());
    let listed = ivy.list(&link, "").unwrap();
    let inc = ivy.include(&channel, "").unwrap();
    confirm.requests.lock().unwrap().clear();
    let mirror = Link::new(alice.identity(), "https://mirror.example.org/", Some(&slug)).unwrap();
    assert!(matches!(
        ivy.list(&mirror, ""),
        Err(DistError::AlreadyListed { seq: 0 })
    ));
    assert!(matches!(
        ivy.include(&channel, ""),
        Err(DistError::AlreadyListed { seq: 1 })
    ));
    assert!(confirm.requests.lock().unwrap().is_empty());
    ivy.unlist(&listed.id()).unwrap();
    let ended_include = ivy.unlist(&inc.id()).unwrap();
    assert_eq!(ended_include.kind(), "unlist");
    assert_eq!(
        confirm.requests.lock().unwrap()[1].statement,
        Statement::Unlist {
            entry: inc.id(),
            listing: Listing {
                link: channel.clone(),
                note: "".into()
            },
            include: true
        }
    );
    let mut decoded = minicbor::Decoder::new(ended_include.body());
    assert_eq!(decoded.array().unwrap(), Some(1));
    assert_eq!(decoded.bytes().unwrap(), inc.id().as_bytes());
    confirm.requests.lock().unwrap().clear();
    assert!(matches!(
        ivy.unlist(&listed.id()),
        Err(DistError::NotListed { .. })
    ));
    assert!(matches!(
        ivy.unlist(&inc.id()),
        Err(DistError::NotListed { .. })
    ));
    assert!(confirm.requests.lock().unwrap().is_empty());
    // A fresh reader sees a channel whose only release was withdrawn.
    alice.withdraw(&release.info.reference, "gone").unwrap();
    let k = tempfile::tempdir().unwrap();
    let no = Arc::new(RecordingConfirm::new([]));
    let other = Dist::open(k.path(), no.clone(), fast_options()).unwrap();
    assert!(matches!(
        other.list(&link, ""),
        Err(DistError::NoRelease { .. })
    ));
    assert!(no.requests.lock().unwrap().is_empty());
}

#[test]
fn ac5_read_listing_changes_in_seq_order() {
    let a = tempfile::tempdir().unwrap();
    let i = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.publish(fixture("1.0.0", false)).unwrap();
    let ah = TestHost::serve(&alice.site_dir());
    let link = Link::new(
        alice.identity(),
        &ah.url,
        Some(&"reading-list".parse().unwrap()),
    )
    .unwrap();
    let channel = Link::new(alice.identity(), &ah.url, None).unwrap();
    let ivy = open(i.path());
    ivy.set_name("Ivy").unwrap();
    let ih = TestHost::serve(&ivy.site_dir());
    let carol = open(c.path());
    carol
        .follow(&Link::new(ivy.identity(), &ih.url, None).unwrap())
        .unwrap();
    let listed = ivy.list(&link, "books").unwrap();
    let included = ivy.include(&channel, "").unwrap();
    ivy.unlist(&listed.id()).unwrap();
    ivy.unlist(&included.id()).unwrap();
    let report = carol.refresh_channel(&ivy.identity()).unwrap();
    assert_eq!(report.changes.len(), 4);
    for (change, seq, id, include) in [
        (&report.changes[0], 1, listed.id(), false),
        (&report.changes[1], 2, included.id(), true),
    ] {
        let Change::Listed(info) = change else {
            panic!("expected Listed")
        };
        assert_eq!((info.seq, info.entry, info.include), (seq, id, include));
    }
    for (change, seq, id) in [
        (&report.changes[2], 3, listed.id()),
        (&report.changes[3], 4, included.id()),
    ] {
        let Change::Unlisted {
            seq: found,
            listing,
        } = change
        else {
            panic!("expected Unlisted")
        };
        assert_eq!(
            (*found, listing.entry, listing.unlisted),
            (seq, id, Some(seq))
        );
    }
    let view = carol.channel(&ivy.identity()).unwrap();
    assert_eq!(view.listings.len(), 2);
    assert!(!view.index);
    assert!(view.listed_by.is_empty());
    assert!(
        carol
            .refresh_channel(&ivy.identity())
            .unwrap()
            .changes
            .is_empty()
    );
}

struct Home {
    dist: Dist,
    host: TestHost,
    dir: tempfile::TempDir,
}
impl Home {
    fn new() -> Self {
        Self::with_options(Options {
            listed_local: true,
            ..fast_options()
        })
    }
    fn with_options(options: Options) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let dist = Dist::open(
            dir.path(),
            Arc::new(RecordingConfirm::new(std::iter::repeat_n(
                Decision::Approve,
                50,
            ))),
            options,
        )
        .unwrap();
        dist.set_name("Owner").unwrap();
        let host = TestHost::serve(&dist.site_dir());
        Self { dist, host, dir }
    }
    fn link(&self) -> Link {
        Link::new(self.dist.identity(), &self.host.url, None).unwrap()
    }
    fn mod_link(&self) -> Link {
        Link::new(
            self.dist.identity(),
            &self.host.url,
            Some(&"reading-list".parse().unwrap()),
        )
        .unwrap()
    }
    fn records(&self) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.dir.path().join("dist/follows.json")).unwrap())
            .unwrap()
    }
}
#[test]
fn ac1_index_choose() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "reading").unwrap();
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(
        reports.iter().map(|r| r.channel).collect::<Vec<_>>(),
        [ivy.dist.identity(), alice.dist.identity()]
    );
    assert_eq!(
        carol
            .dist
            .channel(&alice.dist.identity())
            .unwrap()
            .releases
            .len(),
        1
    );
    let records = carol.records();
    let i = &records["channels"][ivy.dist.identity().to_string()];
    assert_eq!(i["index"], true);
    assert_eq!(i["followed"], false);
    let a = &records["channels"][alice.dist.identity().to_string()];
    assert_eq!(a["index"], false);
    assert_eq!(a["followed"], false);
    assert_eq!(a["bases"], serde_json::json!([alice.host.url]));
    assert!(carol.dist.followed().unwrap().is_empty());
    assert_eq!(
        carol
            .dist
            .channel(&alice.dist.identity())
            .unwrap()
            .listed_by,
        [ivy.dist.identity()]
    );
    let indexes = carol.dist.indexes().unwrap();
    assert_eq!(indexes.len(), 1);
    assert_eq!(indexes[0].mods, 1);
    assert!(indexes[0].skipped.is_empty());
    assert!(matches!(
        carol.dist.choose_index(&alice.mod_link()),
        Err(DistError::BadLink { .. })
    ));
    assert!(matches!(
        carol.dist.choose_index(&carol.link()),
        Err(DistError::OwnChannel)
    ));
    let down = Home::new();
    let down_link = down.link();
    drop(down.host);
    let before = std::fs::read(carol.dir.path().join("dist/follows.json")).unwrap();
    assert!(carol.dist.choose_index(&down_link).is_err());
    assert_eq!(
        std::fs::read(carol.dir.path().join("dist/follows.json")).unwrap(),
        before
    );
}

#[test]
fn ac2_index_wrong_channel() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    let mallory = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    // Replace the listed site's bytes after Ivy has signed the valid listing.
    std::fs::copy(
        mallory.dist.site_dir().join("head"),
        alice.dist.site_dir().join("head"),
    )
    .unwrap();
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert!(matches!(
        &reports[1].outcome,
        Outcome::Failed {
            cause: DistError::WrongChannel { .. }
        }
    ));
    assert_eq!(
        carol.dist.channel(&mallory.dist.identity()).unwrap().head,
        None
    );
    assert_eq!(
        carol.dist.channel(&alice.dist.identity()).unwrap().head,
        None
    );
}

#[test]
fn ac3_index_one_level() {
    let ivy = Home::new();
    let jay = Home::new();
    let ken = Home::new();
    let lou = Home::new();
    let carol = Home::new();
    ken.dist.publish(fixture("1.0.0", false)).unwrap();
    jay.dist.list(&ken.mod_link(), "Ken").unwrap();
    jay.dist.include(&lou.link(), "Lou").unwrap();
    ivy.dist.include(&jay.link(), "Jay").unwrap();
    lou.host.requests.lock().unwrap().clear();
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(
        reports.iter().map(|r| r.channel).collect::<Vec<_>>(),
        [
            ivy.dist.identity(),
            jay.dist.identity(),
            ken.dist.identity()
        ]
    );
    assert_eq!(lou.host.count(), 0);
    assert_eq!(
        carol.dist.indexes().unwrap()[0].included,
        [jay.dist.identity()]
    );
    assert_eq!(
        carol.dist.channel(&ken.dist.identity()).unwrap().listed_by,
        [ivy.dist.identity()]
    );
}

#[test]
fn ac4_index_unlist() {
    for followed in [false, true] {
        let alice = Home::new();
        let ivy = Home::new();
        let carol = Home::new();
        alice.dist.publish(fixture("1.0.0", false)).unwrap();
        let listed = ivy.dist.list(&alice.mod_link(), "").unwrap();
        carol.dist.choose_index(&ivy.link()).unwrap();
        if followed {
            carol.dist.follow(&alice.link()).unwrap();
        }
        ivy.dist.unlist(&listed.id()).unwrap();
        carol.dist.refresh();
        let records = carol.records();
        let a = &records["channels"][alice.dist.identity().to_string()];
        if followed {
            assert_eq!(a["followed"], true);
        } else {
            assert!(a.is_null());
        }
        assert!(
            carol
                .dist
                .channel(&alice.dist.identity())
                .unwrap()
                .listed_by
                .is_empty()
        );
    }
}

#[test]
fn ac5_index_compromised() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let f = tempfile::tempdir().unwrap();
    let fork = Dist::open(
        f.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        fast_options(),
    )
    .unwrap();
    fork.restore(&ivy.dist.backup("secret").unwrap(), "secret")
        .unwrap();
    fork.confirm_own(Some(&ivy.link())).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    let other = fork.set_name("Fork").unwrap();
    std::fs::write(ivy.dist.site_dir().join("head"), other.bytes()).unwrap();
    std::fs::write(ivy.dist.site_dir().join("entries/1"), other.bytes()).unwrap();
    let reports = carol.dist.refresh();
    assert!(matches!(
        reports[0].outcome,
        Outcome::Compromised { seq: 1 }
    ));
    assert!(carol.records()["channels"][alice.dist.identity().to_string()].is_null());
    let count = alice.host.count();
    carol.dist.refresh();
    assert_eq!(alice.host.count(), count);
    assert!(carol.dist.indexes().unwrap()[0].included.is_empty());
    assert_eq!(carol.dist.indexes().unwrap()[0].mods, 0);
}

#[test]
fn ac1_index_cap() {
    for per_index in [false, true] {
        let ivy = Home::new();
        let publishers: Vec<_> = (0..3).map(|_| Home::new()).collect();
        for p in &publishers {
            p.dist.publish(fixture("1.0.0", false)).unwrap();
            ivy.dist.list(&p.mod_link(), "").unwrap();
            p.host.requests.lock().unwrap().clear();
        }
        let carol = Home::with_options(Options {
            listed_local: true,
            max_listed: if per_index { 512 } else { 2 },
            max_listed_per_index: if per_index { 2 } else { 128 },
            ..fast_options()
        });
        let reports = carol.dist.choose_index(&ivy.link()).unwrap();
        assert_eq!(reports.len(), 3);
        assert_eq!(
            publishers
                .iter()
                .map(|p| p.host.count())
                .collect::<Vec<_>>(),
            [3, 3, 0]
        );
        let index = carol.dist.indexes().unwrap().remove(0);
        assert_eq!(index.mods, 2);
        assert_eq!(
            index.skipped,
            vec![Skipped {
                channel: publishers[2].dist.identity(),
                cause: "over the listing limit"
            }]
        );
        assert!(carol.records()["channels"][publishers[2].dist.identity().to_string()].is_null());
    }
}

#[test]
fn ac7_index_stale_mirror() {
    let alice = Home::new();
    let ivy = Home::new();
    let jay = Home::new();
    let carol = Home::new();
    let release = alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let copy = tempfile::tempdir().unwrap();
    alice.dist.export(copy.path()).unwrap();
    let stale = TestHost::serve(copy.path());
    let stale_link = Link::new(
        alice.dist.identity(),
        &stale.url,
        Some(&"reading-list".parse().unwrap()),
    )
    .unwrap();
    ivy.dist.list(&stale_link, "old mirror").unwrap();
    jay.dist.list(&alice.mod_link(), "current").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    alice
        .dist
        .withdraw(&release.info.reference, "gone")
        .unwrap();
    carol.dist.choose_index(&jay.link()).unwrap();
    let a = carol.dist.channel(&alice.dist.identity()).unwrap();
    assert!(a.releases[0].withdrawn.is_some());
    assert!(a.last_success.is_some());
    assert_eq!(a.last_attempt, a.last_success);
    let records = carol.records();
    let a = &records["channels"][alice.dist.identity().to_string()];
    assert_eq!(a["base"], alice.host.url);
    assert_eq!(a["bases"][0], alice.host.url);
    assert!(
        a["bases"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(stale.url))
    );
    carol.dist.refresh();
    assert!(
        carol.dist.channel(&alice.dist.identity()).unwrap().releases[0]
            .withdrawn
            .is_some()
    );
}

#[test]
fn ac8_index_no_local() {
    let alice = Home::new();
    let ivy = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    alice.host.requests.lock().unwrap().clear();
    let carol = Home::with_options(fast_options());
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(alice.host.count(), 0);
    assert_eq!(
        carol.dist.indexes().unwrap()[0].skipped,
        vec![Skipped {
            channel: alice.dist.identity(),
            cause: "local address"
        }]
    );
    assert!(carol.records()["channels"][alice.dist.identity().to_string()].is_null());
}

#[test]
fn ac9_index_own_skipped() {
    let ivy = Home::new();
    let carol = Home::new();
    carol.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&carol.mod_link(), "").unwrap();
    carol.host.requests.lock().unwrap().clear();
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(
        carol.dist.indexes().unwrap()[0].skipped,
        vec![Skipped {
            channel: carol.dist.identity(),
            cause: "own channel"
        }]
    );
    assert_eq!(carol.host.count(), 0);
    assert_eq!(reports.len(), 1);
    assert!(carol.records()["channels"][carol.dist.identity().to_string()].is_null());
    assert!(carol.dist.own().unwrap().listed_by.is_empty());
}

#[test]
fn ac10_index_base_followed_wins() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let mirror_dir = tempfile::tempdir().unwrap();
    alice.dist.export(mirror_dir.path()).unwrap();
    let mirror = TestHost::serve(mirror_dir.path());
    ivy.dist
        .list(
            &Link::new(
                alice.dist.identity(),
                &mirror.url,
                Some(&"reading-list".parse().unwrap()),
            )
            .unwrap(),
            "",
        )
        .unwrap();
    carol.dist.follow(&alice.link()).unwrap();
    mirror.requests.lock().unwrap().clear();
    carol.dist.choose_index(&ivy.link()).unwrap();
    carol.dist.refresh();
    assert_eq!(
        carol.records()["channels"][alice.dist.identity().to_string()]["base"],
        alice.host.url
    );
    assert_eq!(mirror.count(), 0);
    assert_eq!(carol.dist.followed().unwrap().len(), 1);
}

#[test]
fn ac11_index_drop() {
    for followed in [false, true] {
        let alice = Home::new();
        let ivy = Home::new();
        let carol = Home::new();
        alice.dist.publish(fixture("1.0.0", false)).unwrap();
        ivy.dist.list(&alice.mod_link(), "").unwrap();
        carol.dist.choose_index(&ivy.link()).unwrap();
        if followed {
            carol.dist.follow(&ivy.link()).unwrap();
        }
        carol.dist.drop_index(&ivy.dist.identity()).unwrap();
        assert!(carol.dist.indexes().unwrap().is_empty());
        let records = carol.records();
        assert!(records["channels"][alice.dist.identity().to_string()].is_null());
        if followed {
            assert_eq!(
                records["channels"][ivy.dist.identity().to_string()]["followed"],
                true
            );
        } else {
            assert!(records["channels"][ivy.dist.identity().to_string()].is_null());
        }
        for key in [
            ivy.dist.identity(),
            alice.dist.identity(),
            carol.dist.identity(),
        ] {
            assert!(
                matches!(carol.dist.drop_index(&key), Err(DistError::NotIndex {channel}) if channel == key)
            );
        }
    }
}

#[test]
fn ac12_refresh_pass_order() {
    let ivy = Home::new();
    let jay = Home::new();
    let alice = Home::new();
    let ken = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ken.dist.publish(fixture("1.0.0", false)).unwrap();
    jay.dist.list(&ken.mod_link(), "Ken").unwrap();
    ivy.dist.list(&alice.mod_link(), "Alice").unwrap();
    ivy.dist.include(&jay.link(), "Jay").unwrap();
    // Ken occurs in both an include's list and Carol's own list: one read in pass b.
    carol.dist.list(&ken.mod_link(), "Ken too").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    let mut pass_b = vec![
        alice.dist.identity(),
        jay.dist.identity(),
        ken.dist.identity(),
    ];
    pass_b.sort();
    let mut expected = vec![ivy.dist.identity()];
    expected.extend(pass_b);
    for p in [&ivy, &jay, &alice, &ken] {
        p.host.requests.lock().unwrap().clear();
    }
    assert_eq!(
        carol
            .dist
            .refresh()
            .iter()
            .map(|r| r.channel)
            .collect::<Vec<_>>(),
        expected
    );
    for p in [&ivy, &jay, &alice, &ken] {
        assert_eq!(*p.host.requests.lock().unwrap(), ["/head"]);
    }
    carol
        .dist
        .unlist(&carol.dist.own().unwrap().listings[0].entry)
        .unwrap();
    carol.dist.follow(&alice.link()).unwrap();
    let mut pass_a = vec![ivy.dist.identity(), alice.dist.identity()];
    pass_a.sort();
    let mut expected = pass_a;
    expected.extend([jay.dist.identity(), ken.dist.identity()]);
    assert_eq!(
        carol
            .dist
            .refresh()
            .iter()
            .map(|r| r.channel)
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn ac13_step3_records_and_unfollow() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.dist.identity().to_string();
    let path = carol.dir.path().join("dist/follows.json");
    let legacy = serde_json::json!({"version":1,"channels":{key.clone():{"base":alice.host.url,"followed_at":1,"last_attempt":2,"last_success":2}}});
    std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    drop(carol.dist);
    let dist = Dist::open(
        carol.dir.path(),
        Arc::new(RecordingConfirm::new([])),
        Options {
            listed_local: true,
            ..fast_options()
        },
    )
    .unwrap();
    assert_eq!(dist.followed().unwrap().len(), 1);
    assert!(dist.indexes().unwrap().is_empty());
    assert_eq!(dist.refresh().len(), 1);
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    dist.choose_index(&ivy.link()).unwrap();
    dist.follow(&ivy.link()).unwrap();
    dist.unfollow(&ivy.dist.identity()).unwrap();
    assert_eq!(dist.indexes().unwrap().len(), 1);
    dist.unfollow(&alice.dist.identity()).unwrap();
    let records: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(records["channels"][&key]["followed"], false);
    assert!(matches!(
        dist.unfollow(&alice.dist.identity()),
        Err(DistError::NotFollowed { .. })
    ));
    assert_eq!(dist.refresh().len(), 2);
}

#[test]
fn ac6_index_entry_cap() {
    let alice = Home::new();
    let ivy = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    alice.dist.set_name("New name").unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    alice.host.requests.lock().unwrap().clear();
    let carol = Home::with_options(Options {
        listed_local: true,
        max_listed_entries: 1,
        ..fast_options()
    });
    let reports = carol.dist.choose_index(&ivy.link()).unwrap();
    assert!(matches!(
        reports[1].outcome,
        Outcome::Failed {
            cause: DistError::TooManyEntries { .. }
        }
    ));
    assert_eq!(
        *alice.host.requests.lock().unwrap(),
        ["/head", "/entries/0"]
    );
    assert_eq!(
        carol.dist.channel(&alice.dist.identity()).unwrap().head,
        Some(0)
    );
    assert_eq!(
        carol
            .dist
            .channel(&alice.dist.identity())
            .unwrap()
            .last_success,
        None
    );
    alice.host.requests.lock().unwrap().clear();
    let r = carol.dist.refresh_channel(&alice.dist.identity()).unwrap();
    assert!(matches!(
        r.outcome,
        Outcome::Failed {
            cause: DistError::TooManyEntries { .. }
        }
    ));
    assert_eq!(
        *alice.host.requests.lock().unwrap(),
        ["/head", "/entries/1"]
    );
}

#[test]
fn ac7_index_behind_falls_back() {
    let alice = Home::new();
    let ivy = Home::new();
    let jay = Home::new();
    let carol = Home::new();
    let release = alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let copy = tempfile::tempdir().unwrap();
    alice.dist.export(copy.path()).unwrap();
    let stale = TestHost::serve(copy.path());
    ivy.dist
        .list(
            &Link::new(
                alice.dist.identity(),
                &stale.url,
                Some(&"reading-list".parse().unwrap()),
            )
            .unwrap(),
            "",
        )
        .unwrap();
    jay.dist.list(&alice.mod_link(), "").unwrap();
    alice
        .dist
        .withdraw(&release.info.reference, "gone")
        .unwrap();
    carol.dist.choose_index(&jay.link()).unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    let path = carol.dir.path().join("dist/follows.json");
    let mut records = carol.records();
    let a = &mut records["channels"][alice.dist.identity().to_string()];
    a["base"] = serde_json::json!(stale.url);
    a["bases"] = serde_json::json!([stale.url, alice.host.url]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    drop(carol.dist);
    let dist = Dist::open(
        carol.dir.path(),
        Arc::new(RecordingConfirm::new([])),
        Options {
            listed_local: true,
            ..fast_options()
        },
    )
    .unwrap();
    let old_count = stale.count();
    let current_count = alice.host.count();
    assert!(matches!(
        dist.refresh_channel(&alice.dist.identity())
            .unwrap()
            .outcome,
        Outcome::Current { head: 2 }
    ));
    assert_eq!(stale.count(), old_count + 1);
    assert_eq!(alice.host.count(), current_count + 1);
    let view = dist.channel(&alice.dist.identity()).unwrap();
    assert_eq!(view.last_attempt, view.last_success);
    assert_eq!(view.base.as_deref(), Some(alice.host.url.as_str()));
}

#[test]
fn ac3_index_mod_count_includes_whole_channels() {
    let ivy = Home::new();
    let jay = Home::new();
    let carol = Home::new();
    ivy.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.publish(fixture("1.1.0", false)).unwrap();
    let jr = jay.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.include(&jay.link(), "").unwrap();
    ivy.dist.list(&jay.mod_link(), "same mod").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(carol.dist.indexes().unwrap()[0].mods, 2);
    jay.dist.withdraw(&jr.info.reference, "gone").unwrap();
    carol.dist.refresh();
    assert_eq!(carol.dist.indexes().unwrap()[0].mods, 1);
}

#[test]
fn ac1_index_skipped_once() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::with_options(fast_options());
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    ivy.dist.include(&alice.link(), "").unwrap();
    alice.host.requests.lock().unwrap().clear();
    carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(alice.host.count(), 0);
    assert_eq!(
        carol.dist.indexes().unwrap()[0].skipped,
        vec![Skipped {
            channel: alice.dist.identity(),
            cause: "local address"
        }]
    );
}

#[test]
fn ac6_index_cap_across_passes() {
    let ivy = Home::new();
    let other = Home::new();
    let jay = Home::new();
    let lou = Home::new();
    let alice = Home::new();
    let (first, second) = if ivy.dist.identity() < other.dist.identity() {
        (&ivy, &other)
    } else {
        (&other, &ivy)
    };
    first.dist.include(&jay.link(), "").unwrap();
    second.dist.include(&lou.link(), "").unwrap();
    let carol = Home::with_options(Options {
        listed_local: true,
        max_listed: 2,
        ..fast_options()
    });
    carol.dist.choose_index(&first.link()).unwrap();
    carol.dist.choose_index(&second.link()).unwrap();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    jay.dist.list(&alice.mod_link(), "new").unwrap();
    for h in [&jay, &lou, &alice] {
        h.host.requests.lock().unwrap().clear();
    }
    let reports = carol.dist.refresh();
    assert_eq!(reports.len(), 4);
    assert_eq!(
        jay.host
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == "/head")
            .count(),
        1
    );
    assert_eq!(lou.host.count(), 1);
    assert_eq!(alice.host.count(), 0);
    assert!(carol.records()["channels"][alice.dist.identity().to_string()].is_null());
    // The new listing is eligible on the next refresh, still within the cap.
    let reports = carol.dist.refresh();
    assert_eq!(reports.len(), 4);
    assert!(
        carol
            .dist
            .channel(&alice.dist.identity())
            .unwrap()
            .head
            .is_some()
    );
}

#[test]
fn ac8_index_local_check_after_reopen() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    drop(carol.dist);
    let dist = open(carol.dir.path());
    alice.host.requests.lock().unwrap().clear();
    let report = dist.refresh_channel(&alice.dist.identity()).unwrap();
    assert!(matches!(report.outcome, Outcome::Failed { .. }));
    assert_eq!(alice.host.count(), 0);
    dist.refresh();
    assert_eq!(alice.host.count(), 0);
}

#[test]
fn ac6_explicit_links_use_regular_entry_cap() {
    for choose in [false, true] {
        let alice = Home::new();
        let ivy = Home::new();
        alice.dist.publish(fixture("1.0.0", false)).unwrap();
        alice.dist.set_name("Alice").unwrap();
        ivy.dist.list(&alice.mod_link(), "").unwrap();
        let carol = Home::with_options(Options {
            listed_local: true,
            max_listed_entries: 0,
            ..fast_options()
        });
        carol.dist.choose_index(&ivy.link()).unwrap();
        if choose {
            carol.dist.choose_index(&alice.link()).unwrap();
        } else {
            carol.dist.follow(&alice.link()).unwrap();
        }
        assert_eq!(
            carol.dist.channel(&alice.dist.identity()).unwrap().head,
            Some(2)
        );
    }
}

#[test]
fn ac13_record_bases_validated_on_open() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    let path = carol.dir.path().join("dist/follows.json");
    let mut records = carol.records();
    records["channels"][alice.dist.identity().to_string()]["bases"] =
        serde_json::json!(["file:///tmp/"]);
    std::fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
    drop(carol.dist);
    assert!(matches!(
        Dist::open(
            carol.dir.path(),
            Arc::new(RecordingConfirm::new([])),
            fast_options()
        ),
        Err(DistError::Corrupt { .. })
    ));
}

#[test]
fn ac7_index_stale_mirror_after_outage() {
    let alice = Home::new();
    let jay = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    let release = alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let copy = tempfile::tempdir().unwrap();
    alice.dist.export(copy.path()).unwrap();
    let mirror = TestHost::serve(copy.path());
    jay.dist.list(&alice.mod_link(), "real").unwrap();
    ivy.dist
        .list(
            &Link::new(
                alice.dist.identity(),
                &mirror.url,
                Some(&"reading-list".parse().unwrap()),
            )
            .unwrap(),
            "mirror",
        )
        .unwrap();
    carol.dist.choose_index(&jay.link()).unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    let head = std::fs::read(alice.dist.site_dir().join("head")).unwrap();
    std::fs::remove_file(alice.dist.site_dir().join("head")).unwrap();
    let reports = carol.dist.refresh();
    assert!(matches!(
        reports
            .iter()
            .find(|r| r.channel == alice.dist.identity())
            .unwrap()
            .outcome,
        Outcome::Current { head: 1 }
    ));
    assert_eq!(
        carol.records()["channels"][alice.dist.identity().to_string()]["bases"][0],
        mirror.url
    );
    std::fs::write(alice.dist.site_dir().join("head"), head).unwrap();
    alice
        .dist
        .withdraw(&release.info.reference, "gone")
        .unwrap();
    alice.host.requests.lock().unwrap().clear();
    mirror.requests.lock().unwrap().clear();
    for _ in 0..3 {
        carol.dist.refresh();
    }
    let view = carol.dist.channel(&alice.dist.identity()).unwrap();
    assert!(view.releases[0].withdrawn.is_some());
    assert_eq!(view.last_attempt, view.last_success);
    assert_eq!(view.base.as_deref(), Some(alice.host.url.as_str()));
    assert_eq!(
        alice
            .host
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == "/head")
            .count(),
        3
    );
    assert_eq!(
        mirror
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.as_str() == "/head")
            .count(),
        3
    );
}

#[test]
fn ac12_refresh_listed_failure_has_listed_key() {
    let alice = Home::new();
    let ivy = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    ivy.dist.list(&alice.mod_link(), "").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    // An unwritable record destination makes the listed-record sync fail.
    let path = carol.dir.path().join("dist/follows.json");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    let reports = carol.dist.refresh();
    assert_eq!(
        reports.iter().map(|r| r.channel).collect::<Vec<_>>(),
        [ivy.dist.identity(), alice.dist.identity()]
    );
    assert!(
        matches!(&reports[1].outcome, Outcome::Failed {cause: DistError::ExportConflict {path: failed, ..}} if failed == &path)
    );
    assert!(reports.iter().all(|r| r.channel != carol.dist.identity()));
}

#[test]
fn ac7_index_equal_heads_keep_first_base() {
    let alice = Home::new();
    let ivy = Home::new();
    let jay = Home::new();
    let carol = Home::new();
    alice.dist.publish(fixture("1.0.0", false)).unwrap();
    let copy = tempfile::tempdir().unwrap();
    alice.dist.export(copy.path()).unwrap();
    let mirror = TestHost::serve(copy.path());
    let slug = "reading-list".parse().unwrap();
    let mirror_link = Link::new(alice.dist.identity(), &mirror.url, Some(&slug)).unwrap();
    ivy.dist.list(&mirror_link, "").unwrap();
    jay.dist.list(&alice.mod_link(), "").unwrap();
    carol.dist.choose_index(&ivy.link()).unwrap();
    carol.dist.choose_index(&jay.link()).unwrap();
    let key = alice.dist.identity().to_string();
    let bases = carol.records()["channels"][&key]["bases"].clone();
    assert_eq!(bases.as_array().unwrap().len(), 2);
    let (a, m) = (alice.host.count(), mirror.count());
    for _ in 0..2 {
        carol.dist.refresh();
        // Both bases serve head 1: every base is read, the first read keeps the front.
        assert_eq!(carol.records()["channels"][&key]["bases"], bases);
    }
    assert!(alice.host.count() > a && mirror.count() > m);
}

#[test]
fn ac1_skipped_walk_order() {
    let ivy = Home::new();
    let alice = Home::new();
    let bob = Home::new();
    let carol = Home::with_options(fast_options());
    for home in [&alice, &bob, &carol] {
        home.dist.publish(fixture("1.0.0", false)).unwrap();
    }
    let (first, second) = if alice.dist.identity() > bob.dist.identity() {
        (&alice, &bob)
    } else {
        (&bob, &alice)
    };
    ivy.dist.list(&first.mod_link(), "first").unwrap();
    ivy.dist.list(&carol.mod_link(), "own").unwrap();
    ivy.dist.list(&second.mod_link(), "second").unwrap();
    ivy.dist.include(&first.link(), "duplicate").unwrap();
    for home in [&alice, &bob, &carol] {
        home.host.requests.lock().unwrap().clear();
    }
    carol.dist.choose_index(&ivy.link()).unwrap();
    assert_eq!(
        carol.dist.indexes().unwrap()[0].skipped,
        vec![
            Skipped {
                channel: first.dist.identity(),
                cause: "local address"
            },
            Skipped {
                channel: carol.dist.identity(),
                cause: "own channel"
            },
            Skipped {
                channel: second.dist.identity(),
                cause: "local address"
            },
        ]
    );
    for home in [&alice, &bob, &carol] {
        assert_eq!(home.host.count(), 0);
    }
}
