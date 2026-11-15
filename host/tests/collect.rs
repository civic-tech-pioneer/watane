use redb::ReadableDatabase;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use watane_host::{
    CallCode, Caller, EventRecord, Fakes, Harness, Host, Limit, Limits, LocalName, Mode, Source,
    watane_pkg::Grant,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}
fn local(name: &str) -> LocalName {
    LocalName::parse(name).unwrap()
}
fn install(home: &Path, limits: Limits, names: &[&str]) -> Arc<Host> {
    let host = Host::open(home, Mode::Preview, limits).unwrap();
    let folders: Vec<_> = names.iter().map(|n| fixture(n)).collect();
    host.install_folders(&folders, 0).unwrap();
    host
}
fn call(host: &Arc<Host>, name: &str, function: &str, args: Value) -> Result<Value, String> {
    let host = host.clone();
    let (name, function) = (local(name), function.to_owned());
    std::thread::Builder::new()
        .stack_size(16 << 20)
        .spawn(move || {
            host.call(&Caller::Page, &name, &function, args)
                .map_err(|e| format!("{:?}: {}", e.code, e.message))
        })
        .unwrap()
        .join()
        .unwrap()
}
fn withhold_subscribe(host: &Host, name: &str) {
    let grant = host
        .get(&local(name))
        .unwrap()
        .grants
        .into_iter()
        .find(|g| matches!(g.grant, Grant::Subscribe { .. }))
        .unwrap()
        .grant;
    host.revoke(&local(name), &grant).unwrap();
}
/// Whether the state of `local/<slug>` holds `key`. Closes the host first.
fn has_key(home: &Path, slug: &str, key: &str) -> bool {
    let db = redb::Database::open(home.join("state/state.redb")).unwrap();
    let id = blake3::hash(format!("local/{slug}").as_bytes()).to_hex()[..32].to_owned();
    let name = format!("s/{id}");
    let txn = db.begin_read().unwrap();
    match txn.open_table(redb::TableDefinition::<&str, &[u8]>::new(&name)) {
        Ok(table) => table.get(key).unwrap().is_some(),
        Err(_) => false,
    }
}

#[test]
fn ac1_s36_collect_joins_granted_subscribers_in_name_order() {
    let home = tempfile::tempdir().unwrap();
    let host = install(
        home.path(),
        Limits::default(),
        &["menu", "item-c", "item-b", "item-a"],
    );
    withhold_subscribe(&host, "item-c");
    assert_eq!(
        call(&host, "menu", "api.items", json!(null)).unwrap(),
        json!(["a", "b"])
    );
    drop(host);
    assert!(has_key(home.path(), "item-a", "asked"));
    assert!(has_key(home.path(), "item-b", "asked"));
    assert!(!has_key(home.path(), "item-c", "asked"));
}

fn failed_answer_scenario() {
    let home = tempfile::tempdir().unwrap();
    let host = install(
        home.path(),
        Limits::default(),
        &["menu", "item-a", "item-b", "item-bad"],
    );
    assert_eq!(
        call(&host, "menu", "api.items", json!(null)).unwrap(),
        json!(["a", "b"])
    );
    drop(host);
    assert!(has_key(home.path(), "item-a", "asked"));
    assert!(!has_key(home.path(), "item-bad", "key"));
}

#[test]
fn ac2_s37_failed_answer_is_left_out_and_logged() {
    if std::env::var_os("WATANE_COLLECT_CHILD").is_some() {
        failed_answer_scenario();
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ac2_s37_failed_answer_is_left_out_and_logged",
            "--nocapture",
        ])
        .env("WATANE_COLLECT_CHILD", "1")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains(
            "watane: collect mods:menu/menu@0.1.0.ask of menu from item-bad failed: Trap: "
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("from item-a"), "{stderr}");
}

#[test]
fn ac2_s37_spinning_answer_fails_the_collector_with_a_limit() {
    let home = tempfile::tempdir().unwrap();
    let host = install(
        home.path(),
        Limits::default(),
        &["menu", "item-a", "item-spin"],
    );
    let error = host
        .call(&Caller::Page, &local("menu"), "api.items", json!(null))
        .unwrap_err();
    assert!(
        matches!(
            error.code,
            CallCode::Limit(Limit::Time) | CallCode::Limit(Limit::Fuel)
        ),
        "{error:?}"
    );
}

#[test]
fn ac3_s38_collect_edge_makes_a_cycle() {
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Mode::Preview, Limits::default()).unwrap();
    let error = host
        .plan(vec![
            Source::Folder(fixture("menu")),
            Source::Folder(fixture("item-back")),
        ])
        .unwrap_err();
    assert_eq!(error.code(), "cycle");
    let message = error.to_string();
    assert!(
        message.contains("menu") && message.contains("item-back"),
        "{message}"
    );
}

#[test]
fn ac4_s39_harness_collect_is_the_fake_or_empty() {
    let harness = Harness::open(&fixture("menu"), Fakes::new()).unwrap();
    assert_eq!(
        harness.call("api.items", json!({})).unwrap().value,
        json!([])
    );
    let fakes: Fakes = [(
        "mods:menu/menu".to_owned(),
        [("ask".to_owned(), json!(["x"]))].into(),
    )]
    .into();
    let harness = Harness::open(&fixture("menu"), fakes).unwrap();
    assert_eq!(
        harness.call("api.items", json!({})).unwrap().value,
        json!(["x"])
    );
    let harness = Harness::open(&fixture("item-a"), Fakes::new()).unwrap();
    assert_eq!(
        harness.deliver("menu.ask", json!({})).unwrap().value,
        json!(["a"])
    );
}

#[test]
fn ac5_clicked_stays_an_event() {
    let harness = Harness::open(&fixture("menu"), Fakes::new()).unwrap();
    let outcome = harness.call("api.click", json!({"id": "x"})).unwrap();
    assert_eq!(
        outcome.events,
        vec![EventRecord {
            interface: "mods:menu/menu@0.1.0".parse().unwrap(),
            function: "clicked".into(),
            args: json!({"id": "x"}),
        }]
    );
    let home = tempfile::tempdir().unwrap();
    let host = install(home.path(), Limits::default(), &["menu", "item-a"]);
    call(&host, "menu", "api.click", json!({"id": "x"})).unwrap();
    drop(host);
    assert!(has_key(home.path(), "item-a", "clicked"));
    assert!(!has_key(home.path(), "item-a", "asked"));
}

#[test]
fn ac2_s37_failed_answer_keeps_its_fuel_spent() {
    // item-bad burns about 19.5M fuel and then traps. At this limit it just
    // fails, and the 3k fuel left is too little for item-c's answer. If the
    // failed answer gave its fuel back, the list `["c"]` would return.
    let home = tempfile::tempdir().unwrap();
    let limits = Limits {
        fuel: 19_508_000,
        ..Limits::default()
    };
    let host = install(home.path(), limits, &["menu", "item-bad", "item-c"]);
    let error = host
        .call(&Caller::Page, &local("menu"), "api.items", json!(null))
        .unwrap_err();
    assert_eq!(error.code, CallCode::Limit(Limit::Fuel), "{error:?}");
}
