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
fn ac3_check_rejects_builder_inputs() {
    for (file, content, code, name) in [
        (
            "logic/build.rs",
            "fn main() {}",
            "mod.logic_file",
            "logic/build.rs",
        ),
        (
            "logic/Cargo.toml",
            "\n[build-dependencies]\n",
            "mod.cargo_key",
            "build-dependencies",
        ),
        (
            "logic/Cargo.toml",
            "\n[lib]\nproc-macro = true\n",
            "mod.cargo_key",
            "lib.proc-macro",
        ),
        (
            "logic/Cargo.toml",
            "\n[dependencies]\nrand = \"0.9\"\n",
            "mod.dependency",
            "rand",
        ),
    ] {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
        let dir = t.path().join("hello");
        if file.ends_with("Cargo.toml") {
            fs::write(
                dir.join(file),
                format!("[package]\nname = \"hello\"\nversion = \"0.1.0\"\n[workspace]\n{content}"),
            )
            .unwrap();
        } else {
            fs::write(dir.join(file), content).unwrap();
        }
        let (status, v) = run(&dir, &["check", "--json"]);
        assert_eq!(status, 1, "{name}: {v}");
        let error = v["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["code"] == code && e["message"].as_str().unwrap().contains(name))
            .expect("named error");
        assert_eq!(error["file"], file);
        if name == "rand" {
            for allowed in ["wit-bindgen 0.63.0", "serde 1.0.229", "serde_json 1.0.154"] {
                assert!(error["fix"].as_str().unwrap().contains(allowed));
            }
        }
    }
}
