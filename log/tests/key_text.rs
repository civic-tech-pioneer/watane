mod oracle;
use std::str::FromStr;
use watane_log::{EntryId, IdTextError, KeyTextError as K, PublicKey};
#[test]
fn ac3_rfc_key_text_and_fingerprint() {
    let key = PublicKey::from_bytes(&oracle::PUBLIC).unwrap();
    assert_eq!(key.as_bytes(), &oracle::PUBLIC);
    assert_eq!(key.to_string(), oracle::zbase32(&oracle::PUBLIC));
    assert_eq!(
        key.to_string(),
        "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
    );
    assert_eq!(key.fingerprint(), "47pjoycnsr");
    assert_eq!(PublicKey::from_str(&key.to_string()), Ok(key));
    assert_eq!(format!("{key:?}"), format!("PublicKey({key})"));
}
#[test]
fn ac4_key_and_id_text_refusals() {
    let text = oracle::zbase32(&oracle::PUBLIC);
    for n in [51, 53] {
        assert_eq!(PublicKey::from_str(&"y".repeat(n)), Err(K::Length(n)));
    }
    for s in [text.to_uppercase(), format!("0{}", &text[1..])] {
        assert_eq!(PublicKey::from_str(&s), Err(K::Alphabet));
    }
    assert_eq!(
        PublicKey::from_str(&format!("{}b", &text[..51])),
        Err(K::UnusedBits)
    );
    let mut small = [0; 32];
    small[0] = 1;
    assert_eq!(
        PublicKey::from_str(&oracle::zbase32(&small)),
        Err(K::SmallOrder)
    );
    let mut noncanonical = [255; 32];
    noncanonical[0] = 0xee;
    noncanonical[31] = 0x7f;
    assert_eq!(
        PublicKey::from_str(&oracle::zbase32(&noncanonical)),
        Err(K::NotCanonical)
    );
    let id = EntryId::from_bytes([0xab; 32]);
    assert_eq!(id.to_string(), "ab".repeat(32));
    assert_eq!(EntryId::from_str(&id.to_string()), Ok(id));
    assert_eq!(id.as_bytes(), &[0xab; 32]);
    for s in [
        "AB".repeat(32),
        "g".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
    ] {
        assert_eq!(EntryId::from_str(&s), Err(IdTextError));
    }
}
