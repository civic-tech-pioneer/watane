#![cfg(feature = "driver")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};
use support::driver::App;
use support::journey::*;
use support::node::NodeProc;
use support::publish::*;

fn start(home: &Path) -> App {
    let app = App::start(home, &[("WATANE_DRIVER_NET_BIND", "127.0.0.1:0")]);
    assert!(app.wait("main", TIMEOUT));
    until(&app, "main", "document.body.dataset.ready === 'true'");
    app
}
fn machines(app: &App) -> Value {
    invoke(app, "machines", json!({}))
}
fn on(app: &App, on: bool) {
    invoke(app, "machines_on", json!({"on":on}));
}
fn code(app: &App, role: &str) -> String {
    invoke(app, "pair_code", json!({"role":role}))["code"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn pair_consent(app: &App, short: &str, role: &str) {
    consent(app);
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Pair a machine"
    );
    let text = app.eval("consent", "document.querySelector('main').textContent");
    assert!(text.as_str().unwrap().contains(short), "{text}");
    assert!(text.as_str().unwrap().contains(role), "{text}");
    approve(app);
}
fn wait_machines(app: &App, check: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + TIMEOUT;
    loop {
        let view = machines(app);
        if check(&view) {
            return view;
        }
        assert!(Instant::now() < end, "machines: {view}");
        std::thread::yield_now();
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
fn find_on(app: &App, reference: &Value, from: &str) -> Value {
    let end = Instant::now() + TIMEOUT;
    loop {
        let hits = invoke(
            app,
            "search",
            json!({"text":"reading","publisher":null,"interface":null,"origin":null}),
        );
        if let Some(hit) = hits.as_array().unwrap().iter().find(|h| {
            h["release"] == *reference
                && h["verified"] == true
                && h["from"].as_array().unwrap().iter().any(|n| n == from)
        }) {
            return hit.clone();
        }
        assert!(
            Instant::now() < end,
            "AC4: no verified hit from {from}: {hits}"
        );
    }
}
fn no_public_network(home: &Path) {
    let path = home.join("net/book.json");
    let book: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(book["relays"], json!([]), "AC8: {}", path.display());
    assert_eq!(book["dht"], false, "AC8: {}", path.display());
}

#[test]
#[ignore = "slow: J8 on four loopback homes and the real watane-node CLI (builds watane-node)"]
fn ac1_journey_j8() {
    let alice_home = tempfile::tempdir().unwrap();
    let server_home = tempfile::tempdir().unwrap();
    let alice = start(alice_home.path());
    on(&alice, true);
    let first_code = code(&alice, "full");
    let mut joining = NodeProc::spawn(server_home.path(), "join", &[&first_code]);
    let server_key = joining
        .until("this machine: ")
        .trim_start_matches("this machine: ")
        .to_owned();
    pair_consent(&alice, &server_key, "full");
    let joined = joining.until("joined as full; machines: ");
    assert!(
        joined.contains(machines(&alice)["node"].as_str().unwrap()),
        "{joined}"
    );
    assert!(joined.contains(&server_key), "{joined}");
    assert!(joining.exited().success());
    let server = NodeProc::spawn(server_home.path(), "run", &[]);
    assert!(server.until("running as full, node ").contains(&server_key));
    wait_machines(&alice, |v| {
        v["machines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["node"] == server_key && m["role"] == "full" && m["online"] == true)
    });
    // AC2: a spent code fails in a fresh home, without another consent.
    let refused_home = tempfile::tempdir().unwrap();
    let mut reused = NodeProc::spawn(refused_home.path(), "join", &[&first_code]);
    assert_eq!(reused.exited().code(), Some(1));
    assert!(!alice.windows().iter().any(|w| w["label"] == "consent"));

    // AC3: no Site (nor any static HTTP server) has been started. The only
    // listeners so far belong to the actual app and node programs.
    let backup = alice_home.path().join("alice.backup");
    alice.pick(std::slice::from_ref(&backup));
    invoke(
        &alice,
        "identity_backup",
        json!({"passphrase":"twelve words long enough"}),
    );
    let identity = invoke(&alice, "status", json!({}))["identity"]["fingerprint"].clone();
    let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
    let bytes = package.as_bytes().to_vec();
    let package_id = package.id().to_string();
    let server_package = server_home.path().join("dist/packages").join(&package_id);
    let pending = publish(&alice, bytes.clone());
    consent(&alice);
    let deadline = Instant::now() + Duration::from_secs(5);
    approve(&alice);
    let (status, published) = pending.join().unwrap();
    assert_eq!(status, 200, "AC3: {published}");
    loop {
        assert!(
            Instant::now() < deadline,
            "AC3: server did not hold byte-equal package within 5 s"
        );
        if std::fs::read(&server_package).ok().as_deref() == Some(bytes.as_slice()) {
            break;
        }
        std::thread::yield_now();
    }

    // AC4: the second laptop has signed nothing and loses its unused identity.
    let light_home = tempfile::tempdir().unwrap();
    let light = start(light_home.path());
    assert!(
        invoke(&light, "status", json!({}))["identity"]["releases"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    on(&light, true);
    let light_key = machines(&light)["node"].as_str().unwrap().to_owned();
    begin(
        &light,
        "machine_join",
        json!({"code":code(&alice,"light"),"replace":false}),
    );
    pair_consent(&alice, &light_key, "light");
    assert!(finish(&light).get("error").is_none());
    assert_eq!(machines(&light)["machine"], "light");
    assert!(!light_home.path().join("log/identity.key").exists());

    let ivy_home = tempfile::tempdir().unwrap();
    let ivy = dist(ivy_home.path());
    ivy.set_name("Ivy").unwrap();
    let reference = &published["release"];
    let publisher = reference
        .as_str()
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    // Ivy's listing reads Alice's head at the link's location, so Alice's
    // site is served only now, after AC3, and stopped before the light search.
    let alice_site = Site::serve(alice_home.path().join("dist/site"));
    let listed = watane_dist::Link::new(
        publisher,
        &alice_site.base,
        Some(&"reading-list".parse().unwrap()),
    )
    .unwrap();
    let listing = ivy.list(&listed, "good for books").unwrap();
    let ivy_site = Site::serve(ivy.site_dir());
    ivy.set_site_url(&ivy_site.base).unwrap();
    invoke(
        &alice,
        "index_choose",
        json!({"link":ivy.own_link(None).unwrap().to_string()}),
    );
    // This event follows verified ingestion into the real server's holder.
    server.until(&format!(
        "pulled {} to {} from ",
        ivy.identity().unwrap().fingerprint(),
        listing.seq()
    ));
    drop(ivy_site);
    drop(alice_site);
    on(&alice, false); // Only the server can answer the light laptop now.
    let hit = find_on(&light, reference, &server_key);
    assert_eq!(hit["from"], json!([server_key]));
    assert_eq!(hit["verified"], true);
    begin(
        &light,
        "install_release",
        json!({"release":reference,"link":null}),
    );
    consent(&light);
    approve(&light);
    assert!(finish(&light).get("error").is_none());
    invoke(&light, "mod_open", json!({"local":"reading-list"}));
    assert!(light.wait("mod-reading-list", TIMEOUT));
    light.eval("mod-reading-list", "document.querySelector('#title').value='J8 books'; document.querySelector('#add').click(); null");
    until(
        &light,
        "mod-reading-list",
        "document.querySelector('#books').textContent.includes('J8 books')",
    );

    // AC5: installation warmed the server's blob reference. Remove the light
    // cache so the next fetch reaches the server, which checks and resets.
    let light_package = light_home.path().join("dist/packages").join(&package_id);
    std::fs::remove_file(&light_package).unwrap();
    let mut corrupt = bytes.clone();
    corrupt[0] ^= 1;
    std::fs::write(&server_package, corrupt).unwrap();
    let response = fetch(&light, reference);
    assert_eq!(response.status(), 502);
    assert_eq!(
        response.json::<Value>().unwrap()["error"]["code"],
        "fetch_failed"
    );
    wait_machines(&light, |v| {
        v["problems"].as_array().unwrap().iter().any(|p| {
            let text = p["text"].as_str().unwrap();
            p["node"] == server_key
                && ((text.starts_with(&format!("refused {server_key}:"))
                    && text.contains("does not match its hash"))
                    || (text.starts_with(&format!("unreachable {server_key}:"))
                        && text.contains("not served")))
        })
    });
    assert!(!light_package.exists(), "AC5: corrupt bytes were cached");

    // AC6: removal must prevent a fetch that succeeded before removal.
    // Restart before the repaired-file control to clear a possible hash-refusal
    // drop; not-served need not drop the peer. The control and removal refusal
    // below must share one light app session, with only the server able to serve.
    assert!(light.quit().success());
    std::fs::write(&server_package, &bytes).unwrap();
    let light = start(light_home.path());
    find_on(&light, reference, &server_key);
    // The control runs after the same off/on cycle as the refused fetch, so
    // only the removal differs between the two (dist's error text below
    // does not name the server: node drops net's blob error).
    on(&light, false);
    on(&light, true);
    fetched(&light, reference, &bytes);
    std::fs::remove_file(&light_package).unwrap();
    on(&light, false); // Keep its old list until its next request to the server.
    on(&alice, true);
    let removed = invoke(&alice, "machine_remove", json!({"node":light_key}));
    let event = server.until(&format!("machines {}: ", removed["seq"]));
    assert!(!event.contains(&light_key), "AC6: {event}");
    assert!(event.contains(&server_key), "AC6: {event}");
    on(&alice, false);
    on(&light, true);
    assert_eq!(machines(&light)["machine"], "light");
    let response = fetch(&light, reference);
    assert_eq!(response.status(), 502);
    let error = response.json::<Value>().unwrap();
    assert_eq!(error["error"]["code"], "fetch_failed");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(!message.is_empty(), "AC6: {error}");
    // client.rs renders a protocol denial as NetError::Refused. However,
    // light::need_blob reports blob denial as Unreachable, and Node's
    // need_package discards that error before dist's no-base fallback.
    // Check the returned cause chain, not events (Refused events mean a
    // bad object). These literals come from net's error Display/path text.
    if message.contains("not_allowed") {
        assert!(
            message.contains(&format!("node {server_key} refused: not_allowed")),
            "AC6: {error}"
        );
    } else {
        for no_path in [
            "unreachable",
            "no full machine is reachable",
            "no direct path",
        ] {
            assert!(!message.contains(no_path), "AC6: {error}");
        }
    }
    assert_eq!(
        machines(&light)["machine"],
        "light",
        "AC6: refusal must come from the server, not a local Removed state"
    );
    assert!(!light_package.exists());
    assert!(light.quit().success());

    // AC7: the restored identity remains the signer/anchor. It issues the
    // full code; the existing keyless server joins it again with --replace.
    assert!(alice.quit().success());
    drop(server);
    let restored_home = tempfile::tempdir().unwrap();
    let restored = start(restored_home.path());
    restored.pick(&[backup]);
    invoke(
        &restored,
        "identity_restore",
        json!({"passphrase":"twelve words long enough"}),
    );
    assert_eq!(
        invoke(&restored, "status", json!({}))["identity"]["fingerprint"],
        identity
    );
    on(&restored, true);
    let restore_code = code(&restored, "full");
    let mut joining = NodeProc::spawn(server_home.path(), "join", &["--replace", &restore_code]);
    assert_eq!(
        joining.until("this machine: "),
        format!("this machine: {server_key}")
    );
    pair_consent(&restored, &server_key, "full");
    assert!(joining.exited().success());
    let server = NodeProc::spawn(server_home.path(), "run", &[]);
    server.until("running as full");
    let end = Instant::now() + TIMEOUT;
    let hint = loop {
        let hints = invoke(&restored, "own_hints", json!({}));
        if let Some(h) = hints
            .as_array()
            .unwrap()
            .iter()
            .find(|h| h["node"] == server_key && h["seq"].is_number() && h["forked"] == false)
        {
            break h.clone();
        }
        assert!(Instant::now() < end, "AC7: no server hint: {hints}");
    };
    // Only the original publish signed on Alice's channel: its first seq is 0.
    assert_eq!(hint["seq"], 0);
    area(&restored, "#settings");
    until(
        &restored,
        "main",
        &format!(
            "document.querySelector('#own-hints').textContent.includes({}) && document.querySelector('#own-hints').textContent.includes({})",
            json!(server_key),
            json!(format!("position {}", hint["seq"]))
        ),
    );
    let next = update_package("0.2.0", false);
    let (status, refused) = http(restored.port, "application/octet-stream", next.clone());
    assert_eq!(status, 409, "AC7: {refused}");
    assert_eq!(refused["error"]["code"], "publish_refused");
    assert_eq!(
        refused["error"]["message"],
        "Confirm the restored own head before signing: in Settings, identity, choose From your machines, or restore again."
    );
    assert!(!restored.windows().iter().any(|w| w["label"] == "consent"));
    invoke(
        &restored,
        "identity_confirm_hint",
        json!({"node":hint["node"],"seq":hint["seq"],"id":hint["id"]}),
    );
    let pending = publish(&restored, next);
    consent(&restored);
    approve(&restored);
    let (status, next) = pending.join().unwrap();
    assert_eq!(status, 200, "AC7: {next}");
    let releases = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(TIMEOUT)
        .build()
        .unwrap()
        .get(format!("http://localhost:{}/api/releases", restored.port))
        .send()
        .unwrap()
        .json::<Value>()
        .unwrap();
    let last = releases["releases"].as_array().unwrap().last().unwrap();
    assert_eq!(last["release"], next["release"]);
    assert_eq!(last["seq"], hint["seq"].as_u64().unwrap() + 1);

    // AC8: inspect the persisted switches for every machine, including the
    // fresh home whose spent-code join was refused.
    for home in [
        &alice_home,
        &server_home,
        &refused_home,
        &light_home,
        &restored_home,
    ] {
        no_public_network(home.path());
    }
    assert!(restored.quit().success());
    drop(server);
}
