//! S1. This binary holds one test, so that no other test opens a socket
//! while it counts them.
mod support;
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
use support::*;
use watane_net::{Net, Options};

// `/dev/fd` exists on Linux and macOS; `/proc/self/fd` only on Linux. A stat of
// `/dev/fd/<n>` reports the open file itself, so the type says whether it is a socket.
fn sockets() -> usize {
    use std::os::unix::fs::FileTypeExt;
    fs::read_dir("/dev/fd")
        .unwrap()
        .filter_map(|e| fs::metadata(e.ok()?.path()).ok())
        .filter(|m| m.file_type().is_socket())
        .count()
}

#[test]
fn s1_new_home_net_off() {
    let before = sockets();
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("net");
    let net = Net::open(&dir, MemHoldings::empty(), Options::default()).unwrap();

    // the directory and the key file
    let mode = |p: &std::path::Path| fs::metadata(p).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(&dir), 0o700);
    let key_file = dir.join("node.key");
    assert_eq!(mode(&key_file), 0o600);
    let seed: [u8; 32] = fs::read(&key_file).unwrap().try_into().expect("32 bytes");

    // the node key is the public key of that seed, in log's text form
    let key = net.node_key();
    assert_eq!(key.as_bytes(), &public_of(seed));
    let text = key.to_string();
    assert_eq!(text.len(), 52);
    assert_eq!(
        text,
        watane_log::PublicKey::from_bytes(key.as_bytes())
            .unwrap()
            .to_string()
    );
    assert_eq!(key.short(), text[..8]);
    assert_eq!(text.parse::<watane_net::NodeKey>().unwrap(), key);

    // net off: no socket, no address
    assert_eq!(sockets(), before, "a socket exists while net is off");
    assert!(net.addrs().is_empty());

    // a second open keeps the key
    drop(net);
    let again = Net::open(&dir, MemHoldings::empty(), Options::default()).unwrap();
    assert_eq!(again.node_key(), key);
    assert_eq!(fs::read(&key_file).unwrap(), seed);
    assert_eq!(sockets(), before);

    // on: bound; off: closed
    again
        .set_options(Options {
            on: true,
            bind: Some(lo()),
            ..Options::default()
        })
        .unwrap();
    assert_eq!(again.addrs().len(), 1);
    assert!(sockets() > before);
    again.set_options(Options::default()).unwrap();
    assert!(again.addrs().is_empty());
    let mut waited = 0;
    while sockets() > before && waited < 50 {
        std::thread::sleep(Duration::from_millis(100));
        waited += 1;
    }
    assert_eq!(
        sockets(),
        before,
        "a socket is left after net is turned off"
    );
}
