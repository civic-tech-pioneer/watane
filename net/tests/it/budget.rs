#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use iroh::endpoint::Connection;
use std::time::Duration;
use support::*;
use watane_net::Limits;

type Ep = iroh::Endpoint;

async fn endpoint(seed: u8) -> Ep {
    builder([seed; 32]).bind().await.unwrap()
}

async fn connect(ep: &Ep, server: &Node) -> Connection {
    let id = iroh::EndpointId::from_bytes(server.key.as_bytes()).unwrap();
    let addr = iroh::EndpointAddr::from_parts(id, [iroh::TransportAddr::Ip(server.net.addrs()[0])]);
    ep.connect(addr, b"watane/1").await.unwrap()
}

/// One request on a new stream, read to the end; `None` when the
/// connection was closed on us.
async fn ask(conn: &Connection, request: &[u8]) -> Option<Vec<u8>> {
    let (mut send, mut recv) = conn.open_bi().await.ok()?;
    let _ = send.write_all(request).await;
    let _ = send.finish();
    match recv.read_to_end(1 << 20).await {
        Ok(a) => Some(a),
        Err(e) => {
            eprintln!("read: {e:?}");
            None
        }
    }
}

/// The code of a `hello` on `conn` while streams are held open on it. The
/// server counts a held stream when it accepts it, a moment after the client
/// wrote: when `refused` is expected, an `ok` may only mean the count was
/// short, so ask again (the held streams time out in the end, so not for a
/// caller that is served).
async fn hello_beside_held(conn: &Connection, refused: bool) -> String {
    let end = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let code = code_of(ask(conn, &hello_request()).await);
        if code != "ok" || !refused || std::time::Instant::now() >= end {
            return code;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn code_of(answer: Option<Vec<u8>>) -> String {
    match answer {
        Some(a) => err_of(&a).map_or_else(|| "ok".to_owned(), |(c, _)| c),
        None => "closed".to_owned(),
    }
}

async fn closed_soon(conn: &Connection) -> bool {
    tokio::time::timeout(Duration::from_millis(700), conn.closed())
        .await
        .is_ok()
}

/// A `hello` on `conn` is answered `ok` within `secs` seconds (asked every
/// 300 ms; anything but `ok` or `over_budget` fails at once).
async fn ok_within(conn: &Connection, secs: u64) -> bool {
    let end = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        match code_of(ask(conn, &hello_request()).await).as_str() {
            "ok" => return true,
            "over_budget" if std::time::Instant::now() < end => {
                tokio::time::sleep(Duration::from_millis(300)).await
            }
            _ => return false,
        }
    }
}

/// A server of `limits` that trusts the key of seed 200, and one that trusts nobody.
fn servers(limits: Limits) -> (Node, Node) {
    let fx = fixture();
    let trusted = node_key_of_seed([200; 32]);
    (
        node(MemHoldings::new(fx), Some(limits.clone())),
        node_trusting(MemHoldings::new(fx), vec![trusted], Some(limits)),
    )
}

#[test]
fn s9_the_unknown_budgets_default_to_the_spec() {
    // net/TECHNICAL.md 4.2
    let limits = Limits::default();
    assert_eq!(limits.unknown_conns_per_peer, 2);
    assert_eq!(limits.unknown_streams_per_peer, 2);
    assert_eq!(limits.unknown_conns, 16);
    assert_eq!(limits.unknown_bytes_per_sec, 65_536);
}

#[test]
fn s112_oversized_requests_are_refused_before_decoding() {
    const TRIALS: usize = 20;
    // Stream admission charges QUIC bytes already received, including the
    // request itself. If that charge empties the bucket, admission returns
    // over_budget before the request reader can return too_large. Give this
    // fixture room for all trials, including transport overhead.
    let open = node(
        MemHoldings::new(fixture()),
        Some(Limits {
            unknown_bytes_per_sec: 16 << 20,
            unknown_streams_per_peer: TRIALS,
            ..Limits::default()
        }),
    );
    runtime().block_on(async {
        let ep = endpoint(100).await;
        let conn = connect(&ep, &open).await;
        let huge = vec![0xff; 256 * 1024 + 1];
        for trial in 1..=TRIALS {
            assert_eq!(
                code_of(ask(&conn, &huge).await),
                "too_large",
                "trial {trial}"
            );
        }
        assert_eq!(open.holdings.calls(), 0);
    });
}

/// The counts of the default budget, with a byte bucket that the QUIC
/// overhead of a loaded machine cannot drain: a test of the counts alone.
fn counts_only() -> Limits {
    Limits {
        unknown_bytes_per_sec: 16 << 20,
        ..Limits::default()
    }
}

/// The production (default) budget refuses what is beyond it, with a fresh
/// key for each probe. Only refusals are asserted: a drained byte bucket
/// refuses too, so no timing can turn them into a pass or a flake.
#[test]
fn s9_the_default_budget_refuses_an_unknown_key_beyond_it() {
    let (conns, streams, bytes) = (
        node(MemHoldings::new(fixture()), None),
        node(MemHoldings::new(fixture()), None),
        node(MemHoldings::new(fixture()), None),
    );
    runtime().block_on(async {
        // a third connection of one key is closed on it
        let ep = endpoint(130).await;
        let held = [connect(&ep, &conns).await, connect(&ep, &conns).await];
        // any answer shows the server has taken the connection in
        for c in &held {
            assert!(ask(c, &hello_request()).await.is_some());
        }
        let third = connect(&ep, &conns).await;
        let closed = tokio::time::timeout(Duration::from_secs(5), third.closed()).await;
        assert!(closed.is_ok(), "the third connection was kept");

        // a third stream of one key is over_budget (two stay open)
        let ep = endpoint(131).await;
        let conn = connect(&ep, &streams).await;
        let mut held = Vec::new();
        for _ in 0..2 {
            let (mut send, recv) = conn.open_bi().await.unwrap();
            send.write_all(&[0x82]).await.unwrap();
            held.push((send, recv));
        }
        assert_eq!(hello_beside_held(&conn, true).await, "over_budget");

        // more bytes than 64 KiB in a second: the next request is over_budget
        let ep = endpoint(132).await;
        let conn = connect(&ep, &bytes).await;
        let big = code_of(ask(&conn, &vec![0xff; 200_000]).await);
        assert!(big == "bad_request" || big == "over_budget", "{big}");
        assert_eq!(code_of(ask(&conn, &hello_request()).await), "over_budget");
        for server in [&conns, &streams, &bytes] {
            assert_eq!(server.holdings.calls(), 0);
        }
    });
}

#[test]
fn s9_connections_and_streams_over_budget_are_refused_before_work() {
    let plain: Vec<_> = (0..2).map(|_| servers(counts_only())).collect();
    let pooled = servers(Limits {
        unknown_conns: 3,
        ..counts_only()
    });
    let rt = runtime();
    rt.block_on(async {
        // a third connection of one unknown key, beside two held open
        let (open, trusted) = &plain[0];
        for (server, seed, expect_closed) in [(&open, 101u8, true), (&trusted, 200u8, false)] {
            let ep = endpoint(seed).await;
            let held = [connect(&ep, server).await, connect(&ep, server).await];
            for c in &held {
                assert_eq!(code_of(ask(c, &hello_request()).await), "ok");
            }
            let third = connect(&ep, server).await;
            assert_eq!(closed_soon(&third).await, expect_closed, "seed {seed}");
            assert_eq!(open.holdings.calls(), 0);
        }

        // a third concurrent stream of one unknown key
        let (open, trusted) = &plain[1];
        for (server, seed, expect) in [(&open, 102u8, "over_budget"), (&trusted, 200u8, "ok")] {
            let ep = endpoint(seed).await;
            let conn = connect(&ep, server).await;
            let mut held = Vec::new();
            for _ in 0..2 {
                let (mut send, recv) = conn.open_bi().await.unwrap();
                send.write_all(&[0x82]).await.unwrap(); // the start of a request
                held.push((send, recv));
            }
            assert_eq!(
                hello_beside_held(&conn, expect == "over_budget").await,
                expect,
                "seed {seed}"
            );
            assert_eq!(open.holdings.calls(), 0);
        }

        // an unknown connection beyond the pool; a trusted key is not in the
        // pool, so the same server still serves it
        let (_, open) = &pooled;
        let mut held = Vec::new();
        let mut eps = Vec::new();
        for seed in 110..114u8 {
            eps.push(endpoint(seed).await);
        }
        for ep in &eps[..3] {
            let conn = connect(ep, open).await;
            assert_eq!(code_of(ask(&conn, &hello_request()).await), "ok");
            held.push(conn);
        }
        let fourth = connect(&eps[3], open).await;
        assert!(closed_soon(&fourth).await);
        let ep = endpoint(200).await;
        let conn = connect(&ep, open).await;
        assert_eq!(code_of(ask(&conn, &hello_request()).await), "ok");
        assert_eq!(open.holdings.calls(), 0);
    });
}

#[test]
fn s9_the_byte_budget_of_unknown_peers_empties_and_refills() {
    let (open, trusted) = servers(Limits {
        unknown_bytes_per_sec: 4096,
        ..Limits::default()
    });
    let rt = runtime();
    rt.block_on(async {
        for (server, seed, spent) in [(&open, 120u8, true), (&trusted, 200u8, false)] {
            let ep = endpoint(seed).await;
            let conn = connect(&ep, server).await;
            // the bytes iroh moves after the handshake (MTU probes and the
            // like) count too: on a bucket this small they may spend it, so
            // the first hello may wait for a refill
            assert!(ok_within(&conn, 3).await, "seed {seed}: first hello");
            // about 16 KiB of a request that is no request
            let big = code_of(ask(&conn, &vec![0xff; 16_000]).await);
            // most of it arrived before the stream was accepted: the unknown
            // node may already be over budget
            assert!(
                big == "bad_request" || (spent && big == "over_budget"),
                "{big}"
            );
            let next = code_of(ask(&conn, &hello_request()).await);
            assert_eq!(
                next,
                if spent { "over_budget" } else { "ok" },
                "seed {seed}"
            );
            if spent {
                assert!(ok_within(&conn, 3).await, "the bucket did not refill");
            }
        }
        assert_eq!(open.holdings.calls(), 0);
    });
}
