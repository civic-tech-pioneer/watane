#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! Stage A1 streams (TECHNICAL 19.2, 19.9; FUNCTIONAL S48 to S52): `relay`, `latch` and the
//! test streams, with a `reqwest` reader for a page consumer.

use reqwest::Client;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use watane_host::{
    Caller, DeliveryAnswer, Host, Limits, LocalName, Mode, StreamError, TestClock, TestConsumer,
    TestStream,
};

pub(crate) fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
pub(crate) fn local(name: &str) -> LocalName {
    LocalName::parse(name).unwrap()
}
/// A preview home with fixtures installed.
pub(crate) struct Env {
    pub dir: TempDir,
    pub host: Arc<Host>,
}
impl Env {
    pub fn new(names: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::open(dir.path(), Mode::Preview, Limits::default()).unwrap();
        install(&host, names);
        Self { dir, host }
    }
    pub fn relay() -> Self {
        Self::new(&["latch", "relay", "relay-front"])
    }
}
pub(crate) fn install(host: &Arc<Host>, names: &[&str]) {
    host.install_folders(&names.iter().map(|n| fixture(n)).collect::<Vec<_>>(), 0)
        .unwrap();
}
pub(crate) fn call_as(
    host: &Arc<Host>,
    caller: Caller,
    target: &str,
    function: &str,
    args: Value,
) -> Result<Value, watane_host::CallError> {
    let host = Arc::clone(host);
    let target = local(target);
    let function = function.to_owned();
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || host.call(&caller, &target, &function, args))
        .unwrap()
        .join()
        .unwrap()
}
/// A call of `relay` as the page.
pub(crate) fn relay(host: &Arc<Host>, function: &str, args: Value) -> Value {
    call_as(host, Caller::Page, "relay", function, args).unwrap()
}
pub(crate) fn lines(host: &Arc<Host>, function: &str) -> Vec<String> {
    serde_json::from_value(relay(host, function, Value::Null)).unwrap()
}
pub(crate) fn wait(what: &str, timeout: Duration, check: impl Fn() -> bool) {
    let start = Instant::now();
    while !check() {
        assert!(start.elapsed() < timeout, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub(crate) fn settle() {
    std::thread::sleep(Duration::from_millis(300));
}
pub(crate) fn test_stream(host: &Arc<Host>) -> TestStream {
    host.test_stream(TestConsumer::Mod(local("relay"))).unwrap()
}
fn deliveries(s: &TestStream) -> Vec<(Vec<u64>, DeliveryAnswer)> {
    s.deliveries()
        .into_iter()
        .map(|d| (d.seqs, d.answer))
        .collect()
}
fn wait_deliveries(s: &TestStream, n: usize) {
    wait("deliveries", Duration::from_secs(10), || {
        s.deliveries().len() >= n
    });
}
fn set_mode(host: &Arc<Host>, mode: &str) {
    relay(host, "api.mode", json!({"m": mode}));
}
/// The relay holds the batch `[1]`, deferred, so that later pushes join one batch (19.2).
fn defer_first(host: &Arc<Host>, s: &TestStream, first: &[u8]) {
    set_mode(host, "defer");
    s.push(first).unwrap();
    wait_deliveries(s, 1);
}
fn resume(host: &Arc<Host>, s: &TestStream) {
    assert_eq!(
        relay(host, "api.resume", json!({"s": s.id()})),
        json!({"ok":null})
    );
}

#[test]
fn ac1_s48_a_failed_batch_is_redelivered_once_with_the_same_numbers() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    assert_eq!(s.id().len(), 32);
    defer_first(&env.host, &s, b"a");
    s.push(b"b").unwrap();
    set_mode(&env.host, "trap-once");
    resume(&env.host, &s);
    wait_deliveries(&s, 3);
    settle();
    assert_eq!(
        deliveries(&s),
        vec![
            (vec![1], DeliveryAnswer::Deferred),
            (vec![1, 2], DeliveryAnswer::Failed),
            (vec![1, 2], DeliveryAnswer::Taken),
        ]
    );
    assert_eq!(lines(&env.host, "api.received"), ["a", "b"]);
    assert_eq!(s.ended(), None);
}

#[test]
fn ac1_s48_a_second_failure_ends_the_stream_consumer_failed() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    defer_first(&env.host, &s, b"a");
    s.push(b"b").unwrap();
    set_mode(&env.host, "trap");
    resume(&env.host, &s);
    wait("the end", Duration::from_secs(10), || s.ended().is_some());
    assert_eq!(s.ended(), Some(json!("consumer-failed")));
    assert_eq!(
        deliveries(&s),
        vec![
            (vec![1], DeliveryAnswer::Deferred),
            (vec![1, 2], DeliveryAnswer::Failed),
            (vec![1, 2], DeliveryAnswer::Failed),
        ]
    );
    wait("the line", Duration::from_secs(10), || {
        lines(&env.host, "api.ended") == [format!("{} consumer-failed", s.id())]
    });
}

#[test]
fn ac2_s49_a_deferred_batch_waits_for_resume_and_is_not_a_failure() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    defer_first(&env.host, &s, b"a");
    assert_eq!(deliveries(&s), vec![(vec![1], DeliveryAnswer::Deferred)]);
    assert_eq!(lines(&env.host, "api.received"), Vec::<String>::new());
    assert!(s.paused());
    s.push(b"b").unwrap();
    settle();
    assert_eq!(s.deliveries().len(), 1);
    // Two defers in a row are no failure.
    resume(&env.host, &s);
    wait_deliveries(&s, 2);
    assert_eq!(s.deliveries()[1].seqs, [1, 2]);
    assert_eq!(s.deliveries()[1].answer, DeliveryAnswer::Deferred);
    settle();
    assert_eq!(s.ended(), None);
    set_mode(&env.host, "taken");
    resume(&env.host, &s);
    wait_deliveries(&s, 3);
    assert_eq!(
        s.deliveries()[2],
        watane_host::Delivery {
            seqs: vec![1, 2],
            answer: DeliveryAnswer::Taken
        }
    );
    assert_eq!(lines(&env.host, "api.received"), ["a", "b"]);
    settle();
    assert_eq!(s.ended(), None);
    assert!(!s.paused());
}

#[test]
fn ac2_s49_a_stream_deferred_for_five_minutes_ends_consumer_failed() {
    let env = Env::relay();
    let clock = TestClock::new();
    env.host.set_test_clock(clock.clone());
    let s = test_stream(&env.host);
    defer_first(&env.host, &s, b"a");
    clock.advance(Duration::from_secs(299));
    settle();
    assert_eq!(s.ended(), None);
    clock.advance(Duration::from_secs(2));
    wait("the backstop", Duration::from_secs(10), || {
        s.ended().is_some()
    });
    assert_eq!(s.ended(), Some(json!("consumer-failed")));
    wait("the line", Duration::from_secs(10), || {
        lines(&env.host, "api.ended") == [format!("{} consumer-failed", s.id())]
    });
}

#[test]
fn ac2_s49_a_deferred_calls_pushes_and_writes_are_gone() {
    let env = Env::relay();
    let up = test_stream(&env.host);
    // `down`: relay is its producer and, as the top-level caller, its consumer.
    let down = call_as(
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
    // The consumer defers whatever it gets: `down` keeps its items.
    defer_first(&env.host, &up, b"u1");
    assert_eq!(
        relay(&env.host, "api.push", json!({"s": down, "text": "seed"})),
        json!({"ok":null})
    );
    settle();
    for i in 2..=256 {
        up.push(format!("u{i}").as_bytes()).unwrap();
    }
    assert_eq!(up.push(b"over"), Err(StreamError::Full));
    relay(
        &env.host,
        "api.forward",
        json!({"up": up.id(), "down": down, "write": false}),
    );
    resume(&env.host, &up);
    // 256 pushes do not fit beside the seed: the call defers and its pushes are dropped.
    wait_deliveries(&up, 2);
    assert_eq!(up.deliveries()[1].answer, DeliveryAnswer::Deferred);
    assert_eq!(up.deliveries()[1].seqs.len(), 256);
    set_mode(&env.host, "taken");
    assert_eq!(
        relay(&env.host, "api.resume", json!({"s": down})),
        json!({"ok":null})
    );
    // The rolled-back pushes left nothing in `down`; `ready` then resumes `up`, whose batch
    // goes through whole: `down` holds the seed and each `u<n>` once.
    let mut expected = vec!["seed".to_owned()];
    expected.extend((1..=256).map(|i| format!("u{i}")));
    wait("the whole batch", Duration::from_secs(20), || {
        lines(&env.host, "api.received").len() >= 257
    });
    settle();
    assert_eq!(lines(&env.host, "api.received"), expected);
}

#[test]
fn ac3_s50_the_257th_push_is_full_and_the_lag_clock_ends_the_stream() {
    let env = Env::relay();
    let clock = TestClock::new();
    env.host.set_test_clock(clock.clone());
    let s = env
        .host
        .test_stream(TestConsumer::Held { origin: "o".into() })
        .unwrap();
    for _ in 0..256 {
        s.push(b"x").unwrap();
    }
    assert_eq!(s.push(b"x"), Err(StreamError::Full));
    assert!(s.paused());
    clock.advance(Duration::from_secs(9));
    settle();
    assert_eq!(s.ended(), None);
    clock.advance(Duration::from_secs(2));
    wait("lagging", Duration::from_secs(10), || s.ended().is_some());
    assert_eq!(s.ended(), Some(json!("lagging")));
}

#[test]
fn ac3_s50_a_consumer_that_defers_is_never_lagging() {
    let env = Env::relay();
    let clock = TestClock::new();
    env.host.set_test_clock(clock.clone());
    let s = test_stream(&env.host);
    defer_first(&env.host, &s, b"a");
    for _ in 1..256 {
        s.push(b"x").unwrap();
    }
    assert_eq!(s.push(b"x"), Err(StreamError::Full));
    clock.advance(Duration::from_secs(60));
    settle();
    assert_eq!(s.ended(), None);
    // After `resume`, a new clock starts only at a failed push.
    set_mode(&env.host, "defer");
    resume(&env.host, &s);
    wait_deliveries(&s, 2);
    clock.advance(Duration::from_secs(60));
    settle();
    assert_eq!(s.ended(), None);
}

#[test]
fn ac4_s51_one_held_stream_holds_4_mib_less_one_and_the_stream_limit_is_full() {
    let env = Env::relay();
    let s = env
        .host
        .test_stream(TestConsumer::Held {
            origin: "o1".into(),
        })
        .unwrap();
    s.push(&vec![0; 4 << 20]).unwrap();
    assert_eq!(s.push(b"x"), Err(StreamError::Full));
    assert_eq!(s.push(&vec![0; (4 << 20) + 1]), Err(StreamError::TooLarge));
    assert_eq!(s.take().len(), 1);
    s.push(b"x").unwrap();
}

#[test]
#[ignore = "slow: queues 128 MiB for one origin client"]
fn ac4_s51_the_origin_clients_128_mib_refuses_a_push_the_stream_would_take() {
    let env = Env::relay();
    let held: Vec<TestStream> = (0..33)
        .map(|_| {
            env.host
                .test_stream(TestConsumer::Held {
                    origin: "o1".into(),
                })
                .unwrap()
        })
        .collect();
    for s in &held[..32] {
        s.push(&vec![0; (4 << 20) - 1]).unwrap();
    }
    assert_eq!(held[32].push(&[0; 64]), Err(StreamError::Full));
    // The consumer takes; the push fits again.
    assert_eq!(held[0].take().len(), 1);
    held[32].push(&[0; 64]).unwrap();
}

#[test]
#[ignore = "slow: queues 512 MiB for the host"]
fn ac4_s51_the_hosts_512_mib_refuses_a_push() {
    let env = Env::relay();
    let mut streams = vec![];
    for origin in ["o1", "o2", "o3", "o4"] {
        for _ in 0..32 {
            let s = env
                .host
                .test_stream(TestConsumer::Held {
                    origin: origin.into(),
                })
                .unwrap();
            s.push(&vec![0; (4 << 20) - 2]).unwrap();
            streams.push(s);
        }
        // 128 MiB less 64 bytes in all: 32 x (4 MiB - 2) is 128 MiB - 64.
    }
    let fifth = env
        .host
        .test_stream(TestConsumer::Held {
            origin: "o5".into(),
        })
        .unwrap();
    assert_eq!(fifth.push(&[0; 4096]), Err(StreamError::Full));
    fifth.push(&[0; 64]).unwrap();
}

// ---- a page as the consumer ------------------------------------------------------------

pub(crate) struct Page {
    pub response: reqwest::Response,
    buffered: Vec<u8>,
    pub id: String,
}
impl Page {
    pub async fn open(client: &Client, origin: &str) -> Self {
        let response = client.get(format!("{origin}/events")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let mut page = Self {
            response,
            buffered: vec![],
            id: String::new(),
        };
        let first = page.message().await;
        assert_eq!(first["type"], "connection");
        page.id = first["id"].as_str().unwrap().to_owned();
        assert!(page.id.len() == 32 && page.id.bytes().all(|b| b.is_ascii_hexdigit()));
        page
    }
    pub async fn message(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                if let Some(end) = self.buffered.windows(2).position(|w| w == b"\n\n") {
                    let frame =
                        String::from_utf8(self.buffered.drain(..end + 2).collect()).unwrap();
                    if let Some(data) = frame.strip_prefix("data: ") {
                        return serde_json::from_str(data.trim_end()).unwrap();
                    }
                    continue;
                }
                self.buffered
                    .extend(self.response.chunk().await.unwrap().expect("stream ended"));
            }
        })
        .await
        .expect("no message")
    }
    /// The next message that is not `changed`.
    pub async fn next(&mut self) -> Value {
        loop {
            let m = self.message().await;
            if m["type"] != "changed" {
                return m;
            }
        }
    }
}
pub(crate) struct Served {
    pub _env: Env,
    pub served: watane_host::Served,
    pub client: Client,
    pub origin: String,
}
pub(crate) async fn serve_relay() -> Served {
    let env = Env::relay();
    let served = env.host.serve(0, axum::Router::new()).await.unwrap();
    let client = Client::builder()
        .no_proxy()
        .resolve("relay.localhost", ([127, 0, 0, 1], served.port).into())
        .build()
        .unwrap();
    let origin = format!("http://relay.localhost:{}", served.port);
    Served {
        _env: env,
        served,
        client,
        origin,
    }
}
impl Served {
    pub async fn call(&self, function: &str, args: Value, connection: Option<&str>) -> Value {
        let mut body = json!({"fn": function, "args": args});
        if let Some(c) = connection {
            body["connection"] = json!(c);
        }
        let response = self
            .client
            .post(format!("{}/call", self.origin))
            .header("Origin", &self.origin)
            .json(&body)
            .send()
            .await
            .unwrap();
        response.json::<Value>().await.unwrap()
    }
    pub fn host(&self) -> &Arc<Host> {
        &self._env.host
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_s52_a_page_is_a_consumer_through_its_connection_id() {
    let w = serve_relay().await;
    let mut page = Page::open(&w.client, &w.origin).await;
    let opened = w.call("api.open", json!(null), Some(&page.id)).await;
    let s = opened["value"]["ok"].as_str().unwrap().to_owned();
    assert_eq!(s.len(), 32);
    let pushed = w.call("api.push", json!({"s": s, "text": "x"}), None).await;
    assert_eq!(pushed, json!({"value": {"ok": null}}));
    assert_eq!(
        page.next().await,
        json!({"type":"stream","stream":s,"items":[{"seq":1,"body":"x"}]})
    );
    w.call("api.close", json!({"s": s}), None).await;
    assert_eq!(
        page.next().await,
        json!({"type":"stream-end","stream":s,"outcome":"closed"})
    );
    // No id: the stream is not the page's.
    let no_id = w.call("api.open", json!(null), None).await;
    assert_eq!(no_id, json!({"value": {"err": "not-yours"}}));
    // An id that names no live connection is ignored.
    let wrong = w.call("api.open", json!(null), Some(&"0".repeat(32))).await;
    assert_eq!(wrong, json!({"value": {"err": "not-yours"}}));
    // A field `caller` is still refused.
    let response = w
        .client
        .post(format!("{}/call", w.origin))
        .header("Origin", &w.origin)
        .json(&json!({"fn":"api.who","args":null,"caller":"mod:x"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    // A `connection` that is not 32 hex characters is refused (8.2).
    for bad in [json!("0".repeat(31)), json!("g".repeat(32)), json!(1)] {
        let response = w
            .client
            .post(format!("{}/call", w.origin))
            .header("Origin", &w.origin)
            .json(&json!({"fn":"api.who","args":null,"connection":bad}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400, "{bad}");
    }
    // A closed connection ends its streams `gone`.
    let opened = w.call("api.open", json!(null), Some(&page.id)).await;
    let s2 = opened["value"]["ok"].as_str().unwrap().to_owned();
    drop(page);
    let host = Arc::clone(w.host());
    let expected = format!("{s2} gone");
    wait_async("gone", move || {
        lines(&host, "api.ended").contains(&expected)
    })
    .await;
    let ended = lines(w.host(), "api.ended");
    assert_eq!(ended, [format!("{s} closed"), format!("{s2} gone")]);
    w.served.shutdown().await;
}

pub(crate) async fn wait_async(what: &'static str, check: impl Fn() -> bool + Send + 'static) {
    tokio::task::spawn_blocking(move || wait(what, Duration::from_secs(15), check))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac3_s50_a_page_that_takes_a_few_items_and_stops_ends_lagging() {
    let w = serve_relay().await;
    let clock = TestClock::new();
    w.host().set_test_clock(clock.clone());
    let mut page = Page::open(&w.client, &w.origin).await;
    let opened = w.call("api.open", json!(null), Some(&page.id)).await;
    let s = opened["value"]["ok"].as_str().unwrap().to_owned();
    let body = "x".repeat(64 << 10);
    let host = Arc::clone(w.host());
    let stream = s.clone();
    let text = body.clone();
    // The page takes two items, then stops reading.
    let pushed = tokio::task::spawn_blocking(move || {
        let push = |host: &Arc<Host>| relay(host, "api.push", json!({"s": stream, "text": text}));
        push(&host);
        push(&host);
        host
    })
    .await
    .unwrap();
    for _ in 0..2 {
        let m = page.next().await;
        assert_eq!(m["type"], "stream");
    }
    let host = pushed;
    let stream = s.clone();
    let full = tokio::task::spawn_blocking(move || {
        let start = Instant::now();
        loop {
            let r = relay(
                &host,
                "api.push",
                json!({"s": stream, "text": "x".repeat(64 << 10)}),
            );
            if r["err"] == "full" {
                return true;
            }
            assert!(start.elapsed() < Duration::from_secs(60), "never full");
        }
    })
    .await
    .unwrap();
    assert!(full);
    clock.advance(Duration::from_secs(11));
    let host = Arc::clone(w.host());
    let expected = format!("{s} lagging");
    wait_async("lagging", move || {
        lines(&host, "api.ended").contains(&expected)
    })
    .await;
    drop(page);
    w.served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac4_s51_a_page_connection_past_16_mib_ends_and_its_streams_end_lagging() {
    let w = serve_relay().await;
    let page = Page::open(&w.client, &w.origin).await;
    let mut ids = vec![];
    for _ in 0..8 {
        let opened = w.call("api.open", json!(null), Some(&page.id)).await;
        ids.push(opened["value"]["ok"].as_str().unwrap().to_owned());
    }
    let host = Arc::clone(w.host());
    let streams = ids.clone();
    // The page reads nothing: 1 MiB items into 8 streams pass 16 MiB and the socket buffers.
    tokio::task::spawn_blocking(move || {
        let text = "x".repeat(1 << 20);
        let start = Instant::now();
        'outer: loop {
            for s in &streams {
                let r = relay(&host, "api.push", json!({"s": s, "text": text}));
                if r["err"] == "unknown-stream" {
                    break 'outer;
                }
            }
            assert!(start.elapsed() < Duration::from_secs(60), "never ended");
        }
    })
    .await
    .unwrap();
    let host = Arc::clone(w.host());
    let expected: Vec<String> = ids.iter().map(|s| format!("{s} lagging")).collect();
    wait_async("lagging", move || {
        let ended = lines(&host, "api.ended");
        expected.iter().all(|e| ended.contains(e))
    })
    .await;
    drop(page);
    w.served.shutdown().await;
}

// ---- the other rules of 19.2 ---------------------------------------------------------

#[test]
fn ac10_hand_off_moves_a_host_stream_to_a_mod_that_exports_stream_sink() {
    let env = Env::relay();
    for round in 0..20 {
        let s = test_stream(&env.host);
        let payloads = ["a", "b", "c"].map(|item| format!("{round}:{item}"));
        defer_first(&env.host, &s, payloads[0].as_bytes());
        s.push(payloads[1].as_bytes()).unwrap();
        let hand_off = |caller: Caller| {
            call_as(
                &env.host,
                caller,
                "relay",
                "api.hand-off",
                json!({"s": s.id()}),
            )
            .unwrap()
        };
        // The page, and a mod without `stream-sink`, are `not-yours`; the stream stays.
        assert_eq!(hand_off(Caller::Page), json!({"err":"not-yours"}));
        assert_eq!(
            hand_off(Caller::Mod(local("relay-front"))),
            json!({"err":"not-yours"})
        );
        assert_eq!(hand_off(Caller::Mod(local("relay"))), json!({"ok":null}));
        // Once only.
        assert_eq!(
            hand_off(Caller::Mod(local("relay"))),
            json!({"err":"not-yours"})
        );
        // The queued items go with it and the numbering goes on.
        set_mode(&env.host, "taken");
        resume(&env.host, &s);
        s.push(payloads[2].as_bytes()).unwrap();
        let prefix = format!("{round}:");
        wait("all three", Duration::from_secs(10), || {
            lines(&env.host, "api.received")
                .into_iter()
                .filter(|item| item.starts_with(&prefix))
                .eq(payloads.iter().cloned())
        });
        // The callback commits its state before the stream driver records the delivery.
        wait("delivery of sequence 3", Duration::from_secs(10), || {
            s.deliveries()
                .last()
                .and_then(|delivery| delivery.seqs.last())
                == Some(&3)
        });
        s.close("closed");
        wait("stream closed", Duration::from_secs(10), || {
            s.ended().is_some()
        });
    }
    // A mod stream cannot be handed off.
    let own = call_as(
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
    assert_eq!(
        call_as(
            &env.host,
            Caller::Mod(local("relay")),
            "relay",
            "api.hand-off",
            json!({"s": own})
        )
        .unwrap(),
        json!({"err":"not-yours"})
    );
}

#[test]
fn ac10_ready_runs_once_when_a_full_stream_drops_below_half() {
    let env = Env::relay();
    set_mode(&env.host, "defer");
    let call = |function: &str, args: Value| {
        call_as(
            &env.host,
            Caller::Mod(local("relay")),
            "relay",
            function,
            args,
        )
        .unwrap()
    };
    let own = call("api.open", json!(null))["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    let quiet = call("api.open", json!(null))["ok"]
        .as_str()
        .unwrap()
        .to_owned();
    for i in 0..256 {
        assert_eq!(
            call("api.push", json!({"s": own, "text": format!("{i}")})),
            json!({"ok":null}),
            "{i}"
        );
    }
    assert_eq!(
        call("api.push", json!({"s": own, "text": "x"})),
        json!({"err":"full"})
    );
    settle();
    assert!(lines(&env.host, "api.ended").is_empty());
    set_mode(&env.host, "taken");
    assert_eq!(call("api.resume", json!({"s": own})), json!({"ok":null}));
    wait("ready", Duration::from_secs(10), || {
        lines(&env.host, "api.ended").contains(&format!("{own} ready"))
    });
    settle();
    // Once, and only for the stream whose push failed.
    assert_eq!(lines(&env.host, "api.ended"), [format!("{own} ready")]);
    let _ = quiet;
}

#[test]
fn ac10_a_forwarding_mod_defers_on_full_and_resumes_on_ready_in_order() {
    let env = Env::relay();
    let up = test_stream(&env.host);
    set_mode(&env.host, "defer");
    let down = call_as(
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
    relay(
        &env.host,
        "api.forward",
        json!({"up": up.id(), "down": down, "write": false}),
    );
    let mut sent = 0;
    let resend_deadline = Instant::now() + Duration::from_secs(20);
    while sent < 300 {
        match up.push(format!("{sent:03}").as_bytes()) {
            Ok(()) => sent += 1,
            // The producer can outrun the forwarding mod on Windows. Full
            // refuses this item; send it after the downstream resumes.
            Err(watane_host::StreamError::Full) => break,
            Err(error) => panic!("upstream push failed: {error:?}"),
        }
        if sent % 50 == 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    wait("a deferral", Duration::from_secs(10), || {
        up.deliveries()
            .iter()
            .any(|d| d.answer == DeliveryAnswer::Deferred)
    });
    set_mode(&env.host, "taken");
    assert_eq!(
        relay(&env.host, "api.resume", json!({"s": down})),
        json!({"ok":null})
    );
    while sent < 300 {
        match up.push(format!("{sent:03}").as_bytes()) {
            Ok(()) => sent += 1,
            Err(watane_host::StreamError::Full) => {
                assert!(Instant::now() < resend_deadline, "upstream stayed full");
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("upstream push failed after resume: {error:?}"),
        }
    }
    let expected: Vec<String> = (0..300).map(|i| format!("{i:03}")).collect();
    wait("all items", Duration::from_secs(20), || {
        lines(&env.host, "api.received").len() == 300
    });
    assert_eq!(lines(&env.host, "api.received"), expected);
    // When the downstream ends, the mod resumes the deferred upstream.
    set_mode(&env.host, "defer");
    let down2 = call_as(
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
    let up2 = test_stream(&env.host);
    relay(
        &env.host,
        "api.forward",
        json!({"up": up2.id(), "down": down2, "write": false}),
    );
    for i in 0..257 {
        let _ = up2.push(format!("{i}").as_bytes());
    }
    // Fill `down2` through the forward, then close it.
    wait("the second deferral", Duration::from_secs(10), || {
        up2.paused()
    });
    relay(&env.host, "api.close", json!({"s": down2}));
    wait("resumed", Duration::from_secs(10), || {
        up2.deliveries()
            .iter()
            .any(|d| d.answer == DeliveryAnswer::Taken)
    });
}

#[test]
fn ac10_a_consumer_has_at_most_32_open_streams() {
    let env = Env::relay();
    for _ in 0..31 {
        let r = call_as(
            &env.host,
            Caller::Mod(local("relay")),
            "relay",
            "api.open",
            json!(null),
        )
        .unwrap();
        assert!(r["ok"].is_string());
    }
    let last = test_stream(&env.host);
    let r = call_as(
        &env.host,
        Caller::Mod(local("relay")),
        "relay",
        "api.open",
        json!(null),
    )
    .unwrap();
    assert_eq!(r, json!({"err":"full"}));
    drop(last);
    assert_eq!(
        env.host
            .test_stream(TestConsumer::Mod(local("relay")))
            .unwrap_err(),
        StreamError::Full
    );
}

#[test]
fn ac10_an_item_past_4_mib_is_too_large() {
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
    set_mode(&env.host, "defer");
    let push = |n: usize| {
        relay(
            &env.host,
            "api.push",
            json!({"s": s, "text": "x".repeat(n)}),
        )
    };
    assert_eq!(push((4 << 20) + 1), json!({"err":"too-large"}));
    assert_eq!(push(4 << 20), json!({"ok":null}));
}

#[test]
fn ac10_opening_needs_a_call_link_a_mod_caller_or_the_pages_connection() {
    let env = Env::relay();
    // A page call without a live connection, and an agent, are `not-yours`.
    assert_eq!(
        relay(&env.host, "api.open", json!(null)),
        json!({"err":"not-yours"})
    );
    let agent = call_as(
        &env.host,
        Caller::Agent(watane_host::Handle::parse("h1").unwrap()),
        "relay",
        "api.open",
        json!(null),
    )
    .unwrap();
    assert_eq!(agent, json!({"err":"not-yours"}));
    // A mod caller without `stream-sink` opens nothing for itself.
    let front = call_as(
        &env.host,
        Caller::Mod(local("relay-front")),
        "relay",
        "api.open",
        json!(null),
    )
    .unwrap();
    assert_eq!(front, json!({"err":"not-yours"}));
}

#[test]
fn ac10_the_hop_limit_drops_an_item_that_has_been_forwarded_eight_times() {
    let env = Env::relay();
    let open = || {
        call_as(
            &env.host,
            Caller::Mod(local("relay")),
            "relay",
            "api.open",
            json!(null),
        )
        .unwrap()["ok"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let source = test_stream(&env.host);
    let chain: Vec<String> = (0..8).map(|_| open()).collect();
    let forward = |up: &str, down: &str| {
        relay(
            &env.host,
            "api.forward",
            json!({"up": up, "down": down, "write": false}),
        );
    };
    forward(source.id(), &chain[0]);
    // The push to chain[k] has hop k + 1; chain[6] is the last that is under the limit of 8.
    for k in 0..6 {
        forward(&chain[k], &chain[k + 1]);
    }
    source.push(b"hop").unwrap();
    wait("the item at chain[6]", Duration::from_secs(10), || {
        lines(&env.host, "api.received") == ["hop"]
    });
    // One hop more: chain[6] forwards to chain[7], and the item is dropped there.
    forward(&chain[6], &chain[7]);
    source.push(b"late").unwrap();
    settle();
    settle();
    assert_eq!(lines(&env.host, "api.received"), ["hop"]);
}

#[test]
fn ac10_disabling_or_removing_the_consumer_ends_the_stream_gone() {
    let env = Env::relay();
    let a = test_stream(&env.host);
    env.host.disable(&local("relay")).unwrap();
    assert_eq!(a.ended(), Some(json!("gone")));
    env.host.enable(&local("relay")).unwrap();
    let b = test_stream(&env.host);
    env.host.remove(&local("relay"), false).unwrap();
    assert_eq!(b.ended(), Some(json!("gone")));
}

#[test]
fn ac10_close_with_a_host_only_outcome_ends_failed() {
    let env = Env::relay();
    let s = test_stream(&env.host);
    s.close("gone");
    assert_eq!(
        s.ended(),
        Some(json!({"failed": "gone is the host's outcome"}))
    );
    let t = test_stream(&env.host);
    t.close("closed");
    assert_eq!(t.ended(), Some(json!("closed")));
    // A mod's `close` the same way.
    let own = call_as(
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
    set_mode(&env.host, "close-gone");
    relay(&env.host, "api.close", json!({"s": own}));
    wait("the line", Duration::from_secs(10), || {
        lines(&env.host, "api.ended").contains(&format!("{own} failed"))
    });
}

#[test]
fn ac10_a_pushed_item_is_dropped_with_its_chain_when_the_front_mod_fails() {
    let env = Env::relay();
    set_mode(&env.host, "taken");
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
    let err = call_as(
        &env.host,
        Caller::Page,
        "relay-front",
        "api.push-then-fail",
        json!({"s": s, "text": "x"}),
    )
    .unwrap_err();
    assert_eq!(err.code, watane_host::CallCode::Trap);
    settle();
    assert_eq!(lines(&env.host, "api.received"), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac5_disabling_the_pages_mod_writes_stream_end_gone_before_the_connection_closes() {
    let w = serve_relay().await;
    let mut page = Page::open(&w.client, &w.origin).await;
    let mut ids = vec![];
    for _ in 0..2 {
        let opened = w.call("api.open", json!(null), Some(&page.id)).await;
        ids.push(opened["value"]["ok"].as_str().unwrap().to_owned());
    }
    let host = Arc::clone(w.host());
    tokio::task::spawn_blocking(move || host.disable(&local("relay")).unwrap())
        .await
        .unwrap();
    let mut ended = vec![];
    for _ in 0..2 {
        let m = page.next().await;
        assert_eq!(m["type"], "stream-end", "{m}");
        assert_eq!(m["outcome"], "gone");
        ended.push(m["stream"].as_str().unwrap().to_owned());
    }
    ended.sort();
    ids.sort();
    assert_eq!(ended, ids);
    // Then the connection ends.
    let rest = tokio::time::timeout(Duration::from_secs(5), page.response.chunk())
        .await
        .unwrap()
        .unwrap();
    assert!(rest.is_none() || rest.unwrap().is_empty());
    w.served.shutdown().await;
}
