#![allow(clippy::unwrap_used, reason = "tests may panic")]

use watane_pkg::*;
#[path = "support/release.rs"]
mod support;
fn hex(s: &str) -> Vec<u8> {
    data_encoding::HEXLOWER.decode(s.as_bytes()).unwrap()
}
const GOLDEN: &str = concat!(
    "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
    "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
    "189ff4808080f6"
);
fn literal() -> Release {
    Release {
        slug: "hello".parse().unwrap(),
        version: "0.1.0".parse().unwrap(),
        name: "Hello".into(),
        summary: "Says hello.".into(),
        package: "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b"
            .parse()
            .unwrap(),
        size: 159,
        source: false,
        offers: vec![],
        events: vec![],
        grants: vec![],
        origin: None,
    }
}

#[test]
fn ac1_release_golden() {
    let dir = support::hello();
    let m = Mod::read(dir.path()).unwrap();
    let p = pack(dir.path(), false).unwrap();
    let r = Release::new(&m, &p).unwrap();
    assert_eq!(r, literal());
    assert_eq!(r.encode(), hex(GOLDEN));
    assert_eq!(hex(GOLDEN).len(), 73);
    assert_eq!(Release::decode(&hex(GOLDEN)).unwrap(), literal());
    assert_eq!(RELEASE_KIND, "release");
    assert_eq!(MAX_RELEASE_BYTES, 32768);
}
#[test]
fn ac2_grants_and_origin_golden() {
    let mut r = literal();
    let provider = Provider {
        slug: "reading-list".parse().unwrap(),
        publisher: Some(PublisherKey::from_bytes([1; 32])),
    };
    r.grants = vec![
        Grant::Storage,
        Grant::Http {
            host: "api.openlibrary.org".parse().unwrap(),
        },
        Grant::Call {
            interface: "mods:reading-list/books@0.1.0".parse().unwrap(),
            provider: provider.clone(),
        },
        Grant::Subscribe {
            interface: "mods:reading-list/events@0.1.0".parse().unwrap(),
            provider,
        },
    ];
    let grants = hex(concat!(
        "8481008201736170692e6f70656e6c6962726172792e6f7267",
        "8302781d6d6f64733a72656164696e672d6c6973742f626f6f6b7340302e312e30",
        "58200101010101010101010101010101010101010101010101010101010101010101",
        "8303781e6d6f64733a72656164696e672d6c6973742f6576656e747340302e312e30",
        "58200101010101010101010101010101010101010101010101010101010101010101"
    ));
    let mut expected = hex(GOLDEN);
    expected.splice(71..72, grants);
    assert_eq!(r.encode(), expected);
    assert_eq!(Release::decode(&expected).unwrap(), r);
    r.grants.clear();
    r.origin = Some(ReleaseRef {
        publisher: PublisherKey::from_bytes([1; 32]),
        slug: "reading-list".parse().unwrap(),
        version: "0.1.0".parse().unwrap(),
    });
    let origin = hex(&format!(
        "835820{}6c72656164696e672d6c69737465302e312e30",
        "01".repeat(32)
    ));
    let mut expected = hex(GOLDEN);
    expected.splice(72..73, origin);
    assert_eq!(r.encode(), expected);
    assert_eq!(Release::decode(&expected).unwrap(), r);
}
#[test]
fn ac3_local_provider_names_slug() {
    let dir = support::built_fixture("recommendations");
    let m = Mod::read(dir.path()).unwrap();
    let p = pack(dir.path(), false).unwrap();
    let e = Release::new(&m, &p).unwrap_err();
    assert!(matches!(&e,ReleaseError::LocalProvider{slug} if slug.as_str()=="reading-list"));
    assert!(e.to_string().contains("reading-list"));
}
