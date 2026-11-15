mod support;
use support::*;
use watane_log::{Entry, PublicKey};
use watane_net::{Class, Code, HeadInfo, NetError, NodeKey};

fn head(fx: &Fixture, c: &PublicKey) -> Option<HeadInfo> {
    let last = fx.chains.get(c)?.last()?;
    Some(HeadInfo {
        seq: last.seq(),
        id: last.id(),
        forked: *c == fx.cf,
    })
}

fn raws(entries: &[Entry]) -> Vec<Vec<u8>> {
    entries.iter().map(|e| e.bytes().to_vec()).collect()
}

fn refused(r: Result<impl std::fmt::Debug, NetError>, node: &NodeKey, code: Code) {
    match r {
        Err(e @ NetError::Refused { .. }) => {
            let NetError::Refused { node: n, code: c } = &e else {
                unreachable!()
            };
            assert_eq!((n, *c), (node, code));
            assert!(e.to_string().contains(&node.short()), "{e}");
        }
        other => panic!("expected Refused({code}), got {other:?}"),
    }
}

#[test]
fn ac5_hello_from_a_trusted_and_an_unknown_node() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let c = node(MemHoldings::new(fx), None);
    c.net.add_addrs(&a.key, &a.net.addrs());
    let seq = a.net.devices().unwrap().seq;
    for (hello, expected) in [
        (b.net.hello(&a.key).unwrap(), Some(seq)),
        (c.net.hello(&a.key).unwrap(), None),
    ] {
        assert_eq!(hello.devices_seq, expected);
        assert_eq!(hello.version, 1);
        assert_eq!(
            hello.ops,
            [
                "hello", "heads", "entries", "fork", "devices", "pair", "query"
            ]
        );
        let l = hello.limits;
        assert_eq!(l.request, 262_144);
        assert_eq!(l.answer_entries, 4_194_304);
        assert_eq!(l.max_entries, 1000);
        assert_eq!(l.entry, 65_728);
        assert_eq!(l.blob, 67_108_864);
        assert_eq!(l.hits, 200);
        assert_eq!(l.connect_timeout.as_millis(), 10_000);
        assert_eq!(l.deadline.as_millis(), 5_000);
        assert_eq!(l.pair_deadline.as_millis(), 600_000);
        assert_eq!(l.blob_deadline.as_millis(), 120_000);
    }
    assert_eq!(a.net.class_of(&b.key), Class::Trusted);
    assert_eq!(a.net.class_of(&c.key), Class::Unknown);
    assert_eq!(a.net.class_of(&a.key), Class::Trusted);
}

#[test]
fn ac5_heads_all_and_some() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let all = b.net.heads_of(&a.key, &[]).unwrap();
    let expected: Vec<_> = fx.chains.keys().map(|c| (*c, head(fx, c))).collect();
    assert_eq!(all, expected);
    assert!(all.iter().any(|(c, h)| *c == fx.cf && h.unwrap().forked));
    let some = b.net.heads_of(&a.key, &[fx.c1, fx.absent]).unwrap();
    assert_eq!(some, vec![(fx.c1, head(fx, &fx.c1)), (fx.absent, None)]);
}

#[test]
fn ac5_entries_in_original_bytes() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let all = b.net.entries_of(&a.key, &fx.c1, 1, 1000).unwrap();
    assert_eq!(raws(&all), raws(&fx.chains[&fx.c1][1..]));
    let page = b.net.entries_of(&a.key, &fx.c1, 3, 2).unwrap();
    assert_eq!(page.iter().map(|e| e.seq()).collect::<Vec<_>>(), [3, 4]);
    assert_eq!(raws(&page), raws(&fx.chains[&fx.c1][3..5]));
    assert!(b.net.entries_of(&a.key, &fx.c1, 9, 5).unwrap().is_empty());
    assert!(
        b.net
            .entries_of(&a.key, &fx.absent, 0, 5)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ac5_an_answer_over_4_mib_is_cut_to_fit() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let got = b.net.entries_of(&a.key, &fx.cbig, 0, 1000).unwrap();
    let all = &fx.chains[&fx.cbig];
    let total: usize = all.iter().map(|e| e.bytes().len()).sum();
    assert!(total > 4 << 20, "the fixture must be over 4 MiB: {total}");
    let got_total: usize = got.iter().map(|e| e.bytes().len()).sum();
    assert!(got_total < 4 << 20 && got.len() > 50 && got.len() < all.len());
    assert_eq!(raws(&got), raws(&all[..got.len()]));
    // the next page continues
    let rest = b
        .net
        .entries_of(&a.key, &fx.cbig, got.len() as u64, 1000)
        .unwrap();
    assert_eq!(raws(&rest), raws(&all[got.len()..]));
}

#[test]
fn ac5_never_more_than_1000_entries() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let got = b.net.entries_of(&a.key, &fx.c1200, 0, 5000).unwrap();
    assert_eq!(got.len(), 1000);
    assert_eq!(raws(&got), raws(&fx.chains[&fx.c1200][..1000]));
}

#[test]
fn ac5_fork() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let (x, y) = b.net.fork_of(&a.key, &fx.cf).unwrap().unwrap();
    assert_eq!(x.bytes(), fx.fork.0.bytes());
    assert_eq!(y.bytes(), fx.fork.1.bytes());
    assert!(b.net.fork_of(&a.key, &fx.c1).unwrap().is_none());
}

#[test]
fn ac5_an_unknown_node_gets_hello_only() {
    let fx = fixture();
    let (a, _b) = pair(fx);
    let c = node(MemHoldings::new(fx), None);
    c.net.add_addrs(&a.key, &a.net.addrs());
    let before = a.holdings.calls();
    refused(c.net.heads_of(&a.key, &[]), &a.key, Code::NotAllowed);
    refused(
        c.net.entries_of(&a.key, &fx.c1, 0, 10),
        &a.key,
        Code::NotAllowed,
    );
    refused(c.net.fork_of(&a.key, &fx.cf), &a.key, Code::NotAllowed);
    assert_eq!(a.holdings.calls(), before, "the holder was called");
    assert!(c.net.hello(&a.key).is_ok());
}

#[test]
fn ac5_later_and_made_up_ops_are_unknown_op() {
    let fx = fixture();
    let seed = [5u8; 32];
    let a = node_trusting(MemHoldings::new(fx), vec![node_key_of_seed(seed)], None);
    let addr = a.net.addrs()[0];
    use cbor::*;
    // only made-up ops are unknown now; `query` is an op (below)
    for op in ["frobnicate", "subscribe"] {
        let req = array(&[text(op), uint(1)]);
        let answer = raw_call(seed, &a.key, addr, &req);
        assert_eq!(err_code(&answer), "unknown_op", "{op}");
    }
    // `query` from a trusted caller with a malformed query is a bad request
    let before = a.holdings.calls();
    let req = array(&[text("query"), uint(1)]);
    assert_eq!(err_code(&raw_call(seed, &a.key, addr, &req)), "bad_request");
    assert_eq!(a.holdings.calls(), before, "the holder was called");
    // `devices` and `pair` exist now: a wrong argument is a bad request
    let req = array(&[text("pair"), uint(1)]);
    assert_eq!(err_code(&raw_call(seed, &a.key, addr, &req)), "bad_request");
    let req = array(&[text("devices"), bytes(&[1])]);
    assert_eq!(err_code(&raw_call(seed, &a.key, addr, &req)), "bad_request");
    let req = array(&[text("devices"), uint(0)]);
    let answer = raw_call(seed, &a.key, addr, &req);
    assert_eq!(answer[..4], array(&[text("ok")])[..]);
}

/// The answer `["err", code, message]` has this code.
fn err_code(answer: &[u8]) -> String {
    use cbor::*;
    let head = array(&[text("err"), text("x"), text("y")]);
    assert_eq!(answer[0], head[0], "{answer:02x?}");
    assert_eq!(&answer[1..5], &text("err")[..], "{answer:02x?}");
    let len = (answer[5] & 31) as usize;
    String::from_utf8(answer[6..6 + len].to_vec()).unwrap()
}

#[test]
fn ac4_the_callee_refuses_a_request_over_its_limit_before_decoding() {
    let fx = fixture();
    let seed = [6u8; 32];
    let a = node_trusting(MemHoldings::new(fx), vec![node_key_of_seed(seed)], None);
    let addr = a.net.addrs()[0];
    // not even CBOR, and one byte over 256 KiB
    let answer = raw_call(seed, &a.key, addr, &vec![0xff; 256 * 1024 + 1]);
    assert_eq!(err_code(&answer), "too_large");
    // exactly 256 KiB is read, and then refused as a bad request
    let answer = raw_call(seed, &a.key, addr, &vec![0xff; 256 * 1024]);
    assert_eq!(err_code(&answer), "bad_request");
    assert_eq!(a.holdings.calls(), 0);
}

#[test]
fn ac4_the_callee_refuses_what_is_not_a_request() {
    use cbor::*;
    let fx = fixture();
    let seed = [7u8; 32];
    let a = node_trusting(MemHoldings::new(fx), vec![node_key_of_seed(seed)], None);
    let addr = a.net.addrs()[0];
    let c1 = bytes(fx.c1.as_bytes());
    let good = array(&[text("fork"), c1.clone()]);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("trailing", [good.clone(), vec![0]].concat()),
        ("map", vec![0xa0]),
        ("text where array", text("fork")),
        ("empty", vec![]),
        ("non-shortest length", {
            let mut r = vec![0x82, 0x78, 4];
            r.extend(b"fork");
            r.extend([0x58, 32]);
            r.extend(fx.c1.as_bytes());
            r
        }),
        ("non-shortest integer", {
            let mut r = vec![0x82];
            r.extend(text("hello"));
            r.extend([0x18, 0x01]);
            r
        }),
        (
            "indefinite array",
            [vec![0x9f], text("fork"), c1.clone(), vec![0xff]].concat(),
        ),
        ("op not text", array(&[uint(1), c1.clone()])),
        (
            "channel of 31 bytes",
            array(&[text("fork"), bytes(&[1u8; 31])]),
        ),
        (
            "missing argument",
            array(&[text("entries"), c1.clone(), uint(0)]),
        ),
    ];
    for (name, request) in cases {
        let answer = raw_call(seed, &a.key, addr, &request);
        assert_eq!(err_code(&answer), "bad_request", "{name}");
    }
    assert_eq!(a.holdings.calls(), 0);
}

#[test]
fn ac4_the_caller_refuses_to_send_a_request_over_its_limit() {
    let fx = fixture();
    let (a, b) = pair(fx);
    // 34 bytes per channel: 8000 channels are over 256 KiB
    let many: Vec<PublicKey> = (0..8000u32)
        .map(|i| {
            let mut seed = [1u8; 32];
            seed[..4].copy_from_slice(&i.to_be_bytes());
            PublicKey::from_bytes(&public_of(seed)).unwrap()
        })
        .collect();
    let before = a.holdings.calls();
    match b.net.heads_of(&a.key, &many) {
        Err(NetError::TooLarge { node }) => assert_eq!(node, a.key),
        other => panic!("{other:?}"),
    }
    assert_eq!(a.holdings.calls(), before);
}
