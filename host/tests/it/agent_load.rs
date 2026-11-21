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
