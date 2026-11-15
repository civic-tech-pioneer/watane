#![cfg(feature = "driver")]
mod support;
use serde_json::{Value, json};
use std::time::Instant;
use support::{driver::App, journey::*, node::NodeProc, publish::*};

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
#[ignore = "slow: builds workspace watane-node and pairs its real process"]
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
    until(
        &light,
        "main",
        "document.querySelector('#identity-info').textContent.startsWith('This machine holds no identity.')",
    );
    assert_eq!(
        light.eval(
            "main",
            "document.querySelector('#identity-info').textContent"
        ),
        format!(
            "This machine holds no identity. It is a light machine of {}.",
            machines(&anchor)["node"].as_str().unwrap()
        )
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
    let anchor = start(a.path());
    let replica = start(b.path());
    pair_app(&anchor, &replica, "light");
    let short = machines(&replica)["node"].as_str().unwrap().to_owned();
    invoke(&anchor, "machine_role", json!({"node":short,"role":"full"}));
    invoke(&anchor, "machines_expiry", json!({"days":1}));
    wait_machines(&replica, |v| {
        v["expiresDays"] == 1 && v["machine"] == "full"
    });
    invoke(&anchor, "machine_remove", json!({"node":short}));
    let publishing = publish(&anchor, bytes());
    consent(&anchor);
    approve(&anchor);
    let published = publishing.join().unwrap();
    assert_eq!(published.0, 200);
    let end = Instant::now() + TIMEOUT;
    loop {
        let response = reqwest::blocking::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .post(format!("http://localhost:{}/api/fetch", replica.port))
            .json(&json!({"release":published.1["release"]}))
            .send()
            .unwrap();
        if !response.status().is_success() {
            break;
        }
        assert!(Instant::now() < end, "removed machine is still served");
    }
    assert!(replica.quit().success());
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
    invoke(&app, "relay_remove", json!({"url":url}));
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
    let api = include_str!("../src/api.rs");
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
    let mut changed = package;
    changed[0] ^= 1;
    std::fs::write(a.path().join("dist/packages").join(id), changed).unwrap();
    // A light machine asks the full one for this package; receiving its changed
    // bytes must report a refusal, even when fetch subsequently has no web base.
    begin(
        &light,
        "install_release",
        json!({"release":published.1["release"],"link":null}),
    );
    assert!(finish(&light).get("error").is_some());
    let short = machines(&full)["node"].as_str().unwrap().to_owned();
    let view = wait_machines(&light, |v| {
        v["problems"].as_array().unwrap().iter().any(|p| {
            p["text"]
                .as_str()
                .unwrap()
                .starts_with(&format!("refused {short}:"))
        })
    });
    assert!(
        view["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| !p["text"].as_str().unwrap().trim_end().ends_with(':'))
    );
    assert!(full.quit().success());
    // The former direct address is now unreachable and neither fallback is on.
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
                p["text"].as_str().unwrap().contains("relay")
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
