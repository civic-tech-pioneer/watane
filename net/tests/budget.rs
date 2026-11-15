mod support;
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
fn s9_requests_over_limits_and_budgets_are_refused_before_work() {
    let plain: Vec<_> = (0..3).map(|_| servers(Limits::default())).collect();
    let pooled = servers(Limits {
        unknown_conns: 3,
        ..Limits::default()
    });
    let rt = runtime();
    rt.block_on(async {
        // a request over the limit
        let (open, _) = &plain[0];
        let ep = endpoint(100).await;
        let conn = connect(&ep, open).await;
        let huge = vec![0xff; 256 * 1024 + 1];
        assert_eq!(code_of(ask(&conn, &huge).await), "too_large");
        assert_eq!(open.holdings.calls(), 0);
        drop(conn);

        // a third connection of one unknown key, beside two held open
        let (open, trusted) = &plain[1];
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
        let (open, trusted) = &plain[2];
        for (server, seed, expect) in [(&open, 102u8, "over_budget"), (&trusted, 200u8, "ok")] {
            let ep = endpoint(seed).await;
            let conn = connect(&ep, server).await;
            let mut held = Vec::new();
            for _ in 0..2 {
                let (mut send, recv) = conn.open_bi().await.unwrap();
                send.write_all(&[0x82]).await.unwrap(); // the start of a request
                held.push((send, recv));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
            assert_eq!(
                code_of(ask(&conn, &hello_request()).await),
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
