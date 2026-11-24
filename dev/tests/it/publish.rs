#![cfg(feature = "fake-host")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::in_process;

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
    let cache = home.join("build-cache");
    let mut all = vec![
        "publish".as_ref(),
        "--json".as_ref(),
        "--home".as_ref(),
        home.as_os_str(),
    ];
    all.extend(args.iter().map(std::ffi::OsStr::new));
    in_process::dev(dir, &all, &[("WATANE_DEV_CACHE", Some(cache.as_os_str()))])
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
                format!(
                    "mods:{slug} is a local provider (deps.json names no release for it), and a published mod can depend only on released providers"
                )
            );
            assert_eq!(
                error["fix"],
                format!(
                    "Publish {slug} first (if it is already published, that run reports its release), then depend on its release"
                )
            );
            assert_eq!(error["file"], "deps.json");
        }
        assert_eq!(value["next"], value["errors"][0]["fix"]);
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
        format!(
            "Start the watane app, then run: watane-dev publish --json --dir {} --home {} --wait 17",
            crate::support::shell_path(&home),
            crate::support::shell_path(&home)
        )
    );
    assert_eq!(
        value["next"],
        format!(
            "watane-dev publish --json --dir {} --home {} --wait 17",
            crate::support::shell_path(&home),
            crate::support::shell_path(&home)
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
        "package":package.id().to_string(), "source":source, "providers":[]})
    );
    assert_eq!(
        value["next"],
        format!(
            "Put the site folder {} on your static web host, then give the person this link: https://fake.invalid/#{slug}",
            home.canonicalize().unwrap().join("site").display()
        )
    );
    if source && dir.join("logic/src").exists() {
        assert!(package.paths().any(|p| p.starts_with("logic/src/")));
    }
}
#[test]
fn ac3_ac6_page_package_and_answer() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    // This package-source proof uses a stale empty list; duplicate detection is tested separately.
    fs::write(home.path().join("fake.json"), r#"{"releases":[]}"#).unwrap();
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
    // This package-source proof uses a stale empty list; duplicate detection is tested separately.
    fs::write(home.path().join("fake.json"), r#"{"releases":[]}"#).unwrap();
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
    let fix = format!(
        "Correct mod.toml according to the reported rule, then run: watane-dev publish --json --dir {} --home {}",
        crate::support::shell_path(dir),
        crate::support::shell_path(home.path().canonicalize().unwrap())
    );
    assert_eq!(value["errors"][0]["fix"], json!(fix), "{value}");
    assert_eq!(value["next"], json!(fix), "{value}");
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
        "Put the site folder ",
        " on your static web host, then give the person this link: ",
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
    let dir = page(root.path());
    let (code, value) = publish(&dir, home.path(), &[]);
    assert_eq!(code, 4, "{value}");
    assert_eq!(value["errors"][0]["code"], "host.declined");
    assert_eq!(
        value["errors"][0]["message"],
        "The person declined publishing demo from their watane app"
    );
    assert_eq!(
        value["errors"][0]["fix"],
        format!(
            "The person declined in the app. Ask them, then run: watane-dev publish --json --dir {} --home {}",
            crate::support::shell_path(dir),
            crate::support::shell_path(home.path().canonicalize().unwrap())
        )
    );
    assert_eq!(value["next"], value["errors"][0]["fix"]);
}

#[test]
fn ac2_existing_version_reports_release_and_link_without_post() {
    for (link, suffix) in [
        (
            json!("https://example.test/#demo"),
            "its link is https://example.test/#demo",
        ),
        (
            Value::Null,
            "it has no link yet: set your site's URL in the watane app",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(
            home.path().join("fake.json"),
            json!({"releases":[
                {"slug":"demo","version":"0.2.3","release":"key/demo@0.2.3","seq":7,"link":link}
            ]})
            .to_string(),
        )
        .unwrap();
        let _host = start(home.path());
        let dir = page(root.path());
        let (code, value) = publish(&dir, home.path(), &[]);
        assert_eq!(code, 1, "{value}");
        assert_eq!(value["errors"].as_array().unwrap().len(), 1);
        assert_eq!(value["errors"][0]["code"], "publish.version_exists");
        assert_eq!(
            value["errors"][0]["message"],
            format!("demo 0.2.3 is already published as key/demo@0.2.3; {suffix}")
        );
        assert_eq!(value["errors"][0]["file"], "wit/world.wit");
        assert_eq!(
            value["errors"][0]["fix"],
            format!(
                "This version is already published. To publish changes, raise the version in wit/world.wit, then run: watane-dev publish --json --dir {} --home {}",
                crate::support::shell_path(&dir),
                crate::support::shell_path(home.path().canonicalize().unwrap())
            )
        );
        assert_eq!(
            value["next"],
            format!(
                "Raise the version in wit/world.wit, then run: watane-dev publish --json --dir {} --home {}",
                crate::support::shell_path(&dir),
                crate::support::shell_path(home.path().canonicalize().unwrap())
            )
        );
        assert_eq!(
            value["result"],
            json!({"home": home.path().canonicalize().unwrap(), "release": "key/demo@0.2.3", "link": link})
        );
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&dir)
            .args(["publish", "--home"])
            .arg(home.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("release: key/demo@0.2.3\n"), "{text}");
        assert!(
            text.contains(&format!("link: {}\n", link.as_str().unwrap_or("none yet"))),
            "{text}"
        );
        assert!(
            !dir.join("wit/deps").exists(),
            "check pipeline must not run"
        );
        assert!(!home.path().join("requests").exists());
        assert!(!home.path().join("build-cache").exists());
    }
}

#[test]
fn ac1_other_slug_or_version_continues() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("fake.json"),
        json!({"releases":[
            {"slug":"demo","version":"0.2.2","release":"key/demo@0.2.2","seq":1,"link":null},
            {"slug":"other","version":"0.2.3","release":"key/other@0.2.3","seq":2,"link":null}
        ]})
        .to_string(),
    )
    .unwrap();
    let _host = start(home.path());
    let (code, value) = publish(&page(root.path()), home.path(), &[]);
    assert_eq!(code, 0, "{value}");
    assert!(home.path().join("requests/1-publish").exists());
}

#[test]
fn ac2_missing_link_then_retry_recovers_link() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("fake.json"), r#"{"link":null}"#).unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    let (code, value) = publish(&dir, home.path(), &[]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["ok"], true);
    assert!(value["result"]["link"].is_null());
    assert_eq!(
        value["next"],
        format!(
            "Ask the person to set your site's URL in the watane app, then run: watane-dev publish --json --dir {} --home {}",
            value["dir"].as_str().unwrap(),
            crate::support::shell_path(home.path().canonicalize().unwrap())
        )
    );
    fs::write(
        home.path().join("fake.json"),
        r#"{"link":"https://site.test/#demo"}"#,
    )
    .unwrap();
    let (code, value) = publish(&dir, home.path(), &[]);
    assert_eq!(code, 1, "{value}");
    assert_eq!(value["errors"][0]["code"], "publish.version_exists");
    assert_eq!(
        value["errors"][0]["message"],
        format!(
            "demo 0.2.3 is already published as {}/demo@0.2.3; its link is https://site.test/#demo",
            "y".repeat(52)
        )
    );
    assert_eq!(
        value["result"],
        json!({"home": home.path().canonicalize().unwrap(), "release": format!("{}/demo@0.2.3", "y".repeat(52)), "link": "https://site.test/#demo"})
    );
    assert_eq!(
        fs::read_dir(home.path().join("requests")).unwrap().count(),
        1
    );
}

#[test]
fn ac3_local_provider_next_preserves_home_and_quotes_paths() {
    let root = tempfile::tempdir().unwrap();
    let dir = page(root.path());
    let provider = root.path().join("Bob's provider");
    fs::create_dir(&provider).unwrap();
    fs::write(dir.join("deps.json"), r#"{"mods:a":null,"mods:b":null}"#).unwrap();
    fs::write(
        dir.join(".watane-dev.json"),
        r#"{"local_providers":{"b":"../Bob's provider"}}"#,
    )
    .unwrap();
    let home = dir.join("Bob's home");
    fs::create_dir(&home).unwrap();
    let _host = start(&home);
    for home_arg in [Some(home.as_path()), Some(Path::new("Bob's home")), None] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        command.current_dir(&dir).args(["publish", "--json"]);
        if let Some(home_arg) = home_arg {
            command.arg("--home").arg(home_arg);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        let mut expected = format!(
            "watane-dev publish --json --dir {}",
            crate::support::shell_path(provider.canonicalize().unwrap())
        );
        if home_arg.is_some() {
            expected.push_str(&format!(
                " --home {}",
                crate::support::shell_path(home.canonicalize().unwrap())
            ));
        }
        assert_eq!(value["next"], expected);
        assert!(!home.join("requests").exists());
    }
}

fn releases(home: &Path) -> Value {
    let run: Value = serde_json::from_slice(&fs::read(home.join("run.json")).unwrap()).unwrap();
    ureq::get(format!("http://127.0.0.1:{}/api/releases", run["port"]))
        .call()
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap()
}

#[test]
fn ac6_fake_lists_published_releases_in_this_home_and_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = start(home.path());
    assert_eq!(releases(home.path()), json!({"releases":[]}));
    let (code, value) = publish(&page(root.path()), home.path(), &[]);
    assert_eq!(code, 0, "{value}");
    let expected = json!({"releases":[{"slug":"demo","version":"0.2.3",
        "release":format!("{}/demo@0.2.3", "y".repeat(52)),"seq":1,"link":"https://fake.invalid/#demo"}]});
    assert_eq!(releases(home.path()), expected);
    drop(host);
    let _host = start(home.path());
    assert_eq!(releases(home.path()), expected);
}

#[cfg(unix)] // Uses a POSIX symlink fixture.
#[test]
fn ac1_publish_paths_match_explicit_and_default_symlink_home() {
    let root = tempfile::tempdir().unwrap();
    let dir = page(root.path());
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(root.path(), &alias).unwrap();
    let home = alias.join("new-home");
    let resolved = root.path().canonicalize().unwrap().join("new-home");
    for running in [false, true] {
        let _host = running.then(|| start(&home));
        for (attempt, explicit) in [true, false, true, false].into_iter().enumerate() {
            let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
            command
                .args(["publish", "--json", "--dir"])
                .arg(alias.join("demo"))
                .env("WATANE_HOME", &home);
            if explicit {
                command.arg("--home").arg(&home);
            }
            let output = command.output().unwrap();
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            if running {
                // First call publishes, all later calls find the same version.
                assert_eq!(
                    output.status.code(),
                    Some(if attempt == 0 { 0 } else { 1 }),
                    "{value}"
                );
                if attempt > 0 {
                    assert_eq!(value["errors"][0]["code"], "publish.version_exists");
                }
                assert_eq!(value["result"]["home"], json!(resolved));
            } else {
                assert_eq!(output.status.code(), Some(3), "{value}");
                assert_eq!(
                    value["next"],
                    format!(
                        "watane-dev publish --json --dir {}{}",
                        crate::support::shell_path(&dir),
                        if explicit {
                            format!(" --home {}", crate::support::shell_path(&resolved))
                        } else {
                            String::new()
                        }
                    )
                );
            }
            assert_eq!(value["dir"], json!(dir));
        }
    }
}

#[test]
fn ac4_accepted_fake_publish_completes_under_five_seconds() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("fake.json"), r#"{"releases":[]}"#).unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    for args in [&[][..], &["--wait", "30"][..]] {
        let start = std::time::Instant::now();
        let (code, value) = publish(&dir, home.path(), args);
        let elapsed = start.elapsed();
        eprintln!("accepted fake publish {args:?}: {elapsed:?}");
        assert_eq!(code, 0, "{value}");
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "publish took {elapsed:?}: {value}"
        );
    }
}

#[cfg(unix)] // Uses a POSIX symlink fixture.
#[test]
fn ac1_default_home_and_publish_retry_paths_are_canonical() {
    let root = tempfile::tempdir().unwrap();
    let dir = page(root.path());
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(root.path(), &alias).unwrap();
    let home = alias.join(".watane");
    let _host = start(&home);
    let resolved = home.canonicalize().unwrap();
    for (i, command) in ["install", "publish", "publish"].into_iter().enumerate() {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .env("HOME", &alias)
            .env_remove("WATANE_HOME")
            .args([command, "--json", "--dir"])
            .arg(alias.join("demo"))
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["dir"], json!(dir));
        assert_eq!(value["result"]["home"], json!(resolved));
        // The second publish of the same version must reach the version conflict.
        assert_eq!(output.status.success(), i < 2, "{command}: {value}");
        if !output.status.success() {
            assert_eq!(value["errors"][0]["code"], "publish.version_exists");
            assert_eq!(
                value["next"],
                format!(
                    "Raise the version in wit/world.wit, then run: watane-dev publish --json --dir {}",
                    crate::support::shell_path(&dir)
                )
            );
        }
    }
}

#[cfg(unix)] // Uses a POSIX symlink fixture.
#[test]
fn ac1_publish_raced_version_and_no_answer_keep_canonical_retry_paths() {
    for scenario in ["race", "publish_timeout", "releases_timeout"] {
        let timeout = scenario != "race";
        let root = tempfile::tempdir().unwrap();
        let dir = page(root.path());
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(root.path(), &alias).unwrap();
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        let home = root.path().canonicalize().unwrap();
        fs::write(
            home.join("run.json"),
            json!({"watane":1,"pid":std::process::id(),"port":port,"mode":"home"}).to_string(),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            for path in ["/api/status", "/api/releases", "/api/publish"] {
                let request = server
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.url(), path);
                let (status, body) = match path {
                    "/api/status" => (
                        200,
                        json!({"watane":1,"mode":"home","app":true,"home":home,"port":port}),
                    ),
                    "/api/releases" if scenario == "releases_timeout" => {
                        std::thread::sleep(std::time::Duration::from_millis(2200));
                        return;
                    }
                    "/api/releases" => (200, json!({"releases":[]})),
                    _ if timeout => {
                        std::thread::sleep(std::time::Duration::from_millis(1200));
                        return;
                    }
                    _ => (
                        409,
                        json!({"error":{"code":"version_exists","message":"already published"}}),
                    ),
                };
                request
                    .respond(
                        tiny_http::Response::from_string(body.to_string()).with_status_code(status),
                    )
                    .unwrap();
            }
        });
        let (code, value) = publish(&alias.join("demo"), &alias, &["--no-source", "--wait", "1"]);
        worker.join().unwrap();
        assert_eq!(code, if timeout { 4 } else { 1 }, "{value}");
        assert_eq!(value["dir"], json!(dir));
        assert_eq!(
            value["errors"][0]["code"],
            if timeout {
                "host.no_answer"
            } else {
                "publish.version_exists"
            }
        );
        let prefix = if timeout {
            "Run again with a longer --wait: "
        } else {
            "Raise the version in wit/world.wit, then run: "
        };
        assert_eq!(
            value["next"],
            format!(
                "{prefix}watane-dev publish --json --dir {} --home {} --no-source --wait {}",
                crate::support::shell_path(dir),
                crate::support::shell_path(root.path().canonicalize().unwrap()),
                if timeout { 2 } else { 1 }
            )
        );
    }
}

/// S93 outcome 1: every fix and `next` of a failing publish ends in a complete command
/// (`then run: <rerun>`, dev/TECHNICAL 2.4). host.declined's fix and `next` say
/// `then run publish again with --wait <seconds>`, and publish.version_exists's fix says
/// `then run publish again`: neither can be pasted and run.
#[test]
fn regress_s93_declined_and_version_exists_fixes_end_in_a_command() {
    let rerun = |dir: &Path, home: &Path| {
        format!(
            "then run: watane-dev publish --json --dir {} --home {}",
            crate::support::shell_path(dir),
            crate::support::shell_path(home.canonicalize().unwrap())
        )
    };
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
    let host = start(home.path());
    let dir = page(root.path());
    let (code, value) = publish(&dir, home.path(), &[]);
    drop(host);
    assert_eq!(value["errors"][0]["code"], "host.declined", "{value}");
    assert_eq!(code, 4, "{value}");
    for text in [&value["errors"][0]["fix"], &value["next"]] {
        let text = text.as_str().unwrap();
        assert!(text.contains(&rerun(&dir, home.path())), "{text}");
        assert!(!text.contains('<'), "{text}");
    }

    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("fake.json"),
        json!({"releases":[
            {"slug":"demo","version":"0.2.3","release":"key/demo@0.2.3","seq":7,"link":null}
        ]})
        .to_string(),
    )
    .unwrap();
    let _host = start(home.path());
    let dir = page(root.path());
    let (code, value) = publish(&dir, home.path(), &[]);
    assert_eq!(code, 1, "{value}");
    assert_eq!(value["errors"][0]["code"], "publish.version_exists");
    let fix = value["errors"][0]["fix"].as_str().unwrap();
    assert!(fix.ends_with(&rerun(&dir, home.path())), "{fix}");
}
