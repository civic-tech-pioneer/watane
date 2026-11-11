use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Machine {
    temp: tempfile::TempDir,
    vendor: PathBuf,
}
impl Machine {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("home")).unwrap();
        script(&temp.path().join("watane"), "#!/bin/sh\nexit 0\n");
        let vendor = std::env::var_os("WATANE_DEV_VENDOR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("allowlist/vendor"));
        assert!(
            vendor.is_dir(),
            "vendor allowlist first: {}",
            vendor.display()
        );
        Self {
            temp,
            vendor: vendor.canonicalize().unwrap(),
        }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        c.current_dir(self.temp.path())
            .env_remove("WATANE_DEV_TOOLCHAIN")
            .env("WATANE_DEV_VENDOR", &self.vendor)
            .env("WATANE_BIN", self.temp.path().join("watane"))
            .env("WATANE_HOME", self.temp.path().join("home"))
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"));
        c
    }
    fn gaps(&self) -> Command {
        let mut c = self.command();
        c.env("WATANE_DEV_TOOLCHAIN", self.temp.path())
            .env("WATANE_DEV_VENDOR", self.temp.path().join("missing-vendor"))
            .env("WATANE_BIN", self.temp.path().join("missing-watane"));
        c
    }
}
fn script(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
fn report(c: &mut Command, code: i32) -> Value {
    let output = c.args(["doctor", "--json"]).output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn codes(v: &Value) -> Vec<&str> {
    v["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["code"].as_str().unwrap())
        .collect()
}
#[test]
fn ac1_complete_machine_reports_environment() {
    let m = Machine::new();
    let v = report(&mut m.command(), 0);
    assert_eq!(v["command"], "doctor");
    assert!(v["dir"].is_null());
    assert_eq!(v["errors"], json!([]));
    assert_eq!(v["ok"], true);
    let r = &v["result"];
    for key in ["cargo", "rustc", "sysroot", "version"] {
        assert!(r["toolchain"][key].is_string(), "{v}");
    }
    assert!(
        r["toolchain"]["version"]
            .as_str()
            .unwrap()
            .starts_with("rustc ")
    );
    assert_eq!(r["toolchain"]["wasm32_wasip2"], true);
    assert_eq!(r["allowlist"]["vendor"], json!(m.vendor));
    assert_eq!(
        r["allowlist"]["direct"],
        json!(["serde 1.0.229", "serde_json 1.0.154", "wit-bindgen 0.63.0"])
    );
    let lock: toml::Table = toml::from_str(
        &fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("allowlist/Cargo.lock"))
            .unwrap(),
    )
    .unwrap();
    let count = lock["package"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p.get("checksum").is_some())
        .count();
    assert_eq!(r["allowlist"]["crates"], count);
    assert_eq!(
        r["watane"],
        json!(m.temp.path().join("watane").canonicalize().unwrap())
    );
    assert_eq!(
        r["home"],
        json!({"path":m.temp.path().join("home"),"running":false,"app":false})
    );
    assert_eq!(v["next"], "watane-dev new <slug> --json");
}
#[test]
fn ac2_missing_watane_retains_other_parts() {
    let m = Machine::new();
    let missing = m.temp.path().join("missing-watane");
    let v = report(m.command().env("WATANE_BIN", &missing), 3);
    assert_eq!(codes(&v), ["env.watane_missing"]);
    assert!(
        v["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains(missing.to_str().unwrap())
    );
    assert_eq!(
        v["errors"][0]["fix"],
        "Install watane, or set WATANE_BIN to the program"
    );
    assert!(v["result"]["watane"].is_null());
    for key in ["toolchain", "allowlist", "home"] {
        assert!(v["result"][key].is_object());
    }
    assert_eq!(v["next"], "watane-dev doctor --json");
}
#[test]
fn ac3_missing_vendor_names_bootstrap_script() {
    let m = Machine::new();
    let v = report(
        m.command()
            .env("WATANE_DEV_VENDOR", m.temp.path().join("missing")),
        3,
    );
    assert_eq!(codes(&v), ["env.allowlist"]);
    assert!(v["result"]["allowlist"].is_null());
    assert!(
        v["errors"][0]["fix"]
            .as_str()
            .unwrap()
            .contains("scripts/vendor.sh")
    );
}
// S16 (Codex as Bob): a missing vendor folder read as a mismatch, so the agent went to the
// source to learn what was wrong. TECHNICAL 2.2 `env.allowlist`: "the crate allowlist at <path> is missing".
#[test]
#[ignore = "red: S17 fixes it (TECHNICAL 2.2 env.allowlist missing)"]
fn regress_s16_missing_vendor_says_missing() {
    let m = Machine::new();
    let missing = m.temp.path().join("missing");
    let v = report(m.command().env("WATANE_DEV_VENDOR", &missing), 3);
    assert_eq!(
        v["errors"][0]["message"],
        json!(format!(
            "the crate allowlist at {} is missing",
            missing.display()
        ))
    );
}
#[test]
fn ac4_all_gaps_reported_in_order() {
    let m = Machine::new();
    let v = report(&mut m.gaps(), 3);
    assert_eq!(
        codes(&v),
        ["env.toolchain", "env.allowlist", "env.watane_missing"]
    );
    for error in v["errors"].as_array().unwrap() {
        assert!(error["fix"].is_string());
    }
    for key in ["toolchain", "allowlist", "watane"] {
        assert!(v["result"][key].is_null());
    }
}
#[test]
fn ac5_missing_target_retains_toolchain_without_running_cargo() {
    let m = Machine::new();
    let root = m.temp.path();
    fs::create_dir(root.join("bin")).unwrap();
    fs::create_dir(root.join("sysroot")).unwrap();
    script(
        &root.join("bin/cargo"),
        &format!("#!/bin/sh\ntouch '{}'\n", root.join("cargo-ran").display()),
    );
    script(
        &root.join("bin/rustc"),
        &format!(
            "#!/bin/sh\ncase \"$1\" in\n--print) echo '{}' ;;\n--version) echo 'rustc 9.9.9 (fake)' ;;\nesac\n",
            root.join("sysroot").display()
        ),
    );
    let v = report(m.command().env("WATANE_DEV_TOOLCHAIN", root), 3);
    assert_eq!(codes(&v), ["env.no_wasm_target"]);
    assert_eq!(v["errors"][0]["fix"], "rustup target add wasm32-wasip2");
    assert_eq!(v["result"]["toolchain"]["version"], "rustc 9.9.9 (fake)");
    assert_eq!(v["result"]["toolchain"]["wasm32_wasip2"], false);
    assert!(!root.join("cargo-ran").exists());
}
#[test]
fn ac6_stale_home_is_no_error() {
    let m = Machine::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    fs::write(
        m.temp.path().join("home/run.json"),
        json!({"watane":1,"pid":1,"port":port,"mode":"home"}).to_string(),
    )
    .unwrap();
    let v = report(&mut m.command(), 0);
    assert_eq!(v["result"]["home"]["running"], false);
    assert_eq!(v["errors"], json!([]));
}
#[test]
#[ignore = "needs WATANE_BIN"]
fn ac6_real_preview_home_is_running_without_app() {
    let m = Machine::new();
    let binary = std::env::var_os("WATANE_BIN").expect("WATANE_BIN");
    let run = |args: &[&str]| -> Output {
        m.command()
            .env("WATANE_BIN", &binary)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(run(&["new", "hello", "--json"]).status.success());
    let served = run(&["--dir", "hello", "serve", "--json"]);
    assert!(
        served.status.success(),
        "{}",
        String::from_utf8_lossy(&served.stdout)
    );
    let served: Value = serde_json::from_slice(&served.stdout).unwrap();
    let home = served["result"]["home"].as_str().unwrap();
    let output = m
        .command()
        .env("WATANE_BIN", &binary)
        .env("WATANE_HOME", home)
        .args(["doctor", "--json"])
        .output()
        .unwrap();
    let stopped = run(&["--dir", "hello", "serve", "--stop", "--json"]);
    assert!(stopped.status.success());
    assert!(output.status.success());
    let v: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        v["result"]["home"],
        json!({"path":home,"running":true,"app":false})
    );
}
#[test]
fn ac7_text_reports_each_gap_and_summary() {
    let m = Machine::new();
    let output = m.gaps().arg("doctor").output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8(output.stdout).unwrap();
    for code in ["env.toolchain", "env.allowlist", "env.watane_missing"] {
        assert_eq!(
            text.lines()
                .filter(|line| line.starts_with(&format!("error[{code}]")))
                .count(),
            1,
            "{text}"
        );
    }
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("  fix:"))
            .count(),
        3
    );
    assert!(text.contains("doctor: failed"));
    let output = m.command().arg("doctor").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("doctor: ok")
    );
}
#[test]
fn ac8_new_guide_starts_with_doctor_and_fixes() {
    let m = Machine::new();
    assert!(
        m.command()
            .args(["new", "hello", "--json"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let guide = fs::read_to_string(m.temp.path().join("hello/AGENTS.md")).unwrap();
    let order = guide
        .split("## Order of work\n")
        .nth(1)
        .unwrap()
        .split("\n## ")
        .next()
        .unwrap()
        .trim();
    assert!(order.starts_with("`watane-dev doctor --json`"), "{order}");
    assert!(
        order.contains("each error's `fix` before going on"),
        "{order}"
    );
}
