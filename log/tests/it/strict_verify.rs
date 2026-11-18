use crate::oracle;
use watane_log::{DecodeError as D, Entry, KeyTextError};
#[test]
fn ac7_strict_signature_rule() {
    let valid = oracle::entry(0, None, 0, "a", b"");
    Entry::decode(&valid).unwrap();
    let mut malleable = valid.clone();
    let l =
        oracle::hex_array::<32>("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    let start = malleable.len() - 32;
    let mut carry = 0u16;
    for i in 0..32 {
        let sum = malleable[start + i] as u16 + l[i] as u16 + carry;
        malleable[start + i] = sum as u8;
        carry = sum >> 8;
    }
    assert_eq!(Entry::decode(&malleable), Err(D::Signature));
    let mut small = [0; 32];
    small[0] = 1;
    let mut sig = [0; 64];
    sig[..32].copy_from_slice(&small);
    let raw = oracle::cbor::entry(&small, 0, None, 0, "a", b"", &sig);
    assert_eq!(
        Entry::decode(&raw),
        Err(D::Author(KeyTextError::SmallOrder))
    );
    let mut noncanonical = [255; 32];
    noncanonical[0] = 0xee;
    noncanonical[31] = 0x7f;
    assert_eq!(
        Entry::decode(&oracle::cbor::entry(
            &noncanonical,
            0,
            None,
            0,
            "a",
            b"",
            &sig
        )),
        Err(D::Author(KeyTextError::NotCanonical))
    );
    // A small-order or non-canonical R in place of a valid R is refused. Any verifier
    // refuses these, as the equation no longer holds; they do not prove verify_strict.
    for r in [small, noncanonical] {
        let mut b = valid.clone();
        let start = b.len() - 64;
        b[start..start + 32].copy_from_slice(&r);
        assert_eq!(Entry::decode(&b), Err(D::Signature));
    }
}

// Small independent integer arithmetic, with every intermediate reduced modulo L.
fn add_mod(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let l =
        oracle::hex_array::<32>("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
    let mut out = [0; 32];
    let mut carry = 0u16;
    for i in 0..32 {
        let x = a[i] as u16 + b[i] as u16 + carry;
        out[i] = x as u8;
        carry = x >> 8;
    }
    if out.iter().rev().cmp(l.iter().rev()) != std::cmp::Ordering::Less {
        let mut borrow = 0i16;
        for i in 0..32 {
            let x = out[i] as i16 - l[i] as i16 - borrow;
            out[i] = x as u8;
            borrow = i16::from(x < 0);
        }
    }
    out
}
fn reduce(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0; 32];
    for b in bytes.iter().rev() {
        for bit in (0..8).rev() {
            out = add_mod(out, out);
            let mut one = [0; 32];
            one[0] = (b >> bit) & 1;
            out = add_mod(out, one);
        }
    }
    out
}
#[test]
fn ac9_identity_r_separates_strict_from_plain_verify() {
    use ed25519_compact::sha512::Hash;
    let mut a = Hash::hash(oracle::SEED);
    a[0] &= 248;
    a[31] &= 63;
    a[31] |= 64;
    let a = reduce(&a[..32]);
    let mut r = [0; 32];
    r[0] = 1;
    let msg = oracle::cbor::signed(&oracle::PUBLIC, 0, None, 0, "a", b"");
    let mut hash = Hash::new();
    hash.update(r);
    hash.update(oracle::PUBLIC);
    hash.update(&msg);
    let k = reduce(&hash.finalize());
    let mut s = [0; 32];
    let mut power = a;
    for b in k {
        for bit in 0..8 {
            if b & (1 << bit) != 0 {
                s = add_mod(s, power);
            }
            power = add_mod(power, power);
        }
    }
    let mut sig = [0; 64];
    sig[..32].copy_from_slice(&r);
    sig[32..].copy_from_slice(&s);
    use ed25519_dalek::Verifier;
    let pk = ed25519_dalek::VerifyingKey::from_bytes(&oracle::PUBLIC).unwrap();
    pk.verify(&msg, &ed25519_dalek::Signature::from_bytes(&sig))
        .unwrap();
    assert_eq!(
        Entry::decode(&oracle::cbor::entry(
            &oracle::PUBLIC,
            0,
            None,
            0,
            "a",
            b"",
            &sig
        )),
        Err(D::Signature)
    );
}
