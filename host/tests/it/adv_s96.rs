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
            (k.value().to_owned(), serde_json::from_str(v.value()).unwrap())
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
