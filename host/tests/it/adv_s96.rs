#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! S96 adversary tests: attacks on the agent layer's stage A1 boundaries (host TECHNICAL 19).

use super::streams::{Env, call_as, local, wait};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use watane_host::Caller;

fn id_of(identity: &str) -> String {
    blake3::hash(identity.as_bytes()).to_hex()[..32].to_owned()
}
fn timer_rows(home: &Path) -> BTreeMap<String, Value> {
    use redb::{ReadableDatabase, ReadableTable};
    let start = std::time::Instant::now();
    let db = loop {
        match redb::Database::open(home.join("state/state.redb")) {
            Ok(db) => break db,
            Err(e) => {
                assert!(start.elapsed() < Duration::from_secs(5), "{e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    let txn = db.begin_read().unwrap();
    let Ok(table) = txn.open_table(redb::TableDefinition::<&str, &str>::new("timers")) else {
        return BTreeMap::new();
    };
    table
        .iter()
        .unwrap()
        .map(|r| {
            let (k, v) = r.unwrap();
            (
                k.value().to_owned(),
                serde_json::from_str(v.value()).unwrap(),
            )
        })
        .collect()
}

/// 19, Terms: `remove` rewrites every timer that carries the removed mod, so a later mod with the
/// same identity inherits none. The removal itself ends the removed mod's streams, and the
/// producer's `stream-producer.ended` runs after the removal with the removed mod still as its
/// origin client: a timer it sets there is written carrying the removed mod.
#[test]
fn adv_s96_timer_set_in_ended_after_remove_carries_the_removed_origin() {
    let env = Env::new(&["latch", "relay", "advo"]);
    // `relay` calls `advo.open`: relay is the consumer and the origin client of the stream.
    let s = call_as(
        &env.host,
        Caller::Mod(local("relay")),
        "advo",
        "api.open",
        Value::Null,
    )
    .unwrap();
    assert!(s.get("ok").is_some(), "{s}");
    std::thread::sleep(Duration::from_millis(200));
    env.host.remove(&local("relay"), true).unwrap();
    wait("advo's ended", Duration::from_secs(10), || {
        call_as(&env.host, Caller::Page, "advo", "api.done", Value::Null).unwrap() == json!(true)
    });
    let Env { dir, host } = env;
    drop(host);
    let rows = timer_rows(dir.path());
    let row = &rows[&format!("{}/leak", id_of("local/advo"))];
    assert_ne!(
        row["origin"], "local/relay",
        "a timer written after the removal carries the removed mod as origin client: {row}"
    );
}

/// 19, Terms: "at each firing of a timer, the host checks that the origin client is installed and
/// enabled", else the timer fires with its own mod as origin client. The host checks before it
/// waits for the mod's lock: when the origin client is removed while the mod is busy, the firing
/// still runs with the removed mod as origin client, and a timer it sets carries it.
#[test]
fn adv_s96_timer_firing_checks_its_origin_before_the_wait() {
    let env = Env::new(&["latch", "relay", "advo"]);
    call_as(
        &env.host,
        Caller::Mod(local("relay")),
        "advo",
        "api.after",
        json!({"id":"probe","ms":150}),
    )
    .unwrap();
    // `advo` is busy while its timer comes due.
    let host = std::sync::Arc::clone(&env.host);
    let busy = std::thread::spawn(move || {
        call_as(
            &host,
            Caller::Page,
            "advo",
            "api.cancel-slowly",
            json!({"id":"none"}),
        )
        .unwrap()
    });
    std::thread::sleep(Duration::from_millis(250));
    assert!(
        !busy.is_finished(),
        "advo must still be busy at the removal"
    );
    env.host.remove(&local("relay"), true).unwrap();
    busy.join().unwrap();
    wait("the firing", Duration::from_secs(10), || {
        call_as(&env.host, Caller::Page, "advo", "api.fired", Value::Null).unwrap()
            == json!(["probe"])
    });
    let Env { dir, host } = env;
    drop(host);
    let rows = timer_rows(dir.path());
    let row = &rows[&format!("{}/leak2", id_of("local/advo"))];
    assert_ne!(
        row["origin"], "local/relay",
        "the firing ran with the removed mod as origin client: {row}"
    );
}

/// 19.6: "A firing for an id cancelled or replaced meanwhile is ignored." The host reads the row
/// before it waits for the mod's lock, so a cancel committed during that wait does not stop it.
#[test]
fn adv_s96_timer_cancelled_while_the_firing_waits_still_fires() {
    let env = Env::new(&["latch", "relay", "advo"]);
    call_as(
        &env.host,
        Caller::Page,
        "advo",
        "api.after",
        json!({"id":"x","ms":100}),
    )
    .unwrap();
    // This call holds advo's lock past the due time and cancels `x`; its commit comes first.
    call_as(
        &env.host,
        Caller::Page,
        "advo",
        "api.cancel-slowly",
        json!({"id":"x"}),
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1000));
    let fired = call_as(&env.host, Caller::Page, "advo", "api.fired", Value::Null).unwrap();
    assert_eq!(fired, json!([]), "a cancelled timer fired");
}
