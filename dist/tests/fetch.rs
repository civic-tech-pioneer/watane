mod support;
use std::sync::Arc;
use support::*;
use watane_dist::*;

#[test]
fn ac2_fetch_changed_package_never_cached() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let p = alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity().unwrap(), &host.url, None).unwrap())
        .unwrap();
    let id = p.info.release.package;
    let path = alice.site_dir().join("packages").join(id.to_string());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(path, bytes).unwrap();
    assert!(
        matches!(bob.fetch(&p.info.reference), Err(DistError::HashMismatch {url,..}) if url == format!("{}packages/{id}",host.url))
    );
    assert!(!b.path().join("dist/packages").join(id.to_string()).exists());
}
#[test]
fn ac9_fetch_limits_and_timeout() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let p = alice.publish(fixture("1.0.0", false)).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity().unwrap(), &host.url, None).unwrap())
        .unwrap();
    let expected = format!("{}packages/{}", host.url, p.info.release.package);
    *host.mode.lock().unwrap() = Mode::Oversize(p.info.release.size as usize + 1);
    assert!(
        matches!(bob.fetch(&p.info.reference), Err(DistError::TooLarge {url,..}) if url == expected)
    );
    drop(bob);
    // Only this fetch proves a timeout; ordinary requests use the shared limits.
    let bob = Dist::open(
        b.path(),
        std::sync::Arc::new(RecordingConfirm::new([])),
        Options {
            package_timeout: std::time::Duration::from_millis(300),
            ..fast_options()
        },
    )
    .unwrap();
    *host.mode.lock().unwrap() = Mode::Stall(std::time::Duration::from_secs(3));
    assert!(
        matches!(bob.fetch(&p.info.reference), Err(DistError::Timeout {url}) if url == expected)
    );
    assert!(
        !b.path()
            .join("dist/packages")
            .join(p.info.release.package.to_string())
            .exists()
    );
}
#[test]
fn ac10_fetch_cached_offline_without_request() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let bytes = fixture("1.0.0", false);
    let p = alice.publish(bytes.clone()).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity().unwrap(), &host.url, None).unwrap())
        .unwrap();
    bob.fetch(&p.info.reference).unwrap();
    let requests = Arc::clone(&host.requests);
    let count = host.count();
    drop(host);
    assert_eq!(
        bob.fetch(&p.info.reference).unwrap().package.as_bytes(),
        bytes
    );
    assert_eq!(requests.lock().unwrap().len(), count);
}
#[test]
fn ac13_fetch_reference_from_literal_text() {
    use std::os::unix::fs::PermissionsExt;
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    // RFC 8032 test vector 1: a fixed publisher makes the entire reference literal.
    let seed = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
    let seed: Vec<u8> = (0..64)
        .step_by(2)
        .map(|i| u8::from_str_radix(&seed[i..i + 2], 16).unwrap())
        .collect();
    let dir = a.path().join("log");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = dir.join("identity.key");
    std::fs::write(&key, seed).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let alice = open(a.path());
    let bob = open(b.path());
    let bytes = fixture("1.0.0", false);
    alice.publish(bytes.clone()).unwrap();
    let host = TestHost::serve(&alice.site_dir());
    bob.follow(&Link::new(alice.identity().unwrap(), &host.url, None).unwrap())
        .unwrap();
    let reference: ReleaseRef =
        "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy/reading-list@1.0.0"
            .parse()
            .unwrap();
    assert_eq!(bob.fetch(&reference).unwrap().package.as_bytes(), bytes);
}
