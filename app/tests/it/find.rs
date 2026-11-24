#![cfg(feature = "driver")]
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use serde_json::{Value, json};
use support::driver::App;
use support::journey::*;
use support::publish::*;
use watane_dist::{Dist, Link};

const NOTE: &str = "good for books <img src=x onerror=alert(1)>";
struct Fixture {
    alice: Dist,
    ivy: Dist,
    alice_site: Option<Site>,
    _ivy_site: Site,
    _alice_home: tempfile::TempDir,
    _ivy_home: tempfile::TempDir,
    backup: Vec<u8>,
    release: String,
    alice_link: Link,
    ivy_link: Link,
}
impl Fixture {
    fn new() -> Self {
        let alice_home = tempfile::tempdir().unwrap();
        let ivy_home = tempfile::tempdir().unwrap();
        let alice = dist(alice_home.path());
        let backup = alice.backup("twelve words long enough").unwrap();
        alice.set_name("Alice").unwrap();
        let alice_site = Site::serve(alice.site_dir());
        alice.set_site_url(&alice_site.base).unwrap();
        let published = alice.publish(update_package("0.1.0", false)).unwrap();
        let ivy = listing_dist(ivy_home.path());
        ivy.set_name("Ivy").unwrap();
        ivy.list(published.link.as_ref().unwrap(), NOTE).unwrap();
        let ivy_site = Site::serve(ivy.site_dir());
        ivy.set_site_url(&ivy_site.base).unwrap();
        let alice_link = alice.own_link(None).unwrap();
        let ivy_link = ivy.own_link(None).unwrap();
        Self {
            alice,
            ivy,
            alice_site: Some(alice_site),
            _ivy_site: ivy_site,
            _alice_home: alice_home,
            _ivy_home: ivy_home,
            backup,
            release: published.info.reference.to_string(),
            alice_link,
            ivy_link,
        }
    }
    fn allow_local_alice(&self, app: &App) {
        // DD-26 permits local bases only when explicitly chosen or followed.
        // Choosing Alice allows the loopback fixture without following her.
        invoke(
            app,
            "index_choose",
            json!({"link":self.alice_link.to_string()}),
        );
    }
    fn add(&self, app: &App) {
        self.allow_local_alice(app);
        choose(app, &self.ivy_link.to_string());
    }
}
fn text(app: &App, selector: &str) -> String {
    app.eval(
        "main",
        &format!("document.querySelector({})?.textContent", json!(selector)),
    )
    .as_str()
    .unwrap()
    .to_owned()
}
fn contains(app: &App, selector: &str, value: &str) {
    until(
        app,
        "main",
        &format!(
            "Boolean(document.querySelector({})?.textContent.includes({}))",
            json!(selector),
            json!(value)
        ),
    );
}

#[test]
fn ac1_find_add_index() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.add(&app);
    let row = format!("[data-index='{}']", f.ivy.identity().unwrap());
    for value in ["Ivy", &f.ivy.fingerprint().unwrap(), "1 mod", "ok"] {
        contains(&app, &row, value);
    }
    assert!(
        !app.log()["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["cmd"] == "consent_get")
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    app.eval(
        "main",
        &format!(
            "document.querySelector({}).querySelector('.drop-index').click(); null",
            json!(row)
        ),
    );
    until(
        &app,
        "main",
        &format!("!document.querySelector({})", json!(row)),
    );
    assert!(app.quit().success());
}

#[test]
fn ac1_find_search_install() {
    let f = Fixture::new();
    f.ivy.include(&f.alice_link, "Alice's releases").unwrap();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[("WATANE_DRIVER_LISTED_LOCAL", "1")]);
    choose(&app, &f.ivy_link.to_string());
    app.eval("main", "document.querySelector('#search-text').value = 'no-such-mod'; document.querySelector('#search').click(); null");
    contains(&app, "#result-count", "No mods found");
    search_reading(&app);
    // 5.3.5: the card names the mod and says `read <ago>`; the slug is the name's title.
    until(
        &app,
        "main",
        "document.querySelector('#results article h2')?.title === 'reading-list' && /^read (just now|[0-9]+ min ago)$/.test(document.querySelector('#results .checked')?.textContent)",
    );
    for value in [
        "Reading list",
        "0.1.0",
        "Keep books",
        "Alice",
        &f.alice.fingerprint().unwrap(),
        "signature verified",
        "listed by Ivy",
        NOTE,
    ] {
        contains(&app, "#results", value);
    }
    assert_eq!(
        app.eval("main", "document.querySelectorAll('#find img').length"),
        0
    );
    let lines = app.eval("main", "Array.from(document.querySelectorAll('#results article:first-child p'), p => p.textContent).filter(t => t.startsWith('listed by Ivy'))");
    assert_eq!(lines.as_array().unwrap().len(), 1, "{lines}");
    assert!(
        lines[0].as_str().unwrap().contains("through Alice"),
        "{lines}"
    );
    let follows: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("dist/follows.json")).unwrap())
            .unwrap();
    assert_eq!(
        follows["channels"][f.alice.identity().unwrap().to_string()]["followed"],
        false
    );
    app.eval(
        "main",
        "document.querySelector('#results .install-release').click(); null",
    );
    consent(&app);
    let plan = app.eval("consent", "document.body.textContent");
    for value in ["Reading list", "Alice", "signature verified", &f.release] {
        assert!(plan.as_str().unwrap().contains(value), "{plan}");
    }
    let follow_words = format!(
        "Approving also follows Alice ({}) for updates.",
        f.alice.fingerprint().unwrap()
    );
    assert!(plan.as_str().unwrap().contains(&follow_words), "{plan}");
    approve(&app);
    installed(&app, "reading-list");
    let follows: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("dist/follows.json")).unwrap())
            .unwrap();
    assert_eq!(
        follows["channels"][f.alice.identity().unwrap().to_string()]["followed"],
        true
    );
    assert!(
        follows["channels"][f.alice.identity().unwrap().to_string()]
            .get("explicit_given")
            .is_none()
    );
    open(&app, "reading-list");
    until(
        &app,
        "mod-reading-list",
        "!!document.querySelector('#books')",
    );
    assert!(app.quit().success());
}

#[test]
fn ac1_find_stale_mark() {
    let mut f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.add(&app);
    search_reading(&app);
    let follows: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("dist/follows.json")).unwrap())
            .unwrap();
    let checked = follows["channels"][f.alice.identity().unwrap().to_string()]["last_success"]
        .as_u64()
        .unwrap();
    drop(f.alice_site.take());
    refresh_find(&app);
    // 5.3.5: the stale mark is `not read since <ago>` of the last good read.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let ago = |m: u64| match m {
        0 => "not read since just now".to_owned(),
        m => format!("not read since {m} min ago"),
    };
    let minutes = now.saturating_sub(checked) / 60000;
    let marks = json!([ago(minutes), ago(minutes + 1)]);
    until(
        &app,
        "main",
        &format!("{marks}.includes(document.querySelector('#results .checked')?.textContent)"),
    );
    assert!(text(&app, "#results").contains("Reading list"));
    assert!(app.quit().success());
}

#[test]
fn ac1_find_compromised() {
    let f = Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.add(&app);
    search_reading(&app);
    let fork_home = tempfile::tempdir().unwrap();
    let fork = dist(fork_home.path());
    fork.restore(&f.backup, "twelve words long enough").unwrap();
    fork.confirm_own(None).unwrap();
    fork.set_name("Fork Alice").unwrap();
    std::fs::remove_dir_all(f.alice.site_dir()).unwrap();
    copy_dir(&fork.site_dir(), &f.alice.site_dir());
    refresh_find(&app);
    contains(&app, "#results", "compromised");
    assert_eq!(
        app.eval(
            "main",
            "document.querySelector('#results .install-release') === null"
        ),
        true
    );
    assert!(!text(&app, "#results").contains("signature verified"));
    assert!(app.quit().success());
}

#[test]
fn ac2_find_skipped_channel_and_cause() {
    let f = Fixture::new();
    let skipped_home = tempfile::tempdir().unwrap();
    let skipped = dist(skipped_home.path());
    skipped.publish(update_package("0.1.0", false)).unwrap();
    let site = Site::serve(skipped.site_dir());
    let link = Link::new(
        skipped.identity().unwrap(),
        &site.base,
        Some(&"reading-list".parse().unwrap()),
    )
    .unwrap();
    f.ivy.list(&link, "local only").unwrap();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.add(&app);
    let row = format!(
        "#index-notices [data-index='{}']",
        f.ivy.identity().unwrap()
    );
    contains(&app, &row, &skipped.fingerprint().unwrap());
    contains(&app, &row, "local address");
    assert!(app.quit().success());
}

#[test]
fn ac1_find_list_in_my_index() {
    let f = Fixture::new();
    let home = support::app_home();
    listing_dist(home.path())
        .follow_derived(&f.alice_link)
        .unwrap();
    let app = App::start(home.path(), &[("WATANE_DRIVER_LISTED_LOCAL", "1")]);
    app.eval("main", "window.dispatchEvent(new Event('focus')); null");
    until(
        &app,
        "main",
        "!!document.querySelector('#channels .list-mod')",
    );
    app.eval("main", &format!("document.querySelector('#channels .mod-note').value = {}; document.querySelector('#channels .list-mod').click(); null", json!(NOTE)));
    consent(&app);
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "List in your index"
    );
    let plan = app.eval("consent", "document.body.textContent");
    for value in [
        &format!("{}/reading-list", f.alice.identity().unwrap()),
        f.alice_link.base(),
        NOTE,
    ] {
        assert!(plan.as_str().unwrap().contains(value), "{plan}");
    }
    assert_eq!(
        app.eval("consent", "document.querySelectorAll('img').length"),
        0
    );
    approve(&app);
    contains(&app, "#index-notices [data-own]", NOTE);
    assert_eq!(
        app.eval("main", "document.querySelectorAll('#indexes img').length"),
        0
    );
    // Count signed entry files independently of the page's live-listing rows.
    fn entries(dir: &std::path::Path) -> usize {
        if !dir.exists() {
            return 0;
        }
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let p = e.unwrap().path();
                if p.is_dir() {
                    entries(&p)
                } else {
                    usize::from(p.parent().unwrap().file_name().unwrap() == "entries")
                }
            })
            .sum()
    }
    assert_eq!(entries(&home.path().join("dist/site")), 1);
    app.eval(
        "main",
        "document.querySelector('#index-notices [data-own] .unlist').click(); null",
    );
    consent(&app);
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "End a listing"
    );
    approve(&app);
    until(
        &app,
        "main",
        "document.querySelectorAll('#index-notices [data-own] .unlist').length === 0",
    );
    assert_eq!(entries(&home.path().join("dist/site")), 2);
    app.eval("main", "document.querySelector('#channels .include-note').value = 'more books'; document.querySelector('#channels .include-index').click(); null");
    consent(&app);
    assert_eq!(
        app.eval("consent", "document.querySelector('h1').textContent"),
        "Include in your index"
    );
    approve(&app);
    contains(&app, "#index-notices [data-own]", "more books");
    assert_eq!(entries(&home.path().join("dist/site")), 3);
    app.eval(
        "main",
        "document.querySelector('#index-notices [data-own] .unlist').click(); null",
    );
    consent(&app);
    approve(&app);
    until(
        &app,
        "main",
        "document.querySelectorAll('#index-notices [data-own] .unlist').length === 0",
    );
    assert_eq!(entries(&home.path().join("dist/site")), 4);
    app.eval(
        "main",
        "document.querySelector('#channels .use-index').click(); null",
    );
    contains(
        &app,
        &format!("#indexes [data-index='{}']", f.alice.identity().unwrap()),
        "Alice",
    );
    let file: Value =
        serde_json::from_slice(&std::fs::read(home.path().join("dist/follows.json")).unwrap())
            .unwrap();
    let record = &file["channels"][f.alice_link.channel().to_string()];
    assert_eq!(record["index"], true, "{record}");
    assert!(record.get("explicit_given").is_none(), "{record}");
    assert!(app.quit().success());
}

#[test]
fn ac1_find_wording_and_freshness() {
    let script = format!(
        r#"{}
        import assert from 'node:assert/strict';
        assert.equal(modCount(1), '1 mod'); assert.equal(modCount(2), '2 mods');
        const alice = {{name:'Alice', fingerprint:'abcd'}};
        const you = {{name:'Carol', fingerprint:'own', you:true}};
        assert.equal(partyLabel(you), 'you');
        assert.equal(viaLabel({{kind:'index',index:you}}), 'listed by you');
        assert.equal(emptySearch(), 'No mods found');
        assert.equal(derivedAction({{installed:'ratings',verified:true}}), 'Installed');
        assert.deepEqual(viaLabels([
          {{kind:'index',index:alice,through:null}},
          {{kind:'index',index:alice,through:you}}
        ]), ['listed by Alice through you']);
        assert.deepEqual(viaLabels([
          {{kind:'index',index:alice,through:null,note:'direct'}},
          {{kind:'index',index:alice,through:you,note:'included'}}
        ]), ['listed by Alice through you · included · direct']);

        assert.equal(partyLabel(alice), 'Alice (abcd)');
        assert.equal(partyLabel({{name:null, fingerprint:'abcd'}}), 'abcd');
        assert.equal(checkedLabel({{checkedAt:60000, uncheckedSince:null}}, 360000), 'checked 5 min ago');
        assert.equal(checkedLabel({{checkedAt:null, uncheckedSince:null}}, 0), '');
        assert.equal(checkedLabel({{checkedAt:0, uncheckedSince:0}}, 360000), 'could not check since ' + new Date(0).toLocaleString());
        assert.equal(viaLabel({{kind:'index', index:alice, through:null, note:'<img>'}}), 'listed by Alice · <img>');
    "#,
        include_str!("../../ui/words.js")
    );
    assert!(
        std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn ac1_find_no_index() {
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    until(&app, "main", "document.body.dataset.ready === 'true'");
    assert_eq!(app.eval("main", "document.querySelector('#find')?.textContent.includes('Add an index from a link that someone shared with you.')"), true);
    assert_eq!(
        app.eval(
            "main",
            "document.querySelectorAll('#indexes [data-index]:not([data-own])').length"
        ),
        0
    );
    assert_eq!(
        app.eval(
            "main",
            "document.querySelectorAll('#index-notices [data-own]').length"
        ),
        0
    );
    assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
    assert!(app.quit().success());
}

#[test]
fn ac6_latest_action_owns_the_error_line() {
    let script = format!(
        r#"{}
      import assert from 'node:assert/strict';
      let line = '';
      const errors = actionErrors(text => line = text);
      const first = errors.begin();
      const second = errors.begin();
      errors.show(null, second);
      errors.show({{message:'old failure'}}, first);
      assert.equal(line, '');
      const third = errors.begin();
      errors.show({{message:'new failure'}}, third);
      errors.show({{message:'old failure'}}, first);
      assert.equal(line, 'new failure');
    "#,
        include_str!("../../ui/words.js")
    );
    assert!(
        std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .status()
            .unwrap()
            .success()
    );
}

#[test]
fn ac6_find_hides_only_the_empty_own_index() {
    let script = format!(
        r#"{}
        import assert from 'node:assert/strict';
        const own = {{own:true, listings:[], included:[]}};
        assert.deepEqual(visibleIndexes([own]), []);
        const listed = {{...own, listings:[{{kind:'list'}}]}};
        const included = {{...own, included:[{{name:'Ivy'}}]}};
        assert.deepEqual(visibleIndexes([listed]), [listed]);
        assert.deepEqual(visibleIndexes([included]), [included]);
        const other = {{own:false}};
        assert.deepEqual(visibleIndexes([own, other]), [own, other]);
        "#,
        include_str!("../../ui/words.js")
    );
    assert!(
        std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .status()
            .unwrap()
            .success()
    );
}

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "GUI lock serializes tests and guards no data"
)]
fn regress_s113_deny_from_find_does_not_follow() {
    let _gui = support::GUI.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    f.ivy.include(&f.alice_link, "Alice's releases").unwrap();
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    f.add(&app);
    search_reading(&app);
    contains(
        &app,
        "#results",
        "Installing may follow this channel for updates.",
    );
    let followed = || {
        let follows: Value =
            serde_json::from_slice(&std::fs::read(home.path().join("dist/follows.json")).unwrap())
                .unwrap();
        follows["channels"][f.alice.identity().unwrap().to_string()]["followed"].clone()
    };
    assert_eq!(followed(), false);
    app.eval(
        "main",
        "document.querySelector('#results .install-release').click(); null",
    );
    consent(&app);
    let at_consent = followed();
    deny(&app);
    until(
        &app,
        "main",
        "document.querySelector('#action-notice')?.textContent === 'You declined installing Reading list.'",
    );
    assert_eq!(
        app.eval("main", "document.querySelector('#error').textContent"),
        ""
    );
    let after = followed();
    assert!(app.quit().success());
    assert_eq!(at_consent, false, "following happened before consent");
    assert_eq!(after, false, "channel still followed after Deny");
}
