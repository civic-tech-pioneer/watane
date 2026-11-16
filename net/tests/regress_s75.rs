mod support;

use std::{collections::BTreeMap, fs, time::Duration};
use support::*;
use watane_net::{Limits, Query, Role};

/// Loopback disables mDNS; the light machine receives only its pairing code.
fn joined_light() -> (Node, Node, Node) {
    let anchor = node(MemHoldings::custom(BTreeMap::new(), None, true), None);
    let full = node(MemHoldings::empty(), None);
    pair_into(&anchor, &full, Role::Full);
    let light = node(
        MemHoldings::empty(),
        Some(Limits {
            deadline: Duration::from_secs(1),
            connect_timeout: Duration::from_secs(1),
            ..Limits::default()
        }),
    );
    let code = anchor.net.pair_code(Role::Light).unwrap();
    // Unlike pair_into, this adds no addresses and waits only for join.
    try_pair(&anchor, &light, code, false, Some(true)).unwrap();
    (anchor, full, light)
}

#[test]
fn ac1_join_stores_the_full_machines_address_before_returning() {
    let (_anchor, full, light) = joined_light();
    let book: serde_json::Value =
        serde_json::from_slice(&fs::read(light.dir.path().join("net/book.json")).unwrap()).unwrap();
    let stored = &book["addrs"][full.key.to_string()];
    for addr in full.net.addrs() {
        assert!(
            stored
                .as_array()
                .is_some_and(|addrs| addrs.contains(&serde_json::json!(addr.to_string()))),
            "join returned without the full machine's address: {book}"
        );
    }
}

#[test]
fn ac2_light_reaches_the_full_machine_with_the_anchor_closed() {
    let (anchor, full, light) = joined_light();
    anchor.net.close();
    let started = std::time::Instant::now();
    let answers = light.net.query(&Query {
        text: Some("notes".into()),
        publisher: None,
        interface: None,
        origin: None,
        limit: 10,
    });
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        answers.iter().any(|(node, _)| *node == full.key),
        "no answer from the full machine with the anchor off: {answers:?}"
    );
}
