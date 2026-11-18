use crate::support;
use std::fs;
use support::*;
use watane_dist::DistError;
use watane_node::{Machine, Node, NodeError, Options};

#[test]
fn s1_keyless_open_holds_no_identity() {
    let home = tempfile::tempdir().unwrap();
    let node = Node::open_keyless(home.path(), Options::default()).unwrap();
    assert!(home.path().join("log/channels.db").is_file());
    assert!(!home.path().join("log/identity.key").exists());
    assert!(matches!(
        node.publish(package("1.0.0")),
        Err(DistError::NoIdentity)
    ));
    assert_eq!(node.machine(), Machine::Alone);
    assert!(node.dist().identity().is_none());
    node.close();
}

#[test]
fn s1_app_open_keeps_its_identity() {
    let home = tempfile::tempdir().unwrap();
    let node = Node::open(home.path(), Approve::new(), Options::default()).unwrap();
    assert!(node.dist().identity().is_some());
    assert!(home.path().join("log/identity.key").exists());
    let published = node.publish(package("1.0.0")).unwrap();
    assert_eq!(published.entry.seq(), 0);
}

#[test]
fn s1_refuses_host_home_and_home_with_identity() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("home.json"), "{}").unwrap();
    assert!(matches!(
        Node::open_keyless(home.path(), Options::default()),
        Err(NodeError::HostHome { .. })
    ));

    let keyed = tempfile::tempdir().unwrap();
    drop(Node::open(keyed.path(), Approve::new(), Options::default()).unwrap());
    let key_file = keyed.path().join("log/identity.key");
    let before = fs::read(&key_file).unwrap();
    // dist maps log's `HasIdentity` to `DistError::Log`; log's error type is not re-exported.
    match Node::open_keyless(keyed.path(), Options::default()) {
        Err(NodeError::Dist(DistError::Log(e))) => {
            assert!(format!("{e:?}").starts_with("HasIdentity"), "{e:?}")
        }
        other => panic!("expected log's HasIdentity, got {:?}", other.err()),
    }
    assert_eq!(fs::read(&key_file).unwrap(), before);
}

#[test]
fn s2_net_off_follow_and_fetch_without_a_udp_socket() {
    if support::rerun_alone("open::s2_net_off_follow_and_fetch_without_a_udp_socket") {
        return;
    }
    let udp = sockets;
    let alice = Alice::new();
    let home = tempfile::tempdir().unwrap();
    let before = udp();
    let bob = Node::open(home.path(), Approve::new(), Options::default()).unwrap();
    // Net off opens no socket; the follow below uses TCP to the web host, which a
    // pooled connection may keep open, so the count is taken at the open.
    assert_eq!(udp(), before, "a socket exists while net is off");
    bob.follow(&alice.link()).unwrap();
    let fetched = bob.fetch(&alice.published.info.reference).unwrap();
    assert_eq!(
        fetched.package.as_bytes(),
        &*alice.package_bytes(),
        "package bytes equal Alice's"
    );
    bob.close();
}
