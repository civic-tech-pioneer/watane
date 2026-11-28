#![cfg(feature = "driver")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::driver::App;
use support::journey::*;
use support::node::NodeProc;
use support::publish::*;

fn start(home: &std::path::Path) -> App {
    let app = App::start(home, &[("WATANE_DRIVER_NET_BIND", "127.0.0.1:0")]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app
}
fn machines(app: &App) -> Value {
    invoke(app, "machines", json!({}))
}
fn on(app: &App) {
    invoke(app, "machines_on", json!({"on":true}));
}
fn code(app: &App, role: &str) -> String {
    invoke(app, "pair_code", json!({"role":role}))["code"]
        .as_str()
        .unwrap()
        .into()
}
fn ready_consent(app: &App, title: &str) {
    consent(app);
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        title
    );
}
fn wait_machines(app: &App, check: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + TIMEOUT;
    loop {
        let view = machines(app);
        if check(&view) {
            return view;
        }
        assert!(Instant::now() < end, "machines: {view}");
    }
}
fn pair_app(anchor: &App, replica: &App, role: &str) {
    on(anchor);
    on(replica);
    begin(
        replica,
        "machine_join",
        json!({"code":code(anchor,role),"replace":false}),
    );
    ready_consent(anchor, "Pair a machine");
    approve(anchor);
    assert!(finish(replica).get("error").is_none());
}
fn bytes() -> Vec<u8> {
    watane_host::watane_pkg::pack(&fixture("reading-list"), false)
        .unwrap()
        .into_bytes()
}

fn find_on(app: &App, reference: &Value, from: &str) -> Value {
    let end = Instant::now() + TIMEOUT;
    loop {
        let hits = invoke(
            app,
            "search",
            json!({"text":"reading","publisher":null,"interface":null,"origin":null}),
        );
        if let Some(hit) = hits.as_array().unwrap().iter().find(|hit| {
            hit["release"] == *reference
                && hit["verified"] == true
                && hit["from"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|key| key == from)
        }) {
            return hit.clone();
        }
        assert!(
            Instant::now() < end,
            "release {reference} not found through {from}: {hits}"
        );
    }
}
fn fetch(app: &App, reference: &Value) -> reqwest::blocking::Response {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(TIMEOUT)
        .build()
        .unwrap()
        .post(format!("http://localhost:{}/api/fetch", app.port))
        .json(&json!({"release":reference}))
        .send()
        .unwrap()
}
fn fetched(app: &App, reference: &Value, expected: &[u8]) {
    let response = fetch(app, reference);
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().unwrap().as_ref(), expected);
}

#[test]
fn ac1_s28_off_by_default_and_publish() {
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path());
    let view = machines(&app);
    assert_eq!(view["on"], false);
    // run.json's pid is the actual app process, not the test runner.
    let run: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("run.json")).unwrap()).unwrap();
    let pid = run["pid"].as_u64().expect("host records app pid");
    let udp = std::process::Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-i", "UDP"])
        .output()
        .unwrap();
    assert!(
        udp.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&udp.stderr)
    );
    assert!(
        udp.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&udp.stdout)
    );
    assert!(
        app.eval("main", "document.querySelector('#machines').textContent")
            .as_str()
            .unwrap()
            .contains(watane_node::words::net_on())
    );
    let publishing = publish(&app, bytes());
    consent(&app);
    approve(&app);
    assert_eq!(publishing.join().unwrap().0, 200);
    assert!(app.quit().success());
}

#[test]
#[ignore = "slow: only it proves the watane-node CLI side of S29: its printed short key, `status`, and exit 1 `declined` (builds watane-node)"]
fn ac2_s29_pair_full_deny_and_spent_code() {
    let home = tempfile::tempdir().unwrap();
    let full = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let app = start(home.path());
    on(&app);
    let first = code(&app, "full");
    let mut joining = NodeProc::spawn(full.path(), "join", &[&first]);
    let short = joining
        .until("this machine: ")
        .trim_start_matches("this machine: ")
        .to_owned();
    ready_consent(&app, "Pair a machine");
    let text = app.eval("consent", "document.querySelector('main').textContent");
    assert!(text.as_str().unwrap().contains(&short));
    assert!(text.as_str().unwrap().contains("full"));
    approve(&app);
    assert!(joining.exited().success());
    assert_eq!(machines(&app)["machines"].as_array().unwrap().len(), 2);
    let status = std::process::Command::new(support::node::watane_node_bin())
        .arg("--home")
        .arg(full.path())
        .arg("status")
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains(&short));
    assert!(
        String::from_utf8_lossy(&status.stdout).contains(machines(&app)["node"].as_str().unwrap())
    );
    let running = NodeProc::spawn(full.path(), "run", &[]);
    running.until("running as full");
    wait_machines(&app, |v| {
        v["machines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["node"] == short && m["online"] == true)
    });
    let next = code(&app, "full");
    let mut denied = NodeProc::spawn(other.path(), "join", &[&next]);
    denied.until("this machine: ");
    ready_consent(&app, "Pair a machine");
    app.close("consent");
    assert_eq!(denied.exited().code(), Some(1));
    assert!(denied.stderr().contains("declined"));
    assert_eq!(machines(&app)["machines"].as_array().unwrap().len(), 2);
    let reused = tempfile::tempdir().unwrap();
    let mut spent = NodeProc::spawn(reused.path(), "join", &[&first]);
    assert_eq!(spent.exited().code(), Some(1));
    assert!(app.quit().success());
}

#[test]
fn ac3_s30_join_light_and_refuse_used_identity() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let anchor = start(a.path());
    let light = start(b.path());
    let fingerprint = invoke(&light, "status", json!({}))["identity"]["fingerprint"].clone();
    on(&anchor);
    on(&light);
    area(&light, "#settings");
    light.eval("main",&format!("document.querySelector('#join-code').value={}; document.querySelector('#join').click(); null",json!(code(&anchor,"light"))));
    until(
        &light,
        "main",
        "document.querySelector('#join-question')?.open === true",
    );
    assert!(
        light
            .eval(
                "main",
                "document.querySelector('#join-question').textContent"
            )
            .as_str()
            .unwrap()
            .contains(fingerprint.as_str().unwrap())
    );
    assert_eq!(
        light.eval("main", "document.activeElement.id"),
        "join-cancel"
    );
    light.eval(
        "main",
        "document.querySelector('#join-confirm').click(); null",
    );
    ready_consent(&anchor, "Pair a machine");
    approve(&anchor);
    wait_machines(&light, |v| v["machine"] == "light");
    let expected_identity = format!(
        "This machine holds no identity. It is a light machine of {}.",
        machines(&anchor)["node"].as_str().unwrap()
    );
    until(
        &light,
        "main",
        &format!(
            "document.querySelector('#identity-info').textContent === {}",
            json!(expected_identity)
        ),
    );
    assert_eq!(
        light.eval(
            "main",
            "document.querySelector('#identity-info').textContent"
        ),
        expected_identity
    );
    assert!(!b.path().join("log/identity.key").exists());
    let (status, body) = http(light.port, "application/octet-stream", bytes());
    assert_eq!(status, 409);
    assert_eq!(body["error"]["code"], "no_identity");
    let used = start(c.path());
    let publishing = publish(&used, bytes());
    consent(&used);
    approve(&used);
    assert_eq!(publishing.join().unwrap().0, 200);
    let key = c.path().join("log/identity.key");
    let before = std::fs::read(&key).unwrap();
    on(&used);
    begin(
        &used,
        "machine_join",
        json!({"code":code(&anchor,"light"),"replace":false}),
    );
    let error = finish(&used);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("has signed at least one statement")
    );
    assert_eq!(std::fs::read(key).unwrap(), before);
    assert!(used.quit().success());
    assert!(light.quit().success());
    assert!(anchor.quit().success());
}

#[test]
fn ac4_s31_role_expiry_and_removal() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let anchor = start(a.path());
    let light = start(c.path());
    pair_app(&anchor, &light, "light");
    let package = bytes();
    let id = watane_host::watane_pkg::Package::from_bytes(package.clone())
        .unwrap()
        .id()
        .to_string();
    let publishing = publish(&anchor, package.clone());
    consent(&anchor);
    approve(&anchor);
    let (status, published) = publishing.join().unwrap();
    assert_eq!(status, 200);
    let full = start(b.path());
    // Its initial sync copies the existing release and package.
    pair_app(&anchor, &full, "full");
    let full_key = machines(&full)["node"].as_str().unwrap().to_owned();
    let light_key = machines(&light)["node"].as_str().unwrap().to_owned();
    let end = Instant::now() + TIMEOUT;
    while !b.path().join("dist/packages").join(&id).is_file() {
        assert!(
            Instant::now() < end,
            "full machine did not copy the package"
        );
        std::thread::yield_now();
    }
    invoke(
        &anchor,
        "machine_role",
        json!({"node":full_key,"role":"light"}),
    );
    invoke(&anchor, "machines_expiry", json!({"days":1}));
    for app in [&anchor, &full, &light] {
        wait_machines(app, |view| {
            view["expiresDays"] == 1
                && view["machines"].as_array().unwrap().len() == 3
                && view["machines"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["node"] == full_key && m["role"] == "light")
        });
    }
    // Restore the serving role and check all three views again.
    invoke(
        &anchor,
        "machine_role",
        json!({"node":full_key,"role":"full"}),
    );
    for app in [&anchor, &full, &light] {
        wait_machines(app, |view| {
            view["expiresDays"] == 1
                && view["machines"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|m| m["node"] == full_key && m["role"] == "full")
        });
    }
    find_on(&light, &published["release"], &full_key);
    invoke(&anchor, "machines_on", json!({"on":false}));
    // No web base, no local package, and the anchor is off: only full can serve.
    let cached = c.path().join("dist/packages").join(&id);
    assert!(!cached.exists());
    fetched(&light, &published["release"], &package);
    assert!(cached.is_file());

    // Keep the light home on its old list, so its next fetch must ask full;
    // a local Removed state must not make this a vacuous refusal.
    invoke(&light, "machines_on", json!({"on":false}));
    std::fs::remove_file(&cached).unwrap();
    on(&anchor);
    let removed = invoke(&anchor, "machine_remove", json!({"node":light_key}));
    wait_machines(&full, |view| {
        view["seq"] == removed["seq"]
            && view["machines"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["node"] != light_key)
    });
    assert!(
        removed["machines"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["node"] != light_key)
    );
    invoke(&anchor, "machines_on", json!({"on":false}));
    assert_eq!(machines(&full)["on"], true);
    on(&light);
    assert_eq!(machines(&light)["machine"], "light");
    let response = fetch(&light, &published["release"]);
    assert_eq!(response.status(), 502);
    assert_eq!(
        response.json::<Value>().unwrap()["error"]["code"],
        "fetch_failed"
    );
    assert_eq!(machines(&light)["machine"], "light");
    assert!(
        !cached.exists(),
        "removed light machine received the package"
    );
    assert!(light.quit().success());
    assert!(full.quit().success());
    assert!(anchor.quit().success());
}

#[test]
fn ac5_s32_relay_dht_consent_and_persistence() {
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path());
    let url = "https://relay.example.org/";
    begin(&app, "relay_add", json!({"url":url}));
    ready_consent(&app, "Use a relay");
    assert!(
        app.eval("consent", "document.querySelector('main').textContent")
            .as_str()
            .unwrap()
            .contains(&watane_node::words::relay_add(url))
    );
    deny(&app);
    assert_eq!(finish(&app)["error"]["code"], "refused");
    assert_eq!(machines(&app)["relays"], json!([]));
    begin(&app, "relay_add", json!({"url":url}));
    ready_consent(&app, "Use a relay");
    approve(&app);
    assert!(finish(&app).get("error").is_none());
    assert_eq!(machines(&app)["relays"], json!([url]));
    assert_eq!(
        invoke(&app, "relay_remove", json!({"url":url}))["relays"],
        json!([])
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    begin(&app, "dht_set", json!({"on":true}));
    ready_consent(&app, "Turn on the DHT");
    assert!(
        app.eval("consent", "document.querySelector('main').textContent")
            .as_str()
            .unwrap()
            .contains("Mainline DHT")
    );
    approve(&app);
    assert_eq!(finish(&app)["ok"]["dht"], true);
    let book: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("net/book.json")).unwrap()).unwrap();
    assert_eq!(book["dht"], true);
    assert_eq!(invoke(&app, "dht_set", json!({"on":false}))["dht"], false);
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert!(app.quit().success());
}

#[test]
fn ac7_s34_light_search_install_runs_mod() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let full = start(a.path());
    let light = start(b.path());
    let publishing = publish(&full, bytes());
    consent(&full);
    approve(&full);
    assert_eq!(publishing.join().unwrap().0, 200);
    pair_app(&full, &light, "light");
    let end = Instant::now() + TIMEOUT;
    let hit = loop {
        let hits = invoke(
            &light,
            "search",
            json!({"text":"reading","publisher":null,"interface":null,"origin":null}),
        );
        if let Some(hit) = hits.as_array().unwrap().first() {
            break hit.clone();
        }
        assert!(Instant::now() < end);
    };
    assert_eq!(hit["from"], json!([machines(&full)["node"]]));
    assert_eq!(hit["verified"], true);
    begin(
        &light,
        "install_release",
        json!({"release":hit["release"],"link":null}),
    );
    consent(&light);
    approve(&light);
    assert!(finish(&light).get("error").is_none());
    invoke(&light, "mod_open", json!({"local":"reading-list"}));
    assert!(light.wait("mod-reading-list", TIMEOUT));
    light.eval("mod-reading-list", "document.querySelector('#title').value='Paired books'; document.querySelector('#add').click(); null");
    until(
        &light,
        "mod-reading-list",
        "document.querySelector('#books').textContent.includes('Paired books')",
    );
    assert!(light.quit().success());
    assert!(full.quit().success());
}

#[test]
fn ac9_s36_no_net_routes() {
    let api = include_str!("../../src/api.rs");
    for route in api.split(".route(").skip(1) {
        let path = route.trim_start().split('"').nth(1).unwrap();
        for word in ["machine", "relay", "dht", "pair", "net"] {
            assert!(!path.contains(word), "{path}");
        }
    }
}

#[test]
fn ac6_s33_corrupt_full_machine_and_no_path_name_causes() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let full = start(a.path());
    let light = start(b.path());
    let package = bytes();
    let id = watane_host::watane_pkg::Package::from_bytes(package.clone())
        .unwrap()
        .id()
        .to_string();
    let publishing = publish(&full, package.clone());
    consent(&full);
    approve(&full);
    let published = publishing.join().unwrap();
    assert_eq!(published.0, 200);
    pair_app(&full, &light, "light");
    let short = machines(&full)["node"].as_str().unwrap().to_owned();
    let hit = find_on(&light, &published.1["release"], &short);
    // Warm the full machine's blob store with the verified file, then change
    // its referenced bytes. A corrupt file on first admission is refused by
    // the serving holder before it ever sends bytes; that is a different case.
    fetched(&light, &hit["release"], &package);
    std::fs::remove_file(b.path().join("dist/packages").join(&id)).unwrap();
    let mut changed = package;
    changed[0] ^= 1;
    std::fs::write(a.path().join("dist/packages").join(&id), changed).unwrap();
    // The honest holder resets the stream after checking its changed file.
    // Depending on event order, this is a hash refusal or a not-served cause.
    begin(
        &light,
        "install_release",
        json!({"release":published.1["release"],"link":null}),
    );
    // This command enters Core::install_resolved -> Node::fetch ->
    // Node::need_release -> Net::need_blob. The macOS CI log proves only that
    // the UI command outlasted 30 s; it does not identify a product timeout.
    // Keep #105 open until that cause is observed on a macOS runner.
    let deadline = Instant::now() + Duration::from_secs(60);
    while !light.is_true("main", "window.__stageC !== null") {
        assert!(
            Instant::now() < deadline,
            "corrupt-peer install did not finish; last result: {}",
            light.eval("main", "window.__stageC ?? null")
        );
        std::thread::yield_now();
    }
    assert!(light.eval("main", "window.__stageC").get("error").is_some());
    wait_machines(&light, |v| {
        v["problems"].as_array().unwrap().iter().any(|p| {
            let text = p["text"].as_str().unwrap();
            p["node"] == short
                && ((text.starts_with(&format!("refused {short}:"))
                    && text.contains("does not match its hash"))
                    || (text.starts_with(&format!("unreachable {short}:"))
                        && text.contains("not served")))
        })
    });
    assert!(!b.path().join("dist/packages").join(&id).exists());
    assert!(full.quit().success());
    assert!(light.quit().success());
    // A fresh session clears the deliberate bad-peer drop. The paired full
    // machine is now offline, so its saved direct address has no path and
    // neither fallback is on. Do not reuse a session that must ignore it.
    let light = start(b.path());
    let fresh = machines(&light);
    assert_eq!(fresh["on"], true);
    assert_eq!(fresh["dht"], false);
    assert_eq!(fresh["relays"], json!([]));
    let end = Instant::now() + TIMEOUT;
    loop {
        let _ = invoke(
            &light,
            "search",
            json!({"text":"reading","publisher":null,"interface":null,"origin":null}),
        );
        if machines(&light)["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| {
                p["node"] == short
                    && p["text"]
                        .as_str()
                        .unwrap()
                        .starts_with(&format!("unreachable {short}:"))
                    && p["text"].as_str().unwrap().contains("relay")
                    && p["text"].as_str().unwrap().contains("DHT")
            })
        {
            break;
        }
        assert!(Instant::now() < end, "no reachability cause");
    }
    assert!(light.quit().success());
}

#[test]
fn ac8_s35_restore_confirm_machine_hint_and_next_sequence() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let c = tempfile::tempdir().unwrap();
    let old = start(a.path());
    let server = start(b.path());
    pair_app(&old, &server, "full");
    let publishing = publish(&old, bytes());
    consent(&old);
    approve(&old);
    let published = publishing.join().unwrap();
    assert_eq!(published.0, 200);
    let end = Instant::now() + TIMEOUT;
    loop {
        let hits = invoke(
            &server,
            "search",
            json!({"text":"reading","publisher":null,"interface":null,"origin":null}),
        );
        if !hits.as_array().unwrap().is_empty() {
            break;
        }
        assert!(Instant::now() < end);
    }
    let backup = a.path().join("identity.backup");
    old.pick(std::slice::from_ref(&backup));
    invoke(
        &old,
        "identity_backup",
        json!({"passphrase":"twelve character passphrase"}),
    );
    assert!(old.quit().success());
    let new = start(c.path());
    new.pick(&[backup]);
    invoke(
        &new,
        "identity_restore",
        json!({"passphrase":"twelve character passphrase"}),
    );
    on(&new);
    begin(
        &server,
        "machine_join",
        json!({"code":code(&new,"full"),"replace":true}),
    );
    ready_consent(&new, "Pair a machine");
    approve(&new);
    assert!(finish(&server).get("error").is_none());
    let end = Instant::now() + TIMEOUT;
    let hint = loop {
        let hints = invoke(&new, "own_hints", json!({}));
        if let Some(h) = hints
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["seq"].is_number())
        {
            break h.clone();
        }
        assert!(Instant::now() < end);
    };
    area(&new, "#settings");
    let restored = invoke(&new, "status", json!({}));
    until(
        &new,
        "main",
        &format!(
            "document.querySelector('#identity-info').textContent.includes({})",
            restored["identity"]["fingerprint"]
        ),
    );
    until(
        &new,
        "main",
        "document.querySelector('#own-hints').textContent.includes('position')",
    );
    assert!(
        new.eval("main", "document.querySelector('#own-hints').textContent")
            .as_str()
            .unwrap()
            .contains(hint["node"].as_str().unwrap())
    );
    let (status, _) = http(
        new.port,
        "application/octet-stream",
        update_package("0.2.0", false),
    );
    assert_ne!(status, 200);
    invoke(
        &new,
        "identity_confirm_hint",
        json!({"node":hint["node"],"seq":hint["seq"],"id":hint["id"]}),
    );
    let publishing = publish(&new, update_package("0.2.0", false));
    consent(&new);
    approve(&new);
    assert_eq!(publishing.join().unwrap().0, 200);
    let response = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://localhost:{}/api/releases", new.port))
        .send()
        .unwrap()
        .json::<Value>()
        .unwrap();
    assert_eq!(
        response["releases"].as_array().unwrap().last().unwrap()["seq"],
        hint["seq"].as_u64().unwrap() + 1
    );
    assert!(new.quit().success());
    assert!(server.quit().success());
}

#[test]
fn ac9_s36_hostile_stage_g_commands_are_all_refused() {
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path());
    begin_install(&app, &[fixture("hostile")]);
    approve(&app);
    installed(&app, "hostile");
    invoke(&app, "mod_open", json!({"local":"hostile"}));
    assert!(app.wait("mod-hostile", TIMEOUT));
    until(&app, "mod-hostile", "window.__report?.done === true");
    let report = app.eval("mod-hostile", "window.__report");
    for command in [
        "machines",
        "machines_on",
        "pair_code",
        "machine_join",
        "machine_remove",
        "machine_role",
        "machines_expiry",
        "relay_add",
        "relay_remove",
        "dht_set",
        "own_hints",
        "identity_confirm_hint",
    ] {
        let attack = report["attacks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == format!("invoke:{command}"))
            .unwrap();
        assert_eq!(attack["outcome"], "refused", "{attack}");
    }
    assert!(
        app.log()["commands"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["label"] != "mod-hostile")
    );
    assert!(app.quit().success());
}

#[test]
fn ac8_settings_navigation_refreshes_hidden_page() {
    let output = std::process::Command::new("node")
        .args(["-e", r#"
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const source = fs.readFileSync('ui/app.js', 'utf8');
const route = source.slice(source.indexOf('function route() {'), source.indexOf("\nwindow.addEventListener('hashchange', route);"));
const nodes = new Map();
const $ = selector => {
  if (!nodes.has(selector)) nodes.set(selector, {hidden:false, setAttribute() {}, removeAttribute() {}});
  return nodes.get(selector);
};
let refreshes = 0;
const context = { $, location: {hash:'#mods'}, managerTab:'library', startupFailed:false, closeAdd() {}, headerView() {},
  document: {hidden:true}, readRefresh: () => {refreshes++; return Promise.resolve();},
  showError: error => {throw error;} };
vm.createContext(context);
vm.runInContext(route, context);
context.route();
assert.equal(refreshes, 0);
context.location.hash = '#settings'; context.route();
assert.equal($('#settings').hidden, false);
assert.equal(refreshes, 1, 'showing Settings must refresh even in a hidden window');
context.location.hash = '#mods'; context.route();
context.location.hash = '#settings'; context.route();
assert.equal(refreshes, 2, 'returning to Settings must read the latest identity and hints');
context.startupFailed = true;
context.route();
assert.equal($('#settings').hidden, true);
assert.equal($('#library').hidden, false);
assert.equal(refreshes, 2);
"#])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// S75 acceptance: a home whose identity signed a release (here the anchor
// itself) clicks Join. APP-F-54 asks the "unused identity" question only of a
// home that signed nothing; this one must be refused with the cause, never
// told that its identity "has signed nothing" and will be deleted.
#[test]
fn regress_s75_join_question_on_signed_home() {
    let home = tempfile::tempdir().unwrap();
    let app = start(home.path());
    let publishing = publish(&app, bytes());
    consent(&app);
    approve(&app);
    assert_eq!(publishing.join().unwrap().0, 200);
    on(&app);
    area(&app, "#settings");
    app.eval(
        "main",
        "document.querySelector('#join-code').value = 'watane-pair:x'; document.querySelector('#join').click(); null",
    );
    until(
        &app,
        "main",
        "document.querySelector('#join-question').open || document.querySelector('#error').textContent !== ''",
    );
    let question = app.eval(
        "main",
        "document.querySelector('#join-question').open ? document.querySelector('#join-question').textContent : ''",
    );
    assert!(
        !question.as_str().unwrap().contains("has signed nothing"),
        "a home with a published release is told: {question}"
    );
    assert!(app.quit().success());
}

fn wording_test(criterion: &str) {
    let output = std::process::Command::new("node")
        .args(["tests/support/machines.cjs", criterion])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn ac1_s81_machine_line() {
    wording_test("ac1");
}
#[test]
fn ac2_s81_this_machine_row() {
    wording_test("ac2");
}
#[test]
fn ac3_s81_join_disables_before_refresh() {
    wording_test("ac3");
}
#[test]
fn ac4_s81_reach_controls_follow_switch() {
    wording_test("ac4");
}

#[test]
fn ac5_s81_consent_renders_first_signature_line() {
    let output = std::process::Command::new("node")
        .args(["tests/support/consent_dom.mjs", "sign_first"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ac7_s81_specs_record_machine_wording() {
    let technical = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("TECHNICAL.md"),
    )
    .unwrap()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");
    for text in [
        "This machine: <short> · anchor",
        "This machine: <short> · <full|light> of <anchor short>",
        "This machine: <short> · alone",
        "After this, this home cannot join your other machines.",
        "Confirm the restored own head before signing: in Settings, identity, choose From your machines, or restore again.",
    ] {
        assert!(technical.contains(text), "missing wording: {text}");
    }
}
