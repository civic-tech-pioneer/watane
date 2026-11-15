//! Stage N3: kept connections, sync, hints and packages (scenarios S5, S6,
//! S7, S9 and the gate around the other protocols). Entries are signed by
//! hand with the oracle in `support`; nothing here uses `watane_net`'s codecs.
mod support;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use support::*;
use watane_log::{Entry, PublicKey};
use watane_net::{BlobId, Event, Limits, NodeKey, Options, Role, Why};

fn chan(seed: [u8; 32]) -> PublicKey {
    PublicKey::from_bytes(&public_of(seed)).unwrap()
}

fn holding(chains: &[(PublicKey, Vec<Entry>)], anchor: bool) -> Arc<MemHoldings> {
    MemHoldings::custom(chains.iter().cloned().collect(), None, anchor)
}

fn raws(entries: &[Entry]) -> Vec<Vec<u8>> {
    entries.iter().map(|e| e.bytes().to_vec()).collect()
}

fn off() -> Options {
    Options {
        on: false,
        ..options(None)
    }
}

fn pulled(events: &std::sync::mpsc::Receiver<Event>) -> Vec<(PublicKey, u64, NodeKey)> {
    events
        .try_iter()
        .filter_map(|e| match e {
            Event::Pulled { channel, seq, from } => Some((channel, seq, from)),
            _ => None,
        })
        .collect()
}

#[test]
fn ac2_a_paired_machine_pulls_what_it_lacks_within_seconds() {
    let x = [41u8; 32];
    let chain_x = chain(x, 4);
    let a = node(holding(&[(chan(x), chain_x.clone())], true), None);
    let b = node(MemHoldings::empty(), None);
    let events = b.net.events();
    pair_into(&a, &b, Role::Full);
    assert_eq!(b.holdings.raws(&chan(x)), raws(&chain_x));
    assert!(
        b.holdings
            .ingests
            .lock()
            .unwrap()
            .iter()
            .all(|(_, _, why)| *why == Why::Kept)
    );
    assert_eq!(pulled(&events), vec![(chan(x), 3, a.key)]);
}

#[test]
fn ac2_the_timer_syncs_again_without_a_hint() {
    let x = [42u8; 32];
    let full = chain(x, 3);
    let limits = Limits {
        sync_interval: Duration::from_secs(3),
        ..Limits::default()
    };
    let a = node(
        holding(&[(chan(x), full[..2].to_vec())], true),
        Some(limits.clone()),
    );
    let b = node(MemHoldings::empty(), Some(limits));
    pair_into(&a, &b, Role::Full);
    assert_eq!(b.holdings.chains()[&chan(x)].len(), 2);
    // an append nobody told about
    a.holdings.append(full[2].clone());
    assert!(wait_for(10, || b.holdings.chains()[&chan(x)].len() == 3));
}

#[test]
fn ac3_a_light_machine_pulls_only_what_it_holds() {
    let (x, y) = ([43u8; 32], [44u8; 32]);
    let (cx, cy) = (chain(x, 4), chain(y, 2));
    let a = node(holding(&[(chan(x), cx.clone()), (chan(y), cy)], true), None);
    let b = node(holding(&[(chan(x), cx[..1].to_vec())], false), None);
    pair_into(&a, &b, Role::Light);
    let chains = b.holdings.chains();
    assert_eq!(raws(&chains[&chan(x)]), raws(&cx));
    assert!(!chains.contains_key(&chan(y)), "a channel it did not hold");
    assert!(
        b.holdings
            .ingests
            .lock()
            .unwrap()
            .iter()
            .all(|(c, _, why)| *c == chan(x) && *why == Why::Asked)
    );
}

#[test]
fn ac3_not_wanted_stops_the_channel() {
    let x = [45u8; 32];
    let long = chain(x, 5);
    let a = node(holding(&[(chan(x), long)], true), None);
    let holder = MemHoldings::empty();
    holder.not_wanted.store(true, Ordering::SeqCst);
    // pages of two entries
    let paged = Limits {
        max_entries: 2,
        ..Limits::default()
    };
    let b = node(holder, Some(paged));
    let events = b.net.events();
    pair_into(&a, &b, Role::Full);
    // one page was asked; the answer stopped the channel
    assert_eq!(a.holdings.entries_calls.load(Ordering::SeqCst), 1);
    assert_eq!(b.holdings.ingests.lock().unwrap().len(), 1);
    assert!(b.holdings.chains().is_empty());
    assert!(pulled(&events).is_empty());
}

#[test]
fn ac3_a_forked_channel_gets_both_entries() {
    let x = [46u8; 32];
    let base = chain(x, 2);
    let other = Entry::decode(&entry_bytes(
        x,
        1,
        Some(base[0].id().as_bytes()),
        99,
        "note",
        b"the other branch",
    ))
    .unwrap();
    let pair_of = (base[1].clone(), other.clone());
    let a = node(
        MemHoldings::custom(
            BTreeMap::from([(chan(x), base.clone())]),
            Some((chan(x), pair_of)),
            true,
        ),
        None,
    );
    let b = node(holding(&[(chan(x), base.clone())], false), None);
    pair_into(&a, &b, Role::Full);
    let seen: Vec<_> = b.holdings.ingests.lock().unwrap().clone();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(seen.iter().all(|(c, n, _)| *c == chan(x) && *n == 1));
}

/// S5: Alice publishes on the laptop; the server has the entry and the
/// package within seconds. A forged hint changes nothing.
#[test]
fn s5_alice_publishes_and_the_server_has_it_within_seconds() {
    let x = [47u8; 32];
    let entries = chain(x, 3);
    // the laptop is the anchor
    let laptop = node(holding(&[], true), None);
    let server = node(MemHoldings::empty(), None);
    pair_into(&laptop, &server, Role::Full);
    let package: Vec<u8> = (0..300_000u32).map(|i| (i * 31 % 251) as u8).collect();
    let id = laptop.holdings.add_blob(&package);
    server.holdings.want_blob(id, package.len() as u64);
    // a package nobody has: skipped, not stored
    server.holdings.want_blob(BlobId([9; 32]), 10);
    let before = server.holdings.ingested_entries();

    laptop.holdings.append(entries[0].clone());
    laptop.net.told(&chan(x), 0, &entries[0].id());
    assert!(
        wait_for(5, || server.holdings.chains().contains_key(&chan(x))
            && server.holdings.taken_blob(&id).is_some()),
        "the server did not get it in time"
    );
    assert_eq!(server.holdings.raws(&chan(x)), raws(&entries[..1]));
    assert_eq!(server.holdings.taken_blob(&id).unwrap(), package);
    assert_eq!(server.holdings.ingested_entries(), before + 1);
    assert_eq!(
        server.holdings.stored_blobs.lock().unwrap().as_slice(),
        &[id]
    );
    // no temp file is left in the home
    let left: Vec<_> = std::fs::read_dir(server.dir.path().join("net"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(left.is_empty(), "{left:?}");

    // forged hints: a seq above what the laptop has, then a head's seq with
    // an id that is not its head
    let ingests = server.holdings.ingests.lock().unwrap().len();
    laptop
        .net
        .told(&chan(x), 50, &watane_log::EntryId::from_bytes([7u8; 32]));
    assert!(stays(500, || server.holdings.ingests.lock().unwrap().len()
        == ingests));
    laptop.holdings.append(entries[1].clone());
    laptop.net.told(&chan(x), 1, &entries[0].id());
    assert!(stays(500, || server.holdings.ingests.lock().unwrap().len()
        == ingests));
    assert_eq!(server.holdings.chains()[&chan(x)].len(), 1);
    // the true hint still works: the topic was not lost
    laptop.net.told(&chan(x), 1, &entries[1].id());
    assert!(wait_for(5, || server.holdings.chains()[&chan(x)].len() == 2));
}

/// A hint that reaches a machine before the delivering machine holds the
/// head still gets the entry there within seconds: net asks the delivering
/// machine again until its head reaches the hint's seq (5.7). gossip's route
/// is not under a test's control on loopback (direct links always form), so
/// the machine that lacks the head is the teller itself: it tells, then
/// appends 150 ms later. Today's code pulls nothing in the first round.
#[test]
fn ac1_a_hint_that_comes_before_the_head_still_arrives() {
    let x = [60u8; 32];
    let entries = chain(x, 6);
    let laptop = node(holding(&[(chan(x), entries[..1].to_vec())], true), None);
    let light = node(holding(&[(chan(x), entries[..1].to_vec())], false), None);
    pair_into(&laptop, &light, Role::Light);
    assert_eq!(light.holdings.raws(&chan(x)).len(), 1);
    for (i, entry) in entries.iter().enumerate().skip(1) {
        laptop.net.told(&chan(x), i as u64, &entry.id());
        std::thread::sleep(Duration::from_millis(150));
        laptop.holdings.append(entry.clone());
        assert!(
            wait_for(5, || light.holdings.raws(&chan(x)).len() == i + 1),
            "round {i}: the light machine did not get it"
        );
    }
    assert_eq!(light.holdings.raws(&chan(x)), raws(&entries));
}

/// A hint for a seq the delivering machine does not reach: what it has is
/// pulled on the first try, it is asked again (the stop after 20 retries is
/// the unit test in `sync.rs`), and nothing more is ingested.
#[test]
fn ac2_a_hint_for_a_seq_nobody_reaches_pulls_what_there_is_and_asks_again() {
    let x = [61u8; 32];
    let entries = chain(x, 2);
    let laptop = node(holding(&[(chan(x), entries[..1].to_vec())], true), None);
    let server = node(holding(&[], false), None);
    pair_into(&laptop, &server, Role::Full);
    assert_eq!(server.holdings.raws(&chan(x)), raws(&entries[..1]));
    let ingests = server.holdings.ingests.lock().unwrap().len();
    laptop.holdings.append(entries[1].clone());
    laptop
        .net
        .told(&chan(x), 50, &watane_log::EntryId::from_bytes([7u8; 32]));
    assert!(wait_for(1, || server.holdings.raws(&chan(x)).len() == 2));
    // from here on only `heads` calls: one every 250 ms
    let before = laptop.holdings.calls();
    std::thread::sleep(Duration::from_millis(1000));
    let asked = laptop.holdings.calls() - before;
    assert!((3..=25).contains(&asked), "{asked} heads calls");
    assert_eq!(server.holdings.ingests.lock().unwrap().len(), ingests + 1);
    assert_eq!(server.holdings.raws(&chan(x)), raws(&entries));
}

/// S72-5: a hint delivered by a machine that does not hold the channel (a
/// light machine never will) still ends with the head pulled from another
/// trusted machine within seconds (5.7). gossip's route is not under a test's
/// control, so the only machine on the topic that can tell is the light one:
/// the machine that holds the channel is a fake, off the topic, that serves
/// the channel only once the hint is out. Today's code asks the light machine
/// 21 times and drops the hint.
#[test]
fn ac1_a_hint_delivered_by_a_machine_that_lacks_the_channel_pulls_from_another() {
    let x = [62u8; 32];
    let entries = chain(x, 2);
    let (cx, head) = (chan(x), entries[1].id());
    let out = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (heads, answer) = (
        heads_answer_of(&cx, 1, head.as_bytes()),
        entries_answer_of(&raws(&entries[1..])),
    );
    let held = heads_answer_of(&cx, 0, entries[0].id().as_bytes());
    let told = out.clone();
    let hellos = Arc::new(AtomicUsize::new(0));
    let hello = hellos.clone();
    let fake = fake_machine(
        [90u8; 32],
        move |request| match op_of(request) {
            "hello" => {
                hello.fetch_add(1, Ordering::SeqCst);
                Some(hello_answer())
            }
            "heads" if told.load(Ordering::SeqCst) => Some(heads.clone()),
            "heads" => Some(held.clone()),
            "entries" => Some(answer.clone()),
            _ => None,
        },
        None,
    );
    let server = node_trusting(
        holding(&[(cx, entries[..1].to_vec())], true),
        vec![fake.key],
        None,
    );
    server.net.add_addrs(&fake.key, &[fake.addr]);
    let light = node(holding(&[], false), None);
    pair_into(&server, &light, Role::Light);
    // the connect sync with the fake is over before the channel is there
    assert!(wait_for(5, || hellos.load(Ordering::SeqCst) > 0));
    settle(&[&server, &light]);
    out.store(true, Ordering::SeqCst);
    light.net.told(&cx, 1, &head);
    assert!(
        wait_for(4, || server.holdings.raws(&cx) == raws(&entries)),
        "the server did not get the head from the machine that holds it"
    );
}

/// S6: the server is offline while Alice publishes twice; it catches up.
#[test]
fn s6_the_server_catches_up_after_being_offline() {
    let x = [48u8; 32];
    let entries = chain(x, 3);
    let laptop = node(holding(&[(chan(x), entries[..1].to_vec())], true), None);
    let server = node(MemHoldings::empty(), None);
    let down = laptop.net.events();
    pair_into(&laptop, &server, Role::Full);
    assert_eq!(server.holdings.chains()[&chan(x)].len(), 1);
    server.net.set_options(off()).unwrap();
    assert!(wait_for(5, || down
        .try_iter()
        .any(|e| matches!(e, Event::PeerDown(k) if k == server.key))));
    laptop.holdings.append(entries[1].clone());
    laptop.net.told(&chan(x), 1, &entries[1].id());
    laptop.holdings.append(entries[2].clone());
    laptop.net.told(&chan(x), 2, &entries[2].id());
    assert!(stays(300, || server.holdings.chains()[&chan(x)].len() == 1));
    server.net.set_options(options(None)).unwrap();
    assert!(
        wait_for(5, || server.holdings.chains()[&chan(x)].len() == 3),
        "the server did not catch up"
    );
    assert_eq!(server.holdings.raws(&chan(x)), raws(&entries));
}

/// A trusted fake machine that serves what `serve` answers and `blob` for
/// every package request; returns the node that trusts it, the fake, and the
/// number of requests the fake got.
fn with_fake(
    seed: u8,
    holder: Arc<MemHoldings>,
    serve: impl Fn(&str, &[u8]) -> Option<Vec<u8>> + Send + Sync + 'static,
    blob: Option<Vec<u8>>,
) -> (Node, Fake, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let counted = count.clone();
    let fake = fake_machine(
        [seed; 32],
        move |request| {
            counted.fetch_add(1, Ordering::SeqCst);
            let op = op_of(request);
            if op == "hello" {
                return Some(hello_answer());
            }
            serve(op, request)
        },
        blob,
    );
    let n = node_trusting(holder, vec![fake.key], None);
    n.net.add_addrs(&fake.key, &[fake.addr]);
    (n, fake, count)
}

fn refused(events: &std::sync::mpsc::Receiver<Event>, fake: &NodeKey) -> Option<String> {
    let mut found = None;
    wait_for(10, || {
        for e in events.try_iter() {
            if let Event::Refused { node, cause } = e {
                assert_eq!(&node, fake);
                found = Some(cause);
            }
        }
        found.is_some()
    });
    found
}

/// S7: bad objects are refused, the holder sees nothing, the machine is
/// dropped for the session.
#[test]
fn s7_bad_objects_are_refused_and_the_machine_dropped() {
    let x = [49u8; 32];
    let good = chain(x, 3);
    let other = chain([50u8; 32], 3);
    let mut changed = good[0].bytes().to_vec();
    let last = changed.len() - 70;
    changed[last] ^= 1;
    let (cx, head) = (chan(x), good[2].id());
    let heads = heads_answer_of(&cx, 2, head.as_bytes());

    let small = vec![7u8; 100];
    let small_id = BlobId(*blake3::hash(&small).as_bytes());
    let mut truth = 100u64.to_le_bytes().to_vec();
    truth.extend(&small);
    let mut lie = 100u64.to_le_bytes().to_vec();
    lie.extend(vec![8u8; 100]);

    // (name, entries answer, blob bytes, wanted blob size)
    type BadCase = (&'static str, Vec<Vec<u8>>, Option<Vec<u8>>, u64);
    let cases: Vec<BadCase> = vec![
        ("a changed byte", vec![changed], None, 0),
        ("another author", vec![other[0].bytes().to_vec()], None, 0),
        ("bytes that are not the hash", vec![], Some(lie), 100),
        ("more bytes than wanted", vec![], Some(truth), 99),
    ];
    // the cases are independent machines: run them side by side
    let (good, heads) = (&good, &heads);
    std::thread::scope(|scope| {
        for (i, (name, entries, blob, wanted)) in cases.into_iter().enumerate() {
            scope.spawn(move || {
                // the node trusting the fake is the anchor of its own list, which
                // pulls only channels its holder holds (NET-F-21): it holds the first
                // entry of the channel the bad entries are offered for
                let held: Vec<Entry> = if wanted > 0 {
                    vec![]
                } else {
                    good[..1].to_vec()
                };
                let holder = if held.is_empty() {
                    MemHoldings::empty()
                } else {
                    holding(&[(cx, held.clone())], false)
                };
                let answer = entries_answer_of(&entries);
                let heads = heads.clone();
                let (n, fake, count) = with_fake(
                    60 + i as u8,
                    holder.clone(),
                    move |op, _| match op {
                        "heads" => Some(if wanted > 0 {
                            // nothing to pull: the channel is held up to date
                            entries_answer_of(&[])
                        } else {
                            heads.clone()
                        }),
                        "entries" => Some(answer.clone()),
                        _ => None,
                    },
                    blob,
                );
                if wanted > 0 {
                    holder.want_blob(small_id, wanted);
                }
                let events = n.net.events();
                // the node dials the fake from the next tick on; it was told the address
                let cause = refused(&events, &fake.key);
                assert!(cause.is_some(), "{name}: not refused");
                let cause = cause.unwrap();
                // NET-F-32: the cause names the machine by its short key
                assert!(cause.contains(&fake.key.short()), "{name}: {cause}");
                if name == "bytes that are not the hash" {
                    // in watane's words, not iroh's
                    assert_eq!(
                        cause,
                        format!(
                            "package {} from {} does not match its hash",
                            hex(&small_id.0[..4]),
                            fake.key.short()
                        ),
                    );
                }
                assert!(
                    raws(&holder.chains().get(&cx).cloned().unwrap_or_default()) == raws(&held)
                        && holder.ingests.lock().unwrap().is_empty(),
                    "{name}: the holder was given entries"
                );
                assert!(holder.stored_blobs.lock().unwrap().is_empty(), "{name}");
                // dropped: no further requests reach the fake
                // (a redial after the shortest backoff, 500 ms plus a tick, would show)
                std::thread::sleep(Duration::from_millis(150));
                let seen = count.load(Ordering::SeqCst);
                assert!(
                    stays(1100, || count.load(Ordering::SeqCst) == seen),
                    "{name}: still synced"
                );
                assert!(events.try_iter().all(|e| !matches!(e, Event::PeerUp(_))));
            });
        }
    });
}

/// S9 (a): entries nobody asked for are not stored.
#[test]
fn s9_unasked_entries_are_not_stored_and_requests_are_not_forwarded() {
    let x = [51u8; 32];
    let good = chain(x, 6);
    let cx = chan(x);
    let other = chain([52u8; 32], 6);
    // the node holds seq 0..=1 and is told the head is seq 3: it asks for
    // 2..=3 and gets (more than limit / seq before from / another channel)
    let bad_answers: Vec<(&str, Vec<Vec<u8>>)> = vec![
        ("more than limit", raws(&good[2..6])),
        ("before from", raws(&good[1..3])),
        ("another channel", raws(&other[2..4])),
    ];
    for (i, (name, answer)) in bad_answers.into_iter().enumerate() {
        let holder = holding(&[(cx, good[..2].to_vec())], false);
        let heads = heads_answer_of(&cx, 3, good[3].id().as_bytes());
        let answer = entries_answer_of(&answer);
        let (n, fake, _) = with_fake(
            70 + i as u8,
            holder.clone(),
            move |op, _| match op {
                "heads" => Some(heads.clone()),
                "entries" => Some(answer.clone()),
                _ => None,
            },
            None,
        );
        let events = n.net.events();
        assert!(refused(&events, &fake.key).is_some(), "{name}");
        assert!(holder.ingests.lock().unwrap().is_empty(), "{name}");
        assert_eq!(holder.chains()[&cx].len(), 2, "{name}");
    }
}

/// S9 (b): a trusted caller asking for a channel the server lacks gets the
/// server's own answer; no third node is contacted. (A light machine's
/// `query` reaching the server and no third node is `tests/light.rs`.)
#[test]
fn s9_requests_are_answered_from_the_holder_only() {
    let w = [53u8; 32];
    // the server is the anchor of its list; the third node is a full machine
    // that holds a channel the server lacks
    let server = node(holding(&[], true), None);
    let third = node(holding(&[(chan(w), chain(w, 2))], false), None);
    let caller = node(MemHoldings::empty(), None);
    // the anchor keeps only what its participant chose: nothing here
    server.holdings.not_wanted.store(true, Ordering::SeqCst);
    pair_into(&server, &third, Role::Full);
    pair_into(&server, &caller, Role::Light);
    settle(&[&server, &third, &caller]);
    let before = third.holdings.calls();
    let heads = caller.net.heads_of(&server.key, &[chan(w)]).unwrap();
    assert_eq!(heads, vec![(chan(w), None)]);
    assert!(
        caller
            .net
            .entries_of(&server.key, &chan(w), 0, 2)
            .unwrap()
            .is_empty()
    );
    assert!(
        stays(300, || third.holdings.calls() == before),
        "a third node was asked"
    );
}

/// AC6: an unknown node's blob download and gossip join fail, and its
/// gossip message never reaches a hint; a message that is not a hint never
/// reaches the holder either.
#[test]
fn ac6_unknown_nodes_get_nothing_on_blobs_and_gossip() {
    use iroh::address_lookup::memory::MemoryLookup;
    use iroh_gossip::{Gossip, TopicId};
    let x = [54u8; 32];
    // a node with a list (so it is on its topic) that trusts another key
    let target = node_trusting(
        holding(&[(chan(x), chain(x, 2))], true),
        vec![node_key_of_seed([82u8; 32])],
        None,
    );
    let topic = {
        let mut input = b"watane/own/1\0".to_vec();
        input.extend(target.key.as_bytes());
        TopicId::from_bytes(*blake3::hash(&input).as_bytes())
    };
    let package = vec![5u8; 1000];
    let id = target.holdings.add_blob(&package);
    let calls = target.holdings.calls();
    let rt = runtime();
    rt.block_on(async {
        let ep = builder([80u8; 32]).bind().await.unwrap();
        let lookup = MemoryLookup::new();
        ep.address_lookup().unwrap().add(lookup.clone());
        let to = iroh::EndpointAddr::from_parts(
            iroh::EndpointId::from_bytes(target.key.as_bytes()).unwrap(),
            target.net.addrs().into_iter().map(iroh::TransportAddr::Ip),
        );
        lookup.add_endpoint_info(to.clone());
        // blobs
        let conn = ep.connect(to.clone(), iroh_blobs::ALPN).await.unwrap();
        let got =
            iroh_blobs::get::request::get_blob(conn, iroh_blobs::Hash::from_bytes(id.0)).await;
        assert!(got.is_err(), "an unknown node downloaded a package");
        // gossip: the gate closes the connection before iroh-gossip sees it
        let conn = ep.connect(to.clone(), iroh_gossip::ALPN).await.unwrap();
        let closed = tokio::time::timeout(Duration::from_secs(3), conn.closed())
            .await
            .expect("the gossip connection of an unknown node was kept");
        assert!(
            matches!(
                &closed,
                iroh::endpoint::ConnectionError::ApplicationClosed(c)
                    if u64::from(c.error_code) == 403
            ),
            "{closed:?}"
        );
        // and a join fails, even where the target could reach it back
        target
            .net
            .add_addrs(&node_key_of_seed([80u8; 32]), &[ep.bound_sockets()[0]]);
        let gossip = Gossip::builder().spawn(ep.clone());
        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();
        let mut sub = gossip.subscribe(topic, vec![to.id]).await.unwrap();
        let joined = tokio::time::timeout(Duration::from_secs(3), sub.joined()).await;
        assert!(
            joined.is_err() || joined.unwrap().is_err(),
            "joined the topic"
        );
        // a true hint for a seq above the target's head, sent anyway
        let mut hint = vec![0x83, 0x58, 0x20];
        hint.extend(chan(x).as_bytes());
        hint.extend([0x05, 0x58, 0x20]);
        hint.extend([1u8; 32]);
        let (sender, _rx) = sub.split();
        let _ = sender.broadcast(hint.into()).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = router.shutdown().await;
    });
    assert_eq!(target.holdings.calls(), calls, "a hint reached the holder");
}

/// AC4/AC10: a message on the topic that is not exactly a hint is dropped.
#[test]
fn ac4_a_gossip_message_that_is_not_a_hint_reaches_nobody() {
    use iroh::address_lookup::memory::MemoryLookup;
    use iroh_gossip::Gossip;
    let seed = [81u8; 32];
    let rawkey = node_key_of_seed(seed);
    let x = [55u8; 32];
    let target = node_trusting(
        holding(&[(chan(x), chain(x, 2))], false),
        vec![rawkey],
        None,
    );
    let anchor = target.key;
    let topic = {
        let mut input = b"watane/own/1\0".to_vec();
        input.extend(anchor.as_bytes());
        iroh_gossip::TopicId::from_bytes(*blake3::hash(&input).as_bytes())
    };
    let rt = runtime();
    let calls = target.holdings.calls();
    rt.block_on(async {
        let ep = builder(seed).bind().await.unwrap();
        let lookup = MemoryLookup::new();
        ep.address_lookup().unwrap().add(lookup.clone());
        let to = iroh::EndpointAddr::from_parts(
            iroh::EndpointId::from_bytes(anchor.as_bytes()).unwrap(),
            target.net.addrs().into_iter().map(iroh::TransportAddr::Ip),
        );
        lookup.add_endpoint_info(to.clone());
        let gossip = Gossip::builder().spawn(ep.clone());
        let router = iroh::protocol::Router::builder(ep.clone())
            .accept(iroh_gossip::ALPN, gossip.clone())
            .spawn();
        // the target knows where the trusted machine is, so the join
        // completes both ways
        target.net.add_addrs(&rawkey, &[ep.bound_sockets()[0]]);
        let mut sub = gossip.subscribe(topic, vec![to.id]).await.unwrap();
        // a trusted machine is let onto the topic
        tokio::time::timeout(Duration::from_secs(5), sub.joined())
            .await
            .expect("joined in time")
            .unwrap();
        let (sender, _rx) = sub.split();
        // not hints: too short, trailing byte, a text, a long seq form
        let key = chan(x);
        let mut good = vec![0x83, 0x58, 0x20];
        good.extend(key.as_bytes());
        good.extend([0x05, 0x58, 0x20]);
        good.extend([1u8; 32]);
        let mut trailing = good.clone();
        trailing.push(0);
        let junk: Vec<Vec<u8>> = vec![
            good[..good.len() - 1].to_vec(),
            trailing,
            b"hello".to_vec(),
            vec![0x83, 0x00, 0x00, 0x00],
        ];
        for _ in 0..15 {
            for m in &junk {
                let _ = sender.broadcast(m.clone().into()).await;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        assert_eq!(target.holdings.calls(), calls, "junk reached the holder");
        // the control: the true hint on the same topic does reach the holder
        let _ = sender.broadcast(good.clone().into()).await;
        for _ in 0..50 {
            if target.holdings.calls() > calls {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(target.holdings.calls() > calls, "the topic never delivered");
        let _ = router.shutdown().await;
    });
}

/// Every file under `dir`, with its size.
fn files_under(dir: &std::path::Path) -> Vec<(std::path::PathBuf, u64)> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let meta = e.metadata().unwrap();
        if meta.is_dir() {
            out.extend(files_under(&e.path()));
        } else {
            out.push((e.path(), meta.len()));
        }
    }
    out
}

/// AC5: a trusted machine fetches a package the holder gives; the store
/// keeps a reference, not a copy; once the holder no longer gives it, it is
/// not served.
#[test]
fn ac5_a_package_is_served_by_reference_and_not_once_the_holder_drops_it() {
    use iroh::address_lookup::memory::MemoryLookup;
    let seed = [83u8; 32];
    let laptop = node_trusting(holding(&[], true), vec![node_key_of_seed(seed)], None);
    let package: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 253) as u8).collect();
    let id = laptop.holdings.add_blob(&package);
    let hash = iroh_blobs::Hash::from_bytes(id.0);
    let rt = runtime();
    rt.block_on(async {
        let ep = builder(seed).bind().await.unwrap();
        let lookup = MemoryLookup::new();
        ep.address_lookup().unwrap().add(lookup.clone());
        let to = iroh::EndpointAddr::from_parts(
            iroh::EndpointId::from_bytes(laptop.key.as_bytes()).unwrap(),
            laptop.net.addrs().into_iter().map(iroh::TransportAddr::Ip),
        );
        lookup.add_endpoint_info(to.clone());
        let conn = ep.connect(to.clone(), iroh_blobs::ALPN).await.unwrap();
        let got = iroh_blobs::get::request::get_blob(conn.clone(), hash)
            .bytes()
            .await
            .expect("a trusted machine fetches a package the holder gives");
        assert_eq!(got.as_ref(), package.as_slice());
        let blobs = laptop.dir.path().join("net").join("blobs");
        // the store's data files: none (its database holds the outboard and
        // the reference to the holder's file)
        let data: Vec<_> = files_under(&blobs)
            .into_iter()
            .filter(|(p, _)| p.extension().is_some_and(|x| x == "data"))
            .collect();
        assert!(data.is_empty(), "a second copy: {data:?}");
        laptop.holdings.remove_blob(&id);
        let again = iroh_blobs::get::request::get_blob(conn, hash).bytes().await;
        assert!(again.is_err(), "served after the holder dropped it");
    });
}

/// S68-1 AC2: turning net off (`set_options`) and `close` return within a
/// second with kept connections open, and the other side sees them end.
#[test]
fn ac2_close_and_turn_off_are_fast_with_kept_connections_open() {
    let x = [61u8; 32];
    let entries = chain(x, 2);
    let a = node(holding(&[(chan(x), entries.clone())], true), None);
    let b = node(MemHoldings::empty(), None);
    let c = node(MemHoldings::empty(), None);
    let (ea, ec) = (a.net.events(), c.net.events());
    pair_into(&a, &b, Role::Full);
    pair_into(&a, &c, Role::Full);
    assert_eq!(b.holdings.raws(&chan(x)), raws(&entries));
    assert!(wait_for(5, || c.holdings.raws(&chan(x)) == raws(&entries)));
    let bound = Duration::from_secs(1);

    let t = std::time::Instant::now();
    b.net.set_options(off()).unwrap();
    let turned_off = t.elapsed();
    assert!(turned_off < bound, "set_options(off) took {turned_off:?}");
    assert!(b.net.addrs().is_empty());
    assert!(wait_for(5, || ea
        .try_iter()
        .any(|e| matches!(e, Event::PeerDown(k) if k == b.key))));

    // A still keeps its connection to C open
    let t = std::time::Instant::now();
    a.net.close();
    let closed = t.elapsed();
    assert!(closed < bound, "close took {closed:?}");
    assert!(wait_for(5, || ec
        .try_iter()
        .any(|e| matches!(e, Event::PeerDown(k) if k == a.key))));
}
