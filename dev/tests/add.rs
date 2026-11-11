use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
fn run(dir: &Path, args: &[&str]) -> (i32, Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(dir)
        .env("WATANE_DEV_CACHE", dir.join("cache"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}
fn mods() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for slug in ["a", "b", "c"] {
        assert_eq!(run(t.path(), &["new", slug, "--json"]).0, 0);
    }
    t
}
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(snapshot(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}
#[test]
fn ac1_dev_s25_add_copies_wit_and_reports_lines() {
    let t = mods();
    let b = t.path().join("b");
    let world = fs::read(b.join("wit/world.wit")).unwrap();
    let (code, v) = run(&b, &["add", "../a", "--json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["result"],
        json!({"slug":"a","version":"0.1.0","path":"../a","wit_dir":"wit/deps/a","imports":["import mods:a/api@0.1.0;"],"subscriptions":["export mods:a/changes@0.1.0;"]})
    );
    assert_eq!(
        v["next"],
        "Copy the lines that the mod needs into the world in wit/world.wit. Each line is a grant that the participant approves."
    );
    assert_eq!(
        fs::read(b.join("wit/deps/a/world.wit")).unwrap(),
        fs::read(t.path().join("a/wit/world.wit")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(b.join("deps.json")).unwrap(),
        "{\n  \"mods:a\": null\n}\n"
    );
    assert_eq!(fs::read(b.join("wit/world.wit")).unwrap(), world);
    assert_eq!(
        fs::read_to_string(b.join(".watane-dev.json")).unwrap(),
        "{\n  \"local_providers\": {\n    \"a\": \"../a\"\n  }\n}\n"
    );
}
#[test]
fn ac2_dev_s26_cycle_changes_nothing() {
    let t = mods();
    let b = t.path().join("b");
    assert_eq!(run(&t.path().join("a"), &["add", "../b", "--json"]).0, 0);
    let before = snapshot(&b);
    let (code, v) = run(&b, &["add", "../a", "--json"]);
    assert_eq!(code, 1);
    assert_eq!(v["errors"][0]["code"], "mod.dependency_cycle");
    assert_eq!(snapshot(&b), before);
}
#[test]
fn ac3_self_and_missing_provider() {
    let t = mods();
    let b = t.path().join("b");
    let before = snapshot(&b);
    for path in [".", "../b", b.to_str().unwrap()] {
        let (code, v) = run(&b, &["add", path, "--json"]);
        assert_eq!(code, 1);
        assert_eq!(v["errors"][0]["code"], "mod.self_dependency");
    }
    let (code, v) = run(&b, &["add", "../missing", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "usage.provider");
    assert_eq!(snapshot(&b), before);
}
#[test]
fn ac4_local_order_and_missing_folders() {
    let t = mods();
    let a = t.path().join("a");
    let b = t.path().join("b");
    let c = t.path().join("c");
    assert_eq!(run(&b, &["add", "../a", "--json"]).0, 0);
    let world = fs::read_to_string(b.join("wit/world.wit")).unwrap();
    fs::write(
        b.join("wit/world.wit"),
        world.replace("world b {", "world b {\n    import mods:a/api@0.1.0;"),
    )
    .unwrap();
    let (code, v) = run(&c, &["add", "../b", "--json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        watane_dev::deps::local_order(&c).unwrap(),
        vec![
            a.canonicalize().unwrap(),
            b.canonicalize().unwrap(),
            c.canonicalize().unwrap()
        ]
    );
    for missing_entry in [true, false] {
        if missing_entry {
            fs::remove_file(b.join(".watane-dev.json")).unwrap();
        } else {
            fs::write(
                b.join(".watane-dev.json"),
                r#"{"local_providers":{"a":"../gone"}}"#,
            )
            .unwrap();
        }
        let e = watane_dev::deps::local_order(&c).unwrap_err();
        assert_eq!(e.code.as_str(), "mod.provider_missing");
        assert_eq!(e.message, "the local provider a has no known folder");
        assert_eq!(e.fix.as_deref(), Some("Run: watane-dev add <folder of a>"));
    }
}

#[test]
fn ac1_two_adds_write_sorted_local_paths_and_preserve_consumer_world() {
    let t = mods();
    let c = t.path().join("c");
    fs::write(c.join("wit/world.wit"), "unfinished world").unwrap();
    for slug in ["b", "a"] {
        let (code, v) = run(&c, &["add", &format!("../{slug}"), "--json"]);
        assert_eq!(code, 0, "{v}");
    }
    assert_eq!(
        fs::read_to_string(c.join(".watane-dev.json")).unwrap(),
        "{\n  \"local_providers\": {\n    \"a\": \"../a\",\n    \"b\": \"../b\"\n  }\n}\n"
    );
    assert_eq!(
        fs::read_to_string(c.join("wit/world.wit")).unwrap(),
        "unfinished world"
    );
    assert_eq!(
        watane_dev::deps::read_local(&c)
            .unwrap()
            .get("a")
            .map(String::as_str),
        Some("../a")
    );
}
