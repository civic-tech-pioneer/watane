#![cfg(feature = "driver")]
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path};
use support::{
    dev::{dev_run, workspace_dev},
    driver::App,
    journey::*,
    publish::*,
};

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
    let frank_key = frank.identity().to_string();
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
        "document.querySelector('[data-local=stats] details').open = true; null",
    );
    let text = fresh.eval(
        "main",
        "document.querySelector('[data-local=stats] details p[data-key]').textContent",
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
