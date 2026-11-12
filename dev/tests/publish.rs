#![cfg(feature = "fake-host")]
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};
struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(home: &Path) -> Host {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
        .arg("--home")
        .arg(home)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    Host(child)
}
fn page(root: &Path) -> PathBuf {
    let dir = root.join("demo");
    fs::create_dir_all(dir.join("wit")).unwrap();
    fs::create_dir_all(dir.join("ui")).unwrap();
    fs::write(
        dir.join("mod.toml"),
        "name = \"Demo\"\nsummary = \"a page\"\n",
    )
    .unwrap();
    fs::write(
        dir.join("wit/world.wit"),
        "package mods:demo@0.2.3; world demo {}",
    )
    .unwrap();
    fs::write(dir.join("ui/index.html"), "<title>Demo</title>").unwrap();
    dir.canonicalize().unwrap()
}
fn publish(dir: &Path, home: &Path, args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(dir)
        .env("WATANE_DEV_CACHE", home.join("build-cache"))
        .args(["publish", "--json", "--home"])
        .arg(home)
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
#[test]
fn ac1_local_providers_precede_status_and_pipeline() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    fs::write(dir.join("deps.json"), r#"{"mods:a":null,"mods:b":null}"#).unwrap();
    fs::write(dir.join("mod.toml"), "invalid").unwrap();
    for running in [true, false] {
        if !running {
            fs::remove_file(home.path().join("run.json")).unwrap();
        }
        let (code, value) = publish(&dir, home.path(), &[]);
        assert_eq!(code, 1, "{value}");
        assert_eq!(value["errors"].as_array().unwrap().len(), 2);
        for (error, slug) in value["errors"].as_array().unwrap().iter().zip(["a", "b"]) {
            assert_eq!(error["code"], "publish.local_dependency");
            assert_eq!(
                error["message"],
                format!("mods:{slug} is a local provider; it has no release")
            );
            assert_eq!(
                error["fix"],
                format!("Publish {slug} first, then depend on its release")
            );
            assert_eq!(error["file"], "deps.json");
        }
        assert!(value["result"].is_null());
        assert!(!home.path().join("requests").exists());
        assert!(!home.path().join("build-cache").exists());
    }
}

#[test]
fn ac2_no_app_precedes_pipeline() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path().canonicalize().unwrap();
    let (code, value) = publish(&home, &home, &["--wait", "17"]);
    assert_eq!(code, 3, "{value}");
    assert_eq!(value["errors"][0]["code"], "env.host_not_running");
    assert_eq!(
        value["errors"][0]["message"],
        format!("no watane app runs for the home {}", home.display())
    );
    assert_eq!(
        value["errors"][0]["fix"],
        "Start the watane app, then run this command again"
    );
    assert_eq!(
        value["next"],
        format!(
            "watane-dev publish --json --dir {} --home {} --wait 17",
            home.display(),
            home.display()
        )
    );
    assert!(!home.join("build-cache").exists());
}
fn approved(dir: &Path, home: &Path, number: usize, source: bool, slug: &str, version: &str) {
    let (code, value) = publish(dir, home, if source { &[] } else { &["--no-source"] });
    assert_eq!(code, 0, "{value}");
    let package = watane_pkg::Package::from_bytes(
        fs::read(home.join(format!("requests/{number}-publish"))).unwrap(),
    )
    .unwrap();
    assert_eq!(package.has_source(), source);
    if !source {
        assert!(!package.paths().any(|p| p.starts_with("logic/src/")));
    }
    assert_eq!(
        value["result"],
        json!({"home":home.canonicalize().unwrap(),
        "release":format!("{}/{slug}@{version}", "y".repeat(52)),
        "site":home.canonicalize().unwrap().join("site"),
        "link":format!("https://fake.invalid/#{slug}"),
        "package":package.id().to_string(), "source":source})
    );
    assert_eq!(value["next"], "Put the site folder on your static web host");
    if source && dir.join("logic/src").exists() {
        assert!(package.paths().any(|p| p.starts_with("logic/src/")));
    }
}
#[test]
fn ac3_ac6_page_package_and_answer() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    fs::create_dir(dir.join("tests")).unwrap();
    fs::write(dir.join("tests/page.json"), "[]").unwrap();
    approved(&dir, home.path(), 1, false, "demo", "0.2.3");
    approved(&dir, home.path(), 2, true, "demo", "0.2.3");
}
#[test]
#[ignore = "slow: builds and tests a Rust mod to prove publish excludes and includes logic/src"]
fn ac3_logic_source_selection() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    let dir = root.path().join("counter");
    watane_dev::template::new_mod(&dir, "counter").unwrap();
    approved(&dir, home.path(), 1, false, "counter", "0.1.0");
    approved(&dir, home.path(), 2, true, "counter", "0.1.0");
}
#[test]
fn ac5_failed_pipeline_sends_nothing() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    fs::write(dir.join("mod.toml"), "invalid").unwrap();
    let (code, value) = publish(&dir, home.path(), &[]);
    assert_eq!(code, 1, "{value}");
    assert_eq!(value["errors"][0]["code"], "mod.invalid");
    assert_eq!(value["next"], "watane-dev test --json");
    assert!(!home.path().join("requests").exists());
}
#[test]
fn ac7_text_output_and_help_walk() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(&dir)
        .args(["publish", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "publish: ok",
        "release: ",
        "package: ",
        "Put the site folder on your static web host",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    let help = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .args(["publish", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for expected in ["publish", "--no-source", "--home", "--wait", "600", "app"] {
        assert!(help.contains(expected), "{help}");
    }
}

#[test]
fn ac4_declined_names_publishing_and_asks_before_retry() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
    let _host = start(home.path());
    let (code, value) = publish(&page(root.path()), home.path(), &[]);
    assert_eq!(code, 4, "{value}");
    assert_eq!(value["errors"][0]["code"], "host.declined");
    assert_eq!(
        value["errors"][0]["message"],
        "The person declined publishing demo from their watane app"
    );
    assert_eq!(
        value["errors"][0]["fix"],
        "Ask the person whether they want demo published, then run publish again with --wait <seconds>"
    );
    assert_eq!(value["next"], value["errors"][0]["fix"]);
}
