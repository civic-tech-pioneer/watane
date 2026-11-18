#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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
        format!(
            "Copy the lines that the mod needs from result.imports and result.subscriptions into the world in wit/world.wit (result.subscriptions holds `export` lines: they are how the mod answers the provider's events and collects; each line is a grant that the participant approves), then run: watane-dev test --json --dir {}",
            b.canonicalize().unwrap().display()
        )
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
    assert_eq!(v["errors"][0]["message"], "../missing is not a mod folder");
    assert_eq!(v["errors"][0]["fix"], RELATIVE_FIX);
    assert_eq!(snapshot(&b), before);
}
const RELATIVE_FIX: &str = "A relative provider folder is read from the current folder, not from --dir. Run add in the provider's folder or pass its absolute path";
#[test]
fn regress_s12b_add_dot_outside_provider_names_folder_and_cwd_rule() {
    let t = mods();
    let b = t.path().join("b");
    let before = snapshot(&b);
    for (cwd, shown) in [
        (t.path().to_path_buf(), t.path()),
        (t.path().join("a/logic"), &*t.path().join("a/logic")),
    ] {
        let (code, v) = run(&cwd, &["add", ".", "--dir", b.to_str().unwrap(), "--json"]);
        assert_eq!(code, 2, "{v}");
        assert_eq!(v["errors"][0]["code"], "usage.provider");
        let found = shown.canonicalize().unwrap();
        assert_eq!(
            v["errors"][0]["message"],
            format!(
                ". is not a mod folder ({} has no mod.toml)",
                found.display()
            )
        );
        assert_eq!(v["errors"][0]["fix"], RELATIVE_FIX);
    }
    // An absolute canonical path names itself once and needs no current-folder fix.
    let wit = b.canonicalize().unwrap().join("wit");
    let (_, v) = run(&b, &["add", wit.to_str().unwrap(), "--json"]);
    assert_eq!(
        v["errors"][0]["message"],
        format!("{} is not a mod folder", wit.display())
    );
    assert_eq!(v["errors"][0]["fix"], Value::Null);
    assert_eq!(snapshot(&b), before);
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

#[test]
fn ac4_add_resolves_provider_from_current_folder() {
    let t = mods();
    let (code, v) = run(t.path(), &["add", "a", "--dir", "b", "--json"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["path"], "../a");
    let local: Value =
        serde_json::from_slice(&fs::read(t.path().join("b/.watane-dev.json")).unwrap()).unwrap();
    assert_eq!(local, json!({"local_providers":{"a":"../a"}}));
}

#[test]
fn ac4_add_dot_from_provider_with_relative_and_absolute_consumer() {
    let t = mods();
    let provider = t.path().join("a");
    for consumer in [
        "../b".to_string(),
        t.path().join("c").to_str().unwrap().to_string(),
    ] {
        let (code, value) = run(&provider, &["add", ".", "--dir", &consumer, "--json"]);
        assert_eq!(code, 0, "{value}");
        assert_eq!(value["result"]["path"], "../a");
        let local: Value = serde_json::from_slice(
            &fs::read(provider.join(&consumer).join(".watane-dev.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(local, json!({"local_providers":{"a":"../a"}}));
        let before = snapshot(&provider.join(&consumer));
        let (code, absolute) = run(
            &provider,
            &[
                "add",
                provider.to_str().unwrap(),
                "--dir",
                &consumer,
                "--json",
            ],
        );
        assert_eq!(code, 0, "{absolute}");
        assert_eq!(absolute["result"], value["result"]);
        assert_eq!(snapshot(&provider.join(&consumer)), before);
    }
}
