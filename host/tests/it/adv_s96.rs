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

// ---- the fix tests of S96-fix1 ---------------------------------------------------------

fn page(env: &Env, target: &str, function: &str, args: Value) -> Value {
    call_as(&env.host, Caller::Page, target, function, args).unwrap()
}
/// A call that runs on its own thread, for a chain that stays in flight.
fn in_flight(env: &Env, target: &'static str, function: &'static str, args: Value) -> JoinHandle {
    let host = std::sync::Arc::clone(&env.host);
    let thread =
        std::thread::spawn(move || call_as(&host, Caller::Page, target, function, args).unwrap());
    std::thread::sleep(Duration::from_millis(400));
    JoinHandle(thread)
}
struct JoinHandle(std::thread::JoinHandle<Value>);
impl JoinHandle {
    fn finish(self) -> Value {
        self.0.join().unwrap()
    }
}
fn open_for_relay(env: &Env) -> String {
    let s = call_as(
        &env.host,
        Caller::Mod(local("relay")),
        "advo",
        "api.open",
        Value::Null,
    )
    .unwrap();
    s["ok"].as_str().unwrap().to_owned()
}

/// 19.6: a firing in progress is never armed again, so `enable` during a waiting firing does
/// not fire the timer twice.
#[test]
fn ac3_enable_during_a_waiting_firing_fires_once() {
    let env = Env::new(&["latch", "relay", "advo"]);
    page(&env, "advo", "api.after", json!({"id":"x","ms":100}));
    let busy = in_flight(&env, "advo", "api.cancel-slowly", json!({"id":"none"}));
    // The firing is due and waits for advo's lock.
    env.host.enable(&local("advo")).unwrap();
    busy.finish();
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(page(&env, "advo", "api.fired", Value::Null), json!(["x"]));
}

/// 19.2 (Commit, Byte budgets): the per-stream limit counts what every chain in flight has
/// queued: two chains with 2.5 MiB each into one stream of 4 MiB.
#[test]
fn ac4_two_chains_in_flight_cannot_pass_the_stream_limit() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let s = open_for_relay(&env);
    let args = json!({"s": s, "bytes": 5 << 19, "ms": 1000});
    let first = in_flight(&env, "advo-front", "api.push-then-sleep", args);
    let second = page(&env, "advo", "api.push", json!({"s": s, "bytes": 5 << 19}));
    assert!(second.as_str().unwrap().contains("\"full\""), "{second}");
    assert_eq!(first.finish(), json!("ok"));
}

/// 19.6 (`too-many`): 256 live timers, counted across the chains in flight.
#[test]
fn ac4_two_chains_in_flight_cannot_pass_the_timer_limit() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    page(
        &env,
        "advo",
        "api.after-many",
        json!({"n":255,"ms":3_600_000}),
    );
    let args = json!({"id":"a","ms":3_600_000,"sleep":1000});
    let first = in_flight(&env, "advo-front", "api.try-after-then-sleep", args);
    let second = page(
        &env,
        "advo",
        "api.try-after",
        json!({"id":"b","ms":3_600_000}),
    );
    assert!(second.as_str().unwrap().contains("too-many"), "{second}");
    assert_eq!(first.finish(), json!("ok"));
}

/// 19.2 (hand-off "once"): two chains in flight hand off one stream; one gets `not-yours`.
#[test]
fn ac4_two_chains_in_flight_hand_off_a_stream_once() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let s = env
        .host
        .test_stream(watane_host::TestConsumer::Mod(local("relay")))
        .unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.hand-off-then-sleep",
        json!({"s": s.id(), "ms": 1000}),
    );
    let second = call_as(
        &env.host,
        Caller::Mod(local("advo-front")),
        "relay",
        "api.hand-off",
        json!({"s": s.id()}),
    )
    .unwrap();
    assert_eq!(second, json!({"err":"not-yours"}));
    assert_eq!(first.finish(), json!("ok"));
}

/// 19.2 (Commit): the 32 open streams of a consumer count the opens that another chain in
/// flight has queued.
#[test]
fn ac4_two_chains_in_flight_cannot_pass_the_open_limit() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let open = || {
        call_as(
            &env.host,
            Caller::Mod(local("advo-front")),
            "advo",
            "api.open",
            Value::Null,
        )
        .unwrap()
    };
    for _ in 0..31 {
        assert!(open()["ok"].is_string());
    }
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    let second = open();
    assert!(
        second["err"].as_str().unwrap().contains("\"full\""),
        "{second}"
    );
    assert!(first.finish()["ok"].is_string());
}

/// 19.2 (Ends): a mod stream depends on the consumer's `Call` grant of the link that `open` ran
/// through. Revoked, the stream ends `gone` for both sides at once.
#[test]
fn ac5_a_revoked_call_grant_ends_the_streams_opened_through_it() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let opened = page(&env, "advo-front", "api.open", Value::Null);
    let s = opened["ok"].as_str().unwrap().to_owned();
    assert_eq!(
        page(&env, "advo", "api.push", json!({"s": s, "bytes": 1})),
        json!("ok")
    );
    let call = |provider: &str| {
        env.host
            .get(&local("advo-front"))
            .unwrap()
            .grants
            .into_iter()
            .find(|g| {
                matches!(&g.grant, watane_pkg::Grant::Call { interface, .. }
                    if interface.to_string().starts_with(provider))
                    && g.granted
            })
            .unwrap()
            .grant
    };
    // A catalog write that leaves this grant keeps the stream.
    let other = call("mods:relay/");
    env.host.revoke(&local("advo-front"), &other).unwrap();
    assert_eq!(
        page(&env, "advo", "api.push", json!({"s": s, "bytes": 1})),
        json!("ok")
    );
    env.host
        .revoke(&local("advo-front"), &call("mods:advo/"))
        .unwrap();
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "{pushed}"
    );
    let line = format!("{s} gone");
    wait("both ended lines", Duration::from_secs(10), || {
        page(&env, "advo", "api.ended", Value::Null) == json!([line])
            && page(&env, "advo-front", "api.ended", Value::Null) == json!([line])
    });
}

/// 19.6: 256 due timers of one mod fire on one worker, each once.
#[test]
fn ac6_due_timers_of_one_mod_do_not_pile_up_threads() {
    let env = Env::new(&["latch", "relay", "advo"]);
    page(&env, "advo", "api.after-many", json!({"n":256,"ms":100}));
    let busy = in_flight(&env, "advo", "api.cancel-slowly", json!({"id":"none"}));
    let mut most = 0;
    let start = std::time::Instant::now();
    let fired = loop {
        most = most.max(timer_threads());
        let fired: Vec<String> =
            serde_json::from_value(page(&env, "advo", "api.fired", Value::Null)).unwrap();
        if fired.len() >= 256 {
            break fired;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "{}", fired.len());
        std::thread::sleep(Duration::from_millis(5));
    };
    busy.finish();
    let mut sorted = fired.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 256, "a timer fired twice or not at all");
    assert_eq!(fired.len(), 256);
    // The threads of the firings of this process: Linux only.
    if cfg!(target_os = "linux") {
        assert!(most < 64, "{most} timer threads at once");
    }
}
/// Threads of this process named for a timer firing (`/proc/self/task`; other tests of the
/// binary run their own hosts, so only these are counted).
fn timer_threads() -> usize {
    std::fs::read_dir("/proc/self/task")
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter(|t| {
                    std::fs::read_to_string(t.path().join("comm"))
                        .is_ok_and(|n| n.trim() == "watane-timer")
                })
                .count()
        })
        .unwrap_or(0)
}

/// 19.2 (Ends): a stream that is closing already (closed by the producer, items still queued)
/// keeps its outcome when the consumer's grant is revoked: it ends `closed`, not `gone`.
#[test]
fn ac5_a_closing_stream_keeps_its_outcome_when_the_grant_is_revoked() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    page(&env, "advo-front", "api.mode", json!({"m": "slow"}));
    let s = page(&env, "advo-front", "api.open", Value::Null)["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    // The consumer's `items` call is running; the item stays queued until it returns.
    std::thread::sleep(Duration::from_millis(400));
    page(&env, "advo", "api.close", json!({"s": s}));
    let grant = env
        .host
        .get(&local("advo-front"))
        .unwrap()
        .grants
        .into_iter()
        .find(|g| {
            matches!(&g.grant, watane_pkg::Grant::Call { interface, .. }
                if interface.to_string().starts_with("mods:advo/"))
        })
        .unwrap()
        .grant;
    env.host.revoke(&local("advo-front"), &grant).unwrap();
    let line = format!("{s} closed");
    wait("the consumer's end", Duration::from_secs(10), || {
        page(&env, "advo-front", "api.ended", Value::Null) == json!([line])
    });
}

// ---- second adversary pass (fix range a87c5d5..6c2226f) ---------------------------------

/// 19.2 (Ends, Q46): a mod stream depends on the consumer's `Call` grant of the link that
/// `open` ran through; revoked, the stream ends `gone`. `end_lost_streams` runs after the
/// catalog write and only sees streams already applied; the `Open` of a chain still in flight is
/// applied at its commit, which checks the producer and the origin client but not the grant. A
/// revoke while the consumer's chain is in flight leaves a live stream through a revoked grant.
#[test]
fn adv_s96_stream_opened_by_a_chain_in_flight_outlives_the_revoke() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    // `advo.open` returned inside advo-front's chain; the `Open` is queued, not applied.
    let grant = env
        .host
        .get(&local("advo-front"))
        .unwrap()
        .grants
        .into_iter()
        .find(|g| {
            matches!(&g.grant, watane_pkg::Grant::Call { interface, .. }
                if interface.to_string().starts_with("mods:advo/"))
        })
        .unwrap()
        .grant;
    env.host.revoke(&local("advo-front"), &grant).unwrap();
    let opened = first.finish();
    let s = opened["ok"].as_str().unwrap().to_owned();
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "a stream through a revoked Call grant is live: push gave {pushed}"
    );
}

/// 19.6: "A mod that is disabled gets no firing; its timers wait." `disable` does not take the
/// mod's call lock; a firing that waits for the lock while the mod is disabled finds it disabled
/// after the lock, fails `disabled`, and `fire` deletes the row as for a failed firing. `enable`
/// then reloads nothing: the timer never fires. (An update of the mod during the wait fails
/// `no-mod` "removed or updated" and loses the timer the same way.)
#[test]
fn adv_s96_disable_while_a_firing_waits_loses_the_timer() {
    let env = Env::new(&["latch", "relay", "advo"]);
    page(&env, "advo", "api.after", json!({"id":"x","ms":250}));
    // advo is busy past the due time; the firing waits for its lock.
    let busy = in_flight(&env, "advo", "api.cancel-slowly", json!({"id":"none"}));
    env.host.disable(&local("advo")).unwrap();
    busy.finish();
    std::thread::sleep(Duration::from_millis(500));
    env.host.enable(&local("advo")).unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        page(&env, "advo", "api.fired", Value::Null),
        json!(["x"]),
        "a timer due while its mod was disabled never fired after enable"
    );
}

/// 19.2 (Ends): an update of either side ends its streams. `install` ends the streams that
/// exist at the update; the `Open` of a chain in flight is applied at its commit, which checks
/// only that the producer's identity and local name are installed and enabled, both still true
/// after an update. A stream opened by the old version outlives the update of its producer.
#[test]
fn adv_s96_stream_opened_by_a_chain_in_flight_outlives_an_update() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let copy = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/advo");
    copy_dir(&src, copy.path());
    let toml = copy.path().join("mod.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(&toml, text.replace("summary = \"", "summary = \"v2: ")).unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    let before = env.host.get(&local("advo")).unwrap().package;
    env.host
        .install_folders(&[copy.path().to_path_buf()], 0)
        .unwrap();
    assert_ne!(
        env.host.get(&local("advo")).unwrap().package,
        before,
        "not an update"
    );
    let opened = first.finish();
    let s = opened["ok"].as_str().unwrap().to_owned();
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "a stream opened by the producer's old version is live after its update: push gave {pushed}"
    );
}
fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// 19, Terms: "`remove` rewrites the timers that carry the removed mod to their own mod, and a
/// later install with the same identity finds no timer that carries it." The origin client is
/// checked once, when a call starts. A host call of `advo` (its `stream-producer.ended`, origin
/// client `relay`) that is already running when `relay` is removed sets a timer after the
/// removal's rewrite; its commit writes the row with the removed mod as origin client, and a
/// later `local/relay` (same identity) would fire with it.
#[test]
fn adv_s96_timer_set_by_a_call_running_across_the_remove_carries_the_removed_origin() {
    let env = Env::new(&["latch", "relay", "advo"]);
    let s = open_for_relay(&env);
    // The producer closes; its `ended` runs with `relay` as origin client and spins 300 ms.
    page(&env, "advo", "api.close", json!({"s": s}));
    std::thread::sleep(Duration::from_millis(100));
    env.host.remove(&local("relay"), true).unwrap();
    wait("advo's ended", Duration::from_secs(10), || {
        page(&env, "advo", "api.done", Value::Null) == json!(true)
    });
    let Env { dir, host } = env;
    drop(host);
    let rows = timer_rows(dir.path());
    let row = &rows[&format!("{}/leak", id_of("local/advo"))];
    assert_ne!(
        row["origin"], "local/relay",
        "a timer committed after the removal carries the removed mod as origin client: {row}"
    );
}

/// 19.6: an updated mod keeps its timers. A firing that waits for the mod's lock while the mod is
/// updated finds `no-mod` after the lock; it keeps its row, and the update arms it again: the
/// timer fires once, in the new version.
#[test]
fn ac3_update_while_a_firing_waits_keeps_the_timer() {
    let env = Env::new(&["latch", "relay", "advo"]);
    let copy = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/advo");
    copy_dir(&src, copy.path());
    let toml = copy.path().join("mod.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(&toml, text.replace("summary = \"", "summary = \"v2: ")).unwrap();
    page(&env, "advo", "api.after", json!({"id":"x","ms":250}));
    // advo is busy past the due time; the firing waits for its lock.
    let busy = in_flight(&env, "advo", "api.cancel-slowly", json!({"id":"none"}));
    let before = env.host.get(&local("advo")).unwrap().package;
    env.host
        .install_folders(&[copy.path().to_path_buf()], 0)
        .unwrap();
    assert_ne!(env.host.get(&local("advo")).unwrap().package, before);
    assert!(
        !busy.0.is_finished(),
        "the update must land during the wait"
    );
    busy.finish();
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        page(&env, "advo", "api.fired", Value::Null),
        json!(["x"]),
        "a timer whose firing waited across an update of its mod"
    );
}

/// 19.2 (Ends), 19.6 (At apply): the consumer's side too. An `Open` whose consumer was updated
/// while the chain was in flight is applied and ended `gone`.
#[test]
fn ac2_stream_opened_by_a_chain_in_flight_ends_at_an_update_of_its_consumer() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let copy = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/advo-front");
    copy_dir(&src, copy.path());
    let toml = copy.path().join("mod.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(&toml, text.replace("summary = \"", "summary = \"v2: ")).unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    let before = env.host.get(&local("advo-front")).unwrap().package;
    env.host
        .install_folders(&[copy.path().to_path_buf()], 0)
        .unwrap();
    assert_ne!(env.host.get(&local("advo-front")).unwrap().package, before);
    let opened = first.finish();
    let s = opened["ok"].as_str().unwrap().to_owned();
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "a stream opened by the consumer's old version is live after its update: push gave {pushed}"
    );
}

/// 19.6 (At apply): a `HandOff` applies only if its target is still installed, enabled and the
/// install that was called. The target is disabled while the chain is in flight.
#[test]
fn ac1_a_hand_off_to_a_target_disabled_in_flight_ends_the_stream_gone() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let s = env
        .host
        .test_stream(watane_host::TestConsumer::Mod(local("relay")))
        .unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.hand-off-then-sleep",
        json!({"s": s.id(), "ms": 1000}),
    );
    env.host.disable(&local("advo-front")).unwrap();
    assert_eq!(first.finish(), json!("ok"));
    wait("the end", Duration::from_secs(5), || s.ended().is_some());
    assert_eq!(s.ended(), Some(json!("gone")));
    assert!(s.push(b"x").is_err());
}

/// 19.6 (At apply): a producer removed and installed again, the same package, while an `open`
/// is in flight is another install: the stream ends `gone` (the install generation).
#[test]
fn ac2_an_open_in_flight_ends_gone_when_the_producer_is_removed_and_reinstalled() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    env.host.remove(&local("advo"), true).unwrap();
    super::streams::install(&env.host, &["advo"]);
    let s = first.finish()["ok"].as_str().unwrap().to_owned();
    let line = format!("{s} gone");
    wait("the consumer's end", Duration::from_secs(10), || {
        page(&env, "advo-front", "api.ended", Value::Null) == json!([line])
    });
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "{pushed}"
    );
}

/// 19.6 (At apply): a mod the host found on disk at its start has an install generation too, so
/// its remove and reinstall while an `open` is in flight ends the stream `gone`.
#[test]
fn ac2_an_open_in_flight_ends_gone_when_a_producer_loaded_at_start_is_reinstalled() {
    let Env { dir, host } = Env::new(&["latch", "relay", "advo", "advo-front"]);
    drop(host);
    let host = watane_host::Host::open(
        dir.path(),
        watane_host::Mode::Preview,
        watane_host::Limits::default(),
    )
    .unwrap();
    let env = Env { dir, host };
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    env.host.remove(&local("advo"), true).unwrap();
    super::streams::install(&env.host, &["advo"]);
    let s = first.finish()["ok"].as_str().unwrap().to_owned();
    let line = format!("{s} gone");
    wait("the consumer's end", Duration::from_secs(10), || {
        page(&env, "advo-front", "api.ended", Value::Null) == json!([line])
    });
}

// ---- third pass (S96-fix2) ---------------------------------------------------------------

/// 19.6 (At apply): every install, update and removal gives a local name a new install
/// generation, and an `Open` whose side has another generation at apply ends `gone`. The
/// generations live in memory and start empty at `Host::open`: a mod loaded from
/// `installed.json` has none, the `Open` records `None`, and `None` passes every check. After a
/// restart, an update of the producer while an `open` is in flight leaves a live stream opened
/// by the old version (the attack of the second pass, one restart later).
#[test]
fn adv_s96_open_in_flight_outlives_an_update_after_a_restart() {
    let Env { dir, host } = Env::new(&["latch", "relay", "advo", "advo-front"]);
    drop(host);
    std::thread::sleep(Duration::from_millis(300));
    let host = watane_host::Host::open(
        dir.path(),
        watane_host::Mode::Preview,
        watane_host::Limits::default(),
    )
    .unwrap();
    let env = Env { dir, host };
    let copy = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/advo");
    copy_dir(&src, copy.path());
    let toml = copy.path().join("mod.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(&toml, text.replace("summary = \"", "summary = \"v2: ")).unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.open-then-sleep",
        json!({"ms": 1000}),
    );
    let before = env.host.get(&local("advo")).unwrap().package;
    env.host
        .install_folders(&[copy.path().to_path_buf()], 0)
        .unwrap();
    assert_ne!(
        env.host.get(&local("advo")).unwrap().package,
        before,
        "not an update"
    );
    let opened = first.finish();
    let s = opened["ok"].as_str().unwrap().to_owned();
    let pushed = page(&env, "advo", "api.push", json!({"s": s, "bytes": 1}));
    assert!(
        pushed.as_str().unwrap().contains("unknown-stream"),
        "after a restart, a stream opened by the producer's old version is live after its update: push gave {pushed}"
    );
}

/// 19, Terms: "a later install with the same identity finds no timer that carries it." The
/// depth-0 commit checks the origin client again under the catalog lock, by identity alone. A
/// host call of `advo` (its `ended`, origin client `relay`) that runs across `remove` and a new
/// install of `relay` (same identity `local/relay`) finds the identity installed at its commit,
/// and writes the row with the removed install as origin client: the new `relay` inherits it.
#[test]
fn adv_s96_timer_set_across_remove_and_reinstall_carries_the_removed_origin() {
    let env = Env::new(&["latch", "relay", "advo"]);
    let s = open_for_relay(&env);
    // The producer closes; its `ended` runs with `relay` as origin client and spins 300 ms.
    page(&env, "advo", "api.close", json!({"s": s}));
    let start = std::time::Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    env.host.remove(&local("relay"), true).unwrap();
    super::streams::install(&env.host, &["relay"]);
    // advo's `ended` takes advo's lock (not relay's) and spins 300 ms before `after`.
    assert!(
        start.elapsed() < Duration::from_millis(280),
        "the reinstall must land while advo's ended runs: {:?}",
        start.elapsed()
    );
    wait("advo's ended", Duration::from_secs(10), || {
        page(&env, "advo", "api.done", Value::Null) == json!(true)
    });
    let Env { dir, host } = env;
    drop(host);
    let rows = timer_rows(dir.path());
    let row = &rows[&format!("{}/leak", id_of("local/advo"))];
    assert_ne!(
        row["origin"], "local/relay",
        "a timer committed after the removal carries the removed mod as origin client, and the new install inherits it: {row}"
    );
}

/// 19.6 (At apply): "`push`, `close`, `cancel` and `resume` ... act only if the hub still holds
/// that stream with the producer or consumer the call named." `Cancel` (and `Resume`) carry no
/// consumer: the hub ends the stream whoever holds it at apply. `relay`, the consumer, cancels
/// in a chain that stays in flight; meanwhile `relay` hands the stream off to `advo-front`, and
/// that commits first. At apply the stale cancel of the old consumer ends the new consumer's
/// stream.
#[test]
fn adv_s96_cancel_in_flight_ends_the_stream_after_its_hand_off() {
    let env = Env::new(&["latch", "relay", "advo", "advo-front"]);
    let s = env
        .host
        .test_stream(watane_host::TestConsumer::Mod(local("relay")))
        .unwrap();
    let first = in_flight(
        &env,
        "advo-front",
        "api.cancel-then-sleep",
        json!({"s": s.id(), "ms": 1000}),
    );
    let handed = call_as(
        &env.host,
        Caller::Mod(local("advo-front")),
        "relay",
        "api.hand-off",
        json!({"s": s.id()}),
    )
    .unwrap();
    assert_eq!(handed, json!({"ok": null}), "the hand-off must land first");
    first.finish();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        s.ended(),
        None,
        "the old consumer's cancel, applied after the hand-off, ended the new consumer's stream"
    );
}
