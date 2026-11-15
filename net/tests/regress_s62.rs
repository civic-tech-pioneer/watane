mod support;
use support::*;
use watane_net::Role;

/// TECHNICAL 5.2: "a node that sees a higher [devices_seq] asks for it".
/// The anchor does not know the server's address (N2 learns none from a
/// connection), so its push of a new list cannot reach the server. The
/// server then says hello to the anchor, sees the higher `devices_seq`, and
/// should pull the list; it keeps the old one, so a machine Alice removed
/// stays served.
#[test]
fn regress_s62_hello_with_newer_devices_seq_pulls_the_list() {
    let fx = fixture();
    let laptop = node(MemHoldings::anchor(fx), None);
    let server = node(MemHoldings::new(fx), None);
    let code = laptop.net.pair_code(Role::Full).unwrap();
    try_pair(&laptop, &server, code, false, Some(true)).unwrap();
    let newer = laptop.net.set_expiry(30).unwrap();
    assert!(server.net.devices().unwrap().seq < newer.seq);

    let hello = server.net.hello(&laptop.key).unwrap();
    assert_eq!(hello.devices_seq, Some(newer.seq));
    assert!(
        wait_for(5, || server.net.devices().unwrap() == newer),
        "the server saw devices_seq {} and still holds seq {}",
        newer.seq,
        server.net.devices().unwrap().seq
    );
}
