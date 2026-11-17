#![cfg(feature = "driver")]
mod support;
use serde_json::json;
use support::driver::App;
use support::journey::*;
use support::publish::*;
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
    use std::fs;
    use support::dev::{Github, workspace_dev, workspace_fake_gh};

    let dev = workspace_dev();
    let fake_gh = workspace_fake_gh();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    let github = Github::new(&root, &fake_gh);
    let git = |args: &[&str]| github.git(args);
    let alice_home = root.join("alice");
    let alice = App::start(&alice_home, &[]);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let reading = root.join("reading-list");
    copy_dir(&fixture("reading-list"), &reading);
    let command = || github.dev_command(&dev, &alice, &reading, "alice", &["publish", "--push"]);
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
    assert!(
        link.contains(&site.base),
        "{link} does not name {}",
        site.base
    );

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

#[cfg(feature = "real-github")]
#[test]
#[ignore = "real: GitHub minmax-fund"]
fn ac3_j3_real_github() {
    use std::time::{Duration, Instant};
    use support::dev::workspace_dev;
    use support::github::RealGithub;
    use watane_dist::Link;

    assert_ne!(
        std::env::var("WATANE_DRIVER_LISTED_LOCAL").as_deref(),
        Ok("1"),
        "S54 requires the default listed-local guard"
    );
    let dev = workspace_dev();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    let mut github = RealGithub::new();
    let mod_name = github.create();
    let site_name = github.create();
    let repository = format!("https://github.com/{mod_name}");
    let site_repository = format!("https://github.com/{site_name}");
    let alice_home = root.join("alice");
    let alice = App::start(&alice_home, &[]);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let reading = root.join("reading-list");
    copy_dir(&fixture("reading-list"), &reading);
    github.repository(&reading, &repository);
    github.site_origin(&alice.home, &site_name);
    let command = || github.dev_command(&dev, &alice, &reading, &["publish", "--push"]);
    // AC1: the real command waits for Alice's signature in her app.
    let mut first = command();
    let pending = std::thread::spawn(move || first.output().unwrap());
    consent(&alice);
    let text = alice.eval("consent", "document.body.textContent");
    assert!(text.as_str().unwrap().contains("reading-list"), "{text}");
    approve(&alice);
    let output = pending.join().unwrap();
    let published = github.json(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        github.diagnostic(&output)
    );
    assert_eq!(published["ok"], true, "{published}");
    let result = &published["result"];
    let raw = format!("https://raw.githubusercontent.com/{site_name}/main/");
    assert_eq!(result["push"]["site"]["raw"], raw);
    assert!(result["link"].is_null(), "{published}");
    assert!(
        published["next"].as_str().unwrap().contains(&raw),
        "{published}"
    );

    // Inspect fresh HTTPS clones of the pushed repositories.
    let mod_remote = root.join("cloned-mod");
    github.git(&["clone", &repository, mod_remote.to_str().unwrap()]);
    let site_remote = site_repository;
    let release = result["release"].as_str().unwrap();
    assert_eq!(result["push"]["mod"]["tag"], release);
    assert_eq!(
        github
            .git(&["-C", mod_remote.to_str().unwrap(), "tag"])
            .trim(),
        release
    );

    // S54 AC1-3: Pages must serve exactly the head pushed to main.
    let base = github.pages(&site_name);
    assert_eq!(
        base,
        format!(
            "https://minmax-fund.github.io/{}/",
            site_name.split('/').nth(1).unwrap()
        )
    );
    let clone = root.join("pushed-site");
    github.git(&["clone", "-b", "main", &site_remote, clone.to_str().unwrap()]);
    // `head` is binary CBOR: read the checkout of main without text decoding.
    let head = std::fs::read(clone.join("head")).unwrap();
    assert!(!head.is_empty());
    github.wait_served(&format!("{base}head"), &head, Duration::from_secs(600));
    invoke(&alice, "site_set_url", json!({"url":base}));
    let output = command().output().unwrap();
    let retry = github.json(&output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        github.diagnostic(&output)
    );
    assert_eq!(retry["ok"], false, "{retry}");
    assert_eq!(
        retry["errors"][0]["code"], "publish.version_exists",
        "{retry}"
    );
    assert_eq!(retry["result"]["release"], release);
    let link = retry["result"]["link"].as_str().unwrap();
    let alice_link: Link = link.parse().unwrap();
    assert_eq!(alice_link.base(), base);

    // S54 AC3: Bob installs from Pages over TLS.
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
    // Unfollow Alice so the Ivy check exercises a listed record.
    invoke(
        &bob,
        "channel_unfollow",
        json!({"channel":alice_link.channel().to_string()}),
    );
    // S54 AC4: only Ivy is chosen; Alice is neither chosen nor followed.
    let ivy_home = root.join("ivy");
    let ivy = dist(&ivy_home);
    ivy.set_name("Ivy").unwrap();
    ivy.list(&alice_link, "Books from Alice").unwrap();
    let ivy_site = Site::serve(ivy.site_dir());
    ivy.set_site_url(&ivy_site.base).unwrap();
    choose(&bob, &ivy.own_link(None).unwrap().to_string());
    search_reading(&bob);
    let results = bob.eval("main", "document.querySelector('#results').textContent");
    for field in ["reading-list", "Alice", "listed by Ivy"] {
        assert!(results.as_str().unwrap().contains(field), "{results}");
    }
    assert_eq!(
        bob.eval(
            "main",
            "document.querySelector('#results [data-release]').dataset.release"
        ),
        release
    );
    let row = format!("[data-index='{}']", ivy.identity().unwrap());
    let index = bob.eval(
        "main",
        &format!("document.querySelector({}).textContent", json!(row)),
    );
    let index = index.as_str().unwrap();
    assert!(!index.contains("local address"), "{index}");
    assert!(!index.contains("Skipped"), "{index}");
    assert!(
        !index.contains(&alice_link.channel().fingerprint()),
        "{index}"
    );
    let follows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bob_home.join("dist/follows.json")).unwrap())
            .unwrap();
    let alice_record = &follows["channels"][alice_link.channel().to_string()];
    assert_eq!(alice_record["followed"], false);
    assert_eq!(alice_record["index"], false);

    // S54 AC5: prefer the CDN's 404 path; another edge may still answer 5xx.
    let missing_base = format!("{base}missing-{}/", site_name.split('/').nth(1).unwrap());
    github.wait_status(
        &format!("{missing_base}head"),
        404,
        Duration::from_secs(300),
    );
    let missing = Link::new(alice_link.channel(), &missing_base, alice_link.slug()).unwrap();
    // Dan has no follows or index: only the missing link can supply Alice's base.
    let dan_home = root.join("dan");
    let dan = App::start(&dan_home, &[]);
    let started = Instant::now();
    begin(&dan, "install_link", json!({"link":missing.to_string()}));
    let failed = finish(&dan);
    assert!(started.elapsed() < Duration::from_secs(60), "{failed}");
    assert_eq!(failed["error"]["code"], "dist", "{failed}");
    let message = failed["error"]["message"].as_str().unwrap();
    // Pages answers a missing path with 404 or 5xx depending on the edge.
    let http_5xx = message.split_once("HTTP 5").and_then(|(_, rest)| {
        let (digits, tail) = rest.split_at_checked(2)?;
        (digits.bytes().all(|b| b.is_ascii_digit()) && tail.starts_with(" from"))
            .then(|| format!("HTTP 5{digits}"))
    });
    let answer = if message.contains("not found") {
        "not found".to_string()
    } else {
        http_5xx.unwrap_or_else(|| panic!("{message}"))
    };
    eprintln!("S54 missing base: {answer}");
    assert!(
        message.contains(&format!("{missing_base}head")),
        "{message}"
    );
    assert_eq!(
        invoke(&dan, "mods_list", json!({}))
            .as_array()
            .unwrap()
            .len(),
        0
    );

    assert!(dan.quit().success());
    assert!(bob.quit().success());
    assert!(alice.quit().success());
}
