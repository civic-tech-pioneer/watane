use crate::oracle;
use watane_log::{DecodeError as D, Entry, KeyTextError};
#[test]
fn ac5_golden_vectors_and_independent_signed_entry() {
    for (hex, len, seq, prev, time, kind, body, id) in [
        (
            oracle::GOLDEN0,
            126,
            0,
            None,
            1767225600000,
            "profile",
            b"hello".as_slice(),
            oracle::ID0,
        ),
        (
            oracle::GOLDEN1,
            157,
            1,
            Some(oracle::hex_array::<32>(oracle::ID0)),
            1767225601000,
            "release",
            [1, 2, 3].as_slice(),
            oracle::ID1,
        ),
    ] {
        let raw = oracle::hex(hex);
        assert_eq!(raw.len(), len);
        let e = Entry::decode(&raw).unwrap();
        assert_eq!(e.bytes(), raw);
        assert_eq!(e.author().as_bytes(), &oracle::PUBLIC);
        assert_eq!(e.seq(), seq);
        assert_eq!(e.prev().map(|p| *p.as_bytes()), prev);
        assert_eq!(e.time(), time);
        assert_eq!(e.kind(), kind);
        assert_eq!(e.body(), body);
        assert_eq!(e.id().to_string(), id);
        assert_eq!(raw, oracle::entry(seq, prev.as_ref(), time, kind, body));
    }
    let prev = [0x47; 32];
    let raw = oracle::entry(123, Some(&prev), 998, "custom-4", &[0, 255, 22]);
    let e = Entry::decode(&raw).unwrap();
    assert_eq!(e.bytes(), raw);
    assert_eq!(e.author().as_bytes(), &oracle::PUBLIC);
    assert_eq!(e.seq(), 123);
    assert_eq!(e.prev().unwrap().as_bytes(), &prev);
    assert_eq!(e.time(), 998);
    assert_eq!(e.kind(), "custom-4");
    assert_eq!(e.body(), [0, 255, 22]);
    assert_eq!(e.id().as_bytes(), blake3::hash(&raw).as_bytes());
}
fn refusal(bytes: Vec<u8>, expected: D) {
    assert_eq!(Entry::decode(&bytes), Err(expected));
}
#[test]
fn ac6_codec_refusals_and_first_failed_check() {
    refusal(vec![0; 65729], D::TooLarge(65729));
    let valid = oracle::entry(0, None, 0, "a", b"");
    for h in [0x86, 0x88, 0xa7] {
        let mut b = valid.clone();
        b[0] = h;
        refusal(b, D::Malformed("array"));
    }
    let mut b = valid.clone();
    b.push(0);
    refusal(b, D::Malformed("end"));
    let mut b = oracle::entry(5, Some(&[0; 32]), 0, "a", b"");
    b.splice(35..36, [0x18, 5]);
    refusal(b, D::NotDeterministic);
    // Body starts at 40: 87, author(34), seq, null, time, kind(2).
    let mut b = valid.clone();
    b.splice(40..41, [0x5f, 0x40, 0xff]);
    refusal(b, D::NotDeterministic);
    for kind in ["A", &"a".repeat(33), "", "-a", "a_", "é"] {
        refusal(oracle::entry(0, None, 0, kind, b""), D::Kind);
    }
    refusal(
        oracle::entry(0, None, 0, "a", &vec![0; 65537]),
        D::BodyTooLarge(65537),
    );
    for (seq, time) in [(1 << 53, 0), (0, 1 << 53)] {
        refusal(oracle::entry(seq, None, time, "a", b""), D::OutOfRange);
    }
    refusal(oracle::entry(0, Some(&[0; 32]), 0, "a", b""), D::Prev);
    refusal(oracle::entry(1, None, 0, "a", b""), D::Prev);
    let mut b = valid.clone();
    *b.last_mut().unwrap() ^= 1;
    refusal(b, D::Signature);
    // Parse failures name the failed field.
    for (offset, name) in [
        (1, "author"),
        (35, "seq"),
        (36, "prev"),
        (37, "time"),
        (38, "kind"),
        (40, "body"),
        (41, "sig"),
    ] {
        let mut b = valid.clone();
        b[offset] = 0xf5;
        refusal(b, D::Malformed(name));
    }
    let mut b = valid.clone();
    b[3..35].fill(0xff);
    refusal(b, D::Author(KeyTextError::NotCanonical));
    let mut b = oracle::entry(1, None, 1 << 53, "A", b"");
    b[35] = 0x18;
    b.insert(36, 1);
    refusal(b, D::NotDeterministic);
}
#[test]
fn ac5_maximum_entry_and_integer_boundaries() {
    for n in [
        0,
        23,
        24,
        255,
        256,
        65535,
        65536,
        u32::MAX as u64,
        u32::MAX as u64 + 1,
        (1 << 53) - 1,
    ] {
        let raw = oracle::entry(
            n,
            if n == 0 { None } else { Some(&[7; 32]) },
            n,
            &"a".repeat(32),
            &vec![0; 65536],
        );
        let e = Entry::decode(&raw).unwrap();
        assert_eq!(e.seq(), n);
        assert_eq!(e.time(), n);
        if n == (1 << 53) - 1 {
            assert_eq!(raw.len(), 65728);
        }
    }
}
