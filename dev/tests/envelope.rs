use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
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
fn ac1_ac3_public_reports_are_typed() {
    use watane_dev::{
        check::{CheckReport, ProviderReport},
        template::{NewReport, new_mod},
    };
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
    for scenario in (1..=32)
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
    for reference in map
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|name| name.contains("::ac"))
    {
        let name = reference.rsplit("::").next().unwrap();
        assert!(
            sources.contains(&format!("fn {name}(")),
            "unknown proof {reference}"
        );
    }
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
                assert!(
                    text.ends_with(&format!("{expected}: failed\n")),
                    "{args:?}: {text}"
                );
            }
        }
    }
}
