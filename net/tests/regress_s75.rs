#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::time::Duration;
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

/// AC1 and AC2. Addresses from a `devices` answer are held in memory, not in
/// `book.json` (5.10), so AC1 is shown by behaviour: the anchor is closed as
/// soon as `join` returns, so F's address must already be held then.
#[test]
fn ac1_ac2_light_reaches_the_full_machine_with_the_anchor_closed() {
    let (anchor, full, light) = joined_light();
    assert!(!full.net.addrs().is_empty());
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
    // once reached, F's address is kept in book.json (4.5)
    let book: serde_json::Value =
        serde_json::from_slice(&fs::read(light.dir.path().join("net/book.json")).unwrap()).unwrap();
    let stored = book["addrs"][full.key.to_string()].as_array().cloned();
    assert!(
        full.net.addrs().iter().any(|a| stored
            .as_ref()
            .is_some_and(|s| s.contains(&serde_json::json!(a.to_string())))),
        "F reached but its address not in book.json: {book}"
    );
}
