use watane_pkg::*;
#[path = "support/release.rs"]
mod support;
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
fn ac5_every_field_and_first_difference() {
    let dir = support::hello();
    let p = pack(dir.path(), false).unwrap();
    let m = Mod::read(dir.path()).unwrap();
    let fields = [
        "slug", "version", "name", "summary", "package", "size", "source", "offers", "events",
        "grants", "origin",
    ];
    for field in fields {
        let mut r = literal();
        match field {
            "slug" => r.slug = "other".parse().unwrap(),
            "version" => r.version = "1.0.0".parse().unwrap(),
            "name" => r.name = "Other".into(),
            "summary" => r.summary = "Other.".into(),
            "package" => r.package = PackageId::from_bytes([0; 32]),
            "size" => r.size = 160,
            "source" => r.source = true,
            "offers" => r.offers = vec!["mods:hello/api@0.1.0".parse().unwrap()],
            "events" => r.events = vec!["mods:hello/events@0.1.0".parse().unwrap()],
            "grants" => r.grants = vec![Grant::Storage],
            "origin" => {
                r.origin = Some(ReleaseRef {
                    publisher: PublisherKey::from_bytes([1; 32]),
                    slug: "reading-list".parse().unwrap(),
                    version: "0.1.0".parse().unwrap(),
                })
            }
            _ => unreachable!(),
        }
        let e = r.check(&m, &p).unwrap_err();
        assert!(
            matches!(e,ReleaseError::Mismatch{field:f} if f==field),
            "expected {field}: {e}"
        );
        assert!(e.to_string().contains(field));
        // A later difference must not obscure the first one.
        if field != "origin" {
            r.origin = Some(ReleaseRef {
                publisher: PublisherKey::from_bytes([1; 32]),
                slug: "other".parse().unwrap(),
                version: "1.0.0".parse().unwrap(),
            });
        }
        assert!(matches!(r.check(&m,&p),Err(ReleaseError::Mismatch{field:f}) if f==field));
    }
}
#[test]
fn ac6_round_trip_and_changed_package() {
    for name in ["hello", "reading-list"] {
        for source in [false, true] {
            let dir = if name == "hello" {
                support::hello()
            } else {
                support::built_fixture(name)
            };
            let p = pack(dir.path(), source).unwrap();
            let m = Mod::read(dir.path()).unwrap();
            let r = Release::new(&m, &p).unwrap();
            assert_eq!(Release::decode(&r.encode()).unwrap(), r);
            assert_eq!(r.package, p.id());
            let tmp = tempfile::tempdir().unwrap();
            let unpacked = p.unpack(&tmp.path().join("mod")).unwrap();
            assert!(r.check(&unpacked, &p).is_ok());
            let mut bytes = p.as_bytes().to_vec();
            // Change one UI byte without changing the package grammar or mod metadata.
            let i = bytes
                .windows(3)
                .position(|b| b == b"<p>" || b == b"<h1")
                .unwrap();
            bytes[i + 1] = b'b';
            let changed = Package::from_bytes(bytes).unwrap();
            assert!(matches!(
                r.check(&unpacked, &changed),
                Err(ReleaseError::Mismatch { field: "package" })
            ));
        }
    }
}
