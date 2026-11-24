use crate::support;
use support::*;
use watane_dist::*;
#[test]
fn ac12_j3_dist() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let copy = tempfile::tempdir().unwrap();
    let copy_dir = crate::support::export_dir(copy.path());
    let alice = open(a.path());
    alice.set_name("Alice").unwrap();
    let bytes = fixture("1.0.0", false);
    let p = alice.publish(bytes.clone()).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    alice.set_site_url(&host.url).unwrap();
    let link = alice
        .own_link(Some(&"reading-list".parse().unwrap()))
        .unwrap();
    let bob = open(b.path());
    let fetched = bob.open_link(&link).unwrap();
    assert_eq!(fetched.package.as_bytes(), bytes);
    assert_eq!(fetched.entry.bytes(), p.entry.bytes());
    alice.export(&copy_dir).unwrap();
    let path = copy
        .path()
        .join("packages")
        .join(p.info.release.package.to_string());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(path, bytes).unwrap();
    let tampered = TestHost::serve(&copy_dir);
    let fresh = open(c.path());
    let err = fresh
        .open_link(&Link::new(alice.identity().unwrap(), &tampered.url, link.slug()).unwrap())
        .unwrap_err();
    assert!(matches!(err, DistError::HashMismatch { .. }));
    let message = err.to_string().to_lowercase();
    assert!(message.contains("hash mismatch"), "{message}");
    assert!(message.contains(&format!(
        "{}packages/{}",
        tampered.url, p.info.release.package
    )));
}
