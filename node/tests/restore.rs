//! S8 and S9: an app home joins as a replica; Alice restores her identity.
mod support;
use std::fs;
use support::*;
use watane_dist::DistError;
use watane_net::Role;
use watane_node::{Node, NodeError, OwnHint};

#[test]
fn s8_an_unused_identity_joins_and_is_dropped() {
    let (a_dir, h_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let anchor = app(a_dir.path());
    let home = app(h_dir.path());
    let key_file = h_dir.path().join("log/identity.key");
    assert!(key_file.exists());
    let list = pair(&anchor, &home, Role::Light, false);
    let me = key_of(&home);
    assert!(!key_file.exists(), "the key file is gone");
    assert!(list.devices.contains(&(me, Role::Light)));
    let theirs = anchor.net().devices().unwrap();
    assert!(
        theirs.devices.contains(&(me, Role::Light)),
        "the anchor names it light"
    );
    home.close();
    let again = Node::open(h_dir.path(), Approve::new(), Default::default()).unwrap();
    assert!(again.dist().identity().is_none(), "a replica opens keyless");
    assert!(!key_file.exists());
}

#[test]
fn s8_an_identity_that_signed_is_not_dropped() {
    let (a_dir, h_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let anchor = app(a_dir.path());
    let home = app(h_dir.path());
    home.set_name("Alice at home").unwrap();
    let key_file = h_dir.path().join("log/identity.key");
    let before = fs::read(&key_file).unwrap();
    let code = anchor.net().pair_code(Role::Light).unwrap();
    match home.join(&code, false) {
        Err(NodeError::IdentityInUse { fingerprint }) => {
            assert_eq!(Some(fingerprint), home.dist().fingerprint())
        }
        other => panic!("expected IdentityInUse, got {other:?}"),
    }
    assert_eq!(fs::read(&key_file).unwrap(), before);
    assert!(home.dist().identity().is_some());
}

#[test]
fn s9_alice_restores_and_picks_the_servers_hint() {
    let dirs: Vec<_> = (0..3).map(|_| tempfile::tempdir().unwrap()).collect();
    let old = app(dirs[0].path());
    let server = keyless(dirs[1].path());
    pair(&old, &server, Role::Full, false);
    let published = old.publish(package("1.0.0")).unwrap();
    let alice = old.dist().identity().unwrap();
    let (seq, id) = (published.entry.seq(), published.entry.id());
    // the server holds the head (a later sync, if the first hint was lost)
    assert!(wait_for(10, || head(&server, &alice) == Some((seq, id))
        || (server.net().need(&alice).is_ok()
            && head(&server, &alice) == Some((seq, id)))));
    let backup = old.dist().backup("pass").unwrap();
    old.close();

    let new = app(dirs[2].path());
    assert_eq!(new.dist().restore(&backup, "pass").unwrap(), alice);
    pair(&new, &server, Role::Full, true);

    assert!(wait_for(10, || new
        .own_hints()
        .iter()
        .any(|h| h.node == key_of(&server) && h.head.is_some())));
    let hint = new
        .own_hints()
        .into_iter()
        .find(|h| h.node == key_of(&server))
        .unwrap();
    let got = hint.head.unwrap();
    assert_eq!((got.seq, got.id), (seq, id));
    assert!(matches!(
        new.publish(package("0.2.0")),
        Err(DistError::OwnHeadUnconfirmed)
    ));

    let mut wrong = hint.clone();
    wrong.head = Some(watane_net::HeadInfo {
        id: watane_dist::EntryId::from_bytes([5; 32]),
        ..got
    });
    assert!(matches!(
        new.confirm_own_hint(&wrong),
        Err(NodeError::Dist(DistError::HeadNotStored { .. }))
    ));
    let empty = OwnHint {
        node: hint.node,
        head: None,
    };
    assert!(matches!(
        new.confirm_own_hint(&empty),
        Err(NodeError::EmptyHint)
    ));
    assert!(matches!(
        new.publish(package("0.2.0")),
        Err(DistError::OwnHeadUnconfirmed)
    ));

    new.confirm_own_hint(&hint).unwrap();
    let next = new.publish(package("0.2.0")).unwrap();
    assert_eq!(next.entry.seq(), seq + 1);
}
