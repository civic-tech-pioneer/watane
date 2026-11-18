#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod in_process;

use serde_json::Value;
use std::process::Command;

#[test]
fn ac1_no_host_precedes_pipeline() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(home.path())
        .args(["install", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(3), "{value}");
    assert_eq!(value["errors"][0]["code"], "env.host_not_running");
    assert!(
        value["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains(home.path().to_str().unwrap())
    );
    let (dir, home) = (
        home.path().canonicalize().unwrap(),
        home.path().canonicalize().unwrap(),
    );
    assert_eq!(
        value["errors"][0]["fix"],
        format!(
            "Start the watane app, then run: watane-dev install --json --dir {} --home {}",
            dir.display(),
            home.display()
        )
    );
}

/// With no app, install's `next` sends the agent to `doctor --json`, which takes no `--home`,
/// reports `ok: true` about another home and then says `watane-dev new <slug> --json`.
/// The useful next step is to run install again once the person has started the app.
#[test]
fn regress_s12b_no_host_next_repeats_install() {
    let home = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(home.path())
        .args(["install", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["errors"][0]["code"], "env.host_not_running",
        "{value}"
    );
    let next = value["next"].as_str().unwrap_or("");
    assert!(
        next.contains("watane-dev install"),
        "next should repeat the install once the app runs, got {next:?}"
    );
    assert_eq!(
        next,
        format!(
            "watane-dev install --json --dir {} --home {}",
            home.path().canonicalize().unwrap().display(),
            home.path().canonicalize().unwrap().display()
        )
    );
}

/// With no app and `--dir`, install's `next` drops the `--dir`: run from where the agent was,
/// it fails with `usage.not_a_mod` instead of installing the mod once the app runs.
#[test]
fn regress_s12b_no_host_next_keeps_dir() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let mod_dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(elsewhere.path())
        .args(["install", "--json", "--home"])
        .arg(home.path())
        .arg("--dir")
        .arg(mod_dir.path())
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["errors"][0]["code"], "env.host_not_running",
        "{value}"
    );
    let next = value["next"].as_str().unwrap_or("");
    let dir = mod_dir.path().canonicalize().unwrap();
    assert!(
        next.contains("--dir") && next.contains(dir.to_str().unwrap()),
        "next should keep --dir {} so it runs from any folder, got {next:?}",
        dir.display()
    );
    assert_eq!(
        next,
        format!(
            "watane-dev install --json --dir {} --home {}",
            dir.display(),
            home.path().canonicalize().unwrap().display()
        )
    );
}

#[cfg(feature = "fake-host")]
mod fake {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io::{BufRead, BufReader};
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    #[test]
    fn ac2_fake_help_and_wrong_arguments() {
        for args in [
            vec!["--help"],
            vec!["-h"],
            vec![],
            vec!["--unknown"],
            vec!["--home"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
                .args(&args)
                .output()
                .unwrap();
            let help = args == ["--help"] || args == ["-h"];
            assert_eq!(output.status.success(), help, "{args:?}: {output:?}");
            let text = String::from_utf8(if help { output.stdout } else { output.stderr }).unwrap();
            assert!(text.starts_with("Usage:"), "{text}");
            assert!(text.contains("--preview <dir>"), "{text}");
            assert!(text.contains("--home <dir>"), "{text}");
            assert!(text.contains("preview host"), "{text}");
            assert!(text.contains("home host"), "{text}");
        }
    }
    fn start(home: &Path, mode: &str) -> (Host, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fake-watane"))
            .args([mode])
            .arg(home)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        (Host(child), ready)
    }
    fn page(root: &Path, slug: &str) -> PathBuf {
        let dir = root.join(slug);
        fs::create_dir_all(dir.join("wit")).unwrap();
        fs::create_dir_all(dir.join("ui")).unwrap();
        fs::write(
            dir.join("mod.toml"),
            format!("name = \"{slug}\"\nsummary = \"a page\"\n"),
        )
        .unwrap();
        fs::write(
            dir.join("wit/world.wit"),
            format!("package mods:{slug}@0.1.0;\nworld {slug} {{}}\n"),
        )
        .unwrap();
        fs::write(
            dir.join("ui/index.html"),
            "<!doctype html><title>Page</title>",
        )
        .unwrap();
        dir.canonicalize().unwrap()
    }
    fn install(dir: &Path, home: &Path) -> (i32, Value) {
        let cache = home.join("build-cache");
        super::in_process::dev(
            dir,
            &[
                "install".as_ref(),
                "--json".as_ref(),
                "--home".as_ref(),
                home.as_os_str(),
            ],
            &[("WATANE_DEV_CACHE", Some(cache.as_os_str()))],
        )
    }
    #[test]
    fn ac1_install_result_home_matches_explicit_and_default_symlink() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let alias = root.path().join("alias");
        fs::create_dir(&home).unwrap();
        std::os::unix::fs::symlink(&home, &alias).unwrap();
        let (_host, _) = start(&home, "--home");
        let dir = page(root.path(), "demo");
        for explicit in [true, false] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
            command
                .current_dir(&dir)
                .args(["install", "--json"])
                .env("WATANE_HOME", &alias);
            if explicit {
                command.arg("--home").arg(&alias);
            }
            let output = command.output().unwrap();
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(output.status.success(), "{value}");
            assert_eq!(value["result"]["home"], json!(home.canonicalize().unwrap()));
        }
    }

    #[test]
    fn ac2_pipeline_environment_errors_keep_doctor_next() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (_host, _) = start(home.path(), "--home");
        let dir = root.path().join("counter");
        watane_dev::template::new_mod(&dir, "counter").unwrap();
        for (variable, code) in [
            ("WATANE_DEV_TOOLCHAIN", "env.toolchain"),
            ("WATANE_DEV_VENDOR", "env.allowlist"),
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
                .current_dir(&dir)
                .env_remove("WATANE_DEV_TOOLCHAIN")
                .env(variable, root.path().join("missing"))
                .env("WATANE_DEV_CACHE", root.path().join("cache"))
                .args(["install", "--json", "--home"])
                .arg(home.path())
                .output()
                .unwrap();
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(output.status.code(), Some(3), "{value}");
            assert_eq!(value["errors"][0]["code"], code, "{value}");
            assert_eq!(value["next"], "watane-dev doctor --json");
        }
    }

    #[test]
    fn ac3_declined() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
        let (_host, _) = start(home.path(), "--home");
        let (code, value) = install(&page(root.path(), "counter"), home.path());
        assert_eq!(code, 4, "{value}");
        assert_eq!(value["errors"][0]["code"], "host.declined");
        assert_eq!(
            value["errors"][0]["message"],
            "The person declined installing counter in their watane app"
        );
    }
    #[test]
    fn ac3_never_answer_is_no_answer() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("fake.json"), r#"{"answer":"never"}"#).unwrap();
        let (_host, _) = start(home.path(), "--home");
        let dir = page(root.path(), "counter");
        let started = std::time::Instant::now();
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&dir)
            .env("WATANE_DEV_CACHE", home.path().join("build-cache"))
            .args(["install", "--json", "--wait", "1", "--home"])
            .arg(home.path())
            .output()
            .unwrap();
        assert!(started.elapsed().as_secs() < 10);
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(4), "{value}");
        assert_eq!(value["errors"][0]["code"], "host.no_answer");
        assert_eq!(
            value["errors"][0]["message"],
            "no answer from the home host in 1 s"
        );
        let fix = "Run the command again with a longer --wait";
        assert_eq!(value["errors"][0]["fix"], fix, "{value}");
        assert_eq!(value["next"], fix, "{value}");
    }
    #[test]
    fn ac5_declined_next_is_first_fix() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("fake.json"), r#"{"answer":"decline"}"#).unwrap();
        let (_host, _) = start(home.path(), "--home");
        let (code, value) = install(&page(root.path(), "counter"), home.path());
        assert_eq!(code, 4, "{value}");
        assert_eq!(value["errors"][0]["code"], "host.declined");
        assert_eq!(value["next"], value["errors"][0]["fix"]);
    }
    #[test]
    fn ac4_page_only_result_and_failed_pipeline() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (_host, ready) = start(home.path(), "--home");
        let dir = page(root.path(), "counter");
        let (code, value) = install(&dir, home.path());
        assert_eq!(code, 0, "{value}");
        let page = format!("http://counter.localhost:{}/", ready["port"]);
        assert_eq!(
            value["result"]["home"],
            json!(home.path().canonicalize().unwrap())
        );
        let entry = &value["result"]["installed"][0];
        assert_eq!(value["result"]["installed"].as_array().unwrap().len(), 1);
        assert_eq!(entry.as_object().unwrap().len(), 5, "{entry}");
        assert_eq!(entry["slug"], "counter");
        assert_eq!(entry["local"], "counter");
        assert_eq!(entry["version"], "0.1.0");
        assert_eq!(entry["page"], page);
        assert_eq!(entry["package"].as_str().unwrap().len(), 64);
        assert_eq!(value["next"], format!("Give the person this page: {page}"));
        fs::write(dir.join("mod.toml"), "invalid").unwrap();
        let (code, value) = install(&dir, home.path());
        assert_eq!(code, 1, "{value}");
        assert_eq!(
            value["next"],
            "Correct mod.toml according to the reported rule"
        );
        let requests = fs::read_dir(home.path().join("requests")).unwrap().count();
        assert_eq!(requests, 1, "a failed pipeline sends no request");
    }
    #[test]
    #[ignore = "slow: builds and tests two dependent Rust mods before installing them"]
    fn ac4_provider_order_result_and_failed_pipeline() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (_host, ready) = start(home.path(), "--home");
        let a = root.path().canonicalize().unwrap().join("a");
        let b = root.path().canonicalize().unwrap().join("b");
        watane_dev::template::new_mod(&a, "a").unwrap();
        watane_dev::template::new_mod(&b, "b").unwrap();
        watane_dev::deps::add_local(&b, &a).unwrap();
        let world = fs::read_to_string(b.join("wit/world.wit")).unwrap();
        fs::write(
            b.join("wit/world.wit"),
            world.replace("world b {", "world b {\n    import mods:a/api@0.1.0;"),
        )
        .unwrap();
        let (code, value) = install(&b, home.path());
        assert_eq!(code, 0, "{value}");
        let requests: Vec<_> = fs::read_dir(home.path().join("requests"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].file_name().unwrap(), "1-install");
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&requests[0]).unwrap()).unwrap(),
            json!({"folders":[a,b]})
        );
        assert_eq!(
            value["result"]["home"],
            json!(home.path().canonicalize().unwrap())
        );
        for (entry, slug) in value["result"]["installed"]
            .as_array()
            .unwrap()
            .iter()
            .zip(["a", "b"])
        {
            assert_eq!(
                entry
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["local", "package", "page", "slug", "version"]
            );
            assert_eq!(entry["slug"], slug);
            assert_eq!(entry["local"], slug);
            assert_eq!(entry["version"], "0.1.0");
            assert_eq!(
                entry["page"],
                format!("http://{slug}.localhost:{}/", ready["port"])
            );
            assert_eq!(entry["package"].as_str().unwrap().len(), 64);
        }
        assert_eq!(value["result"]["installed"].as_array().unwrap().len(), 2);
        assert_eq!(
            value["next"],
            format!(
                "Give the person this page: http://b.localhost:{}/",
                ready["port"]
            )
        );
        fs::write(a.join("mod.toml"), "invalid").unwrap();
        let (code, value) = install(&b, home.path());
        assert_eq!(code, 1, "{value}");
        assert_eq!(
            value["next"],
            format!("watane-dev test --json --dir {}", a.display())
        );
        assert_eq!(value["errors"][0]["fix"], value["next"]);
        assert_eq!(
            fs::read_dir(home.path().join("requests")).unwrap().count(),
            1
        );
    }
    #[test]
    fn ac6_fake_preview_status_publish_and_ready() {
        let home = tempfile::tempdir().unwrap();
        let (_host, ready) = start(home.path(), "--preview");
        assert_eq!(ready["token"], "wt_fake");
        let _: watane_host::Ready = serde_json::from_value(ready.clone()).unwrap();
        let run = watane_host::RunInfo::read(home.path()).unwrap().unwrap();
        assert_eq!(run.mode, "preview");
        let status =
            watane_dev::api::Client::home_status(&home.path().canonicalize().unwrap()).unwrap();
        assert_eq!(status["app"], false);
        let root = tempfile::tempdir().unwrap();
        let package = watane_pkg::pack(&page(root.path(), "counter"), false).unwrap();
        let mut response = ureq::post(format!("http://127.0.0.1:{}/api/publish", ready["port"]))
            .send(package.as_bytes())
            .unwrap();
        let value: Value = response.body_mut().read_json().unwrap();
        assert_eq!(
            value["release"],
            format!("{}/counter@0.1.0", "y".repeat(52))
        );
        assert_eq!(value["link"], "https://fake.invalid/#counter");
        assert_eq!(
            value["site"],
            json!(home.path().canonicalize().unwrap().join("site"))
        );
        assert_eq!(
            fs::read(home.path().join("requests/1-publish")).unwrap(),
            package.as_bytes()
        );
    }
    #[test]
    fn ac7_text_output() {
        let root = tempfile::tempdir().unwrap();
        let home = tempfile::tempdir().unwrap();
        let (_host, _) = start(home.path(), "--home");
        let dir = page(root.path(), "counter");
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&dir)
            .args(["install", "--home"])
            .arg(home.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("install: ok"));
        assert!(text.contains("installed.0.slug: counter\n"));
        assert!(text.contains("Give the person this page:"));
    }
}

struct Host(std::process::Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
#[ignore = "slow: proves real home host without an app is refused before the pipeline"]
fn ac2_real_home_without_app() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::var_os("WATANE_BIN").expect("set WATANE_BIN"))
        .arg("--home")
        .arg(home.path())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let _host = Host(child);
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(home.path())
        .args(["install", "--json", "--home"])
        .arg(home.path())
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        value["errors"][0]["message"],
        format!(
            "the host of {} runs without the app",
            home.path().canonicalize().unwrap().display()
        )
    );
}

#[test]
fn ac2_no_host_next_preserves_explicit_wait_and_given_home() {
    let root = tempfile::tempdir().unwrap();
    for wait in ["17", "600"] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(root.path())
            .args([
                "install",
                "--json",
                "--home",
                "missing-home",
                "--wait",
                wait,
            ])
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["errors"][0]["code"], "env.host_not_running");
        assert_eq!(
            value["next"],
            format!(
                "watane-dev install --json --dir {0} --home {0}/missing-home --wait {wait}",
                root.path().canonicalize().unwrap().display()
            )
        );
    }
}

#[test]
fn ac2_no_host_next_quotes_home_for_the_shell() {
    let root = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(root.path())
        .args(["install", "--json", "--home", "Alice's home"])
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["errors"][0]["code"], "env.host_not_running",
        "{value}"
    );
    assert_eq!(
        value["next"],
        format!(
            r#"watane-dev install --json --dir {0} --home '{0}/Alice'"'"'s home'"#,
            root.path().canonicalize().unwrap().display()
        )
    );
}

#[test]
fn ac1_no_host_next_quotes_dir_for_the_shell() {
    let root = tempfile::tempdir().unwrap();
    let mod_dir = root.path().join("Alice's mod");
    std::fs::create_dir(&mod_dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(root.path())
        .args(["install", "--json", "--home", "missing-home", "--dir"])
        .arg(&mod_dir)
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["errors"][0]["code"], "env.host_not_running",
        "{value}"
    );
    assert_eq!(
        value["next"],
        format!(
            r#"watane-dev install --json --dir '{0}/Alice'"'"'s mod' --home {0}/missing-home"#,
            root.path().canonicalize().unwrap().display()
        )
    );
}

#[test]
fn ac1_install_next_matches_explicit_and_default_missing_symlink_home() {
    let root = tempfile::tempdir().unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(root.path(), &alias).unwrap();
    for exists in [false, true] {
        let home = alias.join("home");
        if exists {
            std::fs::create_dir(&home).unwrap();
        }
        for explicit in [true, false] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
            command
                .args(["install", "--json", "--dir"])
                .arg(&alias)
                .env("WATANE_HOME", &home);
            if explicit {
                command.arg("--home").arg(&home);
            }
            let output = command.output().unwrap();
            let value: Value = serde_json::from_slice(&output.stdout).unwrap();
            let resolved = root.path().canonicalize().unwrap();
            assert_eq!(output.status.code(), Some(3), "{value}");
            assert_eq!(
                value["next"],
                format!(
                    "watane-dev install --json --dir {}{}",
                    resolved.display(),
                    if explicit {
                        format!(" --home {}", resolved.join("home").display())
                    } else {
                        String::new()
                    }
                )
            );
            assert_eq!(value["dir"], serde_json::json!(resolved));
        }
    }
}

#[test]
fn ac2_missing_home_component_before_parent_is_environment_error() {
    let root = tempfile::tempdir().unwrap();
    for command in ["install", "publish", "search"] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(root.path())
            .args([command, "--json", "--home", "x/../h"])
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(output.status.code(), Some(3), "{value}");
        assert_eq!(value["errors"][0]["code"], "env.host_not_running");
        assert_eq!(
            value["errors"][0]["message"],
            "no watane app runs for the home x/../h (x does not exist)"
        );
        // s86: publish keeps its older fix text.
        let again = match command {
            "install" => {
                "watane-dev install --json --dir ".to_string()
                    + root.path().canonicalize().unwrap().to_str().unwrap()
                    + " --home <folder>"
            }
            "search" => "watane-dev search --json --home <folder>".to_string(),
            _ => "this command again".to_string(),
        };
        let then = if command == "publish" { " " } else { ": " };
        assert_eq!(
            value["errors"][0]["fix"],
            format!(
                "Pass --home the folder that the watane app uses (x does not exist), then run{then}{again}"
            )
        );
    }
}
