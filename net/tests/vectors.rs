//! The signed form of TECHNICAL 4.3, pinned by vectors made with
//! `ed25519-compact` from the seed of RFC 8032 test 1.
mod support;
use support::{cbor::*, *};
use watane_net::{DeviceList, NetError, NodeKey, Role};

const SEED: [u8; 32] = hex32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
const ANCHOR: [u8; 32] = hex32("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
const OTHER_SEED: [u8; 32] = [2; 32];
const TIME: u64 = 1767225600000;

const fn nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        _ => b - b'a' + 10,
    }
}
const fn hex32(s: &str) -> [u8; 32] {
    let mut out = [0; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = nibble(s.as_bytes()[2 * i]) * 16 + nibble(s.as_bytes()[2 * i + 1]);
        i += 1;
    }
    out
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The unsigned items, by hand: anchor, seq, time, expires, devices.
fn items(
    anchor: &[u8; 32],
    seq: u64,
    time: u64,
    expires: u64,
    devices: &[([u8; 32], u64)],
) -> Vec<Vec<u8>> {
    vec![
        bytes(anchor),
        uint(seq),
        uint(time),
        uint(expires),
        array(
            &devices
                .iter()
                .map(|(k, r)| array(&[bytes(k), uint(*r)]))
                .collect::<Vec<_>>(),
        ),
    ]
}
/// The signed object, by hand, signed over `prefix`.
fn object(prefix: &[u8], seed: [u8; 32], items: &[Vec<u8>]) -> Vec<u8> {
    let mut msg = prefix.to_vec();
    msg.extend(array(items));
    let sig = sign(seed, &msg);
    let mut all = items.to_vec();
    all.push(bytes(&sig));
    array(&all)
}
const DEVICES: &[u8] = b"watane/devices/1\0";

fn other() -> [u8; 32] {
    public_of(OTHER_SEED)
}
/// Sorted by key bytes.
fn devices() -> Vec<([u8; 32], u64)> {
    let mut d = vec![(ANCHOR, 1), (other(), 0)];
    d.sort();
    d
}
fn list() -> DeviceList {
    DeviceList {
        anchor: NodeKey::from_bytes(ANCHOR),
        seq: 1,
        time: TIME,
        expires_days: 0,
        devices: devices()
            .into_iter()
            .map(|(k, r)| {
                (
                    NodeKey::from_bytes(k),
                    if r == 1 { Role::Full } else { Role::Light },
                )
            })
            .collect(),
    }
}
fn refused(bytes: &[u8]) {
    assert!(
        matches!(DeviceList::verify(bytes), Err(NetError::BadList { .. })),
        "{}",
        hex(bytes)
    );
}

#[test]
fn ac7_the_golden_vector() {
    assert_eq!(public_of(SEED), ANCHOR);
    let expected = object(DEVICES, SEED, &items(&ANCHOR, 1, TIME, 0, &devices()));
    // the vector, as hex (made by the oracle above, pinned here)
    assert_eq!(hex(&expected), GOLDEN, "the oracle changed");
    let got = list().sign(&SEED).unwrap();
    assert_eq!(got, expected);
    assert_eq!(DeviceList::verify(&got).unwrap(), list());
}

/// 86 | anchor | seq 1 | time | expires 0 | [[other, 0], [anchor, 1]] | signature
const GOLDEN: &str = "865820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a011b0000019b76daa80000828258208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b39400825820d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a015840b696722f77bacb792c93d33d2e572087adaeeca9c237fe0f99b784b48bb9e19253a496079c9345bce33625bcfcff2e6e43b9fd9c9644fcdac73e74bcc43c590c";

#[test]
fn ac7_refused_forms() {
    let sorted = devices();
    let mut unsorted = sorted.clone();
    unsorted.reverse();
    let ok = |items: Vec<Vec<u8>>| object(DEVICES, SEED, &items);
    // sanity: the construction itself verifies
    assert!(DeviceList::verify(&ok(items(&ANCHOR, 1, TIME, 0, &sorted))).is_ok());
    // unsorted, duplicate, no anchor, role 2 (all correctly signed)
    refused(&ok(items(&ANCHOR, 1, TIME, 0, &unsorted)));
    refused(&ok(items(
        &ANCHOR,
        1,
        TIME,
        0,
        &[sorted[0], sorted[0], sorted[1]],
    )));
    refused(&ok(items(&ANCHOR, 1, TIME, 0, &[(other(), 0)])));
    let mut role2 = sorted.clone();
    role2.iter_mut().for_each(|d| d.1 = 2);
    refused(&ok(items(&ANCHOR, 1, TIME, 0, &role2)));
    // a signature over the entry prefix of the same items
    refused(&object(
        b"watane/entry/1\0",
        SEED,
        &items(&ANCHOR, 1, TIME, 0, &sorted),
    ));
    // signed by a key that is not the anchor
    refused(&object(
        DEVICES,
        OTHER_SEED,
        &items(&ANCHOR, 1, TIME, 0, &sorted),
    ));
    // a flipped bit of the signature, and of each item
    let good = ok(items(&ANCHOR, 1, TIME, 0, &sorted));
    for at in [good.len() - 1, good.len() - 40, 6, 40, 60] {
        let mut bad = good.clone();
        bad[at] ^= 1;
        refused(&bad);
    }
    // trailing bytes, truncation, a map, an indefinite array
    refused(&[good.clone(), vec![0]].concat());
    refused(&good[..good.len() - 1]);
    refused(&[0xa0]);
    let mut indefinite = good.clone();
    indefinite[0] = 0x9f;
    indefinite.push(0xff);
    refused(&indefinite);
    // a non-shortest integer: seq 1 as 0x18 0x01 (the signature is over the shortest form)
    let mut seq = ok(items(&ANCHOR, 1, TIME, 0, &sorted));
    let at = 1 + 34; // array head, anchor item, then seq
    assert_eq!(seq[at], 1);
    seq.splice(at..=at, [0x18, 0x01]);
    refused(&seq);
    // too few and too many items
    refused(&array(&[bytes(&ANCHOR)]));
    // expiry past u32
    refused(&ok(items(&ANCHOR, 1, TIME, 1 << 40, &sorted)));
}

#[test]
fn ac7_sign_refuses_a_bad_list_or_the_wrong_key() {
    let mut l = list();
    l.devices.reverse();
    assert!(l.sign(&SEED).is_err());
    let mut l = list();
    l.devices.push(l.devices[0]);
    assert!(l.sign(&SEED).is_err());
    let mut l = list();
    l.devices.retain(|d| d.0 != l.anchor);
    assert!(l.sign(&SEED).is_err());
    assert!(list().sign(&OTHER_SEED).is_err());
}
