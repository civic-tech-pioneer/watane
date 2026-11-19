//! Stage N4, part 1: a light machine asks a full one (S8, the query part of
//! S9, S12), the anchor that keeps only what its holder asks for (NET-F-21)
//! and the follow-ups of S63 (NET-F-32). Entries are signed by hand.
#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

use crate::support;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use support::*;
use watane_log::{Entry, PublicKey};
use watane_net::{BlobId, Event, Limits, NetError, Options, Query, Role, Why};

fn chan(seed: [u8; 32]) -> PublicKey {
    PublicKey::from_bytes(&public_of(seed)).unwrap()
}

fn holding(chains: &[(PublicKey, Vec<Entry>)], anchor: bool) -> Arc<MemHoldings> {
    MemHoldings::custom(chains.iter().cloned().collect(), None, anchor)
}

fn raws(entries: &[Entry]) -> Vec<Vec<u8>> {
    entries.iter().map(|e| e.bytes().to_vec()).collect()
}

fn hit_of(channel: PublicKey, entries: &[Entry]) -> watane_net::Hit {
    let last = entries.last().unwrap();
    watane_net::Hit {
        channel,
        seq: last.seq(),
        entry: last.id(),
    }
}

fn query() -> Query {
    Query {
        text: Some("notes".into()),
        publisher: None,
        interface: None,
        origin: None,
        limit: 10,
    }
}

/// Both know where the other is: the light machine has no mDNS or book yet.
fn introduce(a: &Node, b: &Node) {
    a.net.add_addrs(&b.key, &b.net.addrs());
    b.net.add_addrs(&a.key, &a.net.addrs());
}

fn quick() -> Limits {
    Limits {
        deadline: Duration::from_millis(400),
        connect_timeout: Duration::from_millis(400),
        ..Limits::default()
    }
}

/// Four paired machines that S8 and the query part of S9 share (pairing
/// costs seconds): the anchor laptop, the server and a third machine (both
/// full replicas, which know each other) and Alice's second laptop (light),
/// which reaches both replicas and chooses the server first by key order.
struct Shared {
    anchor: Node,
    server: Node,
    third: Node,
    light: Node,
    cx: Vec<Entry>,
    cw: Vec<Entry>,
    package: Vec<u8>,
    id: BlobId,
}

/// The fixture and how many of its `USERS` tests are done: the last one
/// drops it, so that its folders are removed (a static is never dropped).
static SHARED: std::sync::Mutex<(Option<Shared>, usize)> = std::sync::Mutex::new((None, 0));
const USERS: usize = 2;

struct Fixture(std::sync::MutexGuard<'static, (Option<Shared>, usize)>);

impl std::ops::Deref for Fixture {
    type Target = Shared;
    fn deref(&self) -> &Shared {
        self.0.0.as_ref().expect("the fixture was dropped")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.0.1 += 1;
        if self.0.1 == USERS {
            self.0.0 = None;
        }
    }
}

fn shared() -> Fixture {
    let mut guard = SHARED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if guard.0.is_none() {
        guard.0 = Some({
            let (x, w) = ([71u8; 32], [76u8; 32]);
            let (cx, cw) = (chain(x, 3), chain(w, 2));
            let package = vec![5u8; 3000];
            // neither the anchor nor the light laptop holds x or w
            let anchor = node(holding(&[], true), None);
            let a = node(MemHoldings::empty(), None);
            let b = node(MemHoldings::empty(), None);
            // Both replicas become reachable through devices answers. Among
            // connected replicas the lower key is asked first (5.9).
            let (server, third) = if a.key < b.key { (a, b) } else { (b, a) };
            for entry in &cx {
                server.holdings.append(entry.clone());
            }
            for entry in &cw {
                third.holdings.append(entry.clone());
            }
            let light = node(MemHoldings::empty(), None);
            let links = [
                (server.net.events(), third.key),
                (third.net.events(), server.key),
                (light.net.events(), server.key),
                (server.net.events(), light.key),
                (light.net.events(), third.key),
                (third.net.events(), light.key),
            ];
            // the server reaches the third machine (a server that forwarded a
            // query would ask it) and takes none of its channels; the anchor
            // passes the addresses on (5.10), so this holds from the start
            server
                .holdings
                .not_wanted
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let id = server.holdings.add_blob(&package);
            server.holdings.set_hits(vec![hit_of(chan(x), &cx)]);
            third.holdings.set_hits(vec![hit_of(chan(w), &cw)]);
            pair_unsettled(&anchor, &server, Role::Full, false);
            pair_unsettled(&anchor, &third, Role::Full, false);
            pair_unsettled(&anchor, &light, Role::Light, false);
            introduce(&server, &third);
            introduce(&light, &server);
            // Finish every link's hello before settling: otherwise a late
            // hello could look like a forwarded request in S9.
            for (events, peer) in links {
                assert!(
                    wait_for(10, || events
                        .try_iter()
                        .any(|e| matches!(e, Event::PeerUp(k) if k == peer))),
                    "the fixture did not connect to {}",
                    peer.short()
                );
            }
            settle(&[&anchor, &server, &third, &light]);
            Shared {
                anchor,
                server,
                third,
                light,
                cx,
                cw,
                package,
                id,
            }
        });
    }
    Fixture(guard)
}

/// S8: Alice's second laptop (light) searches and installs through the
/// server (a full replica), not through her anchor laptop.
#[test]
fn s8_a_light_laptop_searches_and_installs_through_the_server() {
    let f = shared();
    let (light, server, anchor) = (&f.light, &f.server, &f.anchor);
    let x = f.cx[0].author();
    let anchor_ingests = anchor.holdings.ingests.lock().unwrap().len();

    let answer = light.net.query(&query());
    assert_eq!(answer.len(), 2, "{answer:?}");
    assert_eq!(answer[0].0, light.key);
    assert!(answer[0].1.is_empty());
    assert_eq!(
        answer[1].0, server.key,
        "the server answered, not the anchor"
    );
    assert_eq!(answer[1].1, vec![hit_of(x, &f.cx)]);
    // the channel of the hit was pulled, with the same bytes, as asked
    assert_eq!(light.holdings.raws(&x), raws(&f.cx));
    assert!(
        light
            .holdings
            .ingests
            .lock()
            .unwrap()
            .iter()
            .all(|(_, _, why)| *why == Why::Asked)
    );
    // the anchor did not take x on the way
    assert_eq!(
        anchor.holdings.ingests.lock().unwrap().len(),
        anchor_ingests
    );
    assert!(anchor.holdings.raws(&x).is_empty());

    // the package comes from the server, byte for byte
    light.net.need_blob(&f.id, f.package.len() as u64).unwrap();
    assert_eq!(light.holdings.taken_blob(&f.id), Some(f.package.clone()));
    // over the limit: no transfer
    let big = BlobId([1; 32]);
    assert!(matches!(
        light.net.need_blob(&big, 65 << 20),
        Err(NetError::TooLarge { .. })
    ));
    // a package nobody has: unreachable, naming the last machine
    let none = BlobId([2; 32]);
    match light.net.need_blob(&none, 10) {
        Err(NetError::Unreachable { node, cause }) => {
            // the last machine asked is the anchor
            assert_eq!(node, f.anchor.key);
            assert!(cause.contains(&hex(&none.0[..4])), "{cause}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(light.holdings.stored_blobs.lock().unwrap().len(), 1);
    // `need` of a channel the server holds, and of one nobody holds
    light.net.need(&x).unwrap();
    light.net.need(&chan([99u8; 32])).unwrap();
}

/// S9, the query part: a light machine's query reaches the server and no
/// third node; the server answers from its own holder only.
#[test]
fn s9_a_light_query_reaches_the_server_and_no_third_node() {
    let f = shared();
    let (light, server, third) = (&f.light, &f.server, &f.third);
    let w = f.cw[0].author();
    assert!(server.holdings.raws(&w).is_empty());
    // the server reaches the third machine: the third, a full replica, pulled
    // the server's channel
    let x = f.cx[0].author();
    assert!(wait_for(5, || third.holdings.raws(&x) == raws(&f.cx)));
    let before = third.holdings.calls();

    let answer = light.net.query(&query());
    assert_eq!(answer[1].0, server.key);
    assert_eq!(answer[1].1, vec![hit_of(f.cx[0].author(), &f.cx)]);
    assert_eq!(
        third.holdings.calls(),
        before,
        "the third machine was searched or read"
    );
    assert!(light.holdings.raws(&w).is_empty());
}

/// S8, the fallback: with the server off the light laptop asks the anchor
/// laptop.
#[test]
fn s8_without_the_server_the_light_laptop_asks_the_anchor() {
    let (y, z) = ([72u8; 32], [73u8; 32]);
    let (cy, cz) = (chain(y, 2), chain(z, 1));
    let laptop = node(holding(&[(chan(y), cy.clone())], true), None);
    let server = node(holding(&[], false), None);
    let light = node(MemHoldings::empty(), Some(quick()));
    laptop.holdings.set_hits(vec![hit_of(chan(y), &cy)]);
    pair_unsettled(&laptop, &server, Role::Full, false);
    pair_into(&laptop, &light, Role::Light);
    introduce(&light, &server);
    server.net.clone().close();

    let answer = light.net.query(&query());
    assert_eq!(answer.len(), 2, "{answer:?}");
    assert_eq!(answer[1].0, laptop.key);
    assert_eq!(light.holdings.raws(&chan(y)), raws(&cy));
    // `need` is the same
    laptop.holdings.append(cz[0].clone());
    light.net.need(&chan(z)).unwrap();
    assert_eq!(light.holdings.raws(&chan(z)), raws(&cz));
}

/// With no full machine reachable: `NoFullMachine`, the own hits only, and
/// no restore hint.
#[test]
fn ac3_with_no_full_machine_reachable_there_is_none() {
    let z = chan([73u8; 32]);
    let laptop = node(holding(&[], true), None);
    let light = node(MemHoldings::empty(), Some(quick()));
    light
        .holdings
        .set_hits(vec![hit_of(z, &chain([73u8; 32], 1))]);
    pair_into(&laptop, &light, Role::Light);
    laptop.net.clone().close();
    let id = BlobId([4; 32]);
    assert!(matches!(light.net.need(&z), Err(NetError::NoFullMachine)));
    assert!(matches!(
        light.net.need_blob(&id, 10),
        Err(NetError::NoFullMachine)
    ));
    let answer = light.net.query(&query());
    assert_eq!(answer.len(), 1, "own hits only");
    assert_eq!(answer[0].0, light.key);
    assert_eq!(answer[0].1.len(), 1);
    assert!(light.net.own_heads(&z).is_empty());
}

/// With net off: `Off`, the own hits only, no restore hint.
#[test]
fn ac3_with_net_off_need_is_off() {
    let z = chan([73u8; 32]);
    let off = node(MemHoldings::empty(), None);
    off.net
        .set_options(Options {
            on: false,
            ..options(None)
        })
        .unwrap();
    let id = BlobId([4; 32]);
    assert!(matches!(off.net.need(&z), Err(NetError::Off)));
    assert!(matches!(off.net.need_blob(&id, 10), Err(NetError::Off)));
    assert_eq!(off.net.query(&query()).len(), 1);
    assert!(off.net.own_heads(&z).is_empty());
}

/// S12: a restored laptop (the new anchor) learns where its head is from the
/// server as a hint; nothing is stored until it asks.
#[test]
fn s12_a_restored_laptop_learns_its_head_as_a_hint() {
    let alice = [77u8; 32];
    let ca = chain(alice, 4);
    let old = node(holding(&[], true), None);
    let server = node(holding(&[(chan(alice), ca.clone())], false), None);
    pair_unsettled(&old, &server, Role::Full, false);
    // the new laptop: a fresh home, its holder is the anchor and holds nothing
    let new = node(holding(&[], true), None);
    pair_with(&new, &server, Role::Full, true);
    assert_eq!(server.net.devices().unwrap().anchor, new.key);

    let heads = new.net.own_heads(&chan(alice));
    assert_eq!(heads.len(), 1, "{heads:?}");
    assert_eq!(heads[0].0, server.key);
    let head = heads[0].1.unwrap();
    assert_eq!(head.seq, 3);
    assert_eq!(head.id, ca[3].id());
    // nothing was ingested, by the hint or by the sync on connect (AC4)
    assert!(new.holdings.ingests.lock().unwrap().is_empty());
    assert!(new.holdings.raws(&chan(alice)).is_empty());
    // the participant confirms: the entries come
    new.net.need(&chan(alice)).unwrap();
    assert_eq!(new.holdings.raws(&chan(alice)), raws(&ca));
    assert!(
        new.holdings
            .ingests
            .lock()
            .unwrap()
            .iter()
            .all(|(_, _, why)| *why == Why::Asked)
    );
    // asking again changes nothing
    new.net.need(&chan(alice)).unwrap();
    assert_eq!(new.holdings.raws(&chan(alice)), raws(&ca));
}

/// NET-F-21: the anchor keeps what its holder asks for, as a light machine
/// does: it follows the channel it holds and takes no other.
#[test]
fn ac4_the_anchor_keeps_only_what_its_holder_asks_for() {
    let (x, y) = ([78u8; 32], [79u8; 32]);
    let (cx, cy) = (chain(x, 4), chain(y, 2));
    let anchor = node(holding(&[(chan(x), cx[..2].to_vec())], true), None);
    let replica = node(
        holding(&[(chan(x), cx.clone()), (chan(y), cy)], false),
        None,
    );
    pair_into(&anchor, &replica, Role::Full);
    let chains: BTreeMap<_, _> = anchor.holdings.chains();
    assert_eq!(raws(&chains[&chan(x)]), raws(&cx), "the held channel");
    assert!(!chains.contains_key(&chan(y)), "a channel it did not hold");
    assert!(
        anchor
            .holdings
            .ingests
            .lock()
            .unwrap()
            .iter()
            .all(|(c, _, why)| *c == chan(x) && *why == Why::Asked)
    );
}

fn unreachable_causes(events: &Receiver<Event>) -> Vec<String> {
    events
        .try_iter()
        .filter_map(|e| match e {
            Event::Unreachable { cause, .. } => Some(cause),
            _ => None,
        })
        .collect()
}

/// NET-F-32 (a): a wanted package over the limit is skipped with a cause.
#[test]
fn ac12_a_package_over_the_limit_gets_a_cause_and_no_transfer() {
    let limits = Limits {
        blob: 10,
        ..Limits::default()
    };
    let a = node(holding(&[], true), None);
    let b = node(holding(&[], false), Some(limits));
    let id = a.holdings.add_blob(&[3u8; 100]);
    b.holdings.want_blob(id, 100);
    let events = b.net.events();
    pair_into(&a, &b, Role::Full);
    let want = format!(
        "package {} is 100 bytes, over this machine's limit of 10",
        hex(&id.0[..4])
    );
    assert!(wait_for(5, || {
        unreachable_causes(&events).contains(&want)
    }));
    assert!(b.holdings.stored_blobs.lock().unwrap().is_empty());
    // the machine is not dropped
    b.net.heads_of(&a.key, &[]).unwrap();
}

/// NET-F-32 (c): every error that names a node names its short key.
#[test]
fn ac12_every_error_that_names_a_node_has_its_short_key() {
    let n = node_key_of_seed([80; 32]);
    let short = n.short();
    let errors = [
        NetError::Unreachable {
            node: n,
            cause: "x".into(),
        },
        NetError::Refused {
            node: n,
            code: watane_net::Code::NotFound,
        },
        NetError::Timeout { node: n },
        NetError::TooLarge { node: n },
        NetError::BadObject {
            node: n,
            cause: "x".into(),
        },
    ];
    for e in errors {
        assert!(e.to_string().contains(&short), "{e}");
    }
}

/// AC5: a page the holder does not take is an error, not `Ok`.
#[test]
fn ac5_need_is_not_ok_when_the_holder_takes_nothing() {
    let x = [81u8; 32];
    let cx = chain(x, 3);
    let laptop = node(holding(&[(chan(x), cx.clone())], true), None);
    let light = node(MemHoldings::empty(), None);
    pair_into(&laptop, &light, Role::Light);
    light
        .holdings
        .not_wanted
        .store(true, std::sync::atomic::Ordering::SeqCst);
    match light.net.need(&chan(x)) {
        Err(NetError::Unreachable { node, cause }) => {
            assert_eq!(node, laptop.key);
            assert!(cause.contains("not wanted"), "{cause}");
        }
        other => panic!("{other:?}"),
    }
    assert!(light.holdings.raws(&chan(x)).is_empty());
    // a compromised channel is the author's fork, not the machine's lie:
    // an error, and the machine is not dropped
    light
        .holdings
        .not_wanted
        .store(false, std::sync::atomic::Ordering::SeqCst);
    light
        .holdings
        .compromised
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let events = light.net.events();
    match light.net.need(&chan(x)) {
        Err(NetError::Unreachable { node, cause }) => {
            assert_eq!(node, laptop.key);
            assert!(cause.contains("compromised"), "{cause}");
        }
        other => panic!("{other:?}"),
    }
    assert!(
        events
            .try_iter()
            .all(|e| !matches!(e, watane_net::Event::Refused { .. }))
    );
    // the holder takes it again: the same call is Ok, from the same machine
    light
        .holdings
        .compromised
        .store(false, std::sync::atomic::Ordering::SeqCst);
    light.net.need(&chan(x)).unwrap();
    assert_eq!(light.holdings.raws(&chan(x)), raws(&cx));
}

/// S87: a connected full machine without the package reports the same failure.
#[test]
fn ac1_s87_missing_package_emits_exactly_one_matching_event() {
    let full = node(holding(&[], true), None);
    let light = node(MemHoldings::empty(), None);
    pair_into(&full, &light, Role::Light);
    let events = light.net.events();
    let id = BlobId([92; 32]);
    let Err(NetError::Unreachable { node, cause }) = light.net.need_blob(&id, 10) else {
        panic!("expected Unreachable");
    };
    assert_eq!(node, full.key);
    assert!(cause.starts_with(&format!("package {} was not fetched: ", hex(&id.0[..4]))));
    let failures: Vec<_> = events
        .try_iter()
        .filter_map(|event| match event {
            Event::Unreachable { node, cause } => Some((node, cause)),
            _ => None,
        })
        .collect();
    assert_eq!(failures, vec![(node, cause)]);
    assert!(light.holdings.stored_blobs.lock().unwrap().is_empty());
}

/// S87: change the file only after iroh-blobs has referenced it.
#[test]
fn ac2_s87_changed_referenced_file_reports_failure_five_times() {
    let mut unreachable = 0;
    let mut bad_object = 0;
    for _ in 0..5 {
        let full = node(holding(&[], true), None);
        let warm = node(MemHoldings::empty(), None);
        let light = node(MemHoldings::empty(), None);
        let bytes = vec![7; 3000];
        let id = full.holdings.add_blob(&bytes);
        pair_into(&full, &warm, Role::Light);
        pair_into(&full, &light, Role::Light);
        warm.net.need_blob(&id, bytes.len() as u64).unwrap();
        let path = watane_net::Holdings::blob(full.holdings.as_ref(), &id).unwrap();
        let mut changed = bytes.clone();
        changed[0] ^= 1;
        std::fs::write(path, changed).unwrap();
        let events = light.net.events();
        match light.net.need_blob(&id, bytes.len() as u64) {
            Err(NetError::Unreachable { node, cause }) => {
                unreachable += 1;
                assert_eq!(node, full.key);
                assert!(cause.contains("not served"), "{cause}");
                let failures: Vec<_> = events
                    .try_iter()
                    .filter_map(|e| match e {
                        Event::Unreachable { node, cause } => Some((node, cause)),
                        _ => None,
                    })
                    .collect();
                assert_eq!(failures, vec![(node, cause)]);
            }
            Err(NetError::BadObject { node, cause }) => {
                bad_object += 1;
                assert_eq!(node, full.key);
                assert!(cause.contains("does not match its hash"), "{cause}");
                assert!(events.try_iter().any(|e| matches!(e,
                    Event::Refused { node: n, cause: c } if n == node && c == cause)));
            }
            other => panic!("{other:?}"),
        }
        assert!(light.holdings.stored_blobs.lock().unwrap().is_empty());
    }
    eprintln!("S87 changed file: Unreachable={unreachable}, BadObject={bad_object}");
}

#[test]
fn ac3_s87_other_blob_errors_emit_no_problem_event() {
    let full = node(holding(&[], true), None);
    let light = node(MemHoldings::empty(), Some(quick()));
    let id = BlobId([93; 32]);
    let events = light.net.events();
    assert!(matches!(
        light.net.need_blob(&id, 10),
        Err(NetError::NoFullMachine)
    ));
    assert!(events.try_iter().next().is_none());
    pair_into(&full, &light, Role::Light);
    let events = light.net.events();
    assert!(matches!(
        light.net.need_blob(&id, 65 << 20),
        Err(NetError::TooLarge { .. })
    ));
    assert!(events.try_iter().next().is_none());
    full.net.remove_device(&light.key).unwrap();
    let events = light.net.events();
    assert!(matches!(
        light.net.need_blob(&id, 10),
        Err(NetError::Refused {
            code: watane_net::Code::NotAllowed,
            ..
        })
    ));
    assert!(
        events
            .try_iter()
            .all(|e| !matches!(e, Event::Unreachable { .. } | Event::Refused { .. }))
    );
    light
        .net
        .set_options(Options {
            on: false,
            ..options(None)
        })
        .unwrap();
    let events = light.net.events();
    assert!(matches!(light.net.need_blob(&id, 10), Err(NetError::Off)));
    assert!(events.try_iter().next().is_none());
}
