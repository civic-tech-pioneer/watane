#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;
fn run(dir: &Path, args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(dir)
        .env("WATANE_DEV_CACHE", dir.join("cache"))
        .args(args)
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        serde_json::from_slice(&output.stdout).unwrap(),
    )
}
#[test]
fn ac1_new_counter_template_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let (code, value) = run(temp.path(), &["--json", "new", "hello"]);
    assert_eq!(code, 0);
    let mut keys: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "command", "dir", "errors", "next", "ok", "result", "schema", "warnings"
        ]
    );
    assert_eq!(value["schema"], 1);
    assert_eq!(value["command"], "new");
    assert_eq!(value["ok"], true);
    assert_eq!(value["errors"], json!([]));
    assert_eq!(value["warnings"], json!([]));
    assert_eq!(
        value["dir"],
        temp.path()
            .canonicalize()
            .unwrap()
            .join("hello")
            .to_str()
            .unwrap()
    );
    assert_eq!(value["result"]["slug"], "hello");
    let files = [
        ".gitignore",
        "AGENTS.md",
        "CLAUDE.md",
        "logic/Cargo.toml",
        "logic/src/lib.rs",
        "mod.toml",
        "tests/counter.json",
        "ui/app.js",
        "ui/index.html",
        "wit/deps/watane-host/host.wit",
        "wit/world.wit",
    ];
    assert_eq!(value["result"]["files"], json!(files));
    fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.push(path.strip_prefix(root).unwrap().to_str().unwrap().into());
            }
        }
    }
    let root = temp.path().join("hello");
    let mut actual = Vec::new();
    walk(&root, &root, &mut actual);
    actual.sort();
    assert_eq!(actual, files);
    for file in &actual {
        assert!(!fs::read_to_string(root.join(file)).unwrap().contains("{{"));
    }
    assert_eq!(
        fs::read_to_string(root.join(".gitignore")).unwrap(),
        "logic.wasm\nlogic/target/\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("wit/deps/watane-host/host.wit")).unwrap(),
        watane_host::HOST_WIT
    );
    assert!(
        fs::read_to_string(root.join("wit/world.wit"))
            .unwrap()
            .starts_with("package mods:hello@0.1.0;")
    );
    assert_eq!(
        value["next"],
        "Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev serve --json"
    );
}
#[test]
fn ac2_existing_and_invalid_slug_change_nothing() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("hello")).unwrap();
    fs::write(temp.path().join("hello/keep"), "unchanged").unwrap();
    let (code, v) = run(temp.path(), &["new", "hello", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "usage.exists");
    assert_eq!(
        fs::read_to_string(temp.path().join("hello/keep")).unwrap(),
        "unchanged"
    );
    assert_eq!(fs::read_dir(temp.path().join("hello")).unwrap().count(), 1);
    // Use an empty directory: macOS can resolve Hello to the existing hello.
    let empty = tempfile::tempdir().unwrap();
    let (code, v) = run(empty.path(), &["new", "Hello", "--json"]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "usage.slug");
    assert!(!empty.path().join("Hello").exists());
    assert_eq!(fs::read_dir(empty.path()).unwrap().count(), 0);
}
#[test]
fn ac8_json_argument_errors_use_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let (code, v) = run(temp.path(), &["--json", "unknown"]);
    assert_eq!(code, 2);
    assert_eq!(v["schema"], 1);
    assert_eq!(v["errors"][0]["code"], "usage.args");
}

#[test]
fn ac1_s56_usage_messages_drop_clap_prefix_and_name_missing_command() {
    let temp = tempfile::tempdir().unwrap();
    let (code, v) = run(temp.path(), &["--json", "--bogus", "check"]);
    assert_eq!(code, 2);
    assert_eq!(v["errors"][0]["code"], "usage.args");
    assert_eq!(
        v["errors"][0]["message"],
        "unexpected argument '--bogus' found"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(temp.path())
        .args(["--bogus", "check"])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        text.starts_with("error[usage.args]: unexpected argument"),
        "{text}"
    );
    assert_eq!(text.matches("error:").count(), 0, "{text}");
    let missing = "no command given: watane-dev needs one of new, check, test, serve, doctor, add, install, publish, search";
    for args in [&[][..], &["--json"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(temp.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("Usage:"), "{text}");
        if args.is_empty() {
            assert!(
                text.starts_with(&format!("error[usage.args]: {missing}\n")),
                "{text}"
            );
        } else {
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["errors"][0]["message"], missing);
        }
    }
}

#[test]
fn ac2_s56_fix1_missing_provider_message_is_exact() {
    let temp = tempfile::tempdir().unwrap();
    let (_, v) = run(temp.path(), &["--json", "add"]);
    assert_eq!(
        v["errors"][0]["message"],
        "the following required arguments were not provided: <PROVIDER>"
    );
}

#[test]
fn regress_s56_missing_argument_message_names_the_argument() {
    // An agent that forgets the slug of `new` or the provider of `add` must read which
    // argument is missing; clap puts that on the line after "error: ...:".
    let temp = tempfile::tempdir().unwrap();
    for (args, wanted) in [
        (&["--json", "new"][..], "<SLUG>"),
        (&["--json", "add"][..], "<PROVIDER>"),
    ] {
        let (code, v) = run(temp.path(), args);
        assert_eq!(code, 2);
        assert_eq!(v["errors"][0]["code"], "usage.args");
        let message = v["errors"][0]["message"].as_str().unwrap();
        assert!(
            !message.ends_with(':'),
            "{args:?}: message ends in a colon with nothing after it: {message}"
        );
        assert!(
            message.contains(wanted),
            "{args:?}: message does not name the missing {wanted}: {message}"
        );
    }
}

#[test]
fn ac1_ac3_public_reports_are_typed() {
    use watane_dev::check::{CheckReport, ProviderReport};
    use watane_dev::template::{NewReport, new_mod};
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("hello");
    let created: NewReport = new_mod(&dir, "hello").unwrap();
    assert_eq!(created.slug, "hello");
    assert_eq!(
        serde_json::to_value(created).unwrap()["files"],
        json!([
            ".gitignore",
            "AGENTS.md",
            "CLAUDE.md",
            "logic/Cargo.toml",
            "logic/src/lib.rs",
            "mod.toml",
            "tests/counter.json",
            "ui/app.js",
            "ui/index.html",
            "wit/deps/watane-host/host.wit",
            "wit/world.wit"
        ])
    );
    let (_, report): (_, CheckReport) = watane_dev::check::check(&dir).unwrap();
    assert_eq!(report.slug, "hello");
    assert!(report.providers.is_empty());
    let provider = ProviderReport {
        slug: "books".into(),
        publisher: None,
        path: Some("../books".into()),
    };
    assert_eq!(
        serde_json::to_value(provider).unwrap(),
        json!({"slug":"books","publisher":null,"path":"../books"})
    );
}

#[test]
fn ac1_new_page_subscribes_to_counted_and_changed() {
    let temp = tempfile::tempdir().unwrap();
    let (code, value) = run(temp.path(), &["new", "hello", "--json"]);
    assert_eq!(code, 0, "{value}");
    let expected = "const count = document.getElementById('count');
const alert = document.getElementById('error');
const show = (value) => { count.textContent = String(value); alert.textContent = ''; };
const fail = (error) => { alert.textContent = error.message; };
const read = () => watane.call('api.get').then(show, fail);
document.getElementById('add').addEventListener('click', () => watane.call('api.add').then(show, fail));
watane.on((message) => {
  if (message.type === 'event' && message.function === 'counted') show(message.args.count);
  else if (message.type === 'changed') read();
});
";
    assert_eq!(
        fs::read_to_string(temp.path().join("hello/ui/app.js")).unwrap(),
        expected
    );
    let spec =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    assert!(spec.contains(&format!("`template/ui/app.js`:\n```js\n{expected}```")));
}

#[test]
fn ac4_ignored_tests_explain_their_cost_or_refusal() {
    for folder in ["tests", "src"] {
        for entry in fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(folder)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                let text = fs::read_to_string(&path).unwrap();
                for line in text
                    .lines()
                    .filter(|line| line.trim().starts_with("#[ignore"))
                {
                    assert!(
                        ["slow:", "timing:", "red:", "sandbox_apply", "unshare"]
                            .iter()
                            .any(|reason| line.contains(reason)),
                        "{}: {line}",
                        path.display()
                    );
                }
            }
        }
    }
}

#[test]
fn ac5_scenario_map_covers_every_scenario_and_names_existing_tests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = fs::read_to_string(root.join("TECHNICAL.md")).unwrap();
    let map = spec
        .split("### 15.2")
        .nth(1)
        .unwrap()
        .split("Removed command assertions")
        .next()
        .unwrap();
    for scenario in (1..=51)
        .map(|n| format!("DEV-S-{n}"))
        .chain(["J1".into(), "S11".into()])
    {
        assert!(
            map.contains(&format!("| {scenario} |")),
            "missing {scenario}"
        );
    }
    let sources: String = ["src", "tests"]
        .into_iter()
        .flat_map(|folder| fs::read_dir(root.join(folder)).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .map(|path| fs::read_to_string(path).unwrap())
        .collect();
    assert_eq!(scenario_proofs(map, &sources), Ok(()));
}

fn scenario_proofs(map: &str, sources: &str) -> Result<(), String> {
    for reference in map
        .lines()
        .filter(|line| {
            !line.starts_with('|')
                || !line
                    .split('|')
                    .nth(3)
                    .is_some_and(|notes| notes.contains("planned: "))
        })
        .flat_map(|line| {
            line.split('`')
                .skip(1)
                .step_by(2)
                .filter(|name| name.contains("::ac"))
        })
    {
        let name = reference.rsplit("::").next().unwrap();
        if !sources.contains(&format!("fn {name}(")) {
            return Err(format!("unknown proof {reference}"));
        }
    }
    Ok(())
}

#[test]
fn ac3_scenario_map_checks_unplanned_proofs_and_skips_planned_rows() {
    let unknown = "| DEV-S-41 | `share::ac1_missing` | integration |";
    assert_eq!(
        scenario_proofs(unknown, "fn ac1_existing() {}"),
        Err("unknown proof share::ac1_missing".into())
    );
    assert_eq!(
        scenario_proofs(
            &format!("{unknown}\n| DEV-S-42 | `share::ac2_future` | planned: S42 |"),
            ""
        ),
        Err("unknown proof share::ac1_missing".into())
    );
    assert_eq!(
        scenario_proofs(
            "| DEV-S-41 | `share::ac1_missing` | integration; planned: S42 |",
            ""
        ),
        Ok(())
    );
    assert_eq!(
        scenario_proofs(
            "| DEV-S-1 | `envelope::ac1_existing` | integration |",
            "fn ac1_existing() {}"
        ),
        Ok(())
    );
}

#[test]
fn ac9_s17_builder_commands_document_bootstrap_and_slow_set() {
    let guide =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("AGENTS.md")).unwrap();
    assert!(guide.contains("scripts/vendor.sh [<out>]"));
    assert!(!guide.contains("--skip regress_s16_missing_vendor_says_missing"));
}

#[test]
fn ac5_argument_errors_name_every_command_in_text_and_json() {
    for (args, expected) in [
        (vec!["new", "--bogus"], "new"),
        (vec!["new"], "new"),
        (vec!["check", "--bogus"], "check"),
        (vec!["serve", "--bogus"], "serve"),
        (vec!["doctor", "--bogus"], "doctor"),
        (vec!["test", "--bogus"], "test"),
        (vec!["add", "--bogus"], "add"),
        (vec!["install", "--bogus"], "install"),
        (vec!["publish", "--wait", "x"], "publish"),
        (vec!["help", "--bogus"], "help"),
        (vec!["frobnicate"], "watane-dev"),
        (vec![], "watane-dev"),
    ] {
        for json in [false, true] {
            let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
                .args(&args)
                .args(if json { vec!["--json"] } else { vec![] })
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2), "{args:?}");
            if json {
                let value: Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(value["command"], expected, "{args:?}");
                assert_eq!(value["errors"][0]["code"], "usage.args");
            } else {
                let text = String::from_utf8(output.stdout).unwrap();
                assert!(text.starts_with("error[usage.args]: "), "{text}");
                assert!(text.contains("  fix: watane-dev --help\n"), "{text}");
                assert!(!text.lines().any(|line| line == ": failed"), "{text}");
                assert!(
                    text.ends_with(&format!("{expected}: failed\n")),
                    "{args:?}: {text}"
                );
            }
        }
    }
}

#[test]
fn ac8_s39_recovery_contract_is_documented() {
    let spec =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    for (section, phrases) in [
        (
            "### 2.2",
            vec![
                "host.failed",
                "could not open its data",
                "folder comes from the violation's file",
            ],
        ),
        (
            "### 9.6",
            vec![
                "release_providers.<slug>.link",
                "from its link <link>",
                "host.failed",
            ],
        ),
        (
            "### 10.1",
            vec![
                "[bad_answer]",
                "<id>.<pid>.tmp",
                "<id>.<other>.tmp",
                "fetch_refused",
                "fetch_failed",
            ],
        ),
        (
            "### 15.2",
            vec!["AC1", "AC2", "AC3", "AC4", "AC5", "AC6", "AC7", "AC8"],
        ),
    ] {
        let contents = spec
            .split(section)
            .nth(1)
            .unwrap()
            .split("\n### ")
            .next()
            .unwrap();
        for phrase in phrases {
            assert!(contents.contains(phrase), "{section} lacks {phrase}");
        }
    }
}

#[test]
fn ac6_s39_specs_cover_paths_derivation_fake_and_argument_errors() {
    let spec =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    for (section, marker) in [
        ("### 2.3", "error[usage.args]: <message>"),
        ("## 3.", "x/../h (x does not exist)"),
        ("### 9.6", "canonical existing ancestor"),
        ("### 9.7", "--dir <canonical mod> [--home <canonical home>]"),
        ("### 10.2", "Remove `logic.wasm`"),
        (
            "### 15.2",
            "ac1_default_home_and_publish_retry_paths_are_canonical",
        ),
        ("### 15.3", "missing or unreadable"),
    ] {
        let body = spec
            .split(section)
            .nth(1)
            .unwrap()
            .split("\n##")
            .next()
            .unwrap();
        assert!(body.contains(marker), "{section} missing {marker}");
    }
}

fn page(root: &Path) -> std::path::PathBuf {
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

/// S86 AC2: the rerun names `--dir` and `--home` only when the command was given them.
#[test]
fn ac2_test_next_carries_dir_and_home_only_when_given() {
    let temp = tempfile::tempdir().unwrap();
    let dir = page(temp.path());
    let cwd = dir.display().to_string();
    let (code, value) = run(&dir, &["test", "--json"]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["next"], "watane-dev serve --json");
    let (code, value) = run(temp.path(), &["test", "--json", "--dir", &cwd]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(
        value["next"],
        format!("watane-dev serve --json --dir {cwd}")
    );
    let (code, value) = run(
        temp.path(),
        &[
            "test",
            "--json",
            "--dir",
            &cwd,
            "--home",
            &format!("{}/.", temp.path().display()),
        ],
    );
    assert_eq!(code, 0, "{value}");
    assert_eq!(
        value["next"],
        format!(
            "watane-dev serve --json --dir {cwd} --home {}",
            temp.path().canonicalize().unwrap().display()
        )
    );
}

/// S86 AC5: a closed stdout ends the program quietly, never with a panic.
#[test]
fn ac5_help_into_a_closed_pipe_exits_quietly() {
    for _ in 0..20 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .arg("--help")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
    // A command's result into a closed pipe keeps the command's status (usage: 2).
    let temp = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .args(["check", "--json", "--dir"])
        .arg(temp.path().join("missing"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// S86 AC4 (goal 7): a local provider with a known folder names it.
#[test]
fn ac4_local_dependency_names_the_known_folder() {
    let temp = tempfile::tempdir().unwrap();
    let dir = page(temp.path());
    fs::write(dir.join("deps.json"), r#"{"mods:a":null}"#).unwrap();
    let provider = temp.path().join("a");
    fs::create_dir(&provider).unwrap();
    fs::write(
        dir.join(".watane-dev.json"),
        json!({"local_providers":{"a":"../a"},"release_providers":{}}).to_string(),
    )
    .unwrap();
    let (code, value) = run(&dir, &["publish", "--json", "--home", "/nowhere"]);
    assert_eq!(code, 1, "{value}");
    assert_eq!(
        value["errors"][0]["message"],
        format!(
            "mods:a is a local provider (deps.json names its folder {}, not a release), and a published mod can depend only on released providers",
            provider.canonicalize().unwrap().display()
        )
    );
}
