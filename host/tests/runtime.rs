#[test]
fn ac1_host_wit_is_the_canonical_package() {
    const EXPECTED: &str = "package watane:host@0.1.0;\n\n/// Private storage of this mod. The writes of a call become visible to\n/// other calls only when the call succeeds.\ninterface state {\n  get: func(key: string) -> option<list<u8>>;\n  put: func(key: string, value: list<u8>);\n  delete: func(key: string);\n}\n\n/// Who made the current call: \"page\", \"agent:<handle>\" or \"mod:<local name>\".\ninterface caller {\n  who: func() -> string;\n}\n";
    assert_eq!(watane_host::HOST_WIT, EXPECTED);
    assert_eq!(std::fs::read("wit/host.wit").unwrap(), EXPECTED.as_bytes());
}

use std::{
    fs,
    path::{Path, PathBuf},
};
use watane_host::{Host, Limits};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
fn copy_fixture(name: &str) -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    copy(&fixture(name), dir.path());
    dir
}
#[test]
fn ac2_install_checks_staged_component_and_grants_declarations() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let installed = host.install_folders(&[fixture("counter")], 1234).unwrap();
    assert!(installed[0].has_logic);
    assert_eq!(
        serde_json::to_value(&installed[0].grants).unwrap(),
        serde_json::json!([{"grant":{"kind":"storage"},"granted":true}])
    );
    assert!(home.path().join("mods/counter/logic.wasm").is_file());
    for bytes in [b"\0asm\x01\0\0\0".as_slice(), b"not wasm"] {
        let invalid = copy_fixture("counter");
        fs::write(invalid.path().join("logic.wasm"), bytes).unwrap();
        let expected = watane_pkg::Mod::read(invalid.path())
            .unwrap()
            .check_component()
            .unwrap_err()
            .to_string();
        let error = host
            .install_folders(&[invalid.path().into()], 1234)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_mod");
        assert_eq!(error.status(), 422);
        assert!(error.to_string().contains(&expected), "{error}");
    }
    let undeclared = copy_fixture("counter");
    let wit = undeclared.path().join("wit/world.wit");
    fs::write(
        &wit,
        fs::read_to_string(&wit)
            .unwrap()
            .replace("  import watane:host/state@0.1.0;\n", ""),
    )
    .unwrap();
    let expected = watane_pkg::Mod::read(undeclared.path())
        .unwrap()
        .check_component()
        .unwrap_err()
        .to_string();
    let error = host
        .install_folders(&[undeclared.path().into()], 1234)
        .unwrap_err();
    assert_eq!(error.status(), 422);
    assert!(error.to_string().contains(&expected), "{error}");
}

use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use watane_host::{CallCode, CallError, Caller, Limit, LocalName};
async fn call(
    host: &Arc<Host>,
    name: &str,
    function: &str,
    args: Value,
) -> Result<Value, CallError> {
    host.call_async(
        Caller::Page,
        LocalName::parse(name).unwrap(),
        function.into(),
        args,
    )
    .await
}
fn install(home: &Path, limits: Limits, names: &[&str]) -> Arc<Host> {
    let host = Host::open(home, watane_host::Mode::Preview, limits).unwrap();
    host.install_folders(&names.iter().map(|n| fixture(n)).collect::<Vec<_>>(), 0)
        .unwrap();
    host
}
#[tokio::test]
async fn ac3_success_commits_and_trap_rolls_back() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["counter"]);
    assert_eq!(
        call(&host, "counter", "api.add", json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    assert_eq!(
        call(&host, "counter", "api.add", json!([])).await.unwrap(),
        json!(2)
    );
    assert_eq!(
        call(&host, "counter", "api.get", json!({})).await.unwrap(),
        json!(2)
    );
    assert_eq!(
        call(&host, "counter", "api.add-then-fail", json!(null))
            .await
            .unwrap_err()
            .code,
        CallCode::Trap
    );
    assert_eq!(
        call(&host, "counter", "api.get", json!(null))
            .await
            .unwrap(),
        json!(2)
    );
    assert_eq!(
        call(&host, "counter", "api.who", json!(null))
            .await
            .unwrap(),
        json!("page")
    );
}
#[tokio::test]
async fn ac4_restart_persists_identity_table() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["counter"]);
    for expected in [1, 2] {
        assert_eq!(
            call(&host, "counter", "api.add", json!(null))
                .await
                .unwrap(),
            json!(expected)
        );
    }
    drop(host);
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    assert_eq!(
        call(&host, "counter", "api.get", json!(null))
            .await
            .unwrap(),
        json!(2)
    );
    drop(host);
    use redb::ReadableDatabase;
    let db = redb::Database::open(home.path().join("state/state.redb")).unwrap();
    let id = blake3::hash(b"local/counter").to_hex()[..32].to_owned();
    let txn = db.begin_read().unwrap();
    let name = format!("s/{id}");
    let table = txn
        .open_table(redb::TableDefinition::<&str, &[u8]>::new(&name))
        .unwrap();
    assert_eq!(table.get("count").unwrap().unwrap().value(), &[2, 0, 0, 0]);
}
#[tokio::test]
async fn ac5_state_is_private_and_reinstall_preserves_it() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["counter", "echo"]);
    call(&host, "counter", "api.add", json!(null))
        .await
        .unwrap();
    assert_eq!(
        call(&host, "echo", "api.read", json!({"key":"count"}))
            .await
            .unwrap(),
        json!(null)
    );
    call(
        &host,
        "echo",
        "api.put",
        json!({"key":"count","value":[99]}),
    )
    .await
    .unwrap();
    host.install_folders(&[fixture("counter")], 0).unwrap();
    assert_eq!(
        call(&host, "counter", "api.get", json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    assert_eq!(
        call(&host, "echo", "api.read", json!({"key":"count"}))
            .await
            .unwrap(),
        json!([99])
    );
    let update = copy_fixture("counter");
    fs::write(update.path().join("ui/app.js"), "// updated").unwrap();
    host.install_folders(&[update.path().into()], 0).unwrap();
    assert_eq!(
        call(&host, "counter", "api.get", json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    call(&host, "echo", "api.delete", json!(["count"]))
        .await
        .unwrap();
    assert_eq!(
        call(&host, "echo", "api.read", json!(["count"]))
            .await
            .unwrap(),
        json!(null)
    );
}
#[tokio::test]
#[ignore = "timing: run on a quiescent machine, see factory/README.md"]
async fn ac6_limits_stop_calls_and_next_instance_answers() {
    for (limits, function, args, limit, minimum, maximum) in [
        (
            Limits::default(),
            "api.spin",
            json!(null),
            Limit::Fuel,
            Duration::ZERO,
            Duration::from_secs(10),
        ),
        (
            Limits::default(),
            "api.grow",
            json!([2000]),
            Limit::Memory,
            Duration::ZERO,
            Duration::from_secs(10),
        ),
        (
            Limits {
                time: Duration::from_millis(200),
                fuel: u64::MAX,
                ..Limits::default()
            },
            "api.spin",
            json!(null),
            Limit::Time,
            Duration::from_millis(200),
            Duration::from_secs(2),
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let host = install(home.path(), limits, &["hog"]);
        let start = Instant::now();
        let err = call(&host, "hog", function, args).await.unwrap_err();
        let elapsed = start.elapsed();
        assert_eq!(err.code, CallCode::Limit(limit), "{err}");
        assert!(elapsed >= minimum && elapsed < maximum, "{elapsed:?}");
        assert_eq!(
            call(&host, "hog", "api.ping", json!(null)).await.unwrap(),
            json!("alive")
        );
    }
}
#[tokio::test]
async fn ac7_no_device_access_and_fresh_instance() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["hog"]);
    assert_eq!(
        call(&host, "hog", "api.probe", json!(null)).await.unwrap(),
        json!(["no-dir", "no-env", "no-socket"])
    );
    for _ in 0..2 {
        assert_eq!(
            call(&host, "hog", "api.fresh", json!(null)).await.unwrap(),
            json!(0)
        );
    }
}
#[tokio::test]
async fn ac9_call_errors_and_json_statuses() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["counter", "hello"]);
    for (name, function, expected) in [
        ("missing", "api.get", CallCode::NoMod),
        ("hello", "api.get", CallCode::NoFunction),
        ("counter", "absent.get", CallCode::NoFunction),
        ("counter", "api.absent", CallCode::NoFunction),
        ("counter", "bad", CallCode::NoFunction),
    ] {
        assert_eq!(
            call(&host, name, function, json!(null))
                .await
                .unwrap_err()
                .code,
            expected
        );
    }
    for (code, status, text, limit) in [
        (CallCode::BadRequest, 400, "bad_request", None),
        (CallCode::NoMod, 404, "no_mod", None),
        (CallCode::NoFunction, 404, "no_function", None),
        (CallCode::Disabled, 409, "disabled", None),
        (CallCode::NotGranted, 422, "not_granted", None),
        (CallCode::Trap, 422, "trap", None),
        (CallCode::Limit(Limit::Fuel), 422, "limit", Some("fuel")),
        (CallCode::Limit(Limit::Memory), 422, "limit", Some("memory")),
        (CallCode::Limit(Limit::Time), 422, "limit", Some("time")),
        (CallCode::Limit(Limit::Depth), 422, "limit", Some("depth")),
        (
            CallCode::Limit(Limit::StateSize),
            422,
            "limit",
            Some("state_size"),
        ),
        (
            CallCode::Limit(Limit::StateQuota),
            422,
            "limit",
            Some("state_quota"),
        ),
        (CallCode::Internal, 500, "internal", None),
    ] {
        let err = CallError {
            code,
            message: "cause".into(),
        };
        assert_eq!(err.status(), status);
        let expected = match limit {
            Some(limit) => json!({"code":text,"limit":limit,"message":"cause"}),
            None => json!({"code":text,"message":"cause"}),
        };
        assert_eq!(serde_json::to_value(err).unwrap(), expected);
    }
}
#[tokio::test]
async fn ac10_state_size_and_quota_fail_atomically() {
    for (limits, function, args, expected) in [
        (
            Limits {
                key_bytes: 3,
                ..Limits::default()
            },
            "api.put",
            json!(["long", [1]]),
            Limit::StateSize,
        ),
        (
            Limits {
                value_bytes: 1,
                ..Limits::default()
            },
            "api.put",
            json!(["k", [1, 2]]),
            Limit::StateSize,
        ),
        (
            Limits {
                call_write_bytes: 5,
                ..Limits::default()
            },
            "api.put-twice",
            json!(null),
            Limit::StateSize,
        ),
        (
            Limits {
                quota_bytes: 5,
                ..Limits::default()
            },
            "api.put-twice",
            json!(null),
            Limit::StateQuota,
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let host = install(home.path(), limits, &["echo"]);
        assert_eq!(
            call(&host, "echo", function, args).await.unwrap_err().code,
            CallCode::Limit(expected)
        );
        for key in ["long", "k", "a", "b"] {
            assert_eq!(
                call(&host, "echo", "api.read", json!([key])).await.unwrap(),
                json!(null)
            );
        }
    }
}

#[test]
fn ac2_links_between_mods_are_refused() {
    for direction in ["import", "export"] {
        let folder = copy_fixture("counter");
        let provider = folder.path().join("wit/deps/other");
        fs::create_dir_all(&provider).unwrap();
        fs::write(provider.join("world.wit"),format!("package mods:other@0.1.0; interface api {{ ping: func(); }} world other {{ {} api; }}",if direction=="export" {"import"} else {"export"})).unwrap();
        fs::write(folder.path().join("deps.json"), r#"{"mods:other":null}"#).unwrap();
        let wit = folder.path().join("wit/world.wit");
        fs::write(
            &wit,
            fs::read_to_string(&wit).unwrap().replace(
                "world counter {",
                &format!("world counter {{ {direction} mods:other/api@0.1.0;"),
            ),
        )
        .unwrap();
        let home = tempfile::tempdir().unwrap();
        let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
        let error = host
            .install_folders(&[folder.path().into()], 0)
            .unwrap_err();
        assert_eq!(error.code(), "invalid_mod");
        assert!(
            error
                .to_string()
                .contains("links between mods arrive in a later step"),
            "{error}"
        );
        assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
    }
}
#[test]
fn ac11_fixtures_are_components_available_without_a_wasm_toolchain() {
    for name in ["counter", "echo", "hog"] {
        let bytes = fs::read(fixture(name).join("logic.wasm")).unwrap();
        assert_eq!(&bytes[..8], b"\0asm\x0d\0\x01\0");
        watane_pkg::Mod::read(&fixture(name))
            .unwrap()
            .check_component()
            .unwrap();
    }
}

#[test]
fn ac2_invalid_core_code_is_refused_before_catalog_commit() {
    let folder = copy_fixture("counter");
    // Component with a core module whose function body contains an invalid opcode.
    // pkg reads the component world; Wasmtime must validate the unused core code.
    let bytes=b"\0asm\x0d\0\x01\0\x01\x19\0asm\x01\0\0\0\x01\x04\x01\x60\0\0\x03\x02\x01\0\x0a\x05\x01\x03\0\xff\x0b";
    let mut component = fs::read(folder.path().join("logic.wasm")).unwrap();
    component.extend_from_slice(&bytes[8..]);
    fs::write(folder.path().join("logic.wasm"), component).unwrap();
    watane_pkg::Mod::read(folder.path())
        .unwrap()
        .check_component()
        .unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let error = host
        .install_folders(&[folder.path().into()], 0)
        .unwrap_err();
    assert_eq!(error.code(), "invalid_mod");
    assert_eq!(error.status(), 422);
    assert!(!home.path().join("installed.json").exists());
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
}

#[test]
fn ac6_shared_memory_cannot_bypass_the_store_limiter() {
    let folder = copy_fixture("counter");
    let mut component = fs::read(folder.path().join("logic.wasm")).unwrap();
    // An unused core module with one shared memory (minimum and maximum: one page).
    component.extend_from_slice(b"\x01\x0e\0asm\x01\0\0\0\x05\x04\x01\x03\x01\x01");
    fs::write(folder.path().join("logic.wasm"), component).unwrap();
    watane_pkg::Mod::read(folder.path())
        .unwrap()
        .check_component()
        .unwrap();
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    let error = host
        .install_folders(&[folder.path().into()], 0)
        .unwrap_err();
    assert_eq!(error.code(), "invalid_mod");
}
