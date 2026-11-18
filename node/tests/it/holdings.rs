use crate::support;
use std::fs;
use std::sync::Arc;
use support::*;
use watane_dist::{Dist, Entry};
use watane_net::{BlobId, Holdings, Ingested, Why};
use watane_node::{DistHoldings, Node, Options};

fn blob(id: &watane_dist::PackageId) -> BlobId {
    BlobId(*id.as_bytes())
}

#[test]
fn s3_holder_answers_from_what_dist_stored() {
    let alice = Alice::new();
    let expected = alice.entries();
    let bob_dir = tempfile::tempdir().unwrap();
    let bob = Node::open_keyless(bob_dir.path(), Options::default()).unwrap();
    bob.follow(&alice.link()).unwrap();
    let release = &alice.published.info;
    bob.fetch(&release.reference).unwrap();

    let h = DistHoldings::empty();
    h.set(Arc::clone(bob.dist())).unwrap();
    let head = expected.last().unwrap();
    let want = Some(watane_net::HeadInfo {
        seq: head.seq(),
        id: head.id(),
        forked: false,
    });
    assert_eq!(h.heads(None), vec![(alice.key, want)]);
    assert_eq!(h.heads(Some(&[alice.key])), vec![(alice.key, want)]);
    assert_eq!(h.entries(&alice.key, 0, 100, usize::MAX), expected);
    assert!(h.fork(&alice.key).is_none());
    let path = h.blob(&blob(&release.release.package)).unwrap();
    assert_eq!(fs::read(path).unwrap(), alice.package_bytes());
    assert!(h.blob(&BlobId([7; 32])).is_none());
    assert!(!h.is_anchor());

    // a withdrawn release's package is not held
    alice.dist.withdraw(&release.reference, "ended").unwrap();
    let all = alice.entries();
    assert!(matches!(
        h.ingest(&alice.key, all, Why::Asked),
        Ingested::Stored(2)
    ));
    assert!(h.blob(&blob(&release.release.package)).is_none());
}

#[test]
fn s3_keyless_home_ingests_and_stores_only_verified() {
    let alice = Alice::new();
    let entries = alice.entries();
    let dir = tempfile::tempdir().unwrap();
    let node = Node::open_keyless(dir.path(), Options::default()).unwrap();
    let h = DistHoldings::empty();
    h.set(Arc::clone(node.dist())).unwrap();

    // a changed byte no longer decodes (the signature covers every byte), so it cannot
    // reach the holder; what can reach it unverified is a list under the wrong channel
    // or with a gap, and neither stores anything.
    let mut bytes = entries[0].bytes().to_vec();
    bytes[10] ^= 1;
    Entry::decode(&bytes).unwrap_err();
    let other = Alice::new();
    let r = h.ingest(&other.key, entries.clone(), Why::Kept);
    assert!(matches!(r, Ingested::Failed(_)), "{r:?}");
    let r = h.ingest(&alice.key, vec![entries[1].clone()], Why::Kept);
    assert!(!matches!(r, Ingested::Stored(_)), "{r:?}");
    assert_eq!(h.heads(Some(&[other.key])), vec![(other.key, None)]);
    assert_eq!(h.heads(Some(&[alice.key])), vec![(alice.key, None)]);

    assert_eq!(
        h.ingest(&alice.key, entries.clone(), Why::Kept),
        Ingested::Stored(1)
    );
    let follows: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("dist/follows.json")).unwrap()).unwrap();
    assert_eq!(follows["channels"][alice.key.to_string()]["kept"], true);
    assert!(!h.is_anchor());

    // store_blob: the right package is stored, a changed file is refused
    let id = alice.published.info.release.package.clone();
    let file = dir.path().join("pkg.bin");
    let good = alice.package_bytes();
    let mut wrong = good.clone();
    wrong[3] ^= 1;
    fs::write(&file, &wrong).unwrap();
    assert!(h.store_blob(&blob(&id), &file).is_err());
    fs::write(&file, &good).unwrap();
    h.store_blob(&blob(&id), &file).unwrap();
    assert!(h.blob(&blob(&id)).is_some());
}

#[test]
fn s3_keyed_home_is_anchor_and_stores_kept_pull_without_record() {
    let alice = Alice::new();
    let dir = tempfile::tempdir().unwrap();
    let dist = Arc::new(Dist::open(dir.path(), Approve::new(), fast()).unwrap());
    let h = DistHoldings::empty();
    h.set(dist).unwrap();
    assert!(h.is_anchor());
    assert_eq!(
        h.ingest(&alice.key, alice.entries(), Why::Kept),
        Ingested::Stored(1)
    );
    // NO-8: no record on the anchor
    assert!(h.heads(None).iter().all(|(k, _)| *k != alice.key));
}
