#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use support::*;
use watane_net::{Class, DeviceList, Event, Hint, Limits, NetError, NodeKey, Role};

fn sorted(mut devices: Vec<(NodeKey, Role)>) -> Vec<(NodeKey, Role)> {
    devices.sort_by_key(|(k, _)| *k.as_bytes());
    devices
}

fn seed_of(n: &Node) -> [u8; 32] {
    fs::read(n.dir.path().join("net/node.key"))
        .unwrap()
        .try_into()
        .unwrap()
}

#[test]
fn s2_server_joins_with_a_code_and_alice_confirms() {
    let fx = fixture();
    let laptop = node(MemHoldings::anchor(fx), None);
    let server = node(MemHoldings::new(fx), None);
    let events = laptop.net.events();

    let code = laptop.net.pair_code(Role::Full).unwrap();
    assert_eq!(code.anchor, laptop.key);
    let hints: Vec<_> = laptop.net.addrs().into_iter().map(Hint::Addr).collect();
    assert!(!hints.is_empty());
    assert_eq!(code.hints, hints);
    // with no list yet, the laptop signed one of itself only, as full
    let first = laptop.net.devices().unwrap();
    assert_eq!(
        (first.seq, first.devices),
        (1, vec![(laptop.key, Role::Full)])
    );
    assert!(server.net.devices().is_none());

    let net = server.net.clone();
    let text = code.to_string();
    let thread = std::thread::spawn(move || net.join(&text.parse().unwrap(), false));
    let asked = loop {
        match events.recv_timeout(Duration::from_secs(10)).unwrap() {
            Event::JoinAsked { node, role } => break (node, role),
            _ => continue,
        }
    };
    assert_eq!(asked, (server.key, Role::Full));
    laptop.net.answer_join(&server.key, true);
    let joined = thread.join().unwrap().unwrap();

    let expected = DeviceList {
        anchor: laptop.key,
        seq: 2,
        time: joined.time,
        expires_days: 0,
        devices: sorted(vec![(laptop.key, Role::Full), (server.key, Role::Full)]),
    };
    assert_eq!(joined, expected);
    assert_eq!(laptop.net.devices().unwrap(), expected);
    assert_eq!(server.net.devices().unwrap(), expected);
    assert_eq!(laptop.net.class_of(&server.key), Class::Trusted);
    assert_eq!(server.net.class_of(&laptop.key), Class::Trusted);
    assert!(wait_for(5, || {
        events
            .try_iter()
            .any(|e| matches!(e, Event::DevicesChanged(l) if l.seq == 2))
    }));

    // each reads the other's channels
    laptop.net.add_addrs(&server.key, &server.net.addrs());
    let heads = server.net.heads_of(&laptop.key, &[fx.c1]).unwrap();
    assert_eq!(heads[0].0, fx.c1);
    assert!(
        laptop.net.heads_of(&server.key, &[fx.c1]).unwrap()[0]
            .1
            .is_some()
    );
    // the code is spent, and the files are the list as signed
    assert_eq!(
        fs::read(server.dir.path().join("net/devices")).unwrap(),
        fs::read(laptop.dir.path().join("net/devices")).unwrap()
    );
    let anchor_text = fs::read_to_string(server.dir.path().join("net/anchor")).unwrap();
    assert_eq!(anchor_text.trim(), laptop.key.to_string());
}

#[test]
fn s3_bad_codes_and_a_no_change_nothing() {
    let fx = fixture();
    // the joiner is one unknown key that dials several times
    let quick = Limits {
        pair_deadline: Duration::from_millis(600),
        ..roomy_limits()
    };
    let laptop = node(MemHoldings::anchor(fx), Some(quick));
    let joiner = node(MemHoldings::new(fx), None);
    let code = laptop.net.pair_code(Role::Full).unwrap();
    let before = laptop.net.devices().unwrap();
    let unchanged = |what: &str| {
        assert_eq!(laptop.net.devices().unwrap(), before, "{what}");
        assert!(joiner.net.devices().is_none(), "{what}");
        // a join that failed keeps no address of the anchor (5.5)
        let book = fs::read(joiner.dir.path().join("net/book.json")).unwrap();
        let book: serde_json::Value = serde_json::from_slice(&book).unwrap();
        assert!(
            book["addrs"].as_object().is_none_or(|m| m.is_empty()),
            "{what}: {book}"
        );
    };

    // the code is spent on arrival: a second use while the participant has
    // not answered yet is refused
    let events = laptop.net.events();
    let net = joiner.net.clone();
    let first = code.clone();
    let pending = std::thread::spawn(move || net.join(&first, false));
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(10)).unwrap(),
        Event::JoinAsked { .. }
    ));
    let other = node(MemHoldings::new(fx), None);
    let r = other.net.join(&code, false);
    assert!(matches!(r, Err(NetError::CodeSpent)), "{r:?}");
    assert!(other.net.devices().is_none());
    laptop.net.answer_join(&joiner.key, false);
    let r = pending.join().unwrap();
    assert!(matches!(r, Err(NetError::Declined)), "{r:?}");
    unchanged("no");

    // a no, and then a second use of the spent code
    let code = laptop.net.pair_code(Role::Full).unwrap();
    let r = try_pair(&laptop, &joiner, code.clone(), false, Some(false));
    assert!(matches!(r, Err(NetError::Declined)), "{r:?}");
    unchanged("no");
    let r = try_pair(&laptop, &joiner, code.clone(), false, None);
    assert!(matches!(r, Err(NetError::CodeSpent)), "{r:?}");
    unchanged("spent");

    // a wrong secret
    let fresh = laptop.net.pair_code(Role::Full).unwrap();
    let mut wrong = fresh.clone();
    wrong.secret[0] ^= 1;
    let r = try_pair(&laptop, &joiner, wrong, false, None);
    assert!(matches!(r, Err(NetError::CodeSpent)), "{r:?}");
    unchanged("wrong");

    // an expired code
    // the code lives as long as the (shortened) pair deadline
    std::thread::sleep(Duration::from_millis(650));
    let r = try_pair(&laptop, &joiner, fresh, false, None);
    assert!(matches!(r, Err(NetError::CodeSpent)), "{r:?}");
    unchanged("expired");

    // nobody answers within the deadline
    let silent = laptop.net.pair_code(Role::Full).unwrap();
    let started = std::time::Instant::now();
    let r = try_pair(&laptop, &joiner, silent, false, None);
    assert!(matches!(r, Err(NetError::Declined)), "{r:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
    unchanged("no answer");

    // a replica gives no code
    assert!(matches!(
        joiner.net.pair_code(Role::Full),
        Err(NetError::NotAnchor)
    ));
}

#[test]
fn s3_a_machine_with_an_anchor_joins_another_only_with_replace() {
    let fx = fixture();
    let one = node(MemHoldings::anchor(fx), None);
    let two = node(MemHoldings::anchor(fx), None);
    let server = node(MemHoldings::new(fx), None);
    pair_into(&one, &server, Role::Full);
    let kept = server.net.devices().unwrap();

    let code = two.net.pair_code(Role::Light).unwrap();
    // refused before any network contact: the code has no listener to ask
    let mut nowhere = code.clone();
    nowhere.hints.clear();
    nowhere.anchor = node_key_of_seed([77; 32]);
    assert!(matches!(
        server.net.join(&nowhere, false),
        Err(NetError::HasAnchor)
    ));
    assert!(matches!(
        server.net.join(&code, false),
        Err(NetError::HasAnchor)
    ));
    assert_eq!(server.net.devices().unwrap(), kept);

    let list = try_pair(&two, &server, code, true, Some(true)).unwrap();
    assert_eq!(list.anchor, two.key);
    assert_eq!(server.net.devices().unwrap(), list);
    assert_eq!(server.net.class_of(&two.key), Class::Trusted);
    assert_eq!(server.net.class_of(&one.key), Class::Unknown);
}

fn answer_err(answer: &[u8]) -> (String, String) {
    err_of(answer).unwrap_or_else(|| panic!("not an err answer: {answer:02x?}"))
}

#[test]
fn s4_an_unknown_node_gets_hello_and_nothing_else() {
    use cbor::*;
    let fx = fixture();
    let a = node(MemHoldings::anchor(fx), None);
    // Pairing and the raw unknown calls share a byte bucket. QUIC overhead can
    // drain the default 64 KiB/s bucket, so give this fixture headroom for op handling.
    let limits = Limits {
        unknown_bytes_per_sec: 16 << 20,
        ..Limits::default()
    };
    let b = node(MemHoldings::new(fx), Some(limits));
    pair_into(&a, &b, Role::Full);
    let seed = [31u8; 32];
    let unknown = node_key_of_seed(seed);
    let addr = b.net.addrs()[0];
    let before = b.holdings.calls();

    // Check the made-up op with a fresh key before the other raw requests.
    let request = array(&[text("frobnicate")]);
    let (code, _) = answer_err(&raw_call([39u8; 32], &b.key, addr, &request));
    assert_eq!(code, "unknown_op");

    // hello: no devices_seq, and no participant key anywhere in the bytes
    let answer = raw_call(seed, &b.key, addr, &hello_request());
    assert_eq!(*answer.last().unwrap(), NULL);
    let mut keys: Vec<[u8; 32]> = fx.chains.keys().map(|k| *k.as_bytes()).collect();
    keys.push(*fx.c1200.as_bytes());
    keys.push(*fx.cbig.as_bytes());
    for key in keys {
        assert!(!answer.windows(32).any(|w| w == key), "a key is in hello");
    }
    // a trusted caller sees the list's seq
    let seq = b.net.devices().unwrap().seq;
    assert_eq!(b.net.hello(&a.key).unwrap().devices_seq, Some(seq));

    let c1 = bytes(fx.c1.as_bytes());
    let trusted_key = bytes(a.key.as_bytes());
    let requests = [
        array(&[text("heads"), array(&[])]),
        array(&[text("heads"), array(std::slice::from_ref(&trusted_key))]),
        array(&[text("entries"), c1.clone(), uint(0), uint(5)]),
        array(&[text("entries"), trusted_key.clone(), uint(0), uint(5)]),
        array(&[text("fork"), c1.clone()]),
        array(&[text("devices"), uint(0)]),
        // a well-formed query: refused before the holder is called
        array(&[
            text("query"),
            array(&[text("x"), vec![NULL], vec![NULL], vec![NULL], uint(10)]),
        ]),
    ];
    // a new key for each call: a raw client may leave its connection to idle
    // out, and the connection budget of one unknown key is small
    for (i, request) in requests.iter().enumerate() {
        let seed = [40 + i as u8; 32];
        let who = node_key_of_seed(seed);
        let (code, message) = answer_err(&raw_call(seed, &b.key, addr, request));
        assert_eq!(code, "not_allowed");
        assert!(message.contains(&who.short()), "{message}");
        assert_eq!(b.net.class_of(&who), Class::Unknown);
    }
    assert_eq!(b.net.class_of(&unknown), Class::Unknown);
    assert_eq!(b.holdings.calls(), before);
}

#[test]
fn s4_devices_answers_a_trusted_caller_only_what_is_newer() {
    use cbor::*;
    let fx = fixture();
    let seed = [32u8; 32];
    let server = node_trusting(MemHoldings::new(fx), vec![node_key_of_seed(seed)], None);
    let addr = server.net.addrs()[0];
    let list = fs::read(server.dir.path().join("net/devices")).unwrap();
    let ask = |seq| {
        raw_call(
            seed,
            &server.key,
            addr,
            &array(&[text("devices"), uint(seq)]),
        )
    };
    let ok = array(&[text("ok")]);
    assert_eq!(
        ask(0),
        concat(&[ok.clone(), array(&[bytes(&list), array(&[])])])
    );
    assert_eq!(ask(1), concat(&[ok, array(&[vec![NULL], array(&[])])]));
}

fn three(fx: &Fixture) -> (Node, Node, Node) {
    let laptop = node(MemHoldings::anchor(fx), None);
    let server = node(MemHoldings::new(fx), None);
    let second = node(MemHoldings::new(fx), None);
    pair_into(&laptop, &server, Role::Full);
    pair_into(&laptop, &second, Role::Light);
    let both = laptop.net.devices().unwrap();
    assert_eq!(both.devices.len(), 3);
    // the server learned the second laptop's list by pull
    assert!(wait_for(5, || server.net.devices().unwrap().seq == both.seq));
    for (x, y) in [(&server, &second), (&second, &server)] {
        x.net.add_addrs(&y.key, &y.net.addrs());
    }
    (laptop, server, second)
}

#[test]
fn s10_removed_or_expired_machines_lose_trust() {
    let fx = fixture();
    let (laptop, server, second) = three(fx);
    second.net.heads_of(&server.key, &[fx.c1]).unwrap();
    let seq = laptop.net.devices().unwrap().seq;

    // a role and an expiry reach the server
    let list = laptop.net.set_role(&second.key, Role::Full).unwrap();
    assert_eq!(list.seq, seq + 1);
    assert!(wait_for(5, || server.net.devices().unwrap() == list));
    let list = laptop.net.set_expiry(30).unwrap();
    assert_eq!((list.seq, list.expires_days), (seq + 2, 30));
    assert!(wait_for(5, || server.net.devices().unwrap() == list));
    let list = laptop.net.set_expiry(0).unwrap();
    assert!(wait_for(5, || server.net.devices().unwrap() == list));

    // removal
    let list = laptop.net.remove_device(&second.key).unwrap();
    assert_eq!(list.seq, seq + 4);
    assert!(list.devices.iter().all(|(k, _)| *k != second.key));
    assert!(wait_for(5, || server.net.devices().unwrap().seq == list.seq));
    assert_eq!(server.net.class_of(&second.key), Class::Unknown);
    match second.net.heads_of(&server.key, &[fx.c1]) {
        Err(NetError::Refused { code, .. }) => assert_eq!(code.as_str(), "not_allowed"),
        other => panic!("{other:?}"),
    }
    // only the anchor changes the list
    assert!(matches!(
        server.net.remove_device(&second.key),
        Err(NetError::NotAnchor)
    ));
    assert!(matches!(
        server.net.set_role(&laptop.key, Role::Light),
        Err(NetError::NotAnchor)
    ));
    assert!(matches!(server.net.set_expiry(1), Err(NetError::NotAnchor)));
}

#[test]
fn s10_a_machine_that_has_not_seen_the_anchor_stops_trusting_the_others() {
    use cbor::*;
    const DAY: u64 = 86_400_000;
    let fx = fixture();
    let laptop = node(MemHoldings::anchor(fx), None);
    let second_seed = [42u8; 32];
    let second = node_key_of_seed(second_seed);

    // the server's home as it was two days ago, with an expiry of one day
    let dir = tempfile::tempdir().unwrap();
    let net_dir = dir.path().join("net");
    let holdings = MemHoldings::new(fx);
    let key = watane_net::Net::open(
        &net_dir,
        Arc::<MemHoldings>::clone(&holdings),
        Default::default(),
    )
    .unwrap()
    .node_key();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let list = DeviceList {
        anchor: laptop.key,
        seq: 3,
        time: now - 2 * DAY,
        expires_days: 1,
        devices: sorted(vec![
            (laptop.key, Role::Full),
            (key, Role::Full),
            (second, Role::Light),
        ]),
    };
    fs::write(
        net_dir.join("devices"),
        list.sign(&seed_of(&laptop)).unwrap(),
    )
    .unwrap();
    fs::write(net_dir.join("anchor"), laptop.key.to_string()).unwrap();
    fs::write(
        net_dir.join("book.json"),
        format!(
            r#"{{"version":1,"on":false,"dht":false,"relays":[],"addrs":{{}},"anchor_seen":{}}}"#,
            now - 2 * DAY
        ),
    )
    .unwrap();
    let net = open_on(&net_dir, holdings, options(None));
    let addr = net.addrs()[0];

    assert_eq!(net.class_of(&second), Class::Unknown);
    let heads = array(&[text("heads"), array(&[])]);
    let (code, _) = answer_err(&raw_call(second_seed, &key, addr, &heads));
    assert_eq!(code, "not_allowed");

    // the anchor calls: the expiry starts again, either way (ND-17)
    laptop.net.add_addrs(&key, &[addr]);
    laptop.net.hello(&key).unwrap();
    assert_eq!(net.class_of(&second), Class::Trusted);
    let answer = raw_call(second_seed, &key, addr, &heads);
    assert!(err_of(&answer).is_none());
    let book: serde_json::Value =
        serde_json::from_slice(&fs::read(net_dir.join("book.json")).unwrap()).unwrap();
    assert!(book["anchor_seen"].as_u64().unwrap() >= now);
}

/// S72-4 AC1, AC2: a join keeps the address at which the anchor answered,
/// so a reopened home reaches it with no hint, mDNS, relay or DHT.
#[test]
fn ac1_a_joined_machine_reaches_the_anchor_after_a_reopen() {
    let fx = fixture();
    let anchor = node(MemHoldings::anchor(fx), None);
    let joiner = node(MemHoldings::new(fx), None);
    let code = anchor.net.pair_code(Role::Full).unwrap();
    try_pair(&anchor, &joiner, code, false, Some(true)).unwrap();

    let Node {
        dir, net, holdings, ..
    } = joiner;
    net.close();
    // only the anchor's address is stored, at most four (AC2)
    let book: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("net/book.json")).unwrap()).unwrap();
    let addrs = book["addrs"].as_object().unwrap();
    assert_eq!(addrs.keys().collect::<Vec<_>>(), [&anchor.key.to_string()]);
    let stored = addrs[&anchor.key.to_string()].as_array().unwrap();
    assert!((1..=4).contains(&stored.len()), "{stored:?}");
    let answered: std::net::SocketAddr = stored[0].as_str().unwrap().parse().unwrap();
    assert!(anchor.net.addrs().contains(&answered), "{answered}");

    let net = open_on(&dir.path().join("net"), holdings, options(None));
    let hello = net.hello(&anchor.key).unwrap();
    assert!(hello.devices_seq.is_some());
}

/// S72-4 AC3: `options` is what open was given merged with `book.json`, then
/// what `set_options` stored.
#[test]
fn ac3_options_gives_the_switches_as_last_stored() {
    let fx = fixture();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("net");
    let h = MemHoldings::new(fx);
    let off = watane_net::Net::open(
        &home,
        Arc::<MemHoldings>::clone(&h),
        watane_net::Options::default(),
    )
    .unwrap();
    assert!(!off.options().on);
    let relay: url::Url = "http://127.0.0.1:9/".parse().unwrap();
    let wanted = watane_net::Options {
        relays: vec![relay.clone()],
        dht: false,
        ..options(None)
    };
    off.set_options(wanted).unwrap();
    let now = off.options();
    assert!(now.on);
    assert_eq!(now.relays, vec![relay.clone()]);
    off.close();

    // reopened: the book's relays and switches; `on` false at open wins
    let again = watane_net::Net::open(&home, Arc::<MemHoldings>::clone(&h), options(None)).unwrap();
    assert_eq!(again.options().relays, vec![relay.clone()]);
    assert!(again.options().on);
    again.close();
    let refused = watane_net::Net::open(
        &home,
        h,
        watane_net::Options {
            on: false,
            ..options(None)
        },
    )
    .unwrap();
    assert!(!refused.options().on);
    assert_eq!(refused.options().relays, vec![relay]);
}
