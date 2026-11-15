mod support;
use std::time::Duration;
use support::*;
use watane_log::PublicKey;
use watane_net::{HeadInfo, Limits, NetError, NodeKey};

fn bad_object(r: Result<impl std::fmt::Debug, NetError>, node: &NodeKey) -> String {
    match r {
        Err(e @ NetError::BadObject { .. }) => {
            let NetError::BadObject { node: n, cause } = &e else {
                unreachable!()
            };
            assert_eq!(n, node);
            assert!(e.to_string().contains(&node.short()), "{e}");
            cause.clone()
        }
        other => panic!("expected BadObject, got {other:?}"),
    }
}

#[test]
fn ac6_bad_entries_are_refused_and_name_the_node() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let c1 = &fx.chains[&fx.c1];
    let c2 = &fx.chains[&fx.c2];
    let set = |e: Vec<_>| a.holdings.faults.lock().unwrap().entries = Some(e);

    // another author than the channel
    set(vec![c2[0].clone()]);
    bad_object(b.net.entries_of(&a.key, &fx.c1, 0, 10), &a.key);
    // out of seq order
    set(vec![c1[0].clone(), c1[2].clone()]);
    bad_object(b.net.entries_of(&a.key, &fx.c1, 0, 10), &a.key);
    set(vec![c1[1].clone(), c1[0].clone()]);
    bad_object(b.net.entries_of(&a.key, &fx.c1, 0, 10), &a.key);
    // not starting at `from`
    set(vec![c1[1].clone(), c1[2].clone()]);
    bad_object(b.net.entries_of(&a.key, &fx.c1, 0, 10), &a.key);
    // the honest answer still passes
    set(c1[..2].to_vec());
    assert_eq!(b.net.entries_of(&a.key, &fx.c1, 0, 2).unwrap().len(), 2);
}

#[test]
fn ac6_bad_heads_are_refused() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let h = |c: PublicKey| {
        (
            c,
            Some(HeadInfo {
                seq: 1,
                id: fx.chains[&fx.c1][1].id(),
                forked: false,
            }),
        )
    };
    let set = |v: Vec<_>| a.holdings.faults.lock().unwrap().heads = Some(v);
    // a channel that was not asked
    set(vec![h(fx.c1), h(fx.c2)]);
    bad_object(b.net.heads_of(&a.key, &[fx.c1]), &a.key);
    // a duplicate
    set(vec![h(fx.c1), h(fx.c1)]);
    bad_object(b.net.heads_of(&a.key, &[fx.c1]), &a.key);
    set(vec![h(fx.c1), h(fx.c1)]);
    bad_object(b.net.heads_of(&a.key, &[]), &a.key);
    // a channel that was asked and is not answered
    set(vec![h(fx.c1)]);
    bad_object(b.net.heads_of(&a.key, &[fx.c1, fx.c2]), &a.key);
}

#[test]
fn ac6_bad_forks_are_refused() {
    let fx = fixture();
    let (a, b) = pair(fx);
    let c1 = &fx.chains[&fx.c1];
    let set = |v| a.holdings.faults.lock().unwrap().fork = Some(v);
    // two different seqs
    set(Some((c1[1].clone(), c1[2].clone())));
    bad_object(b.net.fork_of(&a.key, &fx.c1), &a.key);
    // equal ids
    set(Some((c1[1].clone(), c1[1].clone())));
    bad_object(b.net.fork_of(&a.key, &fx.c1), &a.key);
    // another author than the channel
    set(Some((fx.fork.0.clone(), fx.fork.1.clone())));
    bad_object(b.net.fork_of(&a.key, &fx.c1), &a.key);
    // the honest pair passes
    set(Some((fx.fork.0.clone(), fx.fork.1.clone())));
    assert!(b.net.fork_of(&a.key, &fx.cf).unwrap().is_some());
}

#[test]
fn ac6_an_entry_with_a_changed_byte_does_not_decode() {
    let fx = fixture();
    let e = fx.chains[&fx.c1][0].bytes().to_vec();
    for at in [e.len() / 2, e.len() - 1, 40] {
        let mut changed = e.clone();
        changed[at] ^= 1;
        let answer = cbor::concat(&[cbor::array(&[cbor::text("ok")]), cbor::bytes(&changed)]);
        let fake = fake_callee([8u8; 32], move |_| Some(answer.clone()));
        let b = node(MemHoldings::new(fx), vec![fake.key], None);
        b.net.add_addrs(&fake.key, &[fake.addr]);
        bad_object(b.net.entries_of(&fake.key, &fx.c1, 0, 10), &fake.key);
    }
}

#[test]
fn ac6_more_entries_than_limit_is_refused() {
    let fx = fixture();
    let c1 = &fx.chains[&fx.c1];
    let mut items = vec![cbor::array(&[cbor::text("ok")])];
    items.extend(c1[..3].iter().map(|e| cbor::bytes(e.bytes())));
    let answer = cbor::concat(&items);
    let fake = fake_callee([8u8; 32], move |_| Some(answer.clone()));
    let b = node(MemHoldings::new(fx), vec![fake.key], None);
    b.net.add_addrs(&fake.key, &[fake.addr]);
    bad_object(b.net.entries_of(&fake.key, &fx.c1, 0, 2), &fake.key);
    assert_eq!(b.net.entries_of(&fake.key, &fx.c1, 0, 3).unwrap().len(), 3);
}

#[test]
fn ac6_an_answer_over_its_byte_limit_is_too_large_and_not_read_past() {
    let fx = fixture();
    // a hello answer is at most 1 KiB; this callee sends 2 MiB of garbage
    let fake = fake_callee([8u8; 32], |_| Some(vec![0u8; 2 << 20]));
    let b = node(MemHoldings::new(fx), vec![fake.key], None);
    b.net.add_addrs(&fake.key, &[fake.addr]);
    match b.net.hello(&fake.key) {
        Err(NetError::TooLarge { node }) => assert_eq!(node, fake.key),
        other => panic!("{other:?}"),
    }
    // an entries answer over 4 MiB
    let fake = fake_callee([9u8; 32], |_| {
        Some(cbor::concat(&[
            cbor::array(&[cbor::text("ok")]),
            cbor::bytes(&vec![0u8; 5 << 20]),
        ]))
    });
    b.net.add_addrs(&fake.key, &[fake.addr]);
    match b.net.entries_of(&fake.key, &fx.c1, 0, 10) {
        Err(NetError::TooLarge { node }) => assert_eq!(node, fake.key),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ac6_a_callee_that_does_not_answer_in_time_is_a_timeout() {
    let fx = fixture();
    let limits = Limits {
        deadline: Duration::from_millis(300),
        ..Limits::default()
    };
    let a = node(MemHoldings::new(fx), vec![], None);
    let b = node(MemHoldings::new(fx), vec![a.key], Some(limits));
    a.net.set_options(options(vec![b.key], None)).unwrap();
    b.net.add_addrs(&a.key, &a.net.addrs());
    a.holdings.faults.lock().unwrap().delay = Some(Duration::from_secs(2));
    let started = std::time::Instant::now();
    match b.net.heads_of(&a.key, &[]) {
        Err(e @ NetError::Timeout { .. }) => {
            let NetError::Timeout { node } = &e else {
                unreachable!()
            };
            assert_eq!(*node, a.key);
            assert!(e.to_string().contains(&a.key.short()));
        }
        other => panic!("{other:?}"),
    }
    assert!(started.elapsed() < Duration::from_millis(1500));
    // a fake that never answers
    let fake = fake_callee([4u8; 32], |_| None);
    b.net.add_addrs(&fake.key, &[fake.addr]);
    assert!(matches!(
        b.net.hello(&fake.key),
        Err(NetError::Timeout { .. })
    ));
}

#[test]
fn ac6_a_node_that_cannot_be_reached_is_named() {
    let fx = fixture();
    let b = node(MemHoldings::new(fx), vec![], None);
    let nowhere = node_key_of_seed([2u8; 32]);
    match b.net.hello(&nowhere) {
        Err(e @ NetError::Unreachable { .. }) => assert!(e.to_string().contains(&nowhere.short())),
        other => panic!("{other:?}"),
    }
    b.net.set_options(watane_net::Options::default()).unwrap();
    assert!(matches!(b.net.hello(&nowhere), Err(NetError::Off)));
}
