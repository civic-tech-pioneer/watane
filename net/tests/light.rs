//! Stage N4, part 1: a light machine asks a full one (S8, the query part of
//! S9, S12), the anchor that keeps only what its holder asks for (NET-F-21)
//! and the follow-ups of S63 (NET-F-32). Entries are signed by hand.
mod support;
use std::{
    collections::BTreeMap,
    sync::{Arc, mpsc::Receiver},
    time::Duration,
};
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
/// which knows the server and not the third machine.
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

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    static SHARED: std::sync::OnceLock<std::sync::Mutex<Shared>> = std::sync::OnceLock::new();
    SHARED
        .get_or_init(|| {
            let (x, w) = ([71u8; 32], [76u8; 32]);
            let (cx, cw) = (chain(x, 3), chain(w, 2));
            let package = vec![5u8; 3000];
            // neither the anchor nor the light laptop holds x or w
            let anchor = node(holding(&[], true), None);
            let server = node(holding(&[(chan(x), cx.clone())], false), None);
            let third = node(holding(&[(chan(w), cw.clone())], false), None);
            let light = node(MemHoldings::empty(), None);
            let id = server.holdings.add_blob(&package);
            server.holdings.set_hits(vec![hit_of(chan(x), &cx)]);
            third.holdings.set_hits(vec![hit_of(chan(w), &cw)]);
            pair_into(&anchor, &server, Role::Full);
            pair_into(&anchor, &third, Role::Full);
            pair_into(&anchor, &light, Role::Light);
            // the server reaches the third machine (a server that forwarded a
            // query would ask it) and takes none of its channels
            server
                .holdings
                .not_wanted
                .store(true, std::sync::atomic::Ordering::SeqCst);
            introduce(&server, &third);
            introduce(&light, &server);
            settle(&[&anchor, &server, &third, &light]);
            std::sync::Mutex::new(Shared {
                anchor,
                server,
                third,
                light,
                cx,
                cw,
                package,
                id,
            })
        })
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    assert!(light.net.need(&x).is_ok());
    assert!(light.net.need(&chan([99u8; 32])).is_ok());
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
    pair_into(&laptop, &server, Role::Full);
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
    pair_into(&old, &server, Role::Full);
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
    assert!(b.net.heads_of(&a.key, &[]).is_ok());
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
