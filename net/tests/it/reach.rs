//! Stage N4, part 2: reaching machines (S11, TECHNICAL 5.10). Machines find
//! each other by the addresses they stored, and past that only by what the
//! participant chose: a relay of her own. Nothing here reaches the internet,
//! and no endpoint has mDNS or the DHT on.
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use support::*;
use watane_net::{Event, Limits, NetError, NodeKey, Options, Role};

/// Short deadlines, so that a machine that cannot be reached is known fast.
fn quick() -> Limits {
    Limits {
        deadline: Duration::from_secs(1),
        connect_timeout: Duration::from_secs(1),
        ..Limits::default()
    }
}

/// Stop the node and give back its home, to open it again.
fn closed(node: Node) -> (tempfile::TempDir, std::sync::Arc<MemHoldings>, NodeKey) {
    let Node {
        dir,
        net,
        key,
        holdings,
    } = node;
    net.close();
    (dir, holdings, key)
}

fn reopened(
    home: (tempfile::TempDir, std::sync::Arc<MemHoldings>, NodeKey),
    options: Options,
) -> Node {
    let (dir, holdings, key) = home;
    let net = open_on(&dir.path().join("net"), Arc::clone(&holdings), options);
    assert_eq!(net.node_key(), key);
    Node {
        dir,
        net,
        key,
        holdings,
    }
}

fn peer_up(events: &Receiver<Event>, peer: NodeKey, secs: u64) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match events.recv_timeout(left) {
            Ok(Event::PeerUp(k)) if k == peer => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

fn book(node: &Node) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(node.dir.path().join("net/book.json")).unwrap()).unwrap()
}

/// AC4: the address that worked is stored in `book.json`, and a reopened node
/// redials its paired machine with no `add_addrs`.
#[test]
fn ac4_a_reopened_node_redials_a_machine_from_its_book() {
    let a = node(MemHoldings::custom(Default::default(), None, true), None);
    let b = node(MemHoldings::empty(), None);
    pair_into(&a, &b, Role::Full);
    let b_addr = b.net.addrs()[0].to_string();
    let stored = |n: &Node, of: &NodeKey| book(n)["addrs"][of.to_string()].clone();
    assert!(wait_for(5, || stored(&a, &b.key) == serde_json::json!([b_addr])));
    // the other machine is only ever stored under its own key, newest first, at most 4
    assert!(
        book(&a)["addrs"]
            .as_object()
            .unwrap()
            .keys()
            .all(|k| *k == b.key.to_string())
    );

    // a is closed and opened again on a new port; b stays where it was
    let b_events = b.net.events();
    let events = {
        let home = closed(a);
        let a = reopened(home, options(None));
        let events = a.net.events();
        (a, events)
    };
    let (a, events) = events;
    // Either side's PeerUp shows the redial: b cannot know a's new port. a's own can come before
    // `events()` subscribes, as the open dials at once; b's is lost when b keeps its old link.
    assert!(
        peer_up(&events, b.key, 10) || peer_up(&b_events, a.key, 1),
        "no PeerUp from the stored address"
    );
    a.net.hello(&b.key).unwrap();
}

/// S11: Alice's laptop and a second machine, each on another network now,
/// connect through the relay of her server, once she chose it.
#[test]
fn s11_machines_on_different_networks_connect_through_the_servers_relay() {
    let laptop = node(
        MemHoldings::custom(Default::default(), None, true),
        Some(quick()),
    );
    let second = node(MemHoldings::empty(), Some(quick()));
    let server_options = || Options {
        relay_server: Some(lo()),
        ..options(Some(quick()))
    };
    let server = {
        let dir = tempfile::tempdir().unwrap();
        let holdings = MemHoldings::empty();
        let net = open_on(
            &dir.path().join("net"),
            Arc::clone(&holdings),
            server_options(),
        );
        let key = net.node_key();
        Node {
            dir,
            net,
            key,
            holdings,
        }
    };
    pair_into(&laptop, &second, Role::Full);
    pair_into(&laptop, &server, Role::Full);

    // all three go away and come back on new ports: what each stored of the
    // others is stale, and nothing tells them the new ones
    let homes = (closed(laptop), closed(second), closed(server));
    let laptop = reopened(homes.0, options(Some(quick())));
    let second = reopened(homes.1, options(Some(quick())));
    let server = reopened(homes.2, server_options());
    let server_net = server.net.clone();
    let (laptop_events, second_events) = (laptop.net.events(), second.net.events());

    // the server runs a relay for her machines
    let relay = server_net.relay_url().expect("the relay runs while on");
    assert_eq!(relay.scheme(), "http");
    assert!(relay.as_str().starts_with("http://127.0.0.1:"));
    assert_ne!(relay.port(), Some(0));

    // no relay, no DHT: the laptop cannot reach the second machine, and says what to choose
    let started = Instant::now();
    match laptop.net.hello(&second.key) {
        Err(NetError::Unreachable { node, cause }) => {
            assert_eq!(node, second.key);
            assert!(
                cause.contains("choose a relay or turn on the DHT"),
                "{cause}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(5));
    let told = (|| {
        let end = Instant::now() + Duration::from_secs(5);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match laptop_events.recv_timeout(left) {
                Ok(Event::Unreachable { node, cause }) if node == second.key => return Some(cause),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        None
    })();
    let cause = told.expect("an Event::Unreachable within 5 s");
    assert!(
        cause.contains("choose a relay or turn on the DHT"),
        "{cause}"
    );
    assert!(cause.contains(&second.key.short()), "{cause}");
    // once, not at every redial (the keeper redials within this time)
    let end = Instant::now() + Duration::from_millis(2500);
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        if let Ok(Event::Unreachable { node, cause }) = laptop_events.recv_timeout(left) {
            assert_ne!(node, second.key, "told again: {cause}");
        }
    }

    // she adds the server's relay on both machines
    let with_relay = |limits| Options {
        relays: vec![relay.clone()],
        ..options(Some(limits))
    };
    laptop.net.set_options(with_relay(quick())).unwrap();
    second.net.set_options(with_relay(quick())).unwrap();
    assert!(peer_up(&laptop_events, second.key, 10), "laptop: no PeerUp");
    assert!(peer_up(&second_events, laptop.key, 10), "second: no PeerUp");
    laptop.net.hello(&second.key).unwrap();
    second.net.hello(&laptop.key).unwrap();

    // the relay is for her machines only: a stranger that chose the same
    // relay is not let in, so it cannot reach the second machine through it
    let stranger = node(MemHoldings::empty(), Some(quick()));
    stranger
        .net
        .set_options(Options {
            relays: vec![relay.clone()],
            ..options(Some(quick()))
        })
        .unwrap();
    assert!(matches!(
        stranger.net.hello(&second.key),
        Err(NetError::Unreachable { .. })
    ));

    // the switches are in the book
    assert_eq!(book(&laptop)["relays"], serde_json::json!([relay.as_str()]));
    assert_eq!(book(&laptop)["dht"], false);
    assert_eq!(book(&laptop)["on"], true);

    // AC6: the relay stops when net turns off
    let at = std::net::SocketAddr::from(([127, 0, 0, 1], relay.port().unwrap()));
    let listens = || std::net::TcpStream::connect_timeout(&at, Duration::from_secs(1)).is_ok();
    assert!(listens());
    server_net.set_options(Options::default()).unwrap();
    assert!(server_net.relay_url().is_none());
    assert!(wait_for(2, || !listens()), "the relay still listens");
}

/// AC5: the anchor passes on where it knows the others are, so two machines
/// it paired connect with no `add_addrs` between them.
#[test]
fn ac5_machines_learn_each_others_addresses_from_a_devices_answer() {
    let anchor = node(MemHoldings::custom(Default::default(), None, true), None);
    let (b, c) = (
        node(MemHoldings::empty(), None),
        node(MemHoldings::empty(), None),
    );
    pair_into(&anchor, &b, Role::Full);
    // C can learn B's address and connect during join, before pair_into returns.
    let events = b.net.events();
    pair_into(&anchor, &c, Role::Full);
    // a new list: the anchor tells b, which asks for it and is told where c is
    anchor.net.set_role(&c.key, Role::Light).unwrap();
    assert!(
        peer_up(&events, c.key, 10),
        "b did not reach c by the address the anchor passed on"
    );
    b.net.hello(&c.key).unwrap();
}

/// A machine whose list is older than the caller's passes on no address: the
/// anchor that removed a machine is not told where the removed one is by an
/// honest machine that has not pulled the new list yet, and drops nobody.
#[test]
fn a_callee_with_an_older_list_passes_on_no_address() {
    let anchor = node(MemHoldings::custom(Default::default(), None, true), None);
    let (b, c) = (
        node(MemHoldings::empty(), None),
        node(MemHoldings::empty(), None),
    );
    pair_into(&anchor, &b, Role::Full);
    // Observe the link even when C's post-join pull starts it immediately.
    let b_events = b.net.events();
    pair_into(&anchor, &c, Role::Full);
    anchor.net.set_role(&c.key, Role::Light).unwrap();
    assert!(peer_up(&b_events, c.key, 10), "b knows where c is");
    let events = anchor.net.events();
    let seq = anchor.net.remove_device(&c.key).unwrap().seq;
    assert!(wait_for(5, || b
        .net
        .devices()
        .is_some_and(|l| l.seq == seq)));
    while let Ok(event) = events.recv_timeout(Duration::from_millis(500)) {
        assert!(
            !matches!(&event, Event::Refused { node, .. } if *node == b.key),
            "{event:?}"
        );
    }
    anchor.net.hello(&b.key).unwrap();
}

/// AC1: twenty rebuilds in a row return Ok, and the paired machine is still
/// answered afterwards (the blob store closes before each reopen).
#[test]
fn ac1_twenty_rebuilds_in_a_row_succeed_and_the_node_still_answers() {
    let a = node(
        MemHoldings::custom(Default::default(), None, true),
        Some(quick()),
    );
    let b = node(MemHoldings::empty(), Some(quick()));
    pair_into(&a, &b, Role::Full);
    let lists = [
        vec!["http://127.0.0.1:1/".parse().unwrap()],
        vec!["http://127.0.0.1:2/".parse().unwrap()],
    ];
    for i in 0..20 {
        a.net
            .set_options(Options {
                relays: lists[i % 2].clone(),
                ..options(Some(quick()))
            })
            .unwrap_or_else(|e| panic!("rebuild {i}: {e:?}"));
    }
    b.net.add_addrs(&a.key, &a.net.addrs());
    b.net.hello(&a.key).unwrap();
}

/// AC2: `close()` then `Net::open` on the same folder at once opens, ten times.
#[test]
fn ac2_close_then_open_on_the_same_folder_at_once_ten_times() {
    let mut home = closed(node(MemHoldings::empty(), None));
    for _ in 0..10 {
        home = closed(reopened(home, options(None)));
    }
}
