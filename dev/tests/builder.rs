use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};

struct Fixture {
    temp: tempfile::TempDir,
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("hello");
        let output = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(temp.path())
            .args(["new", "hello", "--json"])
            .output()
            .unwrap();
        assert!(output.status.success());
        Self { temp, dir }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        c.current_dir(&self.dir)
            .args(["serve", "--json"])
            .env("WATANE_DEV_CACHE", self.temp.path().join("cache"))
            .env("WATANE_BIN", self.temp.path().join("missing-host"));
        c
    }
    fn fake_toolchain(&self, target: bool) -> PathBuf {
        let path = self.temp.path().join("toolchain");
        fs::create_dir_all(path.join("bin")).unwrap();
        if target {
            fs::create_dir_all(path.join("lib/rustlib/wasm32-wasip2/lib")).unwrap();
        }
        executable(
            &path.join("bin/cargo"),
            &format!(
                "#!/bin/sh\ntouch '{}'\nexit 99\n",
                self.temp.path().join("cargo-ran").display()
            ),
        );
        executable(
            &path.join("bin/rustc"),
            &format!(
                "#!/bin/sh\ncase \"$1\" in --print) echo '{}' ;; --version) echo 'rustc fake' ;; esac\n",
                path.display()
            ),
        );
        path
    }
}
fn executable(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}
fn output(c: &mut Command) -> (i32, Value) {
    let o = c.output().unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stderr))),
    )
}

#[test]
fn ac4_serve_rejects_before_cargo() {
    for (path, content, code) in [
        ("logic/build.rs", "fn main() {}", "mod.logic_file"),
        (
            "logic/Cargo.toml",
            "[package]\nname = \"hello\"\nversion = \"0.1.0\"\n[lib]\ncrate-type = [\"cdylib\"]\n[workspace]\n[dependencies]\nrand = \"0.9\"",
            "mod.dependency",
        ),
        (
            "logic/Cargo.toml",
            "[package]\nname = \"hello\"\nversion = \"0.1.0\"\n[lib]\ncrate-type = [\"cdylib\"]\n[workspace]\n[build-dependencies]",
            "mod.cargo_key",
        ),
        (
            "logic/Cargo.toml",
            "[package]\nname = \"hello\"\nversion = \"0.1.0\"\n[lib]\ncrate-type = [\"cdylib\"]\nproc-macro = true\n[workspace]",
            "mod.cargo_key",
        ),
    ] {
        let f = Fixture::new();
        fs::write(f.dir.join(path), content).unwrap();
        let (status, v) = output(
            f.command()
                .env("WATANE_DEV_TOOLCHAIN", f.fake_toolchain(true)),
        );
        assert_eq!(status, 1, "{v}");
        assert_eq!(v["errors"][0]["code"], code, "{v}");
        assert!(!f.temp.path().join("cargo-ran").exists());
    }
}

#[test]
fn ac5_missing_target_stops_before_cargo() {
    let f = Fixture::new();
    let (status, v) = output(
        f.command()
            .env("WATANE_DEV_TOOLCHAIN", f.fake_toolchain(false)),
    );
    assert_eq!(status, 3, "{v}");
    assert_eq!(v["errors"][0]["code"], "env.no_wasm_target");
    assert_eq!(v["errors"][0]["fix"], "rustup target add wasm32-wasip2");
    assert!(!f.temp.path().join("cargo-ran").exists());
}

#[test]
fn ac6_build_counter_component() {
    let f = Fixture::new();
    let (status, v) = output(&mut f.command());
    // A missing host is reached only after the successful build.
    assert_eq!(status, 3, "{v}");
    assert_eq!(v["errors"][0]["code"], "env.watane_missing", "{v}");
    let bytes = fs::read(f.dir.join("logic.wasm")).expect("built component");
    assert!(
        bytes
            .windows(b"mods:hello/api@0.1.0".len())
            .any(|w| w == b"mods:hello/api@0.1.0")
    );
    watane_pkg::Mod::read(&f.dir)
        .unwrap()
        .check_component()
        .unwrap();
    assert!(f.dir.join("logic/Cargo.lock").is_file());
    assert!(!f.dir.join("logic/target").exists());
}

#[test]
#[ignore = "needs sandbox-exec permission: sandbox_apply is denied in this environment"]
fn ac6_build_with_network_denied() {
    let exe = std::env::current_exe().unwrap();
    let mut c = if cfg!(target_os = "macos") {
        let mut c = Command::new("/usr/bin/sandbox-exec");
        c.args(["-p", "(version 1)(allow default)(deny network*)"])
            .arg(exe);
        c
    } else {
        let mut c = Command::new("unshare");
        c.arg("-rn").arg(exe);
        c
    };
    let o = c
        .args(["--exact", "ac6_build_counter_component", "--nocapture"])
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn ac7_compile_error_names_mod_file_and_line_and_removes_wasm() {
    let f = Fixture::new();
    let path = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!("{original}\ncompile_error!(\"intentional AC7 failure\");\n"),
    )
    .unwrap();
    let line = original.lines().count() + 2;
    fs::write(f.dir.join("logic.wasm"), "stale").unwrap();
    let (status, v) = output(&mut f.command());
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.compile", "{v}");
    assert_eq!(v["errors"][0]["file"], "logic/src/lib.rs");
    assert_eq!(v["errors"][0]["line"], line);
    assert!(!f.dir.join("logic.wasm").exists());
}

#[test]
fn ac7_build_sees_no_maker_secret() {
    let f = Fixture::new();
    let path = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&path).unwrap();
    fs::write(
        path,
        format!("{original}\nconst _: &str = env!(\"SECRET\");\n"),
    )
    .unwrap();
    let (status, v) = output(f.command().env("SECRET", "must-not-reach-cargo"));
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.compile", "{v}");
    assert!(
        v["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("SECRET")
    );
    assert!(!f.dir.join("logic.wasm").exists());
}

#[test]
fn ac8_missing_vendor_is_environment_error() {
    let f = Fixture::new();
    let (status, v) = output(
        f.command()
            .env("WATANE_DEV_VENDOR", f.temp.path().join("missing-vendor")),
    );
    assert_eq!(status, 3, "{v}");
    assert_eq!(v["errors"][0]["code"], "env.allowlist");
    assert!(!f.dir.join("logic.wasm").exists());
}
