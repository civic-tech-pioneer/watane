#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! R7 (TECHNICAL 19.2, Deferred batch; FUNCTIONAL S69): a host stream that its producer ends
//! while its consumer holds a deferred batch delivers the held and queued items after
//! `resume`, then `ended`; a consumer-side end still drops them (HOST-F-68).

use crate::streams::{Env, lines, relay, settle, test_stream, wait};
use serde_json::json;
use std::time::Duration;

fn set_mode(env: &Env, mode: &str) {
    relay(&env.host, "api.mode", json!({"m": mode}));
}
fn resume(env: &Env, s: &str) {
    assert_eq!(relay(&env.host, "api.resume", json!({"s": s})), json!({"ok":null}));
}

#[test]
fn ac15_a_producer_end_waits_for_the_deferred_consumer_then_delivers_everything() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    set_mode(&env, "defer");
    s.push(b"a").unwrap();
    wait("the deferral", Duration::from_secs(10), || {
        s.deliveries().len() == 1
    });
    s.push(b"b").unwrap();
    s.close("closed");
    settle();
    // `ended` does not overtake the held items.
    assert_eq!(lines(&env.host, "api.ended"), Vec::<String>::new());
    assert_eq!(lines(&env.host, "api.received"), Vec::<String>::new());
    set_mode(&env, "taken");
    resume(&env, s.id());
    wait("ended", Duration::from_secs(10), || {
        !lines(&env.host, "api.ended").is_empty()
    });
    assert_eq!(lines(&env.host, "api.received"), ["a", "b"]);
    assert_eq!(lines(&env.host, "api.ended"), [format!("{} closed", s.id())]);
    assert_eq!(s.ended(), Some(json!("closed")));
}

#[test]
fn ac15_a_failed_end_of_the_producer_delivers_the_held_items_too() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    set_mode(&env, "defer");
    s.push(b"a").unwrap();
    wait("the deferral", Duration::from_secs(10), || {
        s.deliveries().len() == 1
    });
    s.close(r#"{"failed":"boom"}"#);
    settle();
    assert_eq!(lines(&env.host, "api.ended"), Vec::<String>::new());
    set_mode(&env, "taken");
    resume(&env, s.id());
    wait("ended", Duration::from_secs(10), || {
        !lines(&env.host, "api.ended").is_empty()
    });
    assert_eq!(lines(&env.host, "api.received"), ["a"]);
    assert_eq!(lines(&env.host, "api.ended"), [format!("{} failed", s.id())]);
}

#[test]
fn ac15_the_consumers_cancel_still_drops_the_held_items() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    set_mode(&env, "defer");
    s.push(b"a").unwrap();
    wait("the deferral", Duration::from_secs(10), || {
        s.deliveries().len() == 1
    });
    s.push(b"b").unwrap();
    relay(&env.host, "api.cancel", json!({"s": s.id()}));
    wait("ended", Duration::from_secs(10), || {
        !lines(&env.host, "api.ended").is_empty()
    });
    settle();
    assert_eq!(lines(&env.host, "api.ended"), [format!("{} cancelled", s.id())]);
    // The relay's mode stored what it was given while it deferred; nothing more arrived.
    assert_eq!(s.deliveries().len(), 1);
}

#[test]
fn ac15_a_cancel_after_the_producer_ended_drops_the_held_items() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    set_mode(&env, "defer");
    s.push(b"a").unwrap();
    wait("the deferral", Duration::from_secs(10), || {
        s.deliveries().len() == 1
    });
    s.close("closed");
    settle();
    relay(&env.host, "api.cancel", json!({"s": s.id()}));
    wait("ended", Duration::from_secs(10), || {
        !lines(&env.host, "api.ended").is_empty()
    });
    assert_eq!(lines(&env.host, "api.ended"), [format!("{} cancelled", s.id())]);
    assert_eq!(s.deliveries().len(), 1);
}
