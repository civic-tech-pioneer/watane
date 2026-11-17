#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use support::{RecordingConfirm, fixture};
use watane_dist::*;
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(root, &e.path(), out)
            } else {
                out.insert(
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .into(),
                    fs::read(e.path()).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}
#[test]
fn ac6_site_releases_and_withdrawal() {
    let home = tempfile::tempdir().unwrap();
    let c = Arc::new(RecordingConfirm::new([Decision::Approve; 3]));
    let d = Dist::open(
        home.path(),
        Arc::<RecordingConfirm>::clone(&c),
        Options::default(),
    )
    .unwrap();
    let a = d.publish(fixture("1.0.0", false)).unwrap();
    let bytes = fixture("2.0.0", false);
    d.publish(bytes.clone()).unwrap();
    d.withdraw(&a.info.reference, "bug").unwrap();
    assert_eq!(c.requests.lock().unwrap().len(), 3);
    let key = d.identity().unwrap();
    let site = d.site_dir();
    drop(d);
    let log = watane_log::Log::open(&home.path().join("log")).unwrap();
    let entries = log.entries(&key, 0, 10).unwrap();
    let mut expected = BTreeMap::new();
    for e in &entries {
        expected.insert(format!("entries/{}", e.seq()), e.bytes().to_vec());
    }
    expected.insert("head".into(), entries[2].bytes().to_vec());
    expected.insert(format!("packages/{}", blake3::hash(&bytes).to_hex()), bytes);
    assert_eq!(files(&site), expected);
}
#[test]
fn ac7_site_foreign_files_unchanged() {
    let home = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    d.publish(fixture("1.0.0", false)).unwrap();
    fs::write(dest.path().join("index.html"), b"hello").unwrap();
    fs::write(dest.path().join("CNAME"), b"alice.example").unwrap();
    d.export(dest.path()).unwrap();
    assert_eq!(fs::read(dest.path().join("index.html")).unwrap(), b"hello");
    assert_eq!(
        fs::read(dest.path().join("CNAME")).unwrap(),
        b"alice.example"
    );
}
#[test]
fn ac8_site_other_channel_conflict() {
    let alice = tempfile::tempdir().unwrap();
    let carol = tempfile::tempdir().unwrap();
    let a = Dist::open(
        alice.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    let c = Dist::open(
        carol.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    a.publish(fixture("1.0.0", false)).unwrap();
    c.publish(fixture("2.0.0", false)).unwrap();
    let before = files(&c.site_dir());
    assert!(
        matches!(a.export(&c.site_dir()),Err(DistError::ExportConflict {path,..}) if path==c.site_dir().join("head"))
    );
    assert_eq!(files(&c.site_dir()), before);
}
#[test]
fn ac9_site_link_round_trip_and_persistence() {
    let home = tempfile::tempdir().unwrap();
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    let slug = "reading-list".parse().unwrap();
    assert!(matches!(d.own_link(Some(&slug)), Err(DistError::NoSiteUrl)));
    assert!(d.publish(fixture("1.0.0", false)).unwrap().link.is_none());
    d.set_site_url("https://ALICE.example/mods").unwrap();
    let link: Link = d
        .own_link(Some(&slug))
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    assert_eq!(link.channel(), d.identity().unwrap());
    assert_eq!(link.base(), "https://alice.example/mods/");
    assert_eq!(link.slug(), Some(&slug));
    drop(d);
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    assert_eq!(d.own_link(Some(&slug)).unwrap(), link);
    assert_eq!(d.publish(fixture("2.0.0", false)).unwrap().link, Some(link));
}
#[test]
fn ac10a_site_public_modes_on_open_and_export() {
    let home = tempfile::tempdir().unwrap();
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    assert_eq!(
        fs::metadata(d.site_dir()).unwrap().permissions().mode() & 0o777,
        0o755
    );
    d.publish(fixture("1.0.0", false)).unwrap();
    let dest = tempfile::tempdir().unwrap();
    d.export(dest.path()).unwrap();
    for dir in [d.site_dir(), dest.path().into()] {
        for sub in ["", "entries", "packages"] {
            assert_eq!(
                fs::metadata(dir.join(sub)).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
        for name in files(&dir).keys() {
            assert_eq!(
                fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o777,
                0o644
            );
        }
    }
}
#[test]
fn ac10_site_equal_export_does_not_write() {
    let home = tempfile::tempdir().unwrap();
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve])),
        Options::default(),
    )
    .unwrap();
    d.publish(fixture("1.0.0", false)).unwrap();
    let site = d.site_dir();
    let old_time = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1234);
    for path in files(&site).keys() {
        fs::File::options()
            .write(true)
            .open(site.join(path))
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(old_time))
            .unwrap();
    }
    let before: BTreeMap<_, _> = files(&site)
        .keys()
        .map(|p| {
            (
                p.clone(),
                fs::metadata(site.join(p)).unwrap().modified().unwrap(),
            )
        })
        .collect();
    assert!(d.export(&site).unwrap().written.is_empty());
    for (p, time) in before {
        assert_eq!(
            fs::metadata(site.join(p)).unwrap().modified().unwrap(),
            time
        );
    }
}
#[test]
fn site_external_export_keeps_withdrawn_package() {
    let home = tempfile::tempdir().unwrap();
    let dest = tempfile::tempdir().unwrap();
    let d = Dist::open(
        home.path(),
        Arc::new(RecordingConfirm::new([Decision::Approve; 2])),
        Options::default(),
    )
    .unwrap();
    let bytes = fixture("1.0.0", false);
    let name = format!("packages/{}", blake3::hash(&bytes).to_hex());
    let a = d.publish(bytes.clone()).unwrap();
    d.export(dest.path()).unwrap();
    d.withdraw(&a.info.reference, "bug").unwrap();
    d.export(dest.path()).unwrap();
    assert_eq!(files(dest.path()).get(&name), Some(&bytes));
    assert!(!files(&d.site_dir()).contains_key(&name));
}
