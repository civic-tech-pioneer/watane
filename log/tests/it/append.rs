use crate::oracle;
use watane_log::{Error, Log};
#[test]
fn ac1_appends_and_limits() {
    let dir = oracle::directory();
    let log = Log::open(dir.path()).unwrap();
    let mut prev = None;
    for seq in 0..3 {
        let e = log.append("profile", b"hello").unwrap();
        assert_eq!(e.seq(), seq);
        assert_eq!(e.prev().map(|p| *p.as_bytes()), prev);
        let signed = oracle::cbor::signed(
            e.author().as_bytes(),
            seq,
            prev.as_ref(),
            e.time(),
            e.kind(),
            e.body(),
        );
        let sig =
            ed25519_compact::Signature::from_slice(&e.bytes()[e.bytes().len() - 64..]).unwrap();
        ed25519_compact::PublicKey::from_slice(e.author().as_bytes())
            .unwrap()
            .verify(&signed, &sig)
            .unwrap();
        prev = Some(*blake3::hash(e.bytes()).as_bytes());
    }
    let head = log.head(&log.identity().unwrap()).unwrap();
    for kind in ["a".repeat(33), "Upper".into()] {
        assert!(matches!(log.append(&kind, b""), Err(Error::Kind(_))));
    }
    assert!(matches!(
        log.append("a", &vec![0; 65537]),
        Err(Error::BodyTooLarge(65537))
    ));
    assert_eq!(log.head(&log.identity().unwrap()).unwrap(), head);
}
