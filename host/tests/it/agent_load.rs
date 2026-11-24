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

/// Held for a whole measured run: the two load tests never overlap, so each budget is measured alone.
static LOAD: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
#[ignore = "slow: stage A budgets and V8"]
fn ac13_stage_a_budgets_and_v8() {
    // Lock before building the runtime, so no worker blocks on it.
    let _serial = LOAD.lock().unwrap();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
        .block_on(stage_a_budgets_and_v8());
}

async fn stage_a_budgets_and_v8() {
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
    /// The fake's rate (`fake-claude/claude`, "Rate mode"): deltas and entries per second, bytes
    /// of an entry, seconds of the turn.
    const RATE: &str = "50 5 4096 14";
    /// The interrupt comes this long after the turn started: the fake has written the rate for
    /// more than 10 s.
    const INTERRUPT_AFTER: Duration = Duration::from_millis(10_600);

    fn micros() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros()
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
        fn tally(&self, s: &str) -> BTreeMap<String, String> {
            let text = self.call("api.tally", json!({"s": s}));
            text.as_str()
                .unwrap()
                .lines()
                .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                .collect()
        }
        /// A time the consumer stamped in its tally (microseconds), once it is there.
        fn at(&self, s: &str, key: &str) -> Option<u128> {
            self.tally(s).get(key).map(|v| v.parse().unwrap())
        }
    }
    /// Polls `done` every `ms` milliseconds. The times that the budgets use are stamped by the
    /// consumer's sink or by the fake, so a slow poll costs no precision, and a fast one would
    /// only compete with the sink for the consumer's call lock.
    fn wait_for<T>(what: &str, secs: u64, ms: u64, mut done: impl FnMut() -> Option<T>) -> T {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            if let Some(v) = done() {
                return v;
            }
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
    /// The fields of the entry `kind` of the fake's `stamps.<pid>` whose first field is `key`.
    fn stamps(rig: &Rig, pid: &str, kind: &str, key: &str) -> Option<Vec<u128>> {
        let text = std::fs::read_to_string(rig.fake.path().join(format!("stamps.{pid}"))).ok()?;
        text.lines().find_map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            (f[0] == kind && f.get(1) == Some(&key))
                .then(|| f[2..].iter().map(|v| v.parse().unwrap()).collect())
        })
    }
    /// The time the fake stamped for `kind` and `key`, in microseconds.
    fn stamped(rig: &Rig, pid: &str, kind: &str, key: &str) -> Option<u128> {
        stamps(rig, pid, kind, key).map(|v| v[0])
    }
    /// The time of the fake's entry `kind` that has no key (`interrupt`, `result`).
    fn stamp_of(rig: &Rig, pid: &str, kind: &str) -> Option<u128> {
        let text = std::fs::read_to_string(rig.fake.path().join(format!("stamps.{pid}"))).ok()?;
        text.lines().find_map(|l| {
            let f: Vec<&str> = l.split(' ').collect();
            (f[0] == kind).then(|| f.last().unwrap().parse().unwrap())
        })
    }
    /// The delay histogram of a tally (`h<ms>` keys) and its count.
    fn delay_buckets(tally: &BTreeMap<String, String>) -> ([u64; 102], u64) {
        let mut buckets = [0u64; 102];
        for (k, v) in tally {
            if let Some(ms) = k.strip_prefix('h') {
                buckets[ms.parse::<usize>().unwrap()] += v.parse::<u64>().unwrap();
            }
        }
        (buckets, buckets.iter().sum())
    }
    /// The upper edge, in ms, of the bucket that holds the 95th percentile.
    fn bucket_p95(buckets: &[u64; 102], count: u64) -> usize {
        let mut seen = 0;
        for (ms, n) in buckets.iter().enumerate() {
            seen += n;
            if seen as f64 >= count as f64 * 0.95 {
                return ms + 1;
            }
        }
        buckets.len()
    }
    fn p95(mut v: Vec<f64>) -> f64 {
        v.sort_by(f64::total_cmp);
        v[((v.len() as f64 * 0.95) as usize).min(v.len() - 1)]
    }

    #[derive(Debug, Default)]
    struct Marks {
        ask_taken: f64,
        answer: f64,
        interrupt_resolved: f64,
        interrupt_result: f64,
        cancel: f64,
        bad: u64,
        written: u64,
        taken: u64,
        batches: u64,
        /// Per stream: the delta rate and the entry rate per second as the consumer read them,
        /// and the smallest and largest entry (bytes).
        delta_rate: f64,
        /// The turn window (s): the first to the last item, one for both rates.
        window: f64,
        entry_rate: f64,
        entries: u64,
        entry_size: (u64, u64),
        /// The 95th percentile of the stream's item delay (ms, upper edge).
        delay_p95: usize,
        delay_count: u64,
    }

    fn one(rig: &Rig, side: &Side, s: &str) -> Marks {
        let mut m = Marks::default();
        let ms = |from: u128, to: u128| to.saturating_sub(from) as f64 / 1000.0;
        assert_eq!(
            side.call("api.say", json!({"s": s, "text": "go"})),
            json!("ok")
        );
        let said = Instant::now();
        // The first ask: the fake's write to the consumer's sink.
        let first = wait_for("the first ask", 30, 50, || {
            side.tally(s).get("tu1").cloned()
        });
        let pid = first
            .trim_start_matches('t')
            .trim_end_matches('a')
            .to_owned();
        let written = stamped(rig, &pid, "ask", &first).unwrap();
        m.ask_taken = ms(written, side.at(s, "ask1").unwrap());
        // Its answer to the fake's stdin.
        let rid = format!("r{pid}a");
        let t = micros();
        assert_eq!(
            side.call(
                "api.decide",
                json!({"s": s, "ask": "1", "decision": "allow"})
            ),
            json!("ok")
        );
        let at = wait_for("the answer on stdin", 30, 2, || {
            stamped(rig, &pid, "answer", &rid)
        });
        m.answer = ms(t, at);
        // The second ask stays open; the interrupt closes it while the lines flow.
        wait_for("the second ask", 30, 50, || side.at(s, "ask2"));
        std::thread::sleep(INTERRUPT_AFTER.saturating_sub(said.elapsed()));
        let t = micros();
        assert_eq!(side.call("api.interrupt", json!({"s": s})), json!("ok"));
        let resolved = wait_for("ask-resolved", 30, 20, || side.at(s, "res2"));
        m.interrupt_resolved = ms(t, resolved);
        let result = wait_for("the result", 30, 20, || side.at(s, "result"));
        // The result is the one the interrupt caused: an interrupted turn's subtype, and the
        // fake wrote it after it read the interrupt (its stamps), which came after the call.
        let tl = side.tally(s);
        assert_eq!(tl["rsub"], "error_during_execution", "stale result of {s}");
        let interrupted = stamp_of(rig, &pid, "interrupt").expect("the fake stamped the interrupt");
        let wrote = stamp_of(rig, &pid, "result").expect("the fake stamped the result");
        assert!(
            t <= interrupted && interrupted <= wrote,
            "{t} {interrupted} {wrote}"
        );
        assert!(interrupted <= result, "result of {s} before the interrupt");
        m.interrupt_result = ms(t, result);
        // Every line the fake wrote arrived, once and in order.
        let tl = side.tally(s);
        m.written = lines_written(rig, &pid);
        m.taken = tl["next"].parse::<u64>().unwrap() - 1;
        m.bad = tl["bad"].parse().unwrap();
        // The load itself, per stream: the rate of each kind over the run (the first to the last
        // stamp of the fake's own clock) and the size of each entry.
        let num = |k: &str| tl[k].parse::<u64>().unwrap();
        // Both rates over one window: the turn's first to last item of either kind, on the
        // fake's clock.
        let (first, last) = ["d", "e"].iter().fold((u64::MAX, 0), |(lo, hi), k| {
            (
                lo.min(num(&format!("{k}first"))),
                hi.max(num(&format!("{k}last"))),
            )
        });
        m.window = (last - first) as f64 / 1e6;
        let rate = |k: &str| num(&format!("n{k}")) as f64 / m.window;
        m.delta_rate = rate("d");
        m.entry_rate = rate("e");
        m.entries = num("ne");
        m.entry_size = (num("emin"), num("emax"));
        let (buckets, count) = delay_buckets(&tl);
        m.delay_p95 = bucket_p95(&buckets, count);
        m.delay_count = count;
        // `cancel` to `ended`: the stream had not ended, and the `ended` is the cancel's.
        assert!(side.at(s, "ended").is_none(), "{s} ended before the cancel");
        let t = micros();
        side.call("api.cancel", json!({"s": s}));
        let ended = wait_for("ended", 30, 20, || side.at(s, "ended"));
        assert!(ended >= t, "ended of {s} before the cancel");
        m.cancel = ms(t, ended);
        assert!(
            side.call("api.ended", Value::Null)
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l == &json!(format!("{s} cancelled")))
        );
        m.batches = side.tally(s)["batches"].parse().unwrap();
        m
    }

    /// `lines <N> <us>`: the last number the fake wrote.
    fn lines_written(rig: &Rig, pid: &str) -> u64 {
        let text = std::fs::read_to_string(rig.fake.path().join(format!("stamps.{pid}"))).unwrap();
        let l = text.lines().find(|l| l.starts_with("lines ")).unwrap();
        l.split(' ').nth(1).unwrap().parse().unwrap()
    }

    fn loadavg() -> String {
        std::fs::read_to_string("/proc/loadavg")
            .map(|t| t.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|_| "unknown".into())
    }

    #[test]
    #[ignore = "slow: the stage B host budgets, 8 sessions at 50 deltas and 5 entries a second for 10 s"]
    fn ac1_stage_b_load() {
        let _serial = super::LOAD.lock().unwrap();
        let launch = r#"{"version":1,"workspace":"scratch","ask":true,"tools":["Read"]}"#;
        let rig = Rig::new(&[("rate", RATE.as_bytes())]);
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
        // The starts have settled (each stream's `initialized` reached its consumer, after the
        // scratch workspace's row), so the window below holds the load and no start commit.
        for (side, s) in &streams {
            wait_for("initialized", 30, 20, || side.at(s, "init"));
        }
        let load_before = loadavg();
        // The batches that the settled starts already wrote are not in the window.
        let batches_before: u64 = streams
            .iter()
            .map(|(side, s)| side.tally(s)["batches"].parse::<u64>().unwrap())
            .sum();
        let before = rig.host().test_stats();
        let started = Instant::now();
        let marks: Vec<Marks> = std::thread::scope(|scope| {
            let jobs: Vec<_> = streams
                .iter()
                .map(|(side, s)| {
                    let rig = &rig;
                    scope.spawn(move || one(rig, side, s))
                })
                .collect();
            jobs.into_iter().map(|j| j.join().unwrap()).collect()
        });
        let wall = started.elapsed();
        let after = rig.host().test_stats();
        // Item delay, the stamp of the fake's write to the sink's read: the p95 of each session,
        // and the worst of them is the figure of the budget. `count` is every stamped line.
        let delay_p95 = marks.iter().map(|m| m.delay_p95).max().unwrap();
        let count: u64 = marks.iter().map(|m| m.delay_count).sum();
        // The writer's commits by owner: the host's own (timer writes) and the mods' (the
        // consumers' tally, one write per batch), counted on the commit path.
        let commits = after.commits - before.commits;
        let host_commits = after.host_commits - before.host_commits;
        let consumer_commits = commits - host_commits;
        let commit_time = (after.commit_time - before.commit_time).as_secs_f64();
        let host_time = (after.host_commit_time - before.host_commit_time).as_secs_f64();
        let mean = commit_time / commits.max(1) as f64;
        let busy = commit_time / wall.as_secs_f64();
        let host_busy = host_time / wall.as_secs_f64();
        let batches: u64 = marks.iter().map(|m| m.batches).sum::<u64>() - batches_before;
        let col = |f: fn(&Marks) -> f64| p95(marks.iter().map(f).collect());
        let written: u64 = marks.iter().map(|m| m.written).sum();
        println!(
            "stage B host: sessions {STREAMS} lines {written} item-delay p95 {delay_p95} ms \
             over {count} lines, ask taken p95 {:.1} ms, answer to stdin p95 {:.1} ms, \
             interrupt to ask-resolved p95 {:.1} ms, interrupt to result p95 {:.1} ms, \
             cancel to ended p95 {:.1} ms, writer busy {busy:.4} (host {host_busy:.4}, \
             consumers {:.4}; {commits} commits, {host_commits} of the host, mean \
             {:.3} ms, {batches} batches), delta rate {:.1}-{:.1}/s entry rate {:.2}-{:.2}/s over the turn window {:.1}-{:.1} s (first to last item), counters from after every `initialized`, wall {:.1} s, load average before {load_before} after {}",
            col(|m| m.ask_taken),
            col(|m| m.answer),
            col(|m| m.interrupt_resolved),
            col(|m| m.interrupt_result),
            col(|m| m.cancel),
            busy - host_busy,
            mean * 1000.0,
            marks.iter().map(|m| m.delta_rate).fold(f64::MAX, f64::min),
            marks.iter().map(|m| m.delta_rate).fold(0.0, f64::max),
            marks.iter().map(|m| m.entry_rate).fold(f64::MAX, f64::min),
            marks.iter().map(|m| m.entry_rate).fold(0.0, f64::max),
            marks.iter().map(|m| m.window).fold(f64::MAX, f64::min),
            marks.iter().map(|m| m.window).fold(0.0, f64::max),
            wall.as_secs_f64(),
            loadavg(),
        );
        assert!(marks.iter().all(|m| m.bad == 0), "{marks:?}");
        assert!(marks.iter().all(|m| m.taken == m.written), "{marks:?}");
        // The workload, per stream: 50 deltas and 5 entries of 4096 bytes a second, each within
        // 10%, and a turn that the interrupt (not its end) stopped.
        assert!(
            marks.iter().all(|m| (45.0..=55.0).contains(&m.delta_rate)),
            "{marks:?}"
        );
        assert!(
            marks.iter().all(|m| (4.5..=5.5).contains(&m.entry_rate)),
            "{marks:?}"
        );
        assert!(marks.iter().all(|m| m.entries >= 45), "{marks:?}");
        assert!(
            marks.iter().all(|m| m.entry_size == (4096, 4096)),
            "{marks:?}"
        );
        assert!(marks.iter().all(|m| m.written > 10 * 55), "{marks:?}");
        // The mods' commits are at least the consumers' batches (a call that writes state, such
        // as `api.say`, adds its own).
        assert!(
            consumer_commits >= batches,
            "commits by owner: {consumer_commits} {batches}"
        );
        assert_eq!(count, written, "every line has a delay");
        assert!(delay_p95 <= 100, "item delay p95 {delay_p95} ms");
        assert!(col(|m| m.ask_taken) <= 100.0, "{marks:?}");
        assert!(col(|m| m.answer) <= 50.0, "{marks:?}");
        assert!(col(|m| m.interrupt_resolved) <= 1000.0, "{marks:?}");
        assert!(col(|m| m.interrupt_result) <= 1000.0, "{marks:?}");
        assert!(col(|m| m.cancel) <= 250.0, "{marks:?}");
        assert!(busy <= 0.50, "writer busy {busy}");
    }
}
