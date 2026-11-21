#![allow(
    clippy::unwrap_used,
    clippy::disallowed_methods,
    reason = "tests may panic, catch panics, and share lock-only mutexes that guard no data"
)]

use std::fs;
use std::path::Path;
use watane_pkg::{Mod, ModError, Rule, Violation};
use wit_component::{ComponentEncoder, StringEncoding, dummy_module, embed_component_metadata};
use wit_parser::{ManglingAndAbi, Resolve};

const DEFINITIONS: &str = "package mods:hello@0.1.0;
interface api { count: func() -> u32; }
interface changes { changed: func(value: u32); }
interface old { f: func(); }
interface extra { f: func(); }
";
const HOST: &str = "package watane:host@0.1.0;
interface state { get: func(key: string) -> option<list<u8>>; }
interface caller { who: func() -> string; }
interface stream-sink { push: func(item: u32); }";

fn put(dir: &Path, file: &str, bytes: impl AsRef<[u8]>) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn folder(items: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "mod.toml",
        "name = 'Hello'\nsummary = 'Counts.'\n",
    );
    put(dir.path(), "logic/Cargo.toml", "");
    put(
        dir.path(),
        "wit/world.wit",
        format!("{DEFINITIONS} world hello {{ {items} }}"),
    );
    put(dir.path(), "wit/deps/watane-host/host.wit", HOST);
    dir
}

// Independent producer: WIT literals -> dummy core module -> component, never pkg.
fn component(items: &str, dependencies: &[&str]) -> Vec<u8> {
    let mut resolve = Resolve::default();
    resolve.push_str("host.wit", HOST).unwrap();
    for dep in dependencies {
        resolve.push_str("dep.wit", dep).unwrap();
    }
    let package = resolve
        .push_str(
            "world.wit",
            &format!("{DEFINITIONS} world hello {{ {items} }}"),
        )
        .unwrap();
    let world = resolve.select_world(&[package], None).unwrap();
    let mut module = dummy_module(&resolve, world, ManglingAndAbi::Standard32);
    embed_component_metadata(&mut module, &resolve, world, StringEncoding::UTF8).unwrap();
    ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap()
}

fn violations(m: &Mod) -> Vec<Violation> {
    let Err(ModError::Invalid(v)) = m.check_component() else {
        panic!("expected component violations")
    };
    for violation in &v {
        assert_eq!(violation.rule, Rule::Component);
        assert_eq!(violation.file, "logic.wasm");
    }
    v
}

#[test]
fn ac1_page_only_passes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello");
    Mod::read(&path).unwrap().check_component().unwrap();
}

#[test]
fn ac2_declared_host_and_own_imports_pass() {
    let items = "import watane:host/state@0.1.0; import changes; export api;";
    let dir = folder(items);
    put(dir.path(), "logic.wasm", component(items, &[]));
    Mod::read(dir.path()).unwrap().check_component().unwrap();
}

#[test]
fn ac3_size_and_file_type_are_checked_before_decode() {
    let dir = folder("export api;");
    let path = dir.path().join("logic.wasm");
    fs::File::create(&path)
        .unwrap()
        .set_len(67_108_865)
        .unwrap();
    let m = Mod::read(dir.path()).unwrap();
    let v = violations(&m);
    assert_eq!(v.len(), 1);
    assert!(
        v[0].message.contains("67108864") || v[0].message.contains("64 MiB"),
        "{v:?}"
    );
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    let v = violations(&Mod::read(dir.path()).unwrap());
    assert_eq!(v.len(), 1);
    assert!(v[0].message.contains("regular file"), "{v:?}");
}

#[test]
fn ac3_core_module_and_random_bytes_are_not_components() {
    let dir = folder("export api;");
    for bytes in [b"\0asm\x01\0\0\0".as_slice(), b"random bytes"] {
        put(dir.path(), "logic.wasm", bytes);
        let v = violations(&Mod::read(dir.path()).unwrap());
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].message, "logic.wasm is not a WebAssembly component");
    }
}

#[test]
fn ac4_undeclared_interface_function_and_plain_imports_are_refused() {
    let dir = folder("export api;");
    let m = Mod::read(dir.path()).unwrap();
    for (items, deps, name) in [
        (
            "import mods:other/x@0.1.0; export api;",
            vec!["package mods:other@0.1.0; interface x { f: func(); }"],
            "mods:other/x@0.1.0",
        ),
        ("import surprise: func(); export api;", vec![], "surprise"),
        (
            "import alias: interface { f: func(); } export api;",
            vec![],
            "alias",
        ),
        (
            "import watane:host/caller@0.1.0; export api;",
            vec![],
            "watane:host/caller@0.1.0",
        ),
        (
            "import wasi:http/types@0.2.7; export api;",
            vec!["package wasi:http@0.2.7; interface types { f: func(); }"],
            "wasi:http/types@0.2.7",
        ),
    ] {
        put(dir.path(), "logic.wasm", component(items, &deps));
        let v = violations(&m);
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].message,
            format!("logic.wasm imports `{name}`, which wit/world.wit does not declare")
        );
    }
}

#[test]
fn ac4_base_wasi_packages_at_0_2_x_pass() {
    let dir = folder("export api;");
    let m = Mod::read(dir.path()).unwrap();
    for package in ["random", "cli", "clocks", "filesystem", "io", "sockets"] {
        for patch in [0, 7] {
            let interface = if package == "random" {
                "random"
            } else {
                "anything"
            };
            let dep = format!(
                "package wasi:{package}@0.2.{patch}; interface {interface} {{ f: func(); }}"
            );
            let items = format!("import wasi:{package}/{interface}@0.2.{patch}; export api;");
            put(dir.path(), "logic.wasm", component(&items, &[&dep]));
            assert!(m.check_component().is_ok(), "{package}@0.2.{patch}");
        }
    }
}

#[test]
fn ac4_other_wasi_versions_and_packages_are_refused() {
    let dir = folder("export api;");
    let m = Mod::read(dir.path()).unwrap();
    for name in [
        "wasi:random/random@0.3.0",
        "wasi:cli/run@1.2.0",
        "wasi:keyvalue/store@0.2.0",
    ] {
        let (package_interface, version) = name.split_once('@').unwrap();
        let (package, interface) = package_interface.split_once('/').unwrap();
        let dep = format!("package {package}@{version}; interface {interface} {{ f: func(); }}");
        put(
            dir.path(),
            "logic.wasm",
            component(&format!("import {name}; export api;"), &[&dep]),
        );
        let v = violations(&m);
        assert_eq!(v.len(), 1);
        assert_eq!(
            v[0].message,
            format!("logic.wasm imports `{name}`, which wit/world.wit does not declare")
        );
    }
}

#[test]
fn ac5_missing_and_extra_exports_each_have_one_violation() {
    let dir = folder("export api; export old;");
    put(
        dir.path(),
        "logic.wasm",
        component("export api; export extra;", &[]),
    );
    let v = violations(&Mod::read(dir.path()).unwrap());
    assert_eq!(v.len(), 2, "{v:?}");
    for name in ["mods:hello/old@0.1.0", "mods:hello/extra@0.1.0"] {
        assert_eq!(
            v.iter().filter(|v| v.message.contains(name)).count(),
            1,
            "{v:?}"
        );
    }
}

#[test]
fn ac6_truncated_components_never_panic() {
    let dir = folder("export api;");
    let m = Mod::read(dir.path()).unwrap();
    let bytes = component("export api;", &[]);
    // Force a truncated custom section after a complete component header.
    let malformed = [b"\0asm\x0d\0\x01\0".as_slice(), b"\0\x80"].concat();
    for truncated in [&bytes[..7], &bytes[..bytes.len() - 1], &malformed] {
        put(dir.path(), "logic.wasm", truncated);
        let result = std::panic::catch_unwind(|| violations(&m));
        let v = result.expect("malformed component must not panic");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].message, "logic.wasm is not a WebAssembly component");
    }
}

#[test]
fn subscriptions_are_expected_exports() {
    let other = "package mods:other@0.1.0; interface events { tick: func(); } world other { import events; }";
    let items = "export api; export mods:other/events@0.1.0;";
    let dir = folder(items);
    put(dir.path(), "wit/deps/other/events.wit", other);
    put(dir.path(), "deps.json", r#"{"mods:other": null}"#);
    put(dir.path(), "logic.wasm", component(items, &[other]));
    let m = Mod::read(dir.path()).unwrap();
    m.check_component().unwrap();
    put(dir.path(), "logic.wasm", component("export api;", &[]));
    let v = violations(&m);
    assert_eq!(v.len(), 1, "{v:?}");
    assert!(v[0].message.contains("mods:other/events@0.1.0"), "{v:?}");
}

#[test]
fn host_exports_are_expected_exports() {
    let items = "export watane:host/stream-sink@0.1.0;";
    let dir = folder(items);
    put(dir.path(), "logic.wasm", component(items, &[]));
    let m = Mod::read(dir.path()).unwrap();
    m.check_component().unwrap();
    put(dir.path(), "logic.wasm", component("export api;", &[]));
    let v = violations(&m);
    assert_eq!(v.len(), 2, "{v:?}");
    assert!(
        v.iter().any(|v| v.message
            == "logic.wasm does not export `watane:host/stream-sink@0.1.0`, which wit/world.wit declares"),
        "{v:?}"
    );
}

#[test]
fn unbuilt_logic_is_a_component_violation() {
    let dir = folder("export api;");
    let v = violations(&Mod::read(dir.path()).unwrap());
    assert_eq!(v.len(), 1, "{v:?}");
}
