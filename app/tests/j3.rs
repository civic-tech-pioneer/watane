#![cfg(feature = "driver")]
mod support;
use serde_json::json;
use support::{driver::App, journey::*, publish::*};
#[test]
#[ignore = "slow: journey J3"]
fn ac7_journey_j3_identity_publish_install_and_tamper() {
    let alice_home = tempfile::tempdir().unwrap();
    let bob_home = tempfile::tempdir().unwrap();
    let alice = App::start(alice_home.path(), &[]);
    let initial = invoke(&alice, "status", json!({}));
    let fingerprint = initial["identity"]["fingerprint"].as_str().unwrap();
    assert_eq!(initial["identity"]["ownConfirmed"], true);
    assert_eq!(initial["site"]["link"], serde_json::Value::Null);
    assert_eq!(
        std::fs::read_dir(alice_home.path().join("mods"))
            .unwrap()
            .count(),
        0
    );
    let backup = alice_home.path().join("identity.backup");
    alice.pick(std::slice::from_ref(&backup));
    assert_eq!(
        invoke(
            &alice,
            "identity_backup",
            json!({"passphrase":"twelve words long enough"})
        )["path"],
        json!(backup)
    );
    let bob = App::start(bob_home.path(), &[]);
    bob.pick(std::slice::from_ref(&backup));
    assert_eq!(
        invoke(
            &bob,
            "identity_restore",
            json!({"passphrase":"twelve words long enough"})
        )["fingerprint"],
        fingerprint
    );
    assert_eq!(
        invoke(&bob, "status", json!({}))["identity"]["ownConfirmed"],
        false
    );
    assert_eq!(
        invoke(&bob, "identity_confirm_own", json!({"link":null}))["identity"]["fingerprint"],
        fingerprint
    );
    assert!(bob.quit().success());
    // Bob's install uses an independent identity; the restored home above proves S12.
    let bob_install_home = tempfile::tempdir().unwrap();
    let bob = App::start(bob_install_home.path(), &[]);
    let other_backup = bob_install_home.path().join("bob.backup");
    bob.pick(std::slice::from_ref(&other_backup));
    invoke(
        &bob,
        "identity_backup",
        json!({"passphrase":"twelve words long enough"}),
    );
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let site = Site::serve(alice_home.path().join("dist/site"));
    invoke(&alice, "site_set_url", json!({"url":site.base}));
    let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
    let pending = publish(&alice, package.as_bytes().to_vec());
    consent(&alice);
    let text = alice.eval("consent", "document.body.textContent");
    let text = text.as_str().unwrap();
    for value in [
        "release",
        "reading-list",
        "0.1.0",
        &package.id().to_string(),
    ] {
        assert!(text.contains(value), "{text}");
    }
    approve(&alice);
    let (status, published) = pending.join().unwrap();
    assert_eq!(status, 200, "{published}");
    let link = published["link"].as_str().unwrap();
    alice.pick(std::slice::from_ref(&other_backup));
    begin(
        &alice,
        "identity_restore",
        json!({"passphrase":"twelve words long enough"}),
    );
    let refused = finish(&alice);
    assert_eq!(refused["error"]["code"], "dist");
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap()
            .contains("has signed entries; it cannot be replaced"),
        "{refused}"
    );
    assert_eq!(
        invoke(&alice, "status", json!({}))["identity"]["fingerprint"],
        fingerprint
    );
    begin(&bob, "install_link", json!({"link":link}));
    consent(&bob);
    let text = bob.eval("consent", "document.body.textContent");
    let text = text.as_str().unwrap();
    for value in ["Alice", fingerprint, "signature verified"] {
        assert!(text.contains(value), "{text}");
    }
    approve(&bob);
    let result = finish(&bob);
    assert!(result.get("error").is_none(), "{result}");
    let mods = invoke(&bob, "mods_list", json!({}));
    let m = &mods.as_array().unwrap()[0];
    assert_eq!(m["publisher"]["name"], "Alice");
    assert_eq!(m["release"], published["release"]);
    assert!(
        bob_install_home
            .path()
            .join("mods")
            .join(m["local"].as_str().unwrap())
            .is_dir()
    );
    invoke(&bob, "mod_open", json!({"local":m["local"]}));
    assert!(bob.wait(&format!("mod-{}", m["local"].as_str().unwrap()), TIMEOUT));
    assert!(
        bob.eval(
            &format!("mod-{}", m["local"].as_str().unwrap()),
            "document.body.textContent"
        )
        .as_str()
        .unwrap()
        .contains("Reading")
    );
    let package_path = alice_home
        .path()
        .join("dist/site/packages")
        .join(package.id().to_string());
    let mut bytes = std::fs::read(&package_path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(package_path, bytes).unwrap();
    let fresh_home = tempfile::tempdir().unwrap();
    let fresh = App::start(fresh_home.path(), &[]);
    let before = std::fs::read_dir(fresh_home.path().join("mods"))
        .unwrap()
        .count();
    begin(&fresh, "install_link", json!({"link":link}));
    let result = finish(&fresh);
    let message = result["error"]["message"].as_str().unwrap();
    assert!(message.contains("hash mismatch"), "{message}");
    assert!(
        message.contains(&format!("{}packages/{}", site.base, package.id())),
        "{message}"
    );
    assert_eq!(
        std::fs::read_dir(fresh_home.path().join("mods"))
            .unwrap()
            .count(),
        before
    );
    assert!(fresh.quit().success());
    assert!(bob.quit().success());
    assert!(alice.quit().success());
}
