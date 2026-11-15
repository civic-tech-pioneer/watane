mod support;
use std::{collections::BTreeMap, path::Path, sync::Arc};
use support::*;
use watane_dist::*;
fn snapshot(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(root: &Path, p: &Path, out: &mut BTreeMap<std::path::PathBuf, Vec<u8>>) {
        for item in std::fs::read_dir(p).unwrap() {
            let p = item.unwrap().path();
            if p.is_dir() {
                out.insert(p.strip_prefix(root).unwrap().to_owned(), Vec::new());
                visit(root, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().to_owned(),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn assert_keyless_own_paths(d: &Dist, home: &Path) {
    assert_eq!(d.identity(), None);
    assert_eq!(d.fingerprint(), None);
    assert!(!home.join("log/identity.key").exists());
    let key = PublicKey::from_bytes(&[1; 32]).unwrap();
    let link = Link::new(key, "http://127.0.0.1:1/", None).unwrap();
    let reference = format!("{key}/reading-list@1.0.0").parse().unwrap();
    let id = EntryId::from_bytes([0; 32]);
    let before = snapshot(home);
    macro_rules! no {
        ($e:expr) => {
            assert!(
                matches!($e, Err(DistError::NoIdentity)),
                "{}",
                stringify!($e)
            );
        };
    }
    no!(d.publish(vec![]));
    no!(d.set_name(""));
    no!(d.withdraw(&reference, ""));
    no!(d.list(&link, ""));
    no!(d.include(&link, ""));
    no!(d.unlist(&id));
    no!(d.backup(""));
    no!(d.restore(&[], ""));
    no!(d.confirm_own(None));
    no!(d.confirm_own(Some(&link)));
    no!(d.drop_identity());
    no!(d.confirm_own_head(0, &id));
    no!(d.own());
    no!(d.own_confirmed());
    no!(d.own_link(None));
    no!(d.set_site_url("bad"));
    no!(d.export(&home.join("export")));
    assert_eq!(snapshot(home), before);
}
#[test]
fn ac1_keyless_rejects_own_paths_without_changes() {
    let home = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(home.path(), fast_options()).unwrap();
    assert_keyless_own_paths(&d, home.path());

    // Retain a recording Confirm while exercising the same guards after dropping the key.
    let home = tempfile::tempdir().unwrap();
    let confirm = Arc::new(RecordingConfirm::default());
    let d = Dist::open(home.path(), confirm.clone(), fast_options()).unwrap();
    d.drop_identity().unwrap();
    assert_keyless_own_paths(&d, home.path());
    assert!(confirm.requests.lock().unwrap().is_empty());
}
#[test]
fn ac2_keyless_drop_unused_identity_only() {
    let home = tempfile::tempdir().unwrap();
    let d = open(home.path());
    d.set_site_url("http://example.org/").unwrap();
    std::fs::write(d.site_dir().join("old"), b"old").unwrap();
    d.drop_identity().unwrap();
    assert!(!home.path().join("log/identity.key").exists());
    assert!(!home.path().join("dist/site.json").exists());
    assert_eq!(std::fs::read_dir(d.site_dir()).unwrap().count(), 0);
    assert!(matches!(d.publish(vec![]), Err(DistError::NoIdentity)));
    drop(d);
    let d = open(home.path());
    d.set_name("Alice").unwrap();
    let before = std::fs::read(home.path().join("log/identity.key")).unwrap();
    assert!(matches!(
        d.drop_identity(),
        Err(DistError::Log(watane_log::Error::IdentityInUse(_)))
    ));
    assert_eq!(
        std::fs::read(home.path().join("log/identity.key")).unwrap(),
        before
    );
}
#[test]
fn ac1_keyless_empty_follow_fetch_and_changed_package() {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let published = alice.publish(fixture("1.0.0", false)).unwrap();
    let key = alice.identity().unwrap();
    let host = TestHost::serve(&alice.site_dir());
    let h = tempfile::tempdir().unwrap();
    let d = Dist::open_keyless(h.path(), fast_options()).unwrap();
    assert_eq!(d.identity(), None);
    assert_eq!(d.fingerprint(), None);
    assert!(!h.path().join("log/identity.key").exists());
    let link = Link::new(key, &host.url, Some(&published.info.reference.slug)).unwrap();
    let fetched = d.open_link(&link).unwrap();
    assert_eq!(fetched.entry, published.entry);
    let file = alice
        .site_dir()
        .join(format!("packages/{}", published.info.release.package));
    assert_eq!(fetched.package.as_bytes(), std::fs::read(&file).unwrap());
    let h2 = tempfile::tempdir().unwrap();
    let d2 = Dist::open_keyless(h2.path(), fast_options()).unwrap();
    d2.follow(&link).unwrap();
    let mut bad = std::fs::read(&file).unwrap();
    bad[10] ^= 1;
    std::fs::write(file, bad).unwrap();
    assert!(matches!(
        d2.fetch(&published.info.reference),
        Err(DistError::HashMismatch { .. })
    ));
    assert!(
        !h2.path()
            .join(format!("dist/packages/{}", published.info.release.package))
            .exists()
    );
    drop(alice);
    assert!(matches!(
        Dist::open_keyless(a.path(), fast_options()),
        Err(DistError::Log(watane_log::Error::HasIdentity { .. }))
    ));
}
