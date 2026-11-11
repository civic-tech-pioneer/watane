use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use watane_pkg::{Mod, ModError, PublisherKey, Rule, add_provider};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.insert(path.file_name().unwrap().into(), Vec::new());
            for (p, b) in snapshot(&path) {
                result.insert(PathBuf::from(path.file_name().unwrap()).join(p), b);
            }
        } else {
            result.insert(path.file_name().unwrap().into(), fs::read(&path).unwrap());
        }
    }
    result
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        let dest = to.join(p.file_name().unwrap());
        if p.is_dir() {
            copy(&p, &dest);
        } else {
            fs::copy(p, dest).unwrap();
        }
    }
}
fn setup() -> (tempfile::TempDir, tempfile::TempDir, Mod) {
    let p = tempfile::tempdir().unwrap();
    copy(&fixture("reading-list"), p.path());
    let provider = Mod::read(p.path()).unwrap();
    let c = tempfile::tempdir().unwrap();
    copy(&fixture("hello"), c.path());
    (p, c, provider)
}
#[test]
fn ac1_copies_provider_and_host() {
    let (p, c, m) = setup();
    add_provider(c.path(), &m, None).unwrap();
    let expected: BTreeMap<_, _> = fs::read_dir(p.path().join("wit"))
        .unwrap()
        .map(Result::unwrap)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wit"))
        .map(|p| (PathBuf::from(p.file_name().unwrap()), fs::read(p).unwrap()))
        .collect();
    assert_eq!(snapshot(&c.path().join("wit/deps/reading-list")), expected);
    assert_eq!(
        snapshot(&c.path().join("wit/deps/watane-host")),
        snapshot(&p.path().join("wit/deps/watane-host"))
    );
    assert_eq!(
        fs::read_to_string(c.path().join("deps.json")).unwrap(),
        "{\n  \"mods:reading-list\": null\n}\n"
    );
}
#[test]
fn ac2_idempotent() {
    let (_p, c, m) = setup();
    add_provider(c.path(), &m, None).unwrap();
    let before = snapshot(c.path());
    add_provider(c.path(), &m, None).unwrap();
    assert_eq!(snapshot(c.path()), before);
}
#[test]
fn ac3_conflict_writes_nothing() {
    let (_p, c, m) = setup();
    fs::create_dir_all(c.path().join("wit/deps/watane-host")).unwrap();
    fs::write(c.path().join("wit/deps/watane-host/host.wit"), "different").unwrap();
    let before = snapshot(c.path());
    let Err(ModError::Invalid(v)) = add_provider(c.path(), &m, None) else {
        panic!("expected conflict")
    };
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].rule, Rule::Provider);
    assert_eq!(v[0].file, "wit/deps/watane-host");
    assert_eq!(snapshot(c.path()), before);
    assert!(!c.path().join("deps.json").exists());
}
#[test]
fn ac4_preserves_members_canonically() {
    let (_p, c, m) = setup();
    fs::write(
        c.path().join("deps.json"),
        r#"{"mods:tags":null,"mods:aaa":null}"#,
    )
    .unwrap();
    add_provider(c.path(), &m, None).unwrap();
    assert_eq!(
        fs::read_to_string(c.path().join("deps.json")).unwrap(),
        "{\n  \"mods:aaa\": null,\n  \"mods:reading-list\": null,\n  \"mods:tags\": null\n}\n"
    );
}
#[test]
fn ac5_publisher_text() {
    let (_p, c, m) = setup();
    add_provider(c.path(), &m, Some(PublisherKey::from_bytes([1; 32]))).unwrap();
    assert_eq!(
        fs::read_to_string(c.path().join("deps.json")).unwrap(),
        "{\n  \"mods:reading-list\": \"yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo\"\n}\n"
    );
}
#[test]
fn ac6_consumer_can_call_and_subscribe() {
    let (_p, c, m) = setup();
    add_provider(c.path(), &m, None).unwrap();
    fs::copy(
        fixture("recommendations").join("wit/world.wit"),
        c.path().join("wit/world.wit"),
    )
    .unwrap();
    fs::create_dir_all(c.path().join("logic")).unwrap();
    fs::write(c.path().join("logic/Cargo.toml"), "").unwrap();
    let consumer = Mod::read(c.path()).unwrap();
    assert_eq!(
        consumer.world().calls[0].to_string(),
        "mods:reading-list/books@0.1.0"
    );
    assert_eq!(
        consumer.world().subscriptions[0].to_string(),
        "mods:reading-list/events@0.1.0"
    );
}
