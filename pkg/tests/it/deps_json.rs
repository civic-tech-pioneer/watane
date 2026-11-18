use std::fs;
use watane_pkg::*;

#[test]
fn ac2_deps_json() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(Deps::read(dir.path()).unwrap(), Deps::default());
    let mut deps = Deps::default();
    deps.set(
        "tags".parse().unwrap(),
        Some(PublisherKey::from_bytes([1; 32])),
    );
    deps.set("reading-list".parse().unwrap(), None);
    deps.write(dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("deps.json")).unwrap(),
        "{\n  \"mods:reading-list\": null,\n  \"mods:tags\": \"yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo\"\n}\n"
    );
    assert_eq!(Deps::read(dir.path()).unwrap(), deps);
    assert_eq!(deps.get(&"reading-list".parse().unwrap()), Some(None));
    assert_eq!(
        deps.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>(),
        ["reading-list", "tags"]
    );
    assert!(deps.remove(&"tags".parse().unwrap()));
    assert!(deps.remove(&"reading-list".parse().unwrap()));
    assert!(!deps.remove(&"tags".parse().unwrap()));
    deps.write(dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("deps.json")).unwrap(),
        "{}\n"
    );
    assert!(!dir.path().join("deps.json.tmp").exists());
    // A temporary file left by an interrupted write does not block the next one.
    fs::write(dir.path().join("deps.json.tmp"), "stale").unwrap();
    deps.write(dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(dir.path().join("deps.json")).unwrap(),
        "{}\n"
    );
    assert!(!dir.path().join("deps.json.tmp").exists());
    for bad in [
        "[]",
        "null",
        "{",
        "{\"tags\":null}",
        "{\"wasi:tags\":null}",
        "{\"mods:tags\":1}",
        "{\"mods:tags\":\"bad\"}",
        "{\"mods:World\":null}",
    ] {
        fs::write(dir.path().join("deps.json"), bad).unwrap();
        assert!(
            matches!(Deps::read(dir.path()), Err(ModError::Invalid(v)) if v[0].rule == Rule::DepsJson && v[0].file == "deps.json"),
            "{bad}"
        );
    }
    fs::write(dir.path().join("deps.json"), " ".repeat(65537)).unwrap();
    Deps::read(dir.path()).unwrap_err();
    let members: Vec<_> = (0..33).map(|i| format!("\"mods:p{i}\":null")).collect();
    fs::write(
        dir.path().join("deps.json"),
        format!("{{{}}}", members.join(",")),
    )
    .unwrap();
    Deps::read(dir.path()).unwrap_err();
    fs::write(
        dir.path().join("deps.json"),
        format!("{{{}}}", members[..32].join(",")),
    )
    .unwrap();
    assert_eq!(Deps::read(dir.path()).unwrap().iter().count(), 32);
}

#[test]
fn ac2_deps_write_member_limit_is_inclusive() {
    // TECHNICAL.md §4.3: deps.json holds at most 32 members.
    let dir = tempfile::tempdir().unwrap();
    let mut deps = Deps::default();
    for i in 0..32 {
        deps.set(format!("p{i}").parse().unwrap(), None);
    }
    deps.write(dir.path()).unwrap();
    assert_eq!(Deps::read(dir.path()).unwrap(), deps);
    assert_eq!(deps.iter().count(), 32);
    deps.set("p32".parse().unwrap(), None);
    assert!(
        matches!(deps.write(dir.path()), Err(ModError::Invalid(v)) if v[0].rule == Rule::DepsJson)
    );
    // The refused write leaves the 32-member file in place.
    assert_eq!(Deps::read(dir.path()).unwrap().iter().count(), 32);
}
