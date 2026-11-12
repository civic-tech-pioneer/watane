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
