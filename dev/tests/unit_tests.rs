#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

mod support;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

struct Fixture {
    _guard: MutexGuard<'static, ()>,
    temp: tempfile::TempDir,
    dir: PathBuf,
    cache: PathBuf,
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
            cache: support::cache().to_path_buf(),
        }
    }
    fn command(&self, name: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        command
            .current_dir(&self.dir)
            .env("WATANE_DEV_CACHE", &self.cache)
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
/// The folders where Cargo puts `package`'s compiled units under `profile_dir`:
/// `deps/` up to Cargo 1.99, `build/<package>/<hash>/out/` from 1.100 (TECHNICAL 6.4).
fn units(profile_dir: &Path, package: &str) -> Vec<PathBuf> {
    let deps = profile_dir.join("deps");
    if deps.is_dir() {
        return vec![deps];
    }
    fs::read_dir(profile_dir.join("build").join(package))
        .unwrap()
        .map(|e| e.unwrap().path().join("out"))
        .collect()
}
fn line(text: &str, needle: &str) -> usize {
    text.lines().position(|line| line.contains(needle)).unwrap() + 1
}

#[test]
fn ac1_ac2_template_unit_counts_and_warm_pipeline() {
    let cache = support::cache();
    assert_eq!(cache, support::cache());
    let artifact = units(&cache.join("target/wasm32-wasip2/release"), "serde_core")
        .into_iter()
        .flat_map(|dir| fs::read_dir(dir).unwrap())
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("libserde_core-")
                && p.extension().is_some_and(|ext| ext == "rlib")
        })
        .unwrap();
    let before = fs::metadata(&artifact).unwrap().modified().unwrap();
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
    assert_eq!(fs::metadata(artifact).unwrap().modified().unwrap(), before);

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
    assert_eq!(
        v["next"],
        "Fix the unit test fails_on_purpose, then run: watane-dev test --json"
    );
    // The served mod's recovery is a serve run, in the fix as in the next.
    let (status, v) = output(&mut f.command("serve"));
    assert_eq!(status, 1, "{v}");
    let fix = "Fix the unit test fails_on_purpose, then run: watane-dev serve --json";
    assert_eq!(v["errors"][0]["fix"], fix);
    assert_eq!(v["next"], fix);
}

#[test]
fn ac11_endless_test_stops_with_exact_message() {
    endless_limit(Duration::from_secs(60), false);
}

#[test]
#[ignore = "timing: run on a quiescent machine, see factory/README.md"]
fn ac10_endless_test_stops_with_exact_message_within_deadline() {
    endless_limit(Duration::from_secs(7), true);
}

fn endless_limit(deadline: Duration, cached: bool) {
    let f = Fixture::new();
    f.source("#[test]\nfn endless() { loop {} }");
    if cached {
        // Build and check this exact endless source before measuring its stop.
        run_endless(&f, Duration::from_secs(60));
    }
    run_endless(&f, deadline);
}

fn run_endless(f: &Fixture, deadline: Duration) {
    let started = Instant::now();
    let mut child = f
        .command("test")
        .env("WATANE_DEV_UNIT_SECONDS", "2")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(60) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("endless test exceeded 60 s hang guard");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    let status = out.status.code().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        started.elapsed() < deadline,
        "elapsed: {:?}; {v}",
        started.elapsed()
    );
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "test.unit_limit", "{v}");
    assert_eq!(
        v["errors"][0]["message"],
        "unit tests stopped at endless after 2 s"
    );
    assert_eq!(
        v["next"],
        "Fix the unit test endless, then run: watane-dev test --json"
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
#[ignore = "slow: full Rust allocation pipeline reports a mod memory failure"]
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
#[ignore = "slow: Rust unit pipeline traps an actual template host import"]
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
#[ignore = "slow: test-only Rust compilation forwards compiler location"]
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
#[ignore = "slow: full pipeline rebuilds repaired counter logic"]
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
#[ignore = "slow: unit dep-info rejects outside reads before execution"]
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
#[ignore = "slow: isolated stale release and unit dep-info cache"]
fn ac3_fixed_mod_discards_stale_release_and_unit_dependencies() {
    for test_only in [false, true] {
        let mut f = Fixture::new();
        f.cache = f.temp.path().join("cache");
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
            .join(profile);
        let stale = units(&stale, "counter")[0].join("counter-stale.d");
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
