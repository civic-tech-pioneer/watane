#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Stage A1 requests and timers (TECHNICAL 19.6, 7; FUNCTIONAL S53 to S56).

use super::streams::{Env, call_as, fixture, lines, local, relay, settle, test_stream, wait};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use watane_host::{CallCode, Caller, Fakes, Harness, Host, Limits, Mode, RequestRecord};

fn id_of(identity: &str) -> String {
    blake3::hash(identity.as_bytes()).to_hex()[..32].to_owned()
}
/// Closes the host and waits until its workers have let go of `state/state.redb`.
fn close(env: Env) -> tempfile::TempDir {
    let Env { dir, host } = env;
    drop(host);
    dir
}
fn open_db(home: &Path) -> redb::Database {
    let start = std::time::Instant::now();
    loop {
        match redb::Database::open(home.join("state/state.redb")) {
            Ok(db) => return db,
            Err(e) => {
                assert!(start.elapsed() < Duration::from_secs(5), "{e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}
fn timer_rows(home: &Path) -> BTreeMap<String, Value> {
    use redb::{ReadableDatabase, ReadableTable};
    let db = open_db(home);
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
fn state_value(home: &Path, identity: &str, key: &str) -> Option<Vec<u8>> {
    use redb::ReadableDatabase;
    let db = open_db(home);
    let txn = db.begin_read().unwrap();
    let name = format!("s/{}", id_of(identity));
    let table = txn
        .open_table(redb::TableDefinition::<&str, &[u8]>::new(&name))
        .ok()?;
    table.get(key).unwrap().map(|v| v.value().to_vec())
}
fn after(env: &Env, id: &str, ms: u64) -> Value {
    relay(&env.host, "api.after", json!({"id": id, "ms": ms}))
}
fn fired(env: &Env) -> Vec<String> {
    lines(&env.host, "api.fired")
}

#[test]
fn ac6_s53_a_failed_call_leaves_no_timer() {
    let env = Env::relay();
    let err = call_as(
        &env.host,
        Caller::Page,
        "relay",
        "api.after-then-fail",
        json!({"id":"t","ms":10}),
    )
    .unwrap_err();
    assert_eq!(err.code, CallCode::Trap);
    // `relay-front` calls `relay.after`, which succeeds, and then fails itself.
    let err = call_as(
        &env.host,
        Caller::Page,
        "relay-front",
        "api.after-then-fail",
        json!({"id":"t","ms":10}),
    )
    .unwrap_err();
    assert_eq!(err.code, CallCode::Trap);
    settle();
    assert_eq!(fired(&env), Vec::<String>::new());
    let home = close(env);
    assert_eq!(timer_rows(home.path()), BTreeMap::new());
}

#[test]
fn ac6_s53_a_succeeded_call_writes_its_timer_with_the_origin_client() {
    let env = Env::relay();
    assert_eq!(after(&env, "t", 60_000), json!({"ok":null}));
    let home = close(env);
    let rows = timer_rows(home.path());
    let key = format!("{}/t", id_of("local/relay"));
    assert_eq!(rows.keys().collect::<Vec<_>>(), [&key]);
    assert_eq!(rows[&key]["origin"], "local/relay");
    let due = rows[&key]["due"].as_u64().unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(due > now + 50_000 && due <= now + 60_000, "{due} {now}");
}

#[test]
fn ac6_s53_the_harness_records_requests_of_a_succeeded_call_only() {
    let h = Harness::open(&fixture("relay"), Fakes::new()).unwrap();
    let out = h.call("api.after", json!({"id":"t","ms":10})).unwrap();
    assert_eq!(
        out.requests,
        vec![RequestRecord {
            function: "timers.after".into(),
            args: json!({"id":"t","delay-ms":10}),
        }]
    );
    h.call("api.after-then-fail", json!({"id":"t","ms":10}))
        .unwrap_err();
    let out = h.call("api.cancel-timer", json!({"id":"t"})).unwrap();
    assert_eq!(
        out.requests,
        vec![RequestRecord {
            function: "timers.cancel".into(),
            args: json!({"id":"t"}),
        }]
    );
}

#[test]
fn ac11_the_harness_fakes_streams_timers_and_identity() {
    let h = Harness::open(&fixture("relay"), Fakes::new()).unwrap();
    let first = h.call("api.open", json!(null)).unwrap();
    assert_eq!(first.value, json!({"ok":"s1"}));
    assert_eq!(first.requests[0].function, "streams.open");
    assert_eq!(first.requests[0].args, json!({}));
    assert_eq!(
        h.call("api.open", json!(null)).unwrap().value,
        json!({"ok":"s2"})
    );
    assert_eq!(
        h.call("api.push", json!({"s":"s1","text":"x"}))
            .unwrap()
            .value,
        json!({"ok":null})
    );
    assert_eq!(h.call("api.who", json!(null)).unwrap().value, json!("page"));
    // A `Fakes` entry overrides a result.
    let mut fakes = Fakes::new();
    fakes.insert(
        "watane:host/streams".into(),
        [("open".to_owned(), json!({"err":"full"}))].into(),
    );
    let h = Harness::open(&fixture("relay"), fakes).unwrap();
    assert_eq!(
        h.call("api.open", json!(null)).unwrap().value,
        json!({"err":"full"})
    );
}

#[test]
fn ac7_s54_a_timer_fires_once() {
    let env = Env::relay();
    assert_eq!(after(&env, "t", 50), json!({"ok":null}));
    wait("the firing", Duration::from_secs(5), || {
        fired(&env) == ["t"]
    });
    settle();
    assert_eq!(fired(&env), ["t"]);
    let home = close(env);
    assert_eq!(timer_rows(home.path()), BTreeMap::new());
}

#[test]
fn ac7_s54_after_with_a_live_id_replaces_it() {
    let env = Env::relay();
    after(&env, "t", 5000);
    after(&env, "t", 50);
    // Well before the first due time: the replacement fired, not the timer it replaced.
    wait("the firing", Duration::from_secs(2), || {
        fired(&env) == ["t"]
    });
    settle();
    assert_eq!(fired(&env), ["t"]);
}

#[test]
fn ac7_s54_a_cancelled_timer_never_fires_and_a_replaced_one_fires_at_the_new_time() {
    let env = Env::relay();
    after(&env, "t", 300);
    relay(&env.host, "api.cancel-timer", json!({"id":"t"}));
    // The row of a cancelled timer is gone; so is a firing that its heap entry would make.
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(fired(&env), Vec::<String>::new());
    // Cancelling an unknown id is ignored.
    relay(&env.host, "api.cancel-timer", json!({"id":"nope"}));
    // Replaced by a later time: no firing at the old due time.
    after(&env, "u", 200);
    after(&env, "u", 1500);
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(fired(&env), Vec::<String>::new());
    wait("the new time", Duration::from_secs(5), || {
        fired(&env) == ["u"]
    });
}

#[test]
fn ac7_s54_a_timer_due_during_downtime_fires_soon_after_start() {
    let env = Env::relay();
    after(&env, "t", 300);
    let home = close(env);
    std::thread::sleep(Duration::from_millis(500));
    let started = std::time::Instant::now();
    let host = open_again(home.path());
    wait("the firing", Duration::from_secs(1), || {
        lines(&host, "api.fired") == ["t"]
    });
    assert!(started.elapsed() < Duration::from_secs(2));
    settle();
    assert_eq!(lines(&host, "api.fired"), ["t"]);
    drop(host);
    assert_eq!(timer_rows(home.path()), BTreeMap::new());
}
fn open_again(home: &Path) -> Arc<Host> {
    let start = std::time::Instant::now();
    loop {
        match Host::open(home, Mode::Preview, Limits::default()) {
            Ok(host) => return host,
            Err(e) => {
                assert!(start.elapsed() < Duration::from_secs(5), "{e}");
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

#[test]
fn ac7_s54_a_timer_survives_a_restart_that_comes_before_it_is_due() {
    let env = Env::relay();
    after(&env, "t", 1500);
    let home = close(env);
    let host = open_again(home.path());
    wait("the firing", Duration::from_secs(5), || {
        lines(&host, "api.fired") == ["t"]
    });
}

fn insert_row(home: &Path, key: &str, value: &str) {
    std::fs::create_dir_all(home.join("state")).unwrap();
    let db = redb::Database::create(home.join("state/state.redb")).unwrap();
    let txn = db.begin_write().unwrap();
    let mut table = txn
        .open_table(redb::TableDefinition::<&str, &str>::new("timers"))
        .unwrap();
    table.insert(key, value).unwrap();
    drop(table);
    txn.commit().unwrap();
}

#[test]
fn ac7_a_mod_without_timer_sink_gets_its_due_row_deleted() {
    // The scenario runs in a child process so that the host's log line on stderr can be read.
    if std::env::var_os("WATANE_TIMER_SINK_CHILD").is_some() {
        no_timer_sink_scenario();
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "requests::ac7_a_mod_without_timer_sink_gets_its_due_row_deleted",
            "--nocapture",
        ])
        .env("WATANE_TIMER_SINK_CHILD", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("watane: timer x of latch dropped: no timer-sink"),
        "{stderr}"
    );
}
fn no_timer_sink_scenario() {
    let env = Env::relay();
    // `latch` has no `timer-sink`: a row for it is dropped at its due time (19.6).
    let key = format!("{}/x", id_of("local/latch"));
    let home = close(env);
    insert_row(home.path(), &key, r#"{"due":1,"origin":"local/latch"}"#);
    let host = open_again(home.path());
    std::thread::sleep(Duration::from_millis(1000));
    drop(host);
    assert_eq!(timer_rows(home.path()), BTreeMap::new());
}

#[test]
fn ac8_s55_timer_limits() {
    let env = Env::relay();
    for i in 0..256 {
        assert_eq!(
            after(&env, &format!("id{i}"), 600_000),
            json!({"ok":null}),
            "{i}"
        );
    }
    assert_eq!(after(&env, "id256", 600_000), json!({"err":"too-many"}));
    // A live id is replaced, not counted twice.
    assert_eq!(after(&env, "id0", 600_000), json!({"ok":null}));
    assert_eq!(after(&env, &"a".repeat(129), 10), json!({"err":"bad-id"}));
    relay(&env.host, "api.cancel-timer", json!({"id":"id1"}));
    assert_eq!(after(&env, &"a".repeat(128), 600_000), json!({"ok":null}));
    assert_eq!(after(&env, "x", 2_592_000_001), json!({"err":"bad-delay"}));
    relay(&env.host, "api.cancel-timer", json!({"id":"id2"}));
    assert_eq!(after(&env, "x", 2_592_000_000), json!({"ok":null}));
}

#[test]
fn ac8_s55_a_chain_counts_the_timers_it_has_queued() {
    let env = Env::relay();
    for i in 0..256 {
        after(&env, &format!("id{i}"), 600_000);
    }
    // 256 live: a replacement succeeds, a new id does not, also inside one call.
    assert_eq!(after(&env, "id5", 500_000), json!({"ok":null}));
    assert_eq!(after(&env, "new", 500_000), json!({"err":"too-many"}));
}

#[test]
fn ac9_s56_identity_comes_from_the_transport() {
    let env = Env::relay();
    assert_eq!(relay(&env.host, "api.who", json!(null)), json!("page"));
    let front =
        |caller: Caller| call_as(&env.host, caller, "relay-front", "api.who", json!(null)).unwrap();
    assert_eq!(front(Caller::Page), json!("local/relay-front"));
    assert_eq!(
        call_as(
            &env.host,
            Caller::Mod(local("relay-front")),
            "relay",
            "api.who",
            json!(null)
        )
        .unwrap(),
        json!("local/relay-front")
    );
    // An agent: `agent:<handle-name>`. The agent part of S56 that needs a handle of an owner
    // (`agent:h1` through the kernel) waits for A3.
    assert_eq!(
        call_as(
            &env.host,
            Caller::Agent(watane_host::Handle::parse("h1").unwrap()),
            "relay",
            "api.who",
            json!(null)
        )
        .unwrap(),
        json!("agent:h1")
    );
    // An item call of a host stream: `host`, read from the state after the host closed.
    let s = test_stream(&env.host);
    s.push(b"x").unwrap();
    wait("the item", Duration::from_secs(5), || {
        lines(&env.host, "api.received") == ["x"]
    });
    let id = s.id().to_owned();
    drop(s);
    let home = close(env);
    assert_eq!(
        state_value(home.path(), "local/relay", &format!("who/{id}")),
        Some(b"host".to_vec())
    );
}

#[test]
fn ac10_a_mod_stream_item_call_has_the_producer_as_caller() {
    let env = Env::relay();
    let s = call_as(
        &env.host,
        Caller::Mod(local("relay")),
        "relay",
        "api.open",
        json!(null),
    )
    .unwrap()["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    relay(&env.host, "api.push", json!({"s": s, "text": "x"}));
    wait("the item", Duration::from_secs(5), || {
        lines(&env.host, "api.received") == ["x"]
    });
    let home = close(env);
    assert_eq!(
        state_value(home.path(), "local/relay", &format!("who/{s}")),
        Some(b"local/relay".to_vec())
    );
}

#[test]
fn ac10_remove_rewrites_the_timers_that_carry_the_removed_mod() {
    let env = Env::relay();
    // `relay-front` is the origin client of a timer of `relay`.
    call_as(
        &env.host,
        Caller::Mod(local("relay-front")),
        "relay",
        "api.after",
        json!({"id":"c","ms":600_000}),
    )
    .unwrap();
    after(&env, "own", 600_000);
    let key_c = format!("{}/c", id_of("local/relay"));
    env.host.remove(&local("relay-front"), true).unwrap();
    let Env { dir, host } = env;
    drop(host);
    let rows = timer_rows(dir.path());
    assert_eq!(rows[&key_c]["origin"], "local/relay");
    // Removing the owner deletes its timers.
    let host = open_again(dir.path());
    host.remove(&local("relay"), true).unwrap();
    drop(host);
    assert_eq!(timer_rows(dir.path()), BTreeMap::new());
}
