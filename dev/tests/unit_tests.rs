use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

struct Fixture {
    _guard: MutexGuard<'static, ()>,
    temp: tempfile::TempDir,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        // Keep the wall-clock deadline test independent of concurrent cold builds.
        static LOCK: Mutex<()> = Mutex::new(());
        let guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("counter");
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .args(["new", "counter", "--json", "--dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        Self {
            _guard: guard,
            temp,
            dir,
        }
    }
    fn command(&self, name: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        command
            .current_dir(&self.dir)
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env_remove("WATANE_DEV_UNIT_SECONDS")
            .args([name, "--json"]);
        command
    }
    fn source(&self, extra: &str) -> String {
        let path = self.dir.join("logic/src/lib.rs");
        let text = format!("{}\n{extra}\n", fs::read_to_string(&path).unwrap());
        fs::write(path, &text).unwrap();
        text
    }
}
fn output(command: &mut Command) -> (i32, Value) {
    let output = command.output().unwrap();
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code().unwrap(), value)
}
fn line(text: &str, needle: &str) -> usize {
    text.lines().position(|line| line.contains(needle)).unwrap() + 1
}

#[test]
fn ac1_template_unit_counts_and_pipeline() {
    let f = Fixture::new();
    // Independent regex oracle; grep is supplied by both supported operating systems.
    let counted = Command::new("grep")
        .args(["-Ec", r"#\[test\]"])
        .arg(f.dir.join("logic/src/lib.rs"))
        .output()
        .unwrap();
    assert!(counted.status.success());
    let count: u64 = String::from_utf8(counted.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 0, "{v}");
    assert_eq!(v["command"], "test");
    assert_eq!(v["ok"], true);
    assert_eq!(v["result"]["unit"]["passed"], count);
    for field in ["failed", "ignored", "not_run"] {
        assert_eq!(v["result"]["unit"][field], 0, "{v}");
    }
    assert!(v["result"]["check"].is_object());
    assert!(v["result"]["build"].is_object());
    // Independent oracle: count the steps of the template's contract files with serde_json.
    let steps: usize = fs::read_dir(f.dir.join("tests"))
        .unwrap()
        .map(|entry| {
            let file: Value =
                serde_json::from_slice(&fs::read(entry.unwrap().path()).unwrap()).unwrap();
            file["steps"].as_array().unwrap().len()
        })
        .sum();
    assert!(steps > 0);
    assert_eq!(v["result"]["contract"]["files"], 1, "{v}");
    assert_eq!(v["result"]["contract"]["passed"], steps, "{v}");
    assert_eq!(v["result"]["contract"]["failed"], 0, "{v}");
    assert_eq!(v["next"], "watane-dev serve --json");
}

#[test]
fn ac2_panic_names_assert_and_counts_remaining_tests() {
    let f = Fixture::new();
    let text = f.source(
        "#[test]\nfn fails_on_purpose() { assert_eq!(1, 2); }\n#[test]\nfn z_after_failure() {}",
    );
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    let error = &v["errors"][0];
    assert_eq!(error["code"], "test.unit", "{v}");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("fails_on_purpose")
    );
    assert_eq!(error["file"], "logic/src/lib.rs");
    assert_eq!(error["line"], line(&text, "assert_eq!(1, 2)"));
    assert_eq!(v["result"]["unit"]["failed"], 1);
    assert_eq!(v["result"]["unit"]["passed"], 0);
    assert_eq!(v["result"]["unit"]["not_run"], 3);
    assert_eq!(v["next"], "watane-dev test --json");
}

#[test]
#[ignore = "timing: run on a quiescent machine, see factory/README.md"]
fn ac10_endless_test_stops_with_exact_message_within_deadline() {
    let f = Fixture::new();
    // Warm the build cache before timing the endless test, so this measures execution.
    assert_eq!(output(&mut f.command("test")).0, 0);
    f.source("#[test]\nfn endless() { loop {} }");
    let started = Instant::now();
    let (status, v) = output(f.command("test").env("WATANE_DEV_UNIT_SECONDS", "2"));
    assert!(
        started.elapsed() < Duration::from_secs(7),
        "elapsed: {:?}; {v}",
        started.elapsed()
    );
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.unit_limit", "{v}");
    assert_eq!(
        v["errors"][0]["message"],
        "unit tests stopped at endless after 2 s"
    );
}

#[test]
fn ac4_unit_tests_have_no_files_environment_or_network() {
    let f = Fixture::new();
    let escape = f.temp.path().join("escape");
    f.source(&format!(
        r#"
#[test]
fn sandbox() {{
    assert!(std::fs::read_dir("/").is_err());
    assert!(std::env::var("HOME").is_err());
    assert!(std::fs::write({:?}, b"escape").is_err());
    assert!(std::net::TcpStream::connect("127.0.0.1:9").is_err());
}}
"#,
        escape.to_str().unwrap()
    ));
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 0, "{v}");
    assert_eq!(v["result"]["unit"]["passed"], 3);
    assert!(!escape.exists());
}

#[test]
fn ac3_memory_growth_above_limit_is_a_mod_failure() {
    let f = Fixture::new();
    f.source("#[test]\nfn memory_limit() { std::hint::black_box(vec![1u8; 600 << 20]); }");
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.unit_limit", "{v}");
    assert!(
        v["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("memory_limit")
    );
    assert_eq!(v["result"]["unit"]["failed"], 1);
}

#[test]
fn ac4_unit_host_imports_trap() {
    let f = Fixture::new();
    f.source("#[test]\nfn host_imports_trap() { let _ = state::get(\"count\"); }");
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.unit", "{v}");
    let message = v["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("host_imports_trap"), "{v}");
    assert!(message.contains("unknown import"), "{v}");
}

#[test]
fn ac5_logic_requires_contract_file_but_page_only_does_not() {
    let f = Fixture::new();
    let _ = fs::remove_file(f.dir.join("tests/counter.json"));
    for command in ["check", "test"] {
        let (status, v) = output(&mut f.command(command));
        assert_eq!(status, 1, "{v}");
        assert_eq!(v["errors"][0]["code"], "mod.no_contract_tests", "{v}");
        assert_eq!(
            v["errors"][0]["fix"],
            "Add tests/<name>.json (see AGENTS.md)"
        );
    }
    fs::remove_dir_all(f.dir.join("logic")).unwrap();
    fs::write(
        f.dir.join("wit/world.wit"),
        "package mods:counter@0.1.0;\nworld counter {}\n",
    )
    .unwrap();
    for command in ["check", "test"] {
        let (status, v) = output(&mut f.command(command));
        assert_eq!(status, 0, "{v}");
        if command == "test" {
            for part in ["build", "unit", "contract"] {
                assert!(v["result"][part].is_null(), "{v}");
            }
        }
    }
}

#[test]
fn ac6_template_contract_and_agent_guide() {
    let f = Fixture::new();
    let expected = "{\n  \"steps\": [\n    {\"call\": \"api.get\", \"returns\": 0},\n    {\"call\": \"api.add\", \"returns\": 1,\n     \"emits\": [{\"event\": \"changes.counted\", \"args\": {\"count\": 1}}]},\n    {\"call\": \"api.add\", \"returns\": 2},\n    {\"call\": \"api.get\", \"returns\": 2}\n  ]\n}\n";
    assert_eq!(
        fs::read_to_string(f.dir.join("tests/counter.json")).unwrap(),
        expected
    );
    let spec =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md")).unwrap();
    assert!(spec.contains(&format!(
        "`template/tests/counter.json`:\n```json\n{expected}```"
    )));
    let guide = fs::read_to_string(f.dir.join("AGENTS.md")).unwrap();
    assert!(guide.lines().count() <= 150);
    assert!(guide.contains("## Tests"));
    assert!(guide.contains("WebAssembly"));
    for title in ["Order of work", "Rules"] {
        let section = guide
            .split(&format!("## {title}\n"))
            .nth(1)
            .unwrap()
            .split("\n## ")
            .next()
            .unwrap();
        assert!(section.contains("watane-dev test --json"), "{section}");
        assert!(
            section.find("watane-dev test --json").unwrap()
                < section.find("watane-dev serve --json").unwrap()
        );
    }
}

#[test]
fn ac7_manifest_problems_point_to_offending_key() {
    let f = Fixture::new();
    let original = fs::read_to_string(f.dir.join("logic/Cargo.toml")).unwrap();
    for (text, needle, code) in [
        (
            original.replace("[dependencies]", "[dependencies]\nrand = \"0.9\""),
            "rand =",
            "mod.dependency",
        ),
        (
            format!("{original}\n[build-dependencies]\n"),
            "[build-dependencies]",
            "mod.cargo_key",
        ),
    ] {
        fs::write(f.dir.join("logic/Cargo.toml"), &text).unwrap();
        let (status, v) = output(&mut f.command("check"));
        assert_eq!(status, 1, "{v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert_eq!(v["errors"][0]["file"], "logic/Cargo.toml");
        assert_eq!(v["errors"][0]["line"], line(&text, needle));
    }
}

#[test]
fn ac8_test_only_compile_error_has_location() {
    let f = Fixture::new();
    let text = f.source("#[cfg(test)]\ncompile_error!(\"test-only compile failure\");");
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.compile", "{v}");
    assert_eq!(v["errors"][0]["file"], "logic/src/lib.rs");
    assert_eq!(v["errors"][0]["line"], line(&text, "compile_error!"));
    assert!(v["result"]["check"].is_object());
    assert!(v["result"]["build"].is_object());
    assert!(v["result"]["unit"].is_null());
}

#[test]
fn ac9_agent_can_fix_a_broken_counter_and_rerun() {
    let f = Fixture::new();
    let path = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        original.replace("count.saturating_add(1)", "count.saturating_add(2)"),
    )
    .unwrap();
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.unit", "{v}");
    fs::write(path, original).unwrap();
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 0, "{v}");
    assert_eq!(v["result"]["unit"]["passed"], 2);
}

#[test]
fn ac2_unit_build_refuses_outside_reads_before_running_tests() {
    let f = Fixture::new();
    let outside = f.temp.path().join("test-only secret.txt");
    fs::write(&outside, b"S8-unit-only-secret").unwrap();
    f.source(&format!(
        r#"
#[cfg(test)]
mod outside_read {{
    #[test]
    fn must_never_run() {{
        std::hint::black_box(include_bytes!({:?}));
        panic!("S8 unit test ran before reads check");
    }}
}}
"#,
        outside.to_str().unwrap()
    ));
    let (status, v) = output(&mut f.command("test"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.reads_outside", "{v}");
    assert_eq!(
        v["errors"][0]["message"],
        format!(
            "the build read {}, outside the mod",
            outside.canonicalize().unwrap().display()
        )
    );
    assert!(v["result"]["build"].is_object(), "{v}");
    assert!(v["result"]["unit"].is_null(), "{v}");
    assert!(!f.dir.join("logic.wasm").exists());
}

#[test]
fn ac3_fixed_mod_discards_stale_release_and_unit_dependencies() {
    for test_only in [false, true] {
        let f = Fixture::new();
        let source = f.dir.join("logic/src/lib.rs");
        let original = fs::read_to_string(&source).unwrap();
        let outside = f.temp.path().join("outside.txt");
        fs::write(&outside, b"S8-stale-dependency").unwrap();
        let cfg = if test_only { "#[cfg(test)]" } else { "" };
        f.source(&format!(
            "{cfg}\n#[used]\nstatic X: &[u8] = include_bytes!({:?});",
            outside.to_str().unwrap()
        ));
        let (status, v) = output(&mut f.command("test"));
        assert_eq!(status, 1, "{v}");
        assert_eq!(v["errors"][0]["code"], "build.reads_outside", "{v}");
        let profile = if test_only { "debug" } else { "release" };
        let stale = f
            .temp
            .path()
            .join("cache/target/wasm32-wasip2")
            .join(profile)
            .join("deps/counter-stale.d");
        fs::write(&stale, format!("old.wasm: {}\n", outside.display())).unwrap();
        fs::write(&source, original).unwrap();
        let (status, v) = output(&mut f.command("test"));
        assert_eq!(status, 0, "{v}");
        assert!(!stale.exists());
        // Deleting text dep-info must also work when Cargo would otherwise be fresh.
        let (status, v) = output(&mut f.command("test"));
        assert_eq!(status, 0, "{v}");
    }
}
