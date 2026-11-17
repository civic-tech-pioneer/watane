#![cfg(feature = "driver")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;

use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use support::dev::{dev_run, workspace_dev};
use support::driver::App;
use support::journey::*;
use support::publish::*;

fn body(app: &App) -> String {
    app.eval("consent", "document.body.textContent")
        .as_str()
        .unwrap()
        .to_owned()
}
fn title(app: &App) -> Value {
    app.eval("consent", "document.querySelector('h1').textContent")
}
fn contains(text: &str, fields: &[&str]) {
    for field in fields {
        assert!(text.contains(field), "missing {field:?}: {text}");
    }
}
fn participant(home: &Path, name: &str) -> (App, Site, String) {
    let app = App::start(
        home,
        &[
            ("NO_PROXY", "*"),
            ("no_proxy", "*"),
            ("WATANE_DRIVER_LISTED_LOCAL", "1"),
        ],
    );
    until(&app, "main", "document.body.dataset.ready === 'true'");
    begin(&app, "identity_set_name", json!({"name":name}));
    consent(&app);
    approve(&app);
    assert_eq!(finish(&app), json!({"ok":null}));
    let site = Site::serve(home.join("dist/site"));
    invoke(&app, "site_set_url", json!({"url":site.base}));
    let link = invoke(&app, "status", json!({}))["site"]["link"]
        .as_str()
        .unwrap()
        .to_owned();
    (app, site, link)
}
fn published(app: &App, folder: &Path, source: bool, fields: &[&str]) -> Value {
    let package = watane_host::watane_pkg::pack(folder, source).unwrap();
    let pending = publish(app, package.as_bytes().to_vec());
    consent(app);
    assert_eq!(title(app), "Sign a release");
    contains(&body(app), fields);
    approve(app);
    let (status, value) = pending.join().unwrap();
    assert_eq!(status, 200, "{value}");
    value
}
fn channels(home: &Path) -> Value {
    serde_json::from_slice(&fs::read(home.join("dist/follows.json")).unwrap()).unwrap()
}
fn install_result(app: &App, expected_key: &str) {
    let done = finish(app);
    assert!(done.get("error").is_none(), "{done}");
    // The hidden main window skips its polling refresh, and `begin` bypasses
    // the button whose action refreshes; ask the page to refresh as on focus.
    app.eval("main", "window.dispatchEvent(new Event('focus')); null");
    installed(app, "ratings");
    installed(app, "stats");
    let mods = invoke(app, "mods_list", json!({}));
    assert_eq!(mods.as_array().unwrap().len(), 2, "{mods}");
    let ratings = mods
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["slug"] == "ratings")
        .unwrap();
    assert_eq!(ratings["release"], format!("{expected_key}/ratings@0.1.0"));
    // Also exercise the committed components through the real host and page.
    open(app, "stats");
    until(
        app,
        "mod-stats",
        "document.querySelector('#result')?.textContent === '1'",
    );
    assert_eq!(
        channels(&app.home)["channels"][expected_key]["followed"],
        true
    );
}

#[test]
#[ignore = "slow: journey J5"]
fn ac1_journey_j5() {
    let dev = workspace_dev();
    let alice_home = tempfile::tempdir().unwrap();
    let carol_home = tempfile::tempdir().unwrap();
    let dave_home = tempfile::tempdir().unwrap();
    let eve_home = tempfile::tempdir().unwrap();
    let fresh_home = tempfile::tempdir().unwrap();
    let (alice, _alice_site, alice_channel) = participant(alice_home.path(), "Alice");
    let (carol, carol_site, carol_channel) = participant(carol_home.path(), "Carol");
    let (dave, _dave_site, dave_channel) = participant(dave_home.path(), "Dave");
    let (eve, _eve_site, _) = participant(eve_home.path(), "Eve");

    let reading = published(
        &alice,
        &fixture("reading-list"),
        true,
        &["release", "reading-list"],
    );
    let alice_release = reading["release"].as_str().unwrap();
    // Make Alice's profile available to the release consent without installing.
    invoke(&carol, "channel_follow", json!({"link":alice_channel}));
    let work = tempfile::tempdir().unwrap();
    let ratings = work.path().join("ratings");
    dev_run(
        &dev,
        &carol,
        &ratings,
        &[
            "new",
            "ratings",
            "--from",
            reading["link"].as_str().unwrap(),
        ],
    );
    let origin_meta = fs::read_to_string(ratings.join("mod.toml")).unwrap();
    contains(
        &origin_meta,
        &["[origin]", &format!("release = \"{alice_release}\"")],
    );
    assert!(
        fs::read_to_string(ratings.join("wit/world.wit"))
            .unwrap()
            .starts_with("package mods:ratings@")
    );
    assert_eq!(
        watane_host::watane_pkg::Mod::read(&ratings)
            .unwrap()
            .origin()
            .unwrap()
            .to_string(),
        alice_release
    );
    // The agent's implementation is the committed fixture. Keep dev's origin.
    copy_dir(&fixture("ratings"), &ratings);
    fs::write(
        ratings.join("mod.toml"),
        format!(
            "{}\n[origin]{}",
            fs::read_to_string(fixture("ratings").join("mod.toml")).unwrap(),
            origin_meta.split_once("[origin]").unwrap().1
        ),
    )
    .unwrap();
    let carol_release = published(
        &carol,
        &ratings,
        false,
        &["release", "Made from", alice_release, "Alice"],
    );
    let carol_key = carol_channel
        .split('@')
        .next()
        .unwrap()
        .trim_start_matches("watane:");
    let provider = format!("{carol_key}/ratings");

    let stats = work.path().join("stats");
    copy_dir(&fixture("stats"), &stats);
    // add --release must create these artifacts rather than inherit the fixture.
    fs::write(stats.join("deps.json"), "{}").unwrap();
    fs::remove_dir_all(stats.join("wit/deps/ratings")).unwrap();
    let added = dev_run(
        &dev,
        &dave,
        &stats,
        &["add", "--release", carol_release["link"].as_str().unwrap()],
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(stats.join("deps.json")).unwrap()).unwrap(),
        json!({"mods:ratings":carol_key})
    );
    assert!(stats.join("wit/deps/ratings").is_dir());
    // Use the link returned by dev's real fetch, just as dev publish does,
    // without invoking its out-of-scope build/test pipeline.
    let package = watane_host::watane_pkg::pack(&stats, false).unwrap();
    let port = dave.port;
    let link = added["link"].as_str().unwrap().to_owned();
    assert_eq!(link, carol_release["link"]);
    let pending = std::thread::spawn(move || {
        let response = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap()
            .post(format!("http://localhost:{port}/api/publish"))
            .header("content-type", "application/octet-stream")
            .header("Watane-Provider-Links", link)
            .body(package.as_bytes().to_vec())
            .send()
            .unwrap();
        (
            response.status().as_u16(),
            response.json::<Value>().unwrap(),
        )
    });
    consent(&dave);
    assert_eq!(title(&dave), "Sign a release");
    contains(&body(&dave), &["stats"]);
    approve(&dave);
    consent(&dave);
    assert_eq!(title(&dave), "List in your index");
    contains(&body(&dave), &[&provider, "used by stats"]);
    approve(&dave);
    let (status, stats_release) = pending.join().unwrap();
    assert_eq!(status, 200, "{stats_release}");
    assert_eq!(
        stats_release["providers"],
        json!([{"provider":provider,"listed":true}])
    );
    let dave_rows = invoke(&dave, "indexes_list", json!({}));
    assert!(
        dave_rows
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["listings"]
                .as_array()
                .is_some_and(|listings| listings.iter().any(|l| l["kind"] == "list"
                    && l["target"].as_str().is_some_and(|target| target
                        .strip_prefix("ratings by ")
                        .is_some_and(|party| party.contains("Carol")))
                    && l["note"] == "used by stats"))),
        "{dave_rows}"
    );

    invoke(&eve, "channel_follow", json!({"link":dave_channel}));
    let before = channels(eve_home.path());
    assert_eq!(before["channels"].as_object().unwrap().len(), 1, "{before}");
    assert!(before["channels"].get(carol_key).is_none(), "{before}");
    begin(&eve, "install_link", json!({"link":stats_release["link"]}));
    consent(&eve);
    assert_eq!(
        eve.eval(
            "consent",
            "Array.from(document.querySelectorAll('main section h2'), n => n.textContent)"
        ),
        json!(["Ratings", "Stats"])
    );
    approve(&eve);
    install_result(&eve, carol_key);

    let frank_home = tempfile::tempdir().unwrap();
    let frank = dist(frank_home.path());
    frank.set_name("Frank").unwrap();
    let frank_site = Site::serve(frank.site_dir());
    frank.set_site_url(&frank_site.base).unwrap();
    frank
        .publish(
            watane_host::watane_pkg::pack(&fixture("ratings"), false)
                .unwrap()
                .as_bytes()
                .to_vec(),
        )
        .unwrap();
    let frank_key = frank.identity().unwrap().to_string();
    let (fresh, _fresh_site, _) = participant(fresh_home.path(), "Eve");
    // This fresh home has never fetched Carol's package. Keep her channel
    // metadata only so the failed provider can still be named in the pick.
    invoke(&fresh, "channel_follow", json!({"link":dave_channel}));
    choose(&fresh, &dave_channel);
    assert_eq!(
        channels(fresh_home.path())["channels"][carol_key]["followed"],
        false
    );
    assert_eq!(
        fs::read_dir(fresh_home.path().join("mods"))
            .unwrap()
            .count(),
        0
    );
    choose(&fresh, &frank.own_link(None).unwrap().to_string());
    drop(carol_site);
    begin(
        &fresh,
        "install_link",
        json!({"link":stats_release["link"]}),
    );
    consent(&fresh);
    contains(
        &body(&fresh),
        &["ratings by Carol", "is not available", "Frank"],
    );
    assert_eq!(
        fresh.eval(
            "consent",
            "document.querySelectorAll('input[type=radio]').length"
        ),
        1
    );
    fresh.eval(
        "consent",
        "document.querySelector('input[type=radio]').click(); null",
    );
    approve(&fresh);
    consent(&fresh);
    contains(&body(&fresh), &["your pick", "Frank", "Carol"]);
    approve(&fresh);
    install_result(&fresh, &frank_key);
    fresh.eval(
        "main",
        "document.querySelector('[data-tile=stats]').click(); null",
    );
    let text = fresh.eval(
        "main",
        "document.querySelector('[data-local=stats] p[data-key] .grant-text').title",
    );
    contains(
        text.as_str().unwrap(),
        &[
            "Call mods:ratings/api@0.1.0 of ratings by Frank",
            "in place of ratings by Carol",
        ],
    );

    assert!(fresh.quit().success());
    assert!(eve.quit().success());
    assert!(dave.quit().success());
    assert!(carol.quit().success());
    assert!(alice.quit().success());
}

#[test]
#[ignore = "slow: journey J5"]
fn ac2_j5_github() {
    use support::dev::{Github, workspace_fake_gh};

    let dev = workspace_dev();
    let fake_gh = workspace_fake_gh();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    let github = Github::new(&root, &fake_gh);
    let alice = App::start(&root.join("alice"), &[]);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let reading = root.join("reading-list");
    copy_dir(&fixture("reading-list"), &reading);

    // AC1: publish from Alice's real app, then serve only her pushed bytes.
    let mut command = github.dev_command(&dev, &alice, &reading, "alice", &["publish", "--push"]);
    let pending = std::thread::spawn(move || command.output().unwrap());
    consent(&alice);
    contains(&body(&alice), &["release", "reading-list"]);
    approve(&alice);
    let output = pending.join().unwrap();
    let published: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("Alice publish: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(0), "{published}; {output:?}");
    assert_eq!(published["ok"], true, "{published}");
    let alice_release = published["result"]["release"].as_str().unwrap();
    let alice_remote = root.join("remotes/alice/reading-list");
    let site_remote = root.join("remotes/alice/watane-channel");
    let clone = root.join("served-site");
    github.git(&[
        "clone",
        "-b",
        "main",
        site_remote.to_str().unwrap(),
        clone.to_str().unwrap(),
    ]);
    let site = Site::serve(clone);
    invoke(&alice, "site_set_url", json!({"url":site.base}));
    let output = github
        .dev_command(&dev, &alice, &reading, "alice", &["publish", "--push"])
        .output()
        .unwrap();
    let retry: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("Alice publish retry: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(1), "{retry}; {output:?}");
    assert_eq!(retry["ok"], false, "{retry}");
    assert_eq!(
        retry["errors"][0]["code"], "publish.version_exists",
        "{retry}"
    );
    assert_eq!(retry["result"]["release"], alice_release);
    let link = retry["result"]["link"].as_str().unwrap();
    assert!(link.contains(&site.base), "{link}");

    // AC2: Carol follows Alice, and new --from retains the tagged git history.
    let (carol, _carol_site, _) = participant(&root.join("carol"), "Carol");
    let alice_channel = invoke(&alice, "status", json!({}))["site"]["link"].clone();
    invoke(&carol, "channel_follow", json!({"link":alice_channel}));
    let ratings = root.join("ratings");
    let output = github
        .dev_command(
            &dev,
            &carol,
            &ratings,
            "carol",
            &["new", "ratings", "--from", link],
        )
        .output()
        .unwrap();
    let created: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("Carol new: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(0), "{created}; {output:?}");
    assert_eq!(created["ok"], true, "{created}");
    let repository = "https://github.com/alice/reading-list";
    assert_eq!(
        created["result"]["fork"],
        json!({"repository":repository,"tag":alice_release})
    );
    assert!(ratings.join(".git").is_dir());
    let alice_commit = github.git(&[
        "-C",
        alice_remote.to_str().unwrap(),
        "rev-parse",
        &format!("{alice_release}^{{commit}}"),
    ]);
    assert_eq!(
        github.git(&["-C", ratings.to_str().unwrap(), "rev-parse", "HEAD"]),
        alice_commit
    );
    assert_eq!(
        github
            .git(&[
                "-C",
                ratings.to_str().unwrap(),
                "config",
                "remote.upstream.url"
            ])
            .trim(),
        repository
    );
    let origin_meta = fs::read_to_string(ratings.join("mod.toml")).unwrap();
    contains(
        &origin_meta,
        &["[origin]", &format!("release = \"{alice_release}\"")],
    );
    assert!(
        watane_host::watane_pkg::Mod::read(&ratings)
            .unwrap()
            .repository()
            .is_none(),
        "{origin_meta}"
    );

    // AC3: the agent replaces the implementation, keeping origin and .git.
    // Alice's files (her tests/books.json, ui/app.js) are not the ratings mod.
    for entry in fs::read_dir(&ratings).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name() == Some(".git".as_ref()) {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
    }
    copy_dir(&fixture("ratings"), &ratings);
    fs::write(
        ratings.join("mod.toml"),
        format!(
            "{}\n[origin]{}",
            fs::read_to_string(fixture("ratings").join("mod.toml")).unwrap(),
            origin_meta.split_once("[origin]").unwrap().1,
        ),
    )
    .unwrap();
    // dev publish runs the contract tests that the agent writes with its logic.
    fs::create_dir(ratings.join("tests")).unwrap();
    fs::write(
        ratings.join("tests/score.json"),
        r#"{"steps":[{"call":"api.score","returns":1}]}"#,
    )
    .unwrap();
    assert!(ratings.join(".git").is_dir());
    let mut command = github.dev_command(
        &dev,
        &carol,
        &ratings,
        "carol",
        &["publish", "--push", "--owner", "carols-club"],
    );
    let pending = std::thread::spawn(move || command.output().unwrap());
    consent(&carol);
    contains(
        &body(&carol),
        &["release", "ratings", "Made from", alice_release, "Alice"],
    );
    approve(&carol);
    let output = pending.join().unwrap();
    let published: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|e| panic!("Carol publish: {e}; {output:?}"));
    assert_eq!(output.status.code(), Some(0), "{published}; {output:?}");
    assert_eq!(published["ok"], true, "{published}");
    assert_eq!(published["warnings"], json!([]), "{published}");
    assert_eq!(
        published["result"]["push"]["mod"]["forked"], true,
        "{published}"
    );
    assert_eq!(
        published["result"]["push"]["mod"]["repository"],
        "https://github.com/carols-club/ratings"
    );
    assert!(!root.join("remotes/carol/ratings").exists());
    let fork = root.join("remotes/carols-club/ratings");
    assert_eq!(
        fs::read_to_string(fork.join("fake-gh-parent"))
            .unwrap()
            .trim(),
        "alice/reading-list"
    );
    assert_eq!(
        github
            .git(&["-C", fork.to_str().unwrap(), "symbolic-ref", "HEAD"])
            .trim(),
        "refs/heads/watane"
    );
    github.git(&[
        "-C",
        fork.to_str().unwrap(),
        "merge-base",
        "--is-ancestor",
        alice_commit.trim(),
        "watane",
    ]);
    let release = published["result"]["release"].as_str().unwrap();
    assert_eq!(
        github.git(&[
            "-C",
            fork.to_str().unwrap(),
            "rev-parse",
            &format!("{release}^{{commit}}")
        ]),
        github.git(&["-C", fork.to_str().unwrap(), "rev-parse", "watane"])
    );
    assert_eq!(
        github.git(&["-C", fork.to_str().unwrap(), "rev-parse", "main"]),
        github.git(&["-C", alice_remote.to_str().unwrap(), "rev-parse", "main"])
    );
    assert!(carol.quit().success());
    assert!(alice.quit().success());
}

#[cfg(feature = "real-github")]
#[test]
#[ignore = "real: GitHub minmax-fund"]
fn ac3_j5_real_github() {
    use support::github::RealGithub;

    let dev = workspace_dev();
    let work = tempfile::tempdir().unwrap();
    let root = work.path().canonicalize().unwrap();
    let mut github = RealGithub::new();
    let mod_name = github.create();
    let site_name = github.create();
    let carol_mod = github.reserve();
    let carol_slug = carol_mod.strip_prefix("minmax-fund/").unwrap();
    let carol_site = github.create();
    let repository = format!("https://github.com/{mod_name}");
    let site_repository = format!("https://github.com/{site_name}");
    let alice = App::start(&root.join("alice"), &[]);
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let reading = root.join("reading-list");
    copy_dir(&fixture("reading-list"), &reading);
    github.repository(&reading, &repository);
    github.site_origin(&alice.home, &site_name);

    // AC1: publish from Alice's real app, then serve only her pushed bytes.
    let mut command = github.dev_command(&dev, &alice, &reading, &["publish", "--push"]);
    let pending = std::thread::spawn(move || command.output().unwrap());
    consent(&alice);
    contains(&body(&alice), &["release", "reading-list"]);
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
    let raw = format!("https://raw.githubusercontent.com/{site_name}/main/");
    assert_eq!(published["result"]["push"]["site"]["raw"], raw);
    assert!(published["result"]["link"].is_null(), "{published}");
    assert!(
        published["next"].as_str().unwrap().contains(&raw),
        "{published}"
    );
    let alice_release = published["result"]["release"].as_str().unwrap();
    assert_eq!(published["result"]["push"]["mod"]["tag"], alice_release);
    let alice_remote = root.join("cloned-alice");
    github.git(&["clone", &repository, alice_remote.to_str().unwrap()]);
    assert_eq!(
        github
            .git(&["-C", alice_remote.to_str().unwrap(), "tag"])
            .trim(),
        alice_release
    );
    let site_remote = site_repository;
    let clone = root.join("served-site");
    github.git(&["clone", "-b", "main", &site_remote, clone.to_str().unwrap()]);
    assert!(
        !github
            .git(&["-C", clone.to_str().unwrap(), "show", "main:head"])
            .is_empty()
    );
    let site = Site::serve(clone);
    invoke(&alice, "site_set_url", json!({"url":site.base}));
    let output = github
        .dev_command(&dev, &alice, &reading, &["publish", "--push"])
        .output()
        .unwrap();
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
    assert_eq!(retry["result"]["release"], alice_release);
    let link = retry["result"]["link"].as_str().unwrap();
    assert!(link.contains(&site.base), "{link}");

    // AC2: Carol follows Alice, and new --from retains the tagged git history.
    let (carol, _carol_site, _) = participant(&root.join("carol"), "Carol");
    let alice_channel = invoke(&alice, "status", json!({}))["site"]["link"].clone();
    invoke(&carol, "channel_follow", json!({"link":alice_channel}));
    let ratings = root.join("ratings");
    let output = github
        .dev_command(&dev, &carol, &ratings, &["new", "ratings", "--from", link])
        .output()
        .unwrap();
    let created = github.json(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        github.diagnostic(&output)
    );
    assert_eq!(created["ok"], true, "{created}");
    assert_eq!(
        created["result"]["fork"],
        json!({"repository":repository,"tag":alice_release})
    );
    assert!(ratings.join(".git").is_dir());
    let alice_commit = github.git(&[
        "-C",
        alice_remote.to_str().unwrap(),
        "rev-parse",
        &format!("{alice_release}^{{commit}}"),
    ]);
    assert_eq!(
        github.git(&["-C", ratings.to_str().unwrap(), "rev-parse", "HEAD"]),
        alice_commit
    );
    assert_eq!(
        github
            .git(&[
                "-C",
                ratings.to_str().unwrap(),
                "config",
                "remote.upstream.url"
            ])
            .trim(),
        repository
    );
    let origin_meta = fs::read_to_string(ratings.join("mod.toml")).unwrap();
    contains(
        &origin_meta,
        &["[origin]", &format!("release = \"{alice_release}\"")],
    );
    assert!(
        watane_host::watane_pkg::Mod::read(&ratings)
            .unwrap()
            .repository()
            .is_none(),
        "{origin_meta}"
    );

    // AC3: the agent replaces the implementation, keeping origin and .git.
    // Alice's files (her tests/books.json, ui/app.js) are not the ratings mod.
    for entry in fs::read_dir(&ratings).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name() == Some(".git".as_ref()) {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
    }
    copy_dir(&fixture("ratings"), &ratings);
    fs::write(
        ratings.join("mod.toml"),
        format!(
            "{}\n[origin]{}",
            fs::read_to_string(fixture("ratings").join("mod.toml")).unwrap(),
            origin_meta.split_once("[origin]").unwrap().1,
        ),
    )
    .unwrap();
    rename_ratings(&ratings, carol_slug);
    let carol_manifest = watane_host::watane_pkg::Mod::read(&ratings).unwrap();
    assert_eq!(carol_manifest.slug().as_str(), carol_slug);
    assert!(carol_manifest.repository().is_none());
    github.site_origin(&carol.home, &carol_site);
    // dev publish runs the contract tests that the agent writes with its logic.
    fs::create_dir(ratings.join("tests")).unwrap();
    fs::write(
        ratings.join("tests/score.json"),
        r#"{"steps":[{"call":"api.score","returns":1}]}"#,
    )
    .unwrap();
    assert!(ratings.join(".git").is_dir());
    let mut command = github.dev_command(
        &dev,
        &carol,
        &ratings,
        &["publish", "--push", "--owner", "minmax-fund"],
    );
    let pending = std::thread::spawn(move || command.output().unwrap());
    consent(&carol);
    contains(
        &body(&carol),
        &["release", carol_slug, "Made from", alice_release, "Alice"],
    );
    approve(&carol);
    let output = pending.join().unwrap();
    let published = github.json(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        github.diagnostic(&output)
    );
    assert_eq!(published["ok"], true, "{published}");
    assert_eq!(published["warnings"], json!([]), "{published}");
    assert_eq!(
        published["result"]["push"]["mod"]["forked"], true,
        "{published}"
    );
    assert_eq!(
        published["result"]["push"]["mod"]["repository"],
        format!("https://github.com/{carol_mod}")
    );
    let mut command = std::process::Command::new("gh");
    github.env(&mut command);
    let output = command
        .args(["api", &format!("repos/{carol_mod}")])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", github.diagnostic(&output));
    let fork = github.json(&output);
    assert_eq!(fork["parent"]["full_name"], mod_name);
    assert_eq!(fork["default_branch"], "watane");
    let clone = root.join("cloned-carol");
    github.git(&[
        "clone",
        &format!("https://github.com/{carol_mod}"),
        clone.to_str().unwrap(),
    ]);
    let release = published["result"]["release"].as_str().unwrap();
    let commit = github.git(&[
        "-C",
        clone.to_str().unwrap(),
        "rev-parse",
        &format!("{release}^{{commit}}"),
    ]);
    github.git(&[
        "-C",
        clone.to_str().unwrap(),
        "merge-base",
        "--is-ancestor",
        alice_commit.trim(),
        commit.trim(),
    ]);
    assert!(carol.quit().success());
    assert!(alice.quit().success());
}

// Rename the committed fixture's public component export, leaving the core
// implementation and its internal names intact. The exact section guard makes
// a fixture encoding change fail explicitly; no mod build or new tool is needed.
fn rename_ratings(dir: &Path, slug: &str) {
    let wit = dir.join("wit/world.wit");
    fs::write(
        &wit,
        fs::read_to_string(&wit)
            .unwrap()
            .replace("mods:ratings@", &format!("mods:{slug}@")),
    )
    .unwrap();
    let wasm = dir.join("logic.wasm");
    let mut bytes = fs::read(&wasm).unwrap();
    // Component export section: count, external name, instance index, no type.
    let old = b"\x0b\x1c\x01\x00\x16mods:ratings/api@0.1.0\x05\x0f\x00";
    let offsets: Vec<_> = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(i, window)| (window == old).then_some(i))
        .collect();
    assert_eq!(offsets.len(), 1, "ratings fixture export section changed");
    let name = format!("mods:{slug}/api@0.1.0");
    assert!(
        name.len() + 6 < 128,
        "fixture section uses single-byte lengths"
    );
    let mut export = vec![11, (name.len() + 6) as u8, 1, 0, name.len() as u8];
    export.extend_from_slice(name.as_bytes());
    export.extend_from_slice(&[5, 15, 0]);
    bytes.splice(offsets[0]..offsets[0] + old.len(), export);
    fs::write(wasm, bytes).unwrap();
}

#[test]
fn ac3_reserved_slug_keeps_prebuilt_ratings_contract() {
    let work = tempfile::tempdir().unwrap();
    let ratings = work.path().join("ratings");
    copy_dir(&fixture("ratings"), &ratings);
    let slug = "watane-j-a012345678";
    rename_ratings(&ratings, slug);
    let manifest = watane_host::watane_pkg::Mod::read(&ratings).unwrap();
    assert_eq!(manifest.slug().as_str(), slug);
    assert!(manifest.repository().is_none());
    watane_host::watane_pkg::pack(&ratings, true).unwrap();
    let harness = watane_host::Harness::open(&ratings, Default::default()).unwrap();
    assert_eq!(harness.call("api.score", json!({})).unwrap().value, 1);
}
