//! J3 over a local or remote driver channel, with an untrusted web host (TECHNICAL 11).

#![cfg(feature = "driver")]

use crate::support::driver::App;
use crate::support::journey::{TIMEOUT, approve, fixture, invoke, until};
use crate::support::publish::{Site, begin, consent, finish, publish};
use serde_json::json;

#[test]
#[ignore = "vm: J3 between two machines"]
fn ac1_j3_two_machines() {
    let work = tempfile::tempdir().unwrap();
    let remote = std::env::var("WATANE_VM_BOB")
        .ok()
        .zip(std::env::var("WATANE_VM_ALICE_HOME").ok())
        .zip(std::env::var("WATANE_VM_SITE_BASE").ok());
    // Keep the local server alive until all three apps have quit.
    let _site;
    let (bob_command, alice_home, base) = if let Some(((bob, home), base)) = remote {
        let home = std::path::PathBuf::from(home);
        assert!(home.is_dir(), "Alice's home must already exist");
        assert!(
            std::fs::read_dir(&home).unwrap().next().is_none(),
            "Alice's home must be empty"
        );
        assert!(base.ends_with('/'), "site base must end with /");
        _site = None;
        (bob, home, base)
    } else {
        let home = work.path().join("alice");
        let site = Site::serve(home.join("dist/site"));
        let base = site.base.clone();
        _site = Some(site);
        // Shell positional arguments preserve spaces and quotes in both paths.
        let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
        let command = format!(
            "WATANE_DRIVER_SCREEN=hidden sh -c 'exec \"$1\" --home \"$2/$3\" --port 0' sh {} {}",
            quote(env!("CARGO_BIN_EXE_watane-app")),
            quote(work.path().to_str().unwrap())
        );
        (command, home, base)
    };

    // AC2: Alice signs in her own app; only her home is inspected locally.
    let alice = App::start(&alice_home, &[]);
    let status = invoke(&alice, "status", json!({}));
    let fingerprint = status["identity"]["fingerprint"].as_str().unwrap();
    invoke(&alice, "site_set_url", json!({"url":base}));
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));

    let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
    let pending = publish(&alice, package.as_bytes().to_vec());
    consent(&alice);
    approve(&alice);
    let (http_status, published) = pending.join().unwrap();
    assert_eq!(http_status, 200, "{published}");
    let link = published["link"].as_str().unwrap();

    let bob = App::over(&bob_command, ".watane");
    assert!(bob.home.as_os_str().is_empty());
    begin(&bob, "install_link", json!({"link":link}));
    consent(&bob);
    let text = bob.eval("consent", "document.body.textContent");
    for field in ["Alice", fingerprint, "signature verified"] {
        assert!(text.as_str().unwrap().contains(field), "{text}");
    }
    approve(&bob);
    let installed = finish(&bob);
    assert!(installed.get("error").is_none(), "{installed}");
    let mods = invoke(&bob, "mods_list", json!({}));
    assert_eq!(mods.as_array().unwrap().len(), 1);
    let reading = &mods[0];
    assert_eq!(reading["publisher"]["name"], "Alice");
    assert_eq!(reading["release"], published["release"]);
    invoke(&bob, "mod_open", json!({"local":reading["local"]}));
    let label = format!("mod-{}", reading["local"].as_str().unwrap());
    assert!(bob.wait(&label, TIMEOUT));
    assert!(
        bob.eval(&label, "document.body.textContent")
            .as_str()
            .unwrap()
            .contains("Reading")
    );
    until(&bob, &label, "document.querySelector('#add') !== null");
    bob.eval(&label, "document.querySelector('#title').value = 'Dune'; document.querySelector('#add').click(); null");
    until(
        &bob,
        &label,
        "document.querySelector('#books').textContent.includes('Dune') && !document.querySelector('#error').textContent",
    );

    // AC3: a fresh Bob cannot reuse the verified package cached by the first.
    let path = alice_home
        .join("dist/site/packages")
        .join(package.id().to_string());
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] ^= 1;
    std::fs::write(path, bytes).unwrap();
    let fresh = App::over(&bob_command, "bob-tamper");
    begin(&fresh, "install_link", json!({"link":link}));
    let refused = finish(&fresh);
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(message.contains("hash mismatch"), "{message}");
    assert!(
        message.contains(&format!("{base}packages/{}", package.id())),
        "{message}"
    );
    assert_eq!(invoke(&fresh, "mods_list", json!({})), json!([]));
    assert!(fresh.quit().success());
    assert!(bob.quit().success());
    assert!(alice.quit().success());
}
