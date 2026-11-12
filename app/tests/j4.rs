#![cfg(feature = "driver")]
mod support;
use serde_json::{Value, json};
use std::{path::Path, time::Instant};
use support::{driver::App, journey::*, publish::*};

fn publish_version(app: &App, version: &str, network: bool) -> Value {
    let pending = publish(app, update_package(version, network));
    consent(app);
    approve(app);
    let (status, value) = pending.join().unwrap();
    assert_eq!(status, 200, "{value}");
    value
}
fn mod_view(app: &App) -> Value {
    invoke(app, "mods_list", json!({}))[0].clone()
}
fn version(app: &App, expected: &str, no_consent: bool) -> Value {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if no_consent {
            assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
        }
        let m = mod_view(app);
        if m["version"] == expected {
            return m;
        }
        assert!(Instant::now() < deadline, "expected {expected}: {m}");
    }
}
fn backup(app: &App, path: &Path) {
    app.pick(&[path.to_path_buf()]);
    invoke(
        app,
        "identity_backup",
        json!({"passphrase":"twelve words long enough"}),
    );
}
#[test]
#[ignore = "slow: journey J4 with index"]
fn ac8_journey_j4_updates_withdrawal_and_compromise() {
    let alice_home = tempfile::tempdir().unwrap();
    let carol_home = tempfile::tempdir().unwrap();
    let alice = App::start(alice_home.path(), &[]);
    let carol = App::start(carol_home.path(), &[]);
    let saved = alice_home.path().join("alice.backup");
    backup(&alice, &saved);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert!(finish(&alice).get("error").is_none());
    let site = Site::serve(alice_home.path().join("dist/site"));
    invoke(&alice, "site_set_url", json!({"url":site.base}));
    let first = publish_version(&alice, "0.1.0", false);
    let ivy_home = tempfile::tempdir().unwrap();
    let ivy = dist(ivy_home.path());
    ivy.set_name("Ivy").unwrap();
    ivy.list(
        &first["link"].as_str().unwrap().parse().unwrap(),
        "good for books",
    )
    .unwrap();
    let ivy_site = Site::serve(ivy.site_dir());
    ivy.set_site_url(&ivy_site.base).unwrap();
    let alice_link: watane_dist::Link = first["link"].as_str().unwrap().parse().unwrap();
    // Explicitly choose this loopback publisher under dist DD-26. It is
    // still not followed until installation from the result.
    invoke(
        &carol,
        "index_choose",
        json!({"link":watane_dist::Link::new(alice_link.channel(), alice_link.base(), None).unwrap().to_string()}),
    );
    choose(&carol, &ivy.own_link(None).unwrap().to_string());
    search_reading(&carol);
    let hit = carol.eval("main", "document.querySelector('#results').textContent");
    for field in [
        "Reading list",
        "signature verified",
        "listed by Ivy",
        "good for books",
    ] {
        assert!(hit.as_str().unwrap().contains(field), "{hit}");
    }
    carol.eval(
        "main",
        "document.querySelector('#results .install-release').click(); null",
    );
    consent(&carol);
    approve(&carol);
    installed(&carol, "reading-list");
    let local = mod_view(&carol)["local"].as_str().unwrap().to_string();
    invoke(&carol, "mod_open", json!({"local":local}));
    assert!(carol.wait(&format!("mod-{local}"), TIMEOUT));
    carol.eval(&format!("mod-{local}"),"document.querySelector('#title').value = 'Dune'; document.querySelector('#add').click(); null");
    until(
        &carol,
        &format!("mod-{local}"),
        "document.querySelector('#books').textContent.includes('Dune')",
    );
    carol.close(&format!("mod-{local}"));
    let second = publish_version(&alice, "0.2.0", false);
    invoke(&carol, "channels_refresh", json!({}));
    version(&carol, "0.2.0", true);
    invoke(&carol, "mod_open", json!({"local":local}));
    assert!(carol.wait(&format!("mod-{local}"), TIMEOUT));
    until(
        &carol,
        &format!("mod-{local}"),
        "document.querySelector('#books').textContent.includes('Dune')",
    );
    carol.close(&format!("mod-{local}"));
    let third = publish_version(&alice, "0.3.0", true);
    // Refresh must return even though the independent update is awaiting consent.
    invoke(&carol, "channels_refresh", json!({}));
    consent(&carol);
    let text = carol.eval("consent", "document.body.textContent");
    assert!(text.as_str().unwrap().contains("example.org"), "{text}");
    deny(&carol);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let m = mod_view(&carol);
        if m["notice"]
            .as_str()
            .is_some_and(|s| s == "You declined the update to 0.3.0.")
        {
            break;
        }
        assert!(Instant::now() < deadline, "{m}");
    }
    invoke(&carol, "channels_refresh", json!({}));
    assert!(
        mod_view(&carol)["notice"]
            .as_str()
            .unwrap()
            .eq("You declined the update to 0.3.0.")
    );
    assert_eq!(
        mod_view(&carol)["offerGrants"][0]["text"],
        "Connect to example.org"
    );
    assert!(!carol.windows().iter().any(|w| w["label"] == "consent"));
    begin(
        &carol,
        "release_install",
        json!({"release":third["release"]}),
    );
    consent(&carol);
    approve(&carol);
    assert_eq!(finish(&carol)["ok"], json!({"installed":[local]}));
    assert_eq!(version(&carol, "0.3.0", true)["notice"], Value::Null);
    alice.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &alice,
        "main",
        "document.querySelectorAll('#releases .withdraw-release').length === 3",
    );
    let own = invoke(&alice, "status", json!({}));
    assert!(
        own["identity"]["releases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["release"] == third["release"] && r["withdrawn"].is_null())
    );
    begin(
        &alice,
        "release_withdraw",
        json!({"release":third["release"],"reason":"breaks lists"}),
    );
    consent(&alice);
    assert!(
        alice
            .eval("consent", "document.body.textContent")
            .as_str()
            .unwrap()
            .contains("breaks lists")
    );
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    assert_eq!(
        invoke(&alice, "status", json!({}))["identity"]["releases"][2]["withdrawn"],
        "breaks lists"
    );
    invoke(&carol, "channels_refresh", json!({}));
    let m = mod_view(&carol);
    assert_eq!(m["notice"], "Withdrawn by the publisher: breaks lists");
    assert_eq!(m["offer"], second["release"]);
    carol.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &carol,
        "main",
        "!!document.querySelector('button.offer') && document.querySelector('button.offer').textContent === 'Install 0.2.0'",
    );
    assert!(
        carol
            .eval("main", "document.querySelector('.notice').textContent")
            .as_str()
            .unwrap()
            .contains("breaks lists")
    );
    carol.eval(
        "main",
        "document.querySelector('button.offer').click(); null",
    );
    version(&carol, "0.2.0", true);
    invoke(&carol, "mod_open", json!({"local":local}));
    assert!(carol.wait(&format!("mod-{local}"), TIMEOUT));
    until(
        &carol,
        &format!("mod-{local}"),
        "document.querySelector('#books').textContent.includes('Dune')",
    );
    carol.close(&format!("mod-{local}"));
    begin(&carol, "release_install", json!({"release":"bad"}));
    assert_eq!(finish(&carol)["error"]["code"], "bad_input");
    let fork_home = tempfile::tempdir().unwrap();
    let fork = App::start(fork_home.path(), &[]);
    fork.pick(std::slice::from_ref(&saved));
    invoke(
        &fork,
        "identity_restore",
        json!({"passphrase":"twelve words long enough"}),
    );
    invoke(&fork, "identity_confirm_own", json!({"link":null}));
    invoke(&fork, "site_set_url", json!({"url":site.base}));
    publish_version(&fork, "0.4.0", false);
    std::fs::remove_dir_all(alice_home.path().join("dist/site")).unwrap();
    copy_dir(
        &fork_home.path().join("dist/site"),
        &alice_home.path().join("dist/site"),
    );
    let rows = invoke(&carol, "channels_refresh", json!({}));
    assert_eq!(rows[0]["status"], "compromised");
    assert_eq!(
        mod_view(&carol)["notice"],
        "The publisher's channel is compromised"
    );
    carol.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &carol,
        "main",
        "document.querySelector('#results').textContent.includes('compromised')",
    );
    assert_eq!(
        carol.eval(
            "main",
            "document.querySelector('#results .install-release') === null"
        ),
        true
    );
    // The hit is installed, so the missing button alone proves little here;
    // the compromised hit must also lose its verified mark (S20 covers the button).
    assert_eq!(
        carol.eval(
            "main",
            "document.querySelector('#results').textContent.includes('signature verified')"
        ),
        false
    );
    assert!(fork.quit().success());
    assert!(carol.quit().success());
    assert!(alice.quit().success());
}
