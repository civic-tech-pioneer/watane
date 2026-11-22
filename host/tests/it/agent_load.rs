#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! The load of stage A (TECHNICAL 15, "Budgets of stage A" and "V8 record"): 8 host streams
//! of 100 items/s of 1 KiB through `relay` to a page, and the check that no release build
//! holds `test-streams`.

use super::streams::{Page, local, serve_relay};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use watane_host::{TestConsumer, TestStream};

const STREAMS: usize = 8;
const RATE: u64 = 100;
const ITEM: usize = 1024;

fn micros() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros()
}
fn body() -> Vec<u8> {
    let mut b = format!("{:020}", micros()).into_bytes();
    b.resize(ITEM, b'x');
    b
}

struct Run {
    delays: Vec<u128>,
    out_of_order_or_missing: usize,
    items: usize,
    commits: u64,
    commit_time: Duration,
    busy: f64,
    read_per_item: f64,
    written_per_item: f64,
    cancel: Duration,
}
impl Run {
    fn percentile(&self, p: f64) -> f64 {
        let mut d = self.delays.clone();
        d.sort_unstable();
        d[((d.len() as f64 * p) as usize).min(d.len() - 1)] as f64 / 1000.0
    }
}

async fn run(write: bool, seconds: u64) -> Run {
    let w = serve_relay().await;
    let host = Arc::clone(w.host());
    let mut page = Page::open(&w.client, &w.origin).await;
    let mut ups: Vec<TestStream> = vec![];
    let mut downs: Vec<String> = vec![];
    for _ in 0..STREAMS {
        let up = host.test_stream(TestConsumer::Mod(local("relay"))).unwrap();
        let opened = w.call("api.open", json!(null), Some(&page.id)).await;
        let down = opened["value"]["ok"].as_str().unwrap().to_owned();
        w.call(
            "api.forward",
            json!({"up": up.id(), "down": down, "write": write}),
            None,
        )
        .await;
        ups.push(up);
        downs.push(down);
    }
    let before = host.test_stats();
    let started = Instant::now();
    let pusher = std::thread::spawn(move || {
        let total = seconds * RATE;
        for tick in 0..total {
            let due = started + Duration::from_millis(tick * 1000 / RATE);
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            for up in &ups {
                up.push(&body()).unwrap();
            }
        }
        ups
    });
    let expected = (seconds * RATE) as usize;
    let mut next: HashMap<String, u64> = downs.iter().map(|d| (d.clone(), 1)).collect();
    let mut delays = vec![];
    let mut bad = 0;
    let mut items = 0;
    while items < expected * STREAMS {
        let m = page.next().await;
        let now = micros();
        assert_eq!(m["type"], "stream", "{m}");
        let s = m["stream"].as_str().unwrap();
        for item in m["items"].as_array().unwrap() {
            let seq = item["seq"].as_u64().unwrap();
            let want = next.get_mut(s).unwrap();
            if seq != *want {
                bad += 1;
            }
            *want = seq + 1;
            let sent: u128 = item["body"].as_str().unwrap()[..20].parse().unwrap();
            delays.push(now.saturating_sub(sent));
            items += 1;
        }
    }
    let wall = started.elapsed();
    let after = host.test_stats();
    let ups = pusher.join().unwrap();
    // Every stream ended in order: nothing more arrives.
    drop(ups);
    // `cancel` of one page stream, from the page.
    let sent = Instant::now();
    let connection = page.id.clone();
    let cancel = w.call("api.cancel", json!({"s": downs[0]}), Some(&connection));
    let (_, end) = tokio::join!(cancel, async {
        loop {
            let m = page.next().await;
            if m["type"] == "stream-end" && m["stream"] == downs[0] {
                return (Instant::now(), m);
            }
        }
    });
    let cancelled = end.0.duration_since(sent);
    assert_eq!(end.1["outcome"], "cancelled");
    let commit_time = after.commit_time - before.commit_time;
    let run = Run {
        delays,
        out_of_order_or_missing: bad,
        items,
        commits: after.commits - before.commits,
        commit_time,
        busy: commit_time.as_secs_f64() / wall.as_secs_f64(),
        read_per_item: (after.state_bytes_read - before.state_bytes_read) as f64 / items as f64,
        written_per_item: (after.state_bytes_written - before.state_bytes_written) as f64
            / items as f64,
        cancel: cancelled,
    };
    w.served.shutdown().await;
    run
}

fn print(name: &str, r: &Run) {
    let mean = if r.commits == 0 {
        0.0
    } else {
        r.commit_time.as_secs_f64() * 1000.0 / r.commits as f64
    };
    println!(
        "{name}: items {} out-of-order-or-missing {} delay p50 {:.2} ms p95 {:.2} ms \
         cancel-to-end {:.1} ms writer-busy {:.4} commits {} mean-commit {:.3} ms state-bytes read/item {:.2} written/item {:.2}",
        r.items,
        r.out_of_order_or_missing,
        r.percentile(0.5),
        r.percentile(0.95),
        r.cancel.as_secs_f64() * 1000.0,
        r.busy,
        r.commits,
        mean,
        r.read_per_item,
        r.written_per_item
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "slow: stage A budgets and V8"]
async fn ac13_stage_a_budgets_and_v8() {
    let seconds = std::env::var("WATANE_LOAD_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let plain = run(false, seconds).await;
    print("no write", &plain);
    assert_eq!(plain.out_of_order_or_missing, 0);
    assert!(
        plain.percentile(0.95) <= 50.0,
        "p95 {}",
        plain.percentile(0.95)
    );
    assert!(
        plain.cancel <= Duration::from_millis(250),
        "{:?}",
        plain.cancel
    );
    assert!(plain.busy <= 0.10, "writer busy {}", plain.busy);
    // The forwarding mod reads its two routing keys per batch; it writes nothing (15).
    assert_eq!(plain.written_per_item, 0.0);
    let written = run(true, seconds).await;
    print("one write per batch", &written);
    assert_eq!(written.out_of_order_or_missing, 0);
}

#[test]
fn ac12_a_release_build_holds_no_test_streams() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = std::process::Command::new(cargo)
        .args([
            "tree",
            "--offline",
            "-e",
            "features",
            // Inverted over the workspace: a normal dependency of `app` or `dev` that enables
            // the feature shows too, which `-p watane-host` alone does not resolve.
            "-i",
            "watane-host",
            "--workspace",
            "--edges",
            "normal",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("watane-host"), "{text}");
    assert!(!text.contains("test-streams"), "{text}");
}

// ---- Stage B, the host part (TECHNICAL 15, "Budgets of stage B, the host part") ----------

mod stage_b {
    use crate::claude::Rig;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use watane_host::Caller;

    const STREAMS: usize = 8;
    const SMALL: u64 = 10;
    const BIG: u64 = 10_000;

    fn micros() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
    }
    fn can_use(rid: &str, tuid: &str) -> String {
        json!({"type":"control_request","request_id":rid,"request":{
            "subtype":"can_use_tool","tool_name":"Read","input":{"file_path":"a.txt"},
            "tool_use_id":tuid}})
        .to_string()
    }
    /// `assistant` lines `n` in `from..=to`, about 1 KiB each.
    fn entries(from: u64, to: u64) -> String {
        let pad = "x".repeat(1000);
        (from..=to)
            .map(|n| format!(r#"{{"type":"assistant","n":{n},"pad":"{pad}"}}"#) + "\n")
            .collect()
    }
    /// One consumer mod (`claude` or `claude-b`) and the rig that holds the fake.
    struct Side<'a> {
        rig: &'a Rig,
        target: &'static str,
    }
    impl Side<'_> {
        fn call(&self, function: &str, args: Value) -> Value {
            self.rig
                .call_as(Caller::Page, self.target, function, args)
                .unwrap()
        }
        fn decide(&self, s: &str, ask: &str, decision: &str) -> String {
            self.call(
                "api.decide",
                json!({"s": s, "ask": ask, "decision": decision}),
            )
            .as_str()
            .unwrap()
            .to_owned()
        }
        fn ended(&self) -> Vec<String> {
            serde_json::from_value(self.call("api.ended", Value::Null)).unwrap()
        }
        fn log(&self) -> Vec<String> {
            self.rig.log()
        }
    }
    fn tally(rig: &Side, s: &str) -> BTreeMap<String, String> {
        let text = rig.call("api.tally", json!({"s": s}));
        text.as_str()
            .unwrap()
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
            .collect()
    }
    /// Polls `done` every `ms` milliseconds. The times that the budgets use are stamped by the
    /// consumer's sink or are read from the fake's log, so a slow poll costs no precision, and a
    /// fast one would only compete with the sink for the consumer's call lock.
    fn wait_for(what: &str, secs: u64, ms: u64, mut done: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(secs);
        while !done() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
    fn p95(mut v: Vec<f64>) -> f64 {
        v.sort_by(f64::total_cmp);
        v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
    }

    #[derive(Debug, Default)]
    struct Marks {
        small_entries: f64,
        small_ask: f64,
        answer: f64,
        interrupt: f64,
        big_turn: f64,
        cancel: f64,
        bad: u64,
        taken: u64,
    }

    fn one(rig: &Side, s: &str) -> Marks {
        let mut m = Marks::default();
        let ms = |from: u128, to: u128| to.saturating_sub(from) as f64 / 1000.0;
        // Turn 1: 10 entries and an ask.
        // The clock starts when the `say` call has returned: the turn is the fake's, and the
        // call's own chain (one at a time per mod) is not the stream's path.
        assert_eq!(
            rig.call("api.say", json!({"s": s, "text": "one"})),
            json!("ok")
        );
        let t = micros();
        wait_for("turn 1", 30, 25, || {
            let t = tally(rig, s);
            t.get("next").is_some_and(|n| n == &(SMALL + 1).to_string()) && t.contains_key("ask1")
        });
        let tl = tally(rig, s);
        m.small_entries = ms(t, tl["last"].parse().unwrap());
        m.small_ask = ms(t, tl["ask1"].parse().unwrap());
        // Its answer reaches the fake's stdin.
        let rid = format!(
            "r{}a",
            tl["tu1"].trim_start_matches('t').trim_end_matches('a')
        );
        let t = micros();
        assert_eq!(rig.decide(s, "1", "allow"), "ok");
        let seen = |rid: &str| {
            rig.log()
                .iter()
                .any(|l| l.starts_with("IN ") && l.contains(&format!(r#""request_id":"{rid}""#)))
        };
        let mut at = 0;
        wait_for("the answer on stdin", 30, 1, || {
            let now = micros();
            if seen(&rid) {
                at = now;
                true
            } else {
                false
            }
        });
        m.answer = ms(t, at);
        // Turn 2: 10 000 entries after two asks; the first is answered, an interrupt closes the
        // second while the entries flow.
        assert_eq!(
            rig.call("api.say", json!({"s": s, "text": "two"})),
            json!("ok")
        );
        let t2 = micros();
        wait_for("the asks of turn 2", 30, 25, || {
            tally(rig, s).contains_key("ask3")
        });
        assert_eq!(rig.decide(s, "2", "allow"), "ok");
        let t = micros();
        assert_eq!(rig.call("api.interrupt", json!({"s": s})), json!("ok"));
        wait_for("the interrupt's close", 30, 25, || {
            tally(rig, s).contains_key("res3")
        });
        m.interrupt = ms(t, tally(rig, s)["res3"].parse().unwrap());
        let total = SMALL + BIG + 1;
        wait_for("turn 2", 60, 100, || {
            tally(rig, s)
                .get("next")
                .is_some_and(|n| n == &total.to_string())
        });
        let tl = tally(rig, s);
        m.big_turn = ms(t2, tl["last"].parse().unwrap());
        m.bad = tl["bad"].parse().unwrap();
        m.taken = tl["next"].parse::<u64>().unwrap() - 1;
        // `cancel` to `ended`.
        let t = Instant::now();
        rig.call("api.cancel", json!({"s": s}));
        wait_for("ended", 30, 1, || {
            rig.ended().iter().any(|l| l == &format!("{s} cancelled"))
        });
        m.cancel = t.elapsed().as_secs_f64() * 1000.0;
        m
    }

    #[test]
    #[ignore = "slow: the stage B host budgets, 8 streams of 10 010 entries"]
    fn ac9_stage_b_host_budgets() {
        let launch = r#"{"version":1,"workspace":"scratch","ask":true,"tools":["Read"]}"#;
        let rig = Rig::new(&[]);
        let turn1 = can_use("r@P@a", "t@P@a") + "\n" + &entries(1, SMALL);
        let turn2 = can_use("r@P@b", "t@P@b")
            + "\n"
            + &can_use("r@P@c", "t@P@c")
            + "\n"
            + &entries(SMALL + 1, SMALL + BIG);
        rig.put("after.2", turn1.as_bytes());
        rig.put("after.4", turn2.as_bytes());
        // Two consumer mods, `claude` and `claude-b` (the same fixture), each with its own
        // four streams.
        let sides = ["claude", "claude-b"].map(|target| Side { rig: &rig, target });
        for side in &sides {
            side.call("api.quiet", Value::Null);
        }
        let mut streams: Vec<(&Side, String)> = vec![];
        for i in 0..STREAMS {
            let side = &sides[i % 2];
            let s = side.call("api.start", json!({"launch": launch}));
            let s = s.as_str().unwrap().to_owned();
            assert_eq!(s.len(), 32, "{s}");
            streams.push((side, s));
        }
        let before = rig.host().test_stats();
        let started = Instant::now();
        let marks: Vec<Marks> = std::thread::scope(|scope| {
            let jobs: Vec<_> = streams
                .iter()
                .map(|(side, s)| scope.spawn(move || one(side, s)))
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap()).collect()
        });
        let wall = started.elapsed();
        let after = rig.host().test_stats();
        let busy = (after.commit_time - before.commit_time).as_secs_f64() / wall.as_secs_f64();
        let entries_taken: u64 = marks.iter().map(|m| m.taken).sum();
        let written =
            (after.state_bytes_written - before.state_bytes_written) as f64 / entries_taken as f64;
        let col = |f: fn(&Marks) -> f64| p95(marks.iter().map(f).collect());
        println!(
            "stage B host: streams {STREAMS} entries {entries_taken} \
             small-turn p95 {:.1} ms, small-ask p95 {:.1} ms, answer p95 {:.1} ms, \
             big-turn p95 {:.0} ms, interrupt p95 {:.1} ms, cancel p95 {:.1} ms, \
             writer-busy {:.4}, state bytes written per entry {:.2}",
            col(|m| m.small_entries),
            col(|m| m.small_ask),
            col(|m| m.answer),
            col(|m| m.big_turn),
            col(|m| m.interrupt),
            col(|m| m.cancel),
            busy,
            written,
        );
        assert!(marks.iter().all(|m| m.bad == 0), "{marks:?}");
        assert!(marks.iter().all(|m| m.taken == SMALL + BIG), "{marks:?}");
        assert!(col(|m| m.small_entries) <= 50.0);
        assert!(col(|m| m.small_ask) <= 50.0);
        assert!(col(|m| m.answer) <= 50.0);
        assert!(col(|m| m.big_turn) <= 10_000.0);
        assert!(col(|m| m.interrupt) <= 250.0);
        assert!(col(|m| m.cancel) <= 250.0);
        // The consumer fixture writes one small tally per batch; the host writes nothing.
        assert!(busy <= 0.10, "writer busy {busy}");
    }
}
