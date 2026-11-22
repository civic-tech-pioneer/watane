#![allow(clippy::unwrap_used, reason = "tests may panic")]

#[test]
fn ac1_host_wit_is_the_canonical_package() {
    const EXPECTED: &str = "package watane:host@0.1.0;\n\n/// Private storage of this mod. The writes of a call become visible to\n/// other calls only when the call succeeds.\ninterface state {\n  get: func(key: string) -> option<list<u8>>;\n  put: func(key: string, value: list<u8>);\n  delete: func(key: string);\n}\n\n/// Who made the current call: \"page\", \"agent:<handle>\", \"mod:<local name>\" or \"host\".\ninterface caller {\n  who: func() -> string;\n  /// Step 11 (19.1): the caller's identity: \"<publisher>/<slug>\", \"local/<slug>\",\n  /// \"page\", \"agent:<handle-name>\" or \"host\". A pure read, no grant.\n  identity: func() -> string;\n}\n\n/// Step 10: the management of mods (W35). Importing it is the grant\n/// `manage`. Reads and reductions act when the call succeeds; anything\n/// more is a request that the participant decides in the consent view.\n/// A mod is named by its local name.\ninterface manage {\n  /// A grant as pkg names it. `provider` is \"<publisher key>/<slug>\" or \"local/<slug>\".\n  record provided { %interface: string, provider: string }\n  variant grant { storage, http(string), call(provided), subscribe(provided), manage, claude-code, claude-code-shell }\n  record grant-state { grant: grant, granted: bool }\n  enum link-kind { call, subscribe }\n  /// `provider` is the local name of the linked mod; none while unbound.\n  record link { kind: link-kind, %interface: string, provider: option<string>, provider-interface: option<string> }\n  /// Identities: the consumer, the provider that deps.json names, the picked mod.\n  record bind { consumer: string, provider: string, to: string }\n  record mod-info {\n    local: string, identity: string, slug: string, name: string, summary: string,\n    version: string, publisher: option<string>, release: option<string>, origin: option<string>,\n    enabled: bool, has-page: bool, has-logic: bool,\n    grants: list<grant-state>, links: list<link>, binds: list<bind>, error: option<string>,\n  }\n  /// `code`: bad_request, no_mod, not_declared, request_open or refused.\n  record failure { code: string, message: string }\n  variant request-state { pending, approved, denied, failed(string) }\n\n  /// Every installed mod, by local name.\n  mods: func() -> list<mod-info>;\n  /// Make things do less, at once.\n  disable: func(local: string) -> result<_, failure>;\n  withdraw: func(local: string, grant: grant) -> result<_, failure>;\n  close-page: func(local: string) -> result<_, failure>;\n  /// Open or focus the page of a mod (Q31). Gives no authority.\n  open-page: func(local: string) -> result<_, failure>;\n  /// Ask the participant. Each returns the id of the request.\n  request-install: func(release: string, link: option<string>) -> result<string, failure>;\n  request-update: func(local: string) -> result<string, failure>;\n  request-enable: func(local: string) -> result<string, failure>;\n  request-grant: func(local: string, grant: grant) -> result<string, failure>;\n  request-remove: func(local: string, delete-data: bool) -> result<string, failure>;\n  /// The state of a request of this mod; none for an unknown id.\n  request: func(id: string) -> option<request-state>;\n}\n\ninterface streams {                                  // no grant\n  type stream-id = string;                           // 128 random bits as 32 hex chars\n  record item { seq: u64, body: list<u8> }\n  enum sink-outcome { taken, defer }\n  record version-need { found: string, need: string }\n  /// How a stream ended. A producer's `close` may pass only `closed`, `replaced`, `lagging`\n  /// (the kernel's full owner stream, agentic-harness TECHNICAL 9.4) or `failed`; any other\n  /// case ends the stream `failed(\"<case> is the host's outcome\")`.\n  variant outcome {\n    closed, cancelled, lagging, replaced, consumer-failed, idle-timeout,\n    gone,                       // disable, removal or update of either side, lost grant or consent, closed page connection, app quit\n    mismatch(string),           // claude disagreed with the launch: the field (19.3)\n    no-binary, version(version-need),\n    consent-refused(string),    // what (19.7)\n    no-app, no-account,         // 19.4\n    failed(string),\n  }\n  enum stream-error { not-yours, unknown-stream, full, too-large }\n  open: func() -> result<stream-id, stream-error>;\n  push: func(s: stream-id, item: list<u8>) -> result<_, stream-error>;\n  close: func(s: stream-id, outcome: outcome);\n  cancel: func(s: stream-id);\n  hand-off: func(s: stream-id) -> result<_, stream-error>;\n  /// Consumer only: lets a deferred stream go on (19.2).\n  resume: func(s: stream-id) -> result<_, stream-error>;\n}\n\ninterface claude-types {                             // types only, no grant\n  record bad-launch { field: string, reason: string }\n  variant decision { allow(option<string>), deny(string), none }\n  /// `none` only for a check (19.3). `allow(some(json))`: a changed input.\n  variant start-error {\n    bad-launch(bad-launch), destination, forbidden-header(string), too-large, full,\n    not-owner, not-direct,   // the providers' (agentic-harness TECHNICAL 2.2, 10)\n  }\n  variant session-error {\n    not-yours, unknown-stream, slash-command, unknown-ask, not-allowed-here,\n    not-owner, not-direct,\n  }\n}\ninterface claude-code {                              // grant: ClaudeCode\n  use claude-types.{start-error};  use streams.{stream-id};\n  start: func(launch: string) -> result<stream-id, start-error>;\n}\ninterface claude-code-shell {                        // grant: ClaudeCodeShell\n  use claude-types.{start-error};  use streams.{stream-id};\n  start: func(launch: string) -> result<stream-id, start-error>;\n}\ninterface claude-session {                           // no grant; the caller is the stream's consumer\n  use claude-types.{decision, session-error};  use streams.{stream-id};\n  say: func(s: stream-id, content: string) -> result<_, session-error>;\n  answer: func(s: stream-id, ask: string, d: decision) -> result<_, session-error>;\n  interrupt: func(s: stream-id) -> result<_, session-error>;\n  compact: func(s: stream-id) -> result<_, session-error>;\n}\ninterface https-stream {                             // no grant; reaches only the hosts of the mod's Http grants\n  use claude-types.{start-error};  use streams.{stream-id};\n  start: func(request: string) -> result<stream-id, start-error>;\n}\ninterface agent-tools {                              // no grant; a toolset needs consent (19.7)\n  use streams.{stream-id};\n  record tool { name: string, description: string, input-schema: string, parallel: bool, idempotent: bool }\n  variant tools-error { not-granted, unknown-handle, unknown-tool, consent-pending(string), consent-refused(string) }\n  open: func(toolsets: list<string>) -> result<string, tools-error>;\n  describe: func(handle: string) -> result<list<tool>, tools-error>;\n  call: func(handle: string, key: string, tool: string, args: string) -> result<stream-id, tools-error>;\n  close: func(handle: string);\n}\ninterface timers {                                   // no grant\n  enum timer-error { too-many, bad-id, bad-delay }\n  after: func(id: string, delay-ms: u64) -> result<_, timer-error>;\n  cancel: func(id: string);\n}\n\n// host-only exports: a mod exports them to receive; no page, tool or mod can call them\ninterface stream-sink {\n  use streams.{item, sink-outcome, outcome};\n  items: func(s: string, items: list<item>) -> sink-outcome;\n  ended: func(s: string, outcome: outcome);\n}\ninterface stream-producer {\n  use streams.{outcome};\n  ready: func(s: string);\n  ended: func(s: string, outcome: outcome);\n}\ninterface timer-sink { fired: func(id: string); }\n";
    assert_eq!(watane_host::HOST_WIT, EXPECTED);
    assert_eq!(std::fs::read("wit/host.wit").unwrap(), EXPECTED.as_bytes());
}

use std::fs;
use std::path::{Path, PathBuf};
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
use std::sync::Arc;
use std::time::{Duration, Instant};
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
fn ac2_links_require_a_provider_on_preview_install() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    assert_eq!(
        host.install_folders(&[fixture("recs")], 0)
            .unwrap_err()
            .code(),
        "missing_providers"
    );
    assert_eq!(fs::read_dir(home.path().join("mods")).unwrap().count(), 0);
}
#[test]
fn ac11_fixtures_are_components_available_without_a_wasm_toolchain() {
    for name in [
        "counter", "echo", "hog", "shelf", "recs", "loop-a", "loop-b", "cyc-a", "cyc-b",
    ] {
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

#[tokio::test]
async fn ac4_s11_calls_and_ordered_exactly_once_events_commit_only_on_success() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["recs", "shelf"]);
    call(&host, "shelf", "items.add", json!({"title":"Dune"}))
        .await
        .unwrap();
    assert_eq!(
        call(&host, "recs", "api.received", json!(null))
            .await
            .unwrap(),
        json!(["Dune"])
    );
    assert_eq!(
        call(&host, "recs", "api.suggest", json!(null))
            .await
            .unwrap(),
        json!("Dune")
    );
    assert_eq!(
        call(&host, "shelf", "items.add-then-fail", json!(["lost"]))
            .await
            .unwrap_err()
            .code,
        CallCode::Trap
    );
    assert_eq!(
        call(&host, "recs", "api.received", json!(null))
            .await
            .unwrap(),
        json!(["Dune"])
    );
    let ordered_home = tempfile::tempdir().unwrap();
    let host = install(ordered_home.path(), Limits::default(), &["recs", "shelf"]);
    for title in ["A", "B", "C"] {
        call(&host, "shelf", "items.add", json!([title]))
            .await
            .unwrap();
    }
    assert_eq!(
        call(&host, "recs", "api.received", json!(null))
            .await
            .unwrap(),
        json!(["A", "B", "C"])
    );
}
#[tokio::test]
async fn ac5_s11_nested_caller_is_the_consumer() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["recs", "shelf"]);
    assert_eq!(
        call(&host, "recs", "api.who-of-shelf", json!(null))
            .await
            .unwrap(),
        json!("mod:recs")
    );
}
#[tokio::test]
async fn ac11_s11_restart_preserves_links_and_provider_state() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["recs", "shelf"]);
    call(&host, "shelf", "items.add", json!(["Dune"]))
        .await
        .unwrap();
    drop(host);
    let host = Host::open(home.path(), watane_host::Mode::Preview, Limits::default()).unwrap();
    assert_eq!(
        call(&host, "recs", "api.suggest", json!(null))
            .await
            .unwrap(),
        json!("Dune")
    );
}
#[tokio::test]
async fn ac7_s11_event_cycle_stops_after_eight_deliveries() {
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["loop-a", "loop-b"]);
    call(&host, "loop-a", "api.start", json!(null))
        .await
        .unwrap();
    assert_eq!(
        call(&host, "loop-a", "api.count", json!(null))
            .await
            .unwrap(),
        json!(4)
    );
    assert_eq!(
        call(&host, "loop-b", "api.count", json!(null))
            .await
            .unwrap(),
        json!(4)
    );
}

#[tokio::test]
async fn ac6_s11_depth_counts_nested_calls_only() {
    for depth in [1, 0] {
        let home = tempfile::tempdir().unwrap();
        let host = install(
            home.path(),
            Limits {
                depth,
                ..Limits::default()
            },
            &["recs", "shelf"],
        );
        let result = call(&host, "recs", "api.who-of-shelf", json!(null)).await;
        if depth == 1 {
            assert_eq!(result.unwrap(), json!("mod:recs"));
        } else {
            assert_eq!(result.unwrap_err().code, CallCode::Limit(Limit::Depth));
        }
    }
}

#[tokio::test]
async fn ac7_s11_delivery_budget_also_bounds_event_cycles() {
    let home = tempfile::tempdir().unwrap();
    let host = install(
        home.path(),
        Limits {
            hops: 20,
            deliveries: 3,
            ..Limits::default()
        },
        &["loop-a", "loop-b"],
    );
    call(&host, "loop-a", "api.start", json!(null))
        .await
        .unwrap();
    assert_eq!(
        call(&host, "loop-a", "api.count", json!(null))
            .await
            .unwrap(),
        json!(1)
    );
    assert_eq!(
        call(&host, "loop-b", "api.count", json!(null))
            .await
            .unwrap(),
        json!(2)
    );
}
#[test]
fn ac13_s11_fixture_slugs_are_valid() {
    for name in ["shelf", "recs", "loop-a", "loop-b", "cyc-a", "cyc-b"] {
        assert_eq!(
            name.parse::<watane_host::watane_pkg::Slug>()
                .unwrap()
                .to_string(),
            name
        );
    }
}

#[test]
fn ac1_validator_busy_mod_does_not_stall_another_mod() {
    let home = tempfile::tempdir().unwrap();
    let host = install(
        home.path(),
        Limits {
            time: Duration::from_millis(1500),
            fuel: u64::MAX,
            ..Limits::default()
        },
        &["hog", "counter"],
    );
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let spinner = Arc::clone(&host);
    let thread = std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || {
            started_tx.send(()).unwrap();
            spinner.call(
                &Caller::Page,
                &LocalName::parse("hog").unwrap(),
                "api.spin",
                Value::Null,
            )
        })
        .unwrap();
    started_rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let reader = Arc::clone(&host);
    let read = std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || {
            let start = Instant::now();
            let result = reader.call(
                &Caller::Page,
                &LocalName::parse("counter").unwrap(),
                "api.get",
                Value::Null,
            );
            (result, start.elapsed())
        })
        .unwrap()
        .join()
        .unwrap();
    let spin = thread.join().unwrap().unwrap_err();
    assert_eq!(spin.code, CallCode::Limit(Limit::Time));
    assert_eq!(read.0.unwrap(), json!(0));
    assert!(
        read.1 < Duration::from_millis(500),
        "counter waited {:?} for hog",
        read.1
    );
}

#[test]
fn ac5_s82_fixture_host_wit_copies_match_host_wit() {
    fn check(dir: &Path) -> usize {
        let mut count = 0;
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path.file_name().unwrap() != "target" {
                    count += check(&path);
                }
            } else if path.file_name().unwrap() == "host.wit" {
                assert_eq!(
                    fs::read_to_string(&path).unwrap(),
                    watane_host::HOST_WIT,
                    "{}",
                    path.display()
                );
                count += 1;
            }
        }
        count
    }
    assert!(check(Path::new("tests/fixtures")) > 0);
}
