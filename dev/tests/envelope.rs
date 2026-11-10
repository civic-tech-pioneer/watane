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
fn ac1_new_page_template_envelope() {
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
        "mod.toml",
        "ui/app.js",
        "ui/index.html",
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
    assert!(
        fs::read_to_string(root.join("wit/world.wit"))
            .unwrap()
            .starts_with("package mods:hello@0.1.0;")
    );
    assert_eq!(value["next"], "Edit ui/, then run: watane-dev serve --json");
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
fn ac8_public_outcome_types_match_section_12() {
    use watane_dev::outcome::{Class, Code, Outcome, Problem};
    for (code, class, status, text) in [
        (Code::ModInvalid, Class::Mod, 1, "mod.invalid"),
        (Code::UsageArgs, Class::Usage, 2, "usage.args"),
        (Code::EnvWataneMissing, Class::Env, 3, "env.watane_missing"),
        (Code::HostRefused, Class::Refused, 4, "host.refused"),
        (Code::Internal, Class::Internal, 5, "internal"),
    ] {
        assert_eq!(code.class(), class);
        assert_eq!(code, code);
        assert_eq!(serde_json::to_value(code).unwrap(), text);
        let problem = Problem::new(code, "message").at("ui/app.js", Some(12), Some(5));
        let cloned = problem.clone();
        let outcome = Outcome::new("check", None).fail(vec![cloned]);
        let command: &'static str = outcome.command;
        assert_eq!(command, "check");
        assert_eq!(outcome.exit_code(), status);
        assert_eq!(
            serde_json::to_value(problem).unwrap(),
            json!({"code":text,"message":"message","file":"ui/app.js","line":12,"column":5,"fix":null})
        );
    }
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
            "mod.toml",
            "ui/app.js",
            "ui/index.html",
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
