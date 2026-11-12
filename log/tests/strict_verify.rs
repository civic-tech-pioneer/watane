mod oracle;
use watane_log::{DecodeError as D, Entry, KeyTextError};
#[test]
fn ac7_strict_signature_rule() {
    let valid = oracle::entry(0, None, 0, "a", b"");
    assert!(Entry::decode(&valid).is_ok());
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
