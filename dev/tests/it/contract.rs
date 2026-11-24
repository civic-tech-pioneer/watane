#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::in_process;

use crate::support;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct Fixture {
    _temp: tempfile::TempDir,
    dir: PathBuf,
    cache: PathBuf,
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
        Self {
            _temp: temp,
            dir,
            cache: support::cache().to_path_buf(),
        }
    }
    fn run(&self, name: &str) -> (i32, Value) {
        in_process::dev(
            &self.dir,
            &[name, "--json"],
            &[
                ("WATANE_DEV_CACHE", Some(self.cache.as_os_str())),
                ("WATANE_DEV_UNIT_SECONDS", None),
            ],
        )
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
#[ignore = "slow: full template pipeline verifies unit counts and compiled component export"]
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
#[ignore = "slow: full pipeline repairs changed logic after unit and contract failures"]
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
    assert_eq!(
        v["next"],
        "Fix the unit test tests::counts_up_and_stays_at_the_maximum, then run: watane-dev test --json"
    );
    fs::write(path, original).unwrap();
    assert_eq!(f.run("test").0, 0);
}

#[test]
#[ignore = "slow: full template pipeline proves contract failure wiring through the test command"]
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
fn ac6_bad_contract_file_names_file() {
    let f = prebuilt_recommendations();
    fs::write(f.dir.join("tests/counter.json"), "{").unwrap();
    let (_, errors) =
        watane_dev::contract::contract_tests(&f.dir, &"recommendations".parse().unwrap()).unwrap();
    assert_eq!(errors[0].code.as_str(), "test.contract_file");
    assert_eq!(errors[0].file.as_deref(), Some("tests/counter.json"));
}

#[cfg(unix)] // This tests the Unix preview pipeline.
#[test]
#[ignore = "slow: full serve pipeline gates preview creation on contracts"]
fn ac8_serve_does_not_start_preview_for_failed_or_absent_contracts() {
    let f = Fixture::new();
    let hash = blake3::hash(f.dir.canonicalize().unwrap().as_os_str().as_encoded_bytes()).to_hex();
    let preview = f.cache.join(format!("previews/counter-{}", &hash[..8]));
    f.steps(json!([{"call":"api.add","returns":2}]));
    let (code, v) = f.run("serve");
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract");
    assert!(!preview.exists());
    fs::remove_file(f.dir.join("tests/counter.json")).unwrap();
    let (code, v) = f.run("serve");
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "mod.no_contract_tests");
    assert!(!preview.exists());
}

fn prebuilt_recommendations() -> Fixture {
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let destination = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("recommendations");
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recommendations"),
        &dir,
    );
    Fixture {
        cache: temp.path().join("cache"),
        _temp: temp,
        dir,
    }
}

#[test]
fn ac1_s11_fake_answers_provider_call() {
    let f = prebuilt_recommendations();
    let path = f.dir.join("tests/books.json");
    fs::write(&path, json!({"fakes":{"mods:reading-list/books":{"all":["Dune"]}},"steps":[{"call":"api.all","returns":["Dune"]}]}).to_string()).unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 0);
    assert!(report.failures.is_empty());
}

#[test]
fn ac6_contract_recovery_names_file_step_and_call() {
    let f = prebuilt_recommendations();
    fs::write(
        f.dir.join("tests/books.json"),
        r#"{"steps":[{"call":"api.all","returns":["Dune"]}]}"#,
    )
    .unwrap();
    let (_, errors) =
        watane_dev::contract::contract_tests(&f.dir, &"recommendations".parse().unwrap()).unwrap();
    assert_eq!(
        errors[0].fix.as_deref(),
        Some("Fix tests/books.json step 1 (api.all), then run: watane-dev test --json")
    );
}

#[test]
fn ac1_s11_missing_fake_fails() {
    let f = prebuilt_recommendations();
    let path = f.dir.join("tests/books.json");
    fs::write(path, r#"{"steps":[{"call":"api.all","returns":["Dune"]}]}"#).unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code.as_str(), "test.contract");
    assert!(problems[0].message.contains("no fake"), "{problems:?}");
    assert!(
        report.failures[0]
            .got
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("no fake")
    );
}

#[test]
fn ac2_s11_deliver_retains_state_and_emits() {
    let f = prebuilt_recommendations();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(report.passed, 3);
    assert_eq!(report.failed, 0);
    assert!(report.failures.is_empty());
}

#[test]
fn ac2_s11_deliver_compares_emitted_events() {
    let f = prebuilt_recommendations();
    let path = f.dir.join("tests/books.json");
    fs::write(
        &path,
        r#"{"steps":[{"deliver":"events.added","args":{"title":"Emma"},"emits":[]}]}"#,
    )
    .unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code.as_str(), "test.contract");
    assert_eq!(
        report.failures[0].got,
        Some(json!([{"event":"mods:recommendations/changes.received","args":{"title":"Emma"}}]))
    );
}

#[test]
fn ac2_s11_deliver_rejects_non_subscription() {
    let f = prebuilt_recommendations();
    let path = f.dir.join("tests/books.json");
    fs::write(
        &path,
        json!({"steps":[{"deliver":"api.received"}]}).to_string(),
    )
    .unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 1);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].code.as_str(), "test.contract");
    assert!(problems[0].message.contains("no_function"), "{problems:?}");
}

#[test]
fn ac5_s47_test_accepts_prebuilt_wasm_without_manifest() {
    let f = prebuilt_recommendations();
    assert!(!f.dir.join("logic/Cargo.toml").exists());
    let expected = fs::read(f.dir.join("logic.wasm")).unwrap();
    let (code, value) = f.run("test");
    assert_eq!(code, 0, "{value}");
    assert!(value["result"]["contract"].is_object(), "{value}");
    assert_eq!(value["result"]["contract"]["passed"], 3, "{value}");
    assert_eq!(value["result"]["contract"]["failed"], 0, "{value}");
    assert!(value["result"]["build"].is_null(), "{value}");
    assert!(value["result"]["unit"].is_null(), "{value}");
    assert_eq!(fs::read(f.dir.join("logic.wasm")).unwrap(), expected);
    assert!(!f.cache.join("previews").exists());
}

#[test]
fn ac6_contract_problem_reports_failing_step_object_line() {
    let f = prebuilt_recommendations();
    fs::write(
        f.dir.join("tests/books.json"),
        "{\n  \"steps\": [\n    {\"call\":\"api.all\",\"returns\":[\"Dune\"]}\n  ]\n}\n",
    )
    .unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (_, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert_eq!(problems[0].code.as_str(), "test.contract");
    assert_eq!(problems[0].line, Some(3));
    assert_eq!(problems[0].file.as_deref(), Some("tests/books.json"));
}

#[test]
fn ac3_each_contract_file_starts_from_fresh_state() {
    let f = prebuilt_recommendations();
    // Runs before books.json; if its state leaked, books.json would receive ["Emma","Emma"].
    fs::write(
        f.dir.join("tests/a.json"),
        r#"{"steps":[{"deliver":"events.added","args":{"title":"Emma"},"emits":[{"event":"changes.received","args":{"title":"Emma"}}]}]}"#,
    )
    .unwrap();
    let m = watane_pkg::Mod::read(&f.dir).unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&f.dir, m.slug()).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!((report.files, report.passed, report.failed), (2, 4, 0));
}

fn copy_fixture(name: &str, to: &std::path::Path) {
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let destination = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &destination);
            } else {
                fs::copy(entry.path(), destination).unwrap();
            }
        }
    }
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
        to,
    );
}

fn test_in(dir: &std::path::Path, contract: Value) -> (i32, Value) {
    fs::create_dir_all(dir.join("tests")).unwrap();
    fs::write(dir.join("tests/c.json"), contract.to_string()).unwrap();
    in_process::dev(
        dir,
        &["test", "--json"],
        &[
            ("WATANE_DEV_CACHE", Some(support::cache().as_os_str())),
            ("WATANE_DEV_UNIT_SECONDS", None),
        ],
    )
}

#[test]
fn ac_collect_fake_and_answer() {
    let temp = tempfile::tempdir().unwrap();
    let (menu, item) = (temp.path().join("menu"), temp.path().join("item"));
    copy_fixture("menu", &menu);
    copy_fixture("item", &item);
    for (dir, contract) in [
        (&menu, json!({"steps":[{"call":"api.items","returns":[]}]})),
        (
            &menu,
            json!({"fakes":{"mods:menu/menu":{"ask":["x","y"]}},
                   "steps":[{"call":"api.items","returns":["x","y"]}]}),
        ),
        (
            &item,
            json!({"steps":[{"deliver":"menu.ask","returns":["a"]}]}),
        ),
    ] {
        let (code, v) = test_in(dir, contract);
        assert_eq!(code, 0, "{v}");
        assert_eq!(v["result"]["contract"]["passed"], 1, "{v}");
        assert_eq!(v["result"]["contract"]["failed"], 0, "{v}");
    }
    // The answer is checked, not only that the deliver ran.
    let (code, v) = test_in(
        &item,
        json!({"steps":[{"deliver":"menu.ask","returns":["b"]}]}),
    );
    assert_eq!(code, 1, "{v}");
    assert_eq!(
        v["result"]["contract"]["failures"][0]["got"],
        json!(["a"]),
        "{v}"
    );
}

#[test]
fn ac2_own_outlet_fake_key_is_accepted_and_at_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    let menu = temp.path().join("menu");
    copy_fixture("menu", &menu);
    fs::create_dir_all(menu.join("tests")).unwrap();
    let path = menu.join("tests/c.json");
    let slug: watane_pkg::Slug = "menu".parse().unwrap();
    fs::write(
        &path,
        r#"{"fakes":{"mods:menu/menu":{"ask":["x"]}},"steps":[{"call":"api.items","returns":["x"]}]}"#,
    )
    .unwrap();
    let (report, problems) = watane_dev::contract::contract_tests(&menu, &slug).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(report.passed, 1);
    fs::write(
        &path,
        r#"{"fakes":{"mods:menu/menu@0.1.0":{}},"steps":[{"call":"api.items"}]}"#,
    )
    .unwrap();
    let (_, problems) = watane_dev::contract::contract_tests(&menu, &slug).unwrap();
    assert_eq!(problems[0].code.as_str(), "test.contract_file");
}

#[test]
fn ac3_fake_of_a_function_the_interface_lacks_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    let menu = temp.path().join("menu");
    copy_fixture("menu", &menu);
    let (code, v) = test_in(
        &menu,
        json!({"fakes":{"mods:menu/menu":{"asks":["x"]}},
               "steps":[{"call":"api.items","returns":[]}]}),
    );
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract_file", "{v}");
    assert_eq!(v["errors"][0]["file"], "tests/c.json", "{v}");
    assert_eq!(
        v["errors"][0]["message"],
        "tests/c.json: fakes mods:menu/menu has no function asks; its functions: ask, clicked",
        "{v}"
    );
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

/// Host's prebuilt `manager` fixture: logic.wasm, mod.toml and wit.
fn copy_manager(to: &std::path::Path) {
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../host/tests/fixtures/mods/manager"),
        to,
    );
}

#[test]
fn ac_manage_fake_and_records() {
    let temp = tempfile::tempdir().unwrap();
    let manager = temp.path().join("manager");
    copy_manager(&manager);
    let w = json!({
        "local": "weather", "identity": "weather-id", "slug": "weather", "name": "Weather",
        "summary": "Shows the weather.", "version": "0.1.0", "publisher": "alice",
        "release": "weather@0.1.0", "origin": "https://example.org/weather",
        "enabled": true, "has-page": true, "has-logic": true,
        "grants": [{"grant": "storage", "granted": true}],
        "links": [], "binds": [], "error": null
    });
    let disable = json!({"call":"api.disable","args":{"local":"weather"},
        "returns":{"ok":null},
        "manages":[{"fn":"disable","args":{"local":"weather"}}]});
    let first = json!({
        "fakes": {"watane:host/manage": {"mods": [w]}},
        "steps": [
            {"call":"api.mods","returns":[w]},
            disable,
            {"call":"api.request-remove","args":{"local":"notes","delete-data":true},
             "returns":{"ok":"r1"},
             "manages":[{"fn":"request-remove","args":{"local":"notes","delete-data":true}}]}
        ]
    });
    let (code, v) = test_in(&manager, first);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["contract"]["passed"], 3, "{v}");
    let mut wrong = disable.clone();
    wrong["manages"] = json!([]);
    let (code, v) = test_in(&manager, json!({"steps":[wrong]}));
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract", "{v}");
    let failure = &v["result"]["contract"]["failures"][0];
    assert_eq!(failure["step"], 1, "{v}");
    assert_eq!(failure["call"], "api.disable", "{v}");
    assert_eq!(failure["expected"], json!([]), "{v}");
    assert_eq!(
        failure["got"],
        json!([{"fn":"disable","args":{"local":"weather"}}]),
        "{v}"
    );
}

#[test]
fn ac3_fake_of_a_function_manage_lacks_is_refused() {
    let temp = tempfile::tempdir().unwrap();
    let manager = temp.path().join("manager");
    copy_manager(&manager);
    let (code, v) = test_in(
        &manager,
        json!({"fakes":{"watane:host/manage":{"modz":[]}},"steps":[{"call":"api.mods","returns":[]}]}),
    );
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract_file", "{v}");
    let message = v["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("fakes watane:host/manage has no function modz; its functions: ")
            && message.contains("request-remove"),
        "{message}"
    );
}

#[test]
fn ac1_open_page_manage_record_passes_and_missing_record_fails() {
    let temp = tempfile::tempdir().unwrap();
    let manager = temp.path().join("manager");
    copy_manager(&manager);
    let mut step = json!({"call":"api.open-page","args":{"local":"weather"},
        "returns":{"ok":null},
        "manages":[{"fn":"open-page","args":{"local":"weather"}}]});
    let (code, v) = test_in(&manager, json!({"steps":[step]}));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["result"]["contract"]["passed"], 1, "{v}");
    step["manages"] = json!([]);
    let (code, v) = test_in(&manager, json!({"steps":[step]}));
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.contract", "{v}");
    let failure = &v["result"]["contract"]["failures"][0];
    assert_eq!(failure["step"], 1, "{v}");
    assert_eq!(failure["call"], "api.open-page", "{v}");
    assert_eq!(failure["expected"], json!([]), "{v}");
    assert_eq!(
        failure["got"],
        json!([{"fn":"open-page","args":{"local":"weather"}}]),
        "{v}"
    );
}

#[test]
fn ac2_fixture_host_wit_matches_host() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    for entry in fs::read_dir(fixtures).unwrap() {
        let path = entry.unwrap().path().join("wit/deps/watane-host/host.wit");
        if path.exists() {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                watane_host::HOST_WIT,
                "{}",
                path.display()
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}
