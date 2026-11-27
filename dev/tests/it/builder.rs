#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

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
#[ignore = "slow: cold template build and component validation"]
fn ac6_build_counter_component() {
    let f = Fixture::new();
    let (status, v) = output(&mut f.command());
    // A missing host is reached only after the successful build.
    assert_eq!(status, 3, "{v}");
    assert_eq!(v["errors"][0]["code"], "env.watane_missing", "{v}");
    // The tests passed, so `next` must not send the agent back to them.
    assert_eq!(v["next"], "watane-dev doctor --json", "{v}");
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
#[ignore = "slow: cold offline build; sandbox-exec sandbox_apply is denied in this environment"]
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
        .args([
            "--exact",
            "builder::ac6_build_counter_component",
            "--include-ignored",
            "--nocapture",
        ])
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    // An empty filter match also exits 0: require that the build test ran.
    assert!(String::from_utf8_lossy(&o.stdout).contains("1 passed"));
}

#[test]
#[ignore = "slow: cold compiler error removes stale wasm"]
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
#[ignore = "slow: cold serve build clears maker environment"]
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

fn assert_no_marker(dir: &Path, marker: &[u8]) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            assert_no_marker(&path, marker);
        } else {
            let bytes = fs::read(&path).unwrap();
            assert!(
                !bytes.windows(marker.len()).any(|w| w == marker),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
#[ignore = "slow: cold release dep-info rejects outside reads"]
fn ac1_build_refuses_absolute_bytes_strings_and_relative_outside_reads() {
    let f = Fixture::new();
    let source = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&source).unwrap();
    for (mac, relative) in [
        ("include_bytes", false),
        ("include_str", false),
        ("include_bytes", true),
    ] {
        let outside = if relative {
            fs::create_dir_all(f.temp.path().join("cache")).unwrap();
            f.temp.path().join("cache/outside.txt")
        } else {
            f.temp.path().join("outside file.txt")
        };
        let marker = b"S8-distinctive-secret-never-in-mod-84739";
        fs::write(&outside, marker).unwrap();
        let include = if relative {
            "../../../../outside.txt"
        } else {
            outside.to_str().unwrap()
        };
        let as_bytes = if mac == "include_str" {
            ".as_bytes()"
        } else {
            ""
        };
        fs::write(
            &source,
            format!("{original}\n#[used]\nstatic X: &[u8] = {mac}!({include:?}){as_bytes};\n"),
        )
        .unwrap();
        fs::write(f.dir.join("logic.wasm"), b"stale").unwrap();
        let (status, v) = output(&mut {
            let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
            c.current_dir(&f.dir)
                .args(["test", "--json"])
                .env("WATANE_DEV_CACHE", f.temp.path().join("cache"));
            c
        });
        assert_eq!(status, 1, "{v}");
        let errors = v["errors"].as_array().unwrap();
        let error = errors
            .iter()
            .find(|e| e["code"] == "build.reads_outside")
            .unwrap_or_else(|| panic!("{v}"));
        assert_eq!(
            error["message"],
            format!(
                "the build read {}, outside the mod",
                outside.canonicalize().unwrap().display()
            )
        );
        assert!(!f.dir.join("logic.wasm").exists());
        assert_no_marker(&f.dir, marker);
    }
}

/// S93 AC5: a relative `generate!` path that reaches another mod's stale copy in the cache
/// is a read outside the mod, not a compile error about that sibling.
#[test]
#[ignore = "slow: cold build reaches a stale sibling copy through generate!"]
fn s93_ac5_relative_generate_path_into_a_cache_sibling_reads_outside() {
    let f = Fixture::new();
    let sibling = f.temp.path().join("cache/build/other/wit");
    fs::create_dir_all(&sibling).unwrap();
    let wit = fs::read_to_string(f.dir.join("wit/world.wit"))
        .unwrap()
        .replace("hello", "other");
    fs::write(sibling.join("world.wit"), wit).unwrap();
    let source = f.dir.join("logic/src/lib.rs");
    let text = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        text.replace("path: \"../wit\"", "path: \"../../other/wit\""),
    )
    .unwrap();
    let (status, v) = output(&mut {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        c.current_dir(&f.dir)
            .args(["test", "--json"])
            .env("WATANE_DEV_CACHE", f.temp.path().join("cache"));
        c
    });
    assert_eq!(status, 1, "{v}");
    let sibling = sibling.canonicalize().unwrap();
    assert_eq!(v["errors"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.reads_outside", "{v}");
    assert_eq!(
        v["errors"][0]["file"],
        f.dir
            .canonicalize()
            .unwrap()
            .join("logic/src")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        v["errors"][0]["message"],
        format!("the build read {}, outside the mod", sibling.display())
    );
    assert_eq!(
        v["errors"][0]["fix"],
        format!(
            "Point wit_bindgen::generate! in logic/src at this mod's own wit/ folder (path: \"../wit\"), not at {}, then run: watane-dev test --json",
            sibling.display()
        )
    );
}

/// S93 outcome 4: another mod's stale copy in the cache declares its own world (as a real
/// sibling does). A relative `generate!` path into it must read as `build.reads_outside`, not
/// as `build.compile` "World `hello` not found in package `mods:other@0.1.0`".
#[test]
#[ignore = "slow: cold build reaches a stale sibling copy with another world through generate!"]
fn regress_s93_generate_path_into_a_sibling_with_its_own_world_reads_outside() {
    let f = Fixture::new();
    let sibling = f.temp.path().join("cache/build/other/wit");
    fs::create_dir_all(&sibling).unwrap();
    fs::write(
        sibling.join("world.wit"),
        "package mods:other@0.1.0;\n\nworld other {}\n",
    )
    .unwrap();
    let source = f.dir.join("logic/src/lib.rs");
    let text = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        text.replace("path: \"../wit\"", "path: \"../../other/wit\""),
    )
    .unwrap();
    let (status, v) = output(&mut {
        let mut c = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        c.current_dir(&f.dir)
            .args(["test", "--json"])
            .env("WATANE_DEV_CACHE", f.temp.path().join("cache"));
        c
    });
    assert_eq!(status, 1, "{v}");
    let sibling = sibling.canonicalize().unwrap();
    assert_eq!(v["errors"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.reads_outside", "{v}");
    assert_eq!(
        v["errors"][0]["file"],
        f.dir
            .canonicalize()
            .unwrap()
            .join("logic/src")
            .to_str()
            .unwrap()
    );
    assert_eq!(
        v["errors"][0]["message"],
        format!("the build read {}, outside the mod", sibling.display())
    );
}

#[test]
#[ignore = "slow: cold test build clears maker environment"]
fn ac4_test_build_does_not_receive_shell_secret() {
    let f = Fixture::new();
    let source = f.dir.join("logic/src/lib.rs");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(
        source,
        format!("{original}\nconst _: &str = env!(\"SECRET\");"),
    )
    .unwrap();
    let (status, v) = output(
        Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&f.dir)
            .args(["test", "--json"])
            .env("WATANE_DEV_CACHE", f.temp.path().join("cache"))
            .env("SECRET", "abc"),
    );
    assert_eq!(status, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "build.compile", "{v}");
}

#[test]
#[ignore = "slow: two cold caches prove reproducible path-free bytes"]
fn ac5_same_mod_at_different_paths_and_caches_has_identical_path_free_wasm() {
    let f = Fixture::new();
    let parent = f.temp.path().join("a much longer parent directory for Bob");
    fs::create_dir_all(&parent).unwrap();
    let bob = parent.join("hello");
    assert!(
        Command::new("cp")
            .arg("-R")
            .arg(&f.dir)
            .arg(&bob)
            .status()
            .unwrap()
            .success()
    );
    let caches = [f.temp.path().join("alice-cache"), parent.join("bob-cache")];
    let dirs = [&f.dir, &bob];
    let mut artifacts = Vec::new();
    for (dir, cache) in dirs.iter().zip(&caches) {
        let (status, v) = output(
            Command::new(env!("CARGO_BIN_EXE_watane-dev"))
                .current_dir(dir)
                .args(["test", "--json"])
                .env("WATANE_DEV_CACHE", cache),
        );
        assert_eq!(status, 0, "{v}");
        artifacts.push(fs::read(dir.join("logic.wasm")).unwrap());
    }
    assert_eq!(
        artifacts[0], artifacts[1],
        "logic.wasm differs between paths"
    );
    let mut paths = vec![PathBuf::from(std::env::var_os("HOME").unwrap())];
    paths.extend(dirs.into_iter().cloned());
    paths.extend(caches);
    for path in paths {
        for path in [path.clone(), path.canonicalize().unwrap()] {
            let needle = path.to_str().unwrap().as_bytes();
            assert!(
                !artifacts[0].windows(needle.len()).any(|w| w == needle),
                "embedded {}",
                path.display()
            );
        }
    }
}

#[test]
fn ac8_test_and_serve_env_gaps_point_to_doctor() {
    let f = Fixture::new();
    for name in ["test", "serve"] {
        let out = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .current_dir(&f.dir)
            .args([name, "--json"])
            .env("WATANE_DEV_CACHE", f.temp.path().join("cache"))
            .env("WATANE_DEV_TOOLCHAIN", f.fake_toolchain(false))
            .output()
            .unwrap();
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(3), "{value}");
        assert_eq!(value["errors"][0]["code"], "env.no_wasm_target");
        assert_eq!(value["next"], "watane-dev doctor --json");
    }
}

// S47 user-sim: `new --from` keeps logic.wasm when the package has no logic
// source, and its `next` says `watane-dev test --json`. That run must neither
// delete the only logic the mod has nor fail with an opaque internal error.
#[test]
fn regress_s47_test_keeps_packaged_logic_without_source() {
    let f = Fixture::new();
    let dir = f.temp.path().join("ratings");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ratings");
    fs::create_dir_all(&dir).unwrap();
    let status = Command::new("cp")
        .arg("-R")
        .arg(format!("{}/.", fixture.display()))
        .arg(&dir)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(!dir.join("logic").exists());
    // As in a derived source package: contract tests, no logic crate.
    fs::create_dir(dir.join("tests")).unwrap();
    fs::write(
        dir.join("tests/ratings.json"),
        r#"{"steps":[{"call":"api.get","returns":0}]}"#,
    )
    .unwrap();
    let expected = fs::read(dir.join("logic.wasm")).unwrap();
    let (status, v) = output(
        Command::new(env!("CARGO_BIN_EXE_watane-dev"))
            .args(["test", "--json", "--dir"])
            .arg(&dir)
            .env("WATANE_DEV_CACHE", f.temp.path().join("cache"))
            .env("WATANE_BIN", f.temp.path().join("missing-host"))
            .env("WATANE_DEV_TOOLCHAIN", f.fake_toolchain(true)),
    );
    assert!(
        fs::read(dir.join("logic.wasm")).is_ok_and(|bytes| bytes == expected),
        "test deleted or changed the packaged logic.wasm: {v}"
    );
    for e in v["errors"].as_array().into_iter().flatten() {
        assert_ne!(e["code"], "internal", "{v}");
    }
    // AC2: the kept component runs contracts without a build or unit run.
    assert_eq!(status, 0, "{v}");
    assert!(v["result"]["check"].is_object(), "{v}");
    assert_eq!(v["result"]["contract"]["passed"], 1, "{v}");
    assert!(v["result"]["build"].is_null(), "{v}");
    assert!(v["result"]["unit"].is_null(), "{v}");
}
