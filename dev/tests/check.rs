use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
fn run(dir: &Path, args: &[&str]) -> (i32, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(dir)
        .env("WATANE_DEV_CACHE", dir.join("cache"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn ac2_check_counter_mod_and_rejections() {
    let t = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("missing", t.path().join("outside-link")).unwrap();
    let (c, v) = run(t.path(), &["check", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(v["errors"][0]["code"], "usage.not_a_mod");
    fs::remove_file(t.path().join("outside-link")).unwrap();
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(
        v["result"],
        json!({"slug":"hello","version":"0.1.0","name":"Hello","page":true,"logic":true,"grants":["storage"],"exports":["mods:hello/api@0.1.0"],"events":["mods:hello/changes@0.1.0"],"calls":[],"subscriptions":[],"providers":[]})
    );
    fs::write(dir.join("mod.toml"), "name = \"Hello\"\n").unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 1);
    assert_eq!(v["errors"][0]["code"], "mod.invalid");
    assert_eq!(v["errors"][0]["file"], "mod.toml");
    fs::write(
        dir.join("mod.toml"),
        "name = \"Hello\"\nsummary = \"Hello page\"\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("missing", dir.join("link")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 1);
    assert_eq!(v["errors"][0]["code"], "mod.symlink");
    assert_eq!(v["errors"][0]["file"], "link");
}
#[test]
fn check_missing_dir_and_git_symlink() {
    let t = tempfile::tempdir().unwrap();
    let (c, v) = run(t.path(), &["--dir", "nowhere", "check", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(v["errors"][0]["code"], "usage.not_a_mod");
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    fs::create_dir(dir.join(".git")).unwrap();
    std::os::unix::fs::symlink("missing", dir.join(".git/link")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 0, "{v}");
}

#[test]
fn ac2_check_repairs_host_wit() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    fs::write(dir.join("wit/deps/watane-host/host.wit"), "changed").unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["ok"], true);
    assert_eq!(
        fs::read_to_string(dir.join("wit/deps/watane-host/host.wit")).unwrap(),
        watane_host::HOST_WIT
    );
}

#[test]
fn ac3_unused_provider_fix_respects_managed_files() {
    let t = tempfile::tempdir().unwrap();
    for slug in ["consumer", "provider"] {
        assert_eq!(run(t.path(), &["new", slug, "--json"]).0, 0);
    }
    let dir = t.path().join("consumer");
    assert_eq!(run(&dir, &["add", "../provider", "--json"]).0, 0);
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|error| error["file"] == "deps.json")
        .unwrap();
    assert_eq!(
        error["fix"],
        "Import the provider's interface in wit/world.wit, or ask before deleting wit/deps/provider/ and mods:provider from deps.json"
    );
}
