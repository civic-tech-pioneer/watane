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

#[test]
#[ignore = "slow: journey J3"]
fn ac2_j3_github() {
    use std::{fs, process::Command};
    use support::dev::{dev_command, workspace_dev, workspace_fake_gh};

    let dev = workspace_dev();
    let fake_gh = workspace_fake_gh();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("remotes")).unwrap();
    fs::create_dir(root.join("bin")).unwrap();
    std::os::unix::fs::symlink(fake_gh, root.join("bin/gh")).unwrap();
    fs::write(root.join("gitconfig"), format!(
        "[user]\n name = Alice\n email = alice@example.test\n[url \"file://{}/remotes/\"]\n insteadOf = https://github.com/\n", root.display()
    )).unwrap();
    let path = std::env::join_paths(
        std::iter::once(root.join("bin"))
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let alice_home = root.join("alice");
    let alice = App::start(&alice_home, &[]);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let reading = root.join("reading-list");
    copy_dir(&fixture("reading-list"), &reading);
    let command = || {
        let mut command = dev_command(&dev, &alice, &reading, &["publish", "--push"]);
        command
            .env("PATH", &path)
            .env("GIT_CONFIG_GLOBAL", root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("FAKE_GH_ROOT", root.join("remotes"))
            .env("FAKE_GH_USER", "alice");
        command
    };
    // AC1: the real command waits for Alice's signature in her app.
    let mut first = command();
    let pending = std::thread::spawn(move || first.output().unwrap());
    consent(&alice);
    let text = alice.eval("consent", "document.body.textContent");
    assert!(text.as_str().unwrap().contains("reading-list"), "{text}");
    approve(&alice);
    let output = pending.join().unwrap();
    let published: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("publish: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(0), "{published}; {output:?}");
    assert_eq!(published["ok"], true, "{published}");
    let result = &published["result"];
    let raw = "https://raw.githubusercontent.com/alice/watane-channel/main/";
    assert_eq!(result["push"]["site"]["raw"], raw);
    assert!(result["link"].is_null(), "{published}");
    assert!(
        published["next"].as_str().unwrap().contains(raw),
        "{published}"
    );

    // AC2: inspect the bare remotes, not Alice's working copies.
    let mod_remote = root.join("remotes/alice/reading-list");
    let site_remote = root.join("remotes/alice/watane-channel");
    let release = result["release"].as_str().unwrap();
    assert_eq!(result["push"]["mod"]["tag"], release);
    assert_eq!(
        git(&["-C", mod_remote.to_str().unwrap(), "tag"]).trim(),
        release
    );
    assert!(!git(&["-C", site_remote.to_str().unwrap(), "show", "main:head"]).is_empty());
    let manifest = fs::read_to_string(reading.join("mod.toml")).unwrap();
    assert!(
        manifest.contains("repository = \"https://github.com/alice/reading-list\""),
        "{manifest}"
    );

    // AC3: serve only a fresh clone of the pushed site, then ask dev for its link.
    let clone = root.join("served-site");
    git(&[
        "clone",
        "-b",
        "main",
        site_remote.to_str().unwrap(),
        clone.to_str().unwrap(),
    ]);
    let site = Site::serve(clone);
    invoke(&alice, "site_set_url", json!({"url":site.base}));
    let output = command().output().unwrap();
    let retry: serde_json::Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("publish retry: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(1), "{retry}; {output:?}");
    assert_eq!(retry["ok"], false, "{retry}");
    assert_eq!(
        retry["errors"][0]["code"], "publish.version_exists",
        "{retry}"
    );
    assert_eq!(retry["result"]["release"], release);
    let link = retry["result"]["link"].as_str().unwrap();

    // AC4: Bob has a fresh identity and installs the bytes from the clone.
    let bob_home = root.join("bob");
    let bob = App::start(&bob_home, &[]);
    begin(&bob, "install_link", json!({"link":link}));
    consent(&bob);
    let text = bob.eval("consent", "document.body.textContent");
    for field in ["Alice", "signature verified"] {
        assert!(text.as_str().unwrap().contains(field), "{text}");
    }
    approve(&bob);
    let installed = finish(&bob);
    assert!(installed.get("error").is_none(), "{installed}");
    let mods = invoke(&bob, "mods_list", json!({}));
    assert_eq!(mods.as_array().unwrap().len(), 1, "{mods}");
    let reading = &mods[0];
    assert_eq!(reading["publisher"]["name"], "Alice");
    assert_eq!(reading["release"], release);
    invoke(&bob, "mod_open", json!({"local":reading["local"]}));
    let label = format!("mod-{}", reading["local"].as_str().unwrap());
    assert!(bob.wait(&label, TIMEOUT));
    assert!(
        bob.eval(&label, "document.body.textContent")
            .as_str()
            .unwrap()
            .contains("Reading")
    );
    assert!(bob.quit().success());
    assert!(alice.quit().success());
}
