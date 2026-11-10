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
fn ac3_check_page_mod_and_rejections() {
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
        json!({"slug":"hello","version":"0.1.0","name":"Hello","page":true,"logic":false,"grants":[],"exports":[],"events":[],"calls":[],"subscriptions":[],"providers":[]})
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
