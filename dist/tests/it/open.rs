use crate::support;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use support::RecordingConfirm;
use watane_dist::{Dist, DistError, Options};
#[test]
fn ac5_open_offline_locked_reopen_modes() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Dist>();
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(
        home.path(),
        Arc::<RecordingConfirm>::clone(&c),
        Options::default(),
    )
    .unwrap();
    let key = dist.identity().unwrap();
    assert_eq!(dist.fingerprint().unwrap(), key.to_string()[..10]);
    assert!(matches!(
        Dist::open(
            home.path(),
            Arc::<RecordingConfirm>::clone(&c),
            Options::default()
        ),
        Err(DistError::Locked { .. })
    ));
    for (path, mode) in [
        ("dist", 0o700),
        ("dist/packages", 0o700),
        ("dist/tmp", 0o700),
        ("dist/site", 0o755),
    ] {
        assert_eq!(
            std::fs::metadata(home.path().join(path))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            mode
        );
    }
    assert_eq!(
        std::fs::metadata(home.path().join("dist/lock"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    std::fs::write(home.path().join("dist/tmp/stale"), b"old").unwrap();
    drop(dist);
    let dist = Dist::open(
        home.path(),
        Arc::<RecordingConfirm>::clone(&c),
        Options::default(),
    )
    .unwrap();
    assert_eq!(dist.identity().unwrap(), key);
    assert!(!home.path().join("dist/tmp/stale").exists());
    assert!(c.requests.lock().unwrap().is_empty());
}

#[test]
fn ac5_open_metadata_and_corrupt_files() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([]));
    let dist = Dist::open(
        home.path(),
        Arc::<RecordingConfirm>::clone(&c),
        Options::default(),
    )
    .unwrap();
    let own = dist.identity().unwrap();
    assert_eq!(dist.own().unwrap().base, None);
    assert_eq!(dist.site_dir(), home.path().join("dist/site"));
    drop(dist);
    let other: watane_dist::PublicKey = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy"
        .parse()
        .unwrap();
    assert_ne!(other, own);
    let follows = serde_json::json!({"version":1,"channels":{other.to_string():{"base":"https://alice.example.org/mods","followed_at":1,"last_attempt":2,"last_success":2}}});
    let path = home.path().join("dist/follows.json");
    std::fs::write(&path, follows.to_string()).unwrap();
    std::fs::write(
        home.path().join("dist/site.json"),
        r#"{"version":1,"url":"https://own.example.org/mods"}"#,
    )
    .unwrap();
    let dist = Dist::open(
        home.path(),
        Arc::<RecordingConfirm>::clone(&c),
        Options::default(),
    )
    .unwrap();
    assert_eq!(
        dist.own().unwrap().base.as_deref(),
        Some("https://own.example.org/mods/")
    );
    let channel = dist.channel(&other).unwrap();
    assert!(channel.followed);
    assert_eq!(channel.last_success, Some(2));
    assert_eq!(
        channel.base.as_deref(),
        Some("https://alice.example.org/mods/")
    );
    assert!(
        dist.latest(&other, &"reading-list".parse().unwrap())
            .unwrap()
            .is_none()
    );
    drop(dist);
    for name in ["follows.json", "site.json"] {
        let path = home.path().join("dist").join(name);
        let original = std::fs::read(&path).unwrap();
        for bad in [
            "broken",
            r#"{"version":2,"channels":{},"url":"https://example.org/"}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(
                matches!(Dist::open(home.path(),Arc::<RecordingConfirm>::clone(&c),Options::default()),Err(DistError::Corrupt { path: found, .. }) if found == path)
            );
        }
        std::fs::write(&path, original).unwrap();
    }
    assert!(c.requests.lock().unwrap().is_empty());
}
