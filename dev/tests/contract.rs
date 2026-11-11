use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

struct Fixture {
    temp: tempfile::TempDir,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("counter");
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .args(["new", "counter", "--json", "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(output.status.success());
        Self { temp, dir }
    }
    fn run(&self, name: &str) -> (i32, Value) {
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&self.dir)
            .args([name, "--json"])
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env_remove("WATANE_DEV_UNIT_SECONDS")
            .output()
            .unwrap();
        let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "{e}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output.status.code().unwrap(), value)
    }
    fn steps(&self, steps: Value) {
        fs::write(
            self.dir.join("tests/counter.json"),
            json!({"steps":steps}).to_string(),
        )
        .unwrap();
    }
}

#[test]
fn ac1_template_counts_and_component_export() {
    let f = Fixture::new();
    let steps: usize = fs::read_dir(f.dir.join("tests"))
        .unwrap()
        .map(|entry| {
            let value: Value =
                serde_json::from_slice(&fs::read(entry.unwrap().path()).unwrap()).unwrap();
            value["steps"].as_array().unwrap().len()
        })
        .sum();
    let counted = Command::new("grep")
        .args(["-Ec", r"#\[test\]"])
        .arg(f.dir.join("logic/src/lib.rs"))
        .output()
        .unwrap();
    assert!(counted.status.success());
    let units: u64 = String::from_utf8(counted.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let (code, v) = f.run("test");
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["contract"]["passed"], steps);
    assert_eq!(v["result"]["contract"]["files"], 1);
    assert_eq!(v["result"]["contract"]["failed"], 0);
    assert_eq!(v["result"]["unit"]["passed"], units);
    let engine = wasmtime::Engine::default();
    let component =
        wasmtime::component::Component::from_file(&engine, f.dir.join("logic.wasm")).unwrap();
    assert!(
        component
            .component_type()
            .exports(&engine)
            .any(|(name, _)| name == "mods:counter/api@0.1.0")
    );
}

#[test]
fn ac2_broken_add_reports_step_and_can_be_fixed() {
    let f = Fixture::new();
    let path = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&path).unwrap();
    assert_eq!(original.matches("count.saturating_add(1)").count(), 1);
    fs::write(
        &path,
        original.replace("count.saturating_add(1)", "count.saturating_add(2)"),
    )
    .unwrap();
    let (code, v) = f.run("test");
    assert_eq!(code, 1, "{v}");
    let error = v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["code"] == "test.contract")
        .expect("contract error");
    assert_eq!(
        error["message"],
        "tests/counter.json step 2 (api.add): expected 1, got 2"
    );
    assert_eq!(error["file"], "tests/counter.json");
    let failure = &v["result"]["contract"]["failures"][0];
    for (key, expected) in [
        ("file", json!("tests/counter.json")),
        ("step", json!(2)),
        ("call", json!("api.add")),
        ("expected", json!(1)),
        ("got", json!(2)),
    ] {
        assert_eq!(failure[key], expected, "{v}");
    }
    assert_eq!(v["next"], "watane-dev test --json");
    fs::write(path, original).unwrap();
    assert_eq!(f.run("test").0, 0);
}

#[test]
fn ac3_literal_fresh_counter_failure_and_fresh_state_per_file() {
    let f = Fixture::new();
    f.steps(json!([{ "call": "api.add", "returns": 2 }]));
    // Runs before counter.json, and must not change its initial state.
    fs::write(
        f.dir.join("tests/a.json"),
        r#"{"steps":[{"call":"api.add","returns":1}]}"#,
    )
    .unwrap();
    let (code, v) = f.run("test");
    assert_eq!(code, 1, "{v}");
    assert_eq!(
        v["errors"][0]["message"],
        "tests/counter.json step 1 (api.add): expected 2, got 1"
    );
    assert_eq!(v["result"]["contract"]["files"], 2);
    assert_eq!(v["result"]["contract"]["passed"], 1);
    assert_eq!(v["result"]["contract"]["failed"], 1);
}

#[test]
fn ac4_events_match_short_and_full_names_and_exact_lists() {
    let f = Fixture::new();
    for (events, passes) in [
        (
            json!([{"event":"changes.counted","args":{"count":1}}]),
            true,
        ),
        (
            json!([{"event":"mods:counter/changes.counted","args":{"count":1.0}}]),
            true,
        ),
        (
            json!([{"event":"changes.counted","args":{"count":2}}]),
            false,
        ),
        (json!([]), false),
        (
            json!([{"event":"changes.counted","args":{"count":1}},{"event":"changes.counted","args":{"count":1}}]),
            false,
        ),
    ] {
        f.steps(json!([{"call":"api.add","emits":events}]));
        let (code, v) = f.run("test");
        assert_eq!(code, i32::from(!passes), "{v}");
        if !passes {
            assert_eq!(v["errors"][0]["code"], "test.contract");
            assert_eq!(v["result"]["contract"]["failures"][0]["step"], 1);
            assert_eq!(v["result"]["contract"]["failures"][0]["expected"], events);
            assert_eq!(
                v["result"]["contract"]["failures"][0]["got"],
                json!([{"event":"mods:counter/changes.counted","args":{"count":1}}])
            );
        }
    }
}

#[test]
fn ac5_expected_call_failures() {
    let f = Fixture::new();
    for (function, fails, passes) in [
        ("api.missing", json!(true), true),
        ("api.missing", json!("no_function:"), true),
        ("api.missing", json!("different error"), false),
        ("api.get", json!(true), false),
    ] {
        f.steps(json!([{"call":function,"fails":fails}]));
        let (code, v) = f.run("test");
        assert_eq!(code, i32::from(!passes), "{v}");
        if !passes {
            assert_eq!(v["errors"][0]["code"], "test.contract", "{v}");
        }
    }
}

#[test]
fn ac6_bad_contract_file_names_file() {
    let f = Fixture::new();
    fs::write(f.dir.join("tests/counter.json"), "{").unwrap();
    let (code, v) = f.run("test");
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract_file");
    assert_eq!(v["errors"][0]["file"], "tests/counter.json");
}

#[test]
fn ac8_serve_does_not_start_preview_for_failed_or_absent_contracts() {
    let f = Fixture::new();
    f.steps(json!([{"call":"api.add","returns":2}]));
    let (code, v) = f.run("serve");
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert!(!f.temp.path().join("cache/previews").exists());
    fs::remove_file(f.dir.join("tests/counter.json")).unwrap();
    let (code, v) = f.run("serve");
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "mod.no_contract_tests");
    assert!(!f.temp.path().join("cache/previews").exists());
}
