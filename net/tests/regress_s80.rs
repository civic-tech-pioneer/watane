#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;

use std::collections::BTreeMap;
use std::time::Duration;
use support::*;
use watane_log::PublicKey;
use watane_net::{BlobId, Code, Limits, NetError, Role};

fn quick() -> Limits {
    Limits {
        deadline: Duration::from_millis(800),
        connect_timeout: Duration::from_millis(800),
        ..Limits::default()
    }
}

/// The text of an error and of every cause under it.
fn chain_text(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut next = e.source();
    while let Some(inner) = next {
        text.push_str(" / ");
        text.push_str(&inner.to_string());
        next = inner.source();
    }
    text
}

/// An anchor, a full machine that holds a package and a channel, and a light
/// machine that the anchor then removes; the full machine has the new list.
fn removed_light() -> (Node, Node, Node, BlobId, PublicKey) {
    let seed = [81u8; 32];
    let entries = chain(seed, 2);
    let channel = entries[0].author();
    let anchor = node(MemHoldings::custom(BTreeMap::new(), None, true), None);
    let full = node(MemHoldings::empty(), None);
    for e in &entries {
        full.holdings.append(e.clone());
    }
    let id = full.holdings.add_blob(&[9u8; 2000]);
    let light = node(MemHoldings::empty(), Some(quick()));
    pair_into(&anchor, &full, Role::Full);
    pair_into(&anchor, &light, Role::Light);
    light.net.add_addrs(&full.key, &full.net.addrs());
    let seq = anchor.net.remove_device(&light.key).unwrap().seq;
    assert!(wait_for(5, || full
        .net
        .devices()
        .is_some_and(|l| l.seq == seq)));
    (anchor, full, light, id, channel)
}

#[test]
fn ac1_a_removed_light_machine_is_refused_not_unreachable() {
    let (anchor, full, light, id, channel) = removed_light();
    for r in [light.net.need_blob(&id, 2000), light.net.need(&channel)] {
        match r {
            Err(e @ NetError::Refused { node, code }) => {
                // the anchor is a full machine too, and refuses as well
                assert!(node == full.key || node == anchor.key);
                assert_eq!(code, Code::NotAllowed);
                assert_eq!(
                    e.to_string(),
                    format!("node {} refused: not_allowed", node.short())
                );
            }
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn ac2_a_code_the_anchor_refuses_is_spent() {
    let anchor = node(MemHoldings::custom(BTreeMap::new(), None, true), None);
    let joiner = node(MemHoldings::empty(), Some(quick()));
    let code = anchor.net.pair_code(Role::Full).unwrap();
    let mut wrong = code.clone();
    wrong.secret[0] ^= 1;
    let r = try_pair(&anchor, &joiner, wrong, false, None);
    let Err(e @ NetError::CodeSpent) = r else {
        panic!("{r:?}")
    };
    assert_eq!(
        e.to_string(),
        "the pairing code was used or has expired; make a new one on the anchor"
    );
    assert_eq!(NetError::BadCode.to_string(), "not a pairing code");
}

#[test]
fn ac3_a_node_with_known_addresses_that_is_off_does_not_say_no_direct_path() {
    let anchor = node(MemHoldings::custom(BTreeMap::new(), None, true), None);
    let joiner = node(MemHoldings::empty(), Some(quick()));
    let code = anchor.net.pair_code(Role::Full).unwrap();
    let key = anchor.key;
    anchor.net.close();
    let e = joiner.net.join(&code, false).unwrap_err();
    let text = chain_text(&e);
    assert!(!text.contains("no direct path"), "{text}");
    assert_eq!(text.matches(&key.short()).count(), 1, "{text}");
    assert!(text.contains("did not answer at its"), "{text}");
    assert!(text.contains("known address"), "{text}");
}

#[test]
fn ac3_a_node_with_no_known_address_says_no_direct_path_once() {
    let joiner = node(MemHoldings::empty(), Some(quick()));
    let stranger = node_key_of_seed([82u8; 32]);
    let e = joiner.net.hello(&stranger).unwrap_err();
    let text = chain_text(&e);
    assert_eq!(text.matches("no direct path").count(), 1, "{text}");
    assert_eq!(text.matches(&stranger.short()).count(), 1, "{text}");
}
