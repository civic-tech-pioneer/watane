#![allow(clippy::unwrap_used, reason = "tests may panic")]

use watane_host::watane_pkg::{self as pkg, Grant};
#[test]
fn ac4_build_on_fixtures_have_the_declared_contracts_and_page() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in ["ratings", "ratings-nofit", "stats"] {
        let folder = root.join(name);
        let package = pkg::pack(&folder, false).unwrap();
        assert!(!package.as_bytes().is_empty());
        let m = pkg::Mod::read(&folder).unwrap();
        if name != "stats" {
            assert_eq!(m.slug().to_string(), "ratings");
            let offers_api = m
                .world()
                .exports
                .iter()
                .any(|i| i.to_string() == "mods:ratings/api@0.1.0");
            assert_eq!(offers_api, name == "ratings");
        }
        if name == "stats" {
            assert!(m.grants().iter().any(|g| matches!(g, Grant::Call { interface, .. } if interface.to_string() == "mods:ratings/api@0.1.0")));
            assert!(folder.join("ui/index.html").exists());
        }
    }
}

#[tokio::test]
async fn ac4_stats_calls_ratings_and_preserves_observable_state() {
    use watane_host::{Caller, Host, LocalName, Mode, Requester, Source};
    let home = tempfile::tempdir().unwrap();
    let host = Host::open(home.path(), Mode::Preview, Default::default()).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let plan = host
        .plan(vec![
            Source::Folder(root.join("stats")),
            Source::Folder(root.join("ratings")),
        ])
        .unwrap();
    host.install(plan, Requester::App).await.unwrap();
    for expected in [1, 2] {
        assert_eq!(
            host.call_async(
                Caller::Page,
                LocalName::parse("stats").unwrap(),
                "api.result".into(),
                serde_json::json!({})
            )
            .await
            .unwrap(),
            expected
        );
    }
}

#[cfg(feature = "driver")]
mod support;
#[cfg(feature = "driver")]
mod windows {
    use super::support::driver::App;
    use super::support::journey::*;
    use super::support::publish::*;
    use serde_json::{Value, json};
    use watane_dist::{Dist, Link};
    struct Publisher {
        dist: Dist,
        site: Option<Site>,
        _home: tempfile::TempDir,
    }
    impl Publisher {
        fn new(name: &str) -> Self {
            let home = tempfile::tempdir().unwrap();
            let dist = dist(home.path());
            dist.set_name(name).unwrap();
            let site = Site::serve(dist.site_dir());
            dist.set_site_url(&site.base).unwrap();
            Self {
                dist,
                site: Some(site),
                _home: home,
            }
        }
        fn link(&self) -> Link {
            self.dist.own_link(None).unwrap()
        }
        fn publish(
            &self,
            fixture_name: &str,
            name: &str,
            provider: Option<&str>,
            origin: Option<&str>,
        ) -> watane_dist::Published {
            let temp = tempfile::tempdir().unwrap();
            copy_dir(&fixture(fixture_name), temp.path());
            let mut meta = format!("name = {name:?}\nsummary = \"Observable ratings calls.\"\n");
            if let Some(origin) = origin {
                meta.push_str(&format!("\n[origin]\nrelease = {origin:?}\n"));
            }
            std::fs::write(temp.path().join("mod.toml"), meta).unwrap();
            if let Some(key) = provider {
                std::fs::write(
                    temp.path().join("deps.json"),
                    json!({"mods:ratings":key}).to_string(),
                )
                .unwrap();
            }
            self.dist
                .publish(
                    watane_host::watane_pkg::pack(temp.path(), false)
                        .unwrap()
                        .as_bytes()
                        .to_vec(),
                )
                .unwrap()
        }
    }
    struct Fixture {
        carol: Publisher,
        dave: Publisher,
        frank: Publisher,
        stats: Link,
    }
    impl Fixture {
        fn new(fit: bool) -> Self {
            let carol = Publisher::new("Carol");
            let ratings = carol
                .publish("ratings", "Ratings", None, None)
                .link
                .unwrap();
            let dave = Publisher::new("Dave");
            dave.dist.follow(&carol.link()).unwrap();
            dave.dist.list(&ratings, "used by stats").unwrap();
            let stats = dave
                .publish(
                    "stats",
                    "Stats",
                    Some(&carol.dist.identity().unwrap().to_string()),
                    None,
                )
                .link
                .unwrap();
            let frank = Publisher::new("Frank");
            frank.publish(
                if fit { "ratings" } else { "ratings-nofit" },
                "Frank ratings",
                None,
                None,
            );
            Self {
                carol,
                dave,
                frank,
                stats,
            }
        }
        fn eve(&self) -> tempfile::TempDir {
            let home = app_home();
            let eve = dist(home.path());
            // Explicit choices permit loopback sites without a driver exception.
            eve.choose_index(&self.carol.link()).unwrap();
            eve.choose_index(&self.frank.link()).unwrap();
            eve.follow(&self.dave.link()).unwrap();
            home
        }
    }
    /// An empty app home with its marker, so the test can seed Eve's dist before the app starts.
    fn app_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        drop(
            watane_host::Host::open(
                home.path(),
                watane_host::Mode::Home { app: None },
                Default::default(),
            )
            .unwrap(),
        );
        home
    }
    fn start(home: &std::path::Path) -> App {
        let app = App::start(home, &[]);
        until(&app, "main", "document.body.dataset.ready === 'true'");
        app
    }
    fn begin_link(app: &App, link: &Link) {
        begin(app, "install_link", json!({"link":link.to_string()}));
    }
    fn followed(home: &std::path::Path, key: &str) {
        let follows: Value =
            serde_json::from_slice(&std::fs::read(home.join("dist/follows.json")).unwrap())
                .unwrap();
        assert_eq!(follows["channels"][key]["followed"], true);
    }
    fn empty(home: &std::path::Path) {
        assert_eq!(std::fs::read_dir(home.join("mods")).unwrap().count(), 0);
    }
    fn body(app: &App, window: &str) -> String {
        app.eval(window, "document.body.textContent")
            .as_str()
            .unwrap()
            .into()
    }
    fn details(app: &App, local: &str) {
        app.eval(
            "main",
            &format!("document.querySelector('[data-tile={local}]').click(); null"),
        );
    }
    /// The hidden window does not poll, and `begin` bypasses the page's refresh.
    fn shown(app: &App, local: &str) {
        app.eval("main", "window.dispatchEvent(new Event('focus')); null");
        installed(app, local);
    }
    fn runs(app: &App) {
        shown(app, "ratings");
        shown(app, "stats");
        open(app, "stats");
        until(
            app,
            "mod-stats",
            "document.querySelector('#result')?.textContent === '1'",
        );
    }
    #[test]
    fn ac5_s23_provider_first_then_pick_and_deny() {
        let mut f = Fixture::new(true);
        let first = f.eve();
        let app = start(first.path());
        begin_link(&app, &f.stats);
        consent(&app);
        let names = app.eval(
            "consent",
            "Array.from(document.querySelectorAll('main section h2'), n => n.textContent)",
        );
        assert_eq!(names, json!(["Ratings", "Stats"]));
        approve(&app);
        assert!(finish(&app).get("error").is_none());
        runs(&app);
        followed(first.path(), &f.carol.dist.identity().unwrap().to_string());
        assert!(app.quit().success());
        let picked = f.eve();
        let denied = f.eve();
        drop(f.carol.site.take());
        let stored = dist(picked.path());
        assert!(
            stored
                .followed()
                .unwrap()
                .iter()
                .all(|v| v.channel != f.frank.dist.identity().unwrap())
        );
        drop(stored);
        let app = start(picked.path());
        begin_link(&app, &f.stats);
        consent(&app);
        let text = body(&app, "consent");
        for value in [
            "ratings by Carol",
            "is not available",
            &format!("by Frank ({})", f.frank.dist.fingerprint().unwrap()),
            f.carol.link().base(),
        ] {
            assert!(text.contains(value), "{text}");
        }
        assert_eq!(
            app.eval(
                "consent",
                "document.querySelectorAll('input[type=radio]').length"
            ),
            1
        );
        app.eval(
            "consent",
            "document.querySelector('input[type=radio]').click(); null",
        );
        approve(&app);
        consent(&app);
        let text = body(&app, "consent");
        assert!(
            text.contains("your pick") && text.contains("Frank") && text.contains("Carol"),
            "{text}"
        );
        approve(&app);
        assert!(finish(&app).get("error").is_none());
        runs(&app);
        details(&app, "stats");
        let text = body(&app, "main");
        assert!(
            text.contains("of Frank ratings by Frank")
                && text.contains("in place of ratings by Carol"),
            "{text}"
        );
        followed(picked.path(), &f.frank.dist.identity().unwrap().to_string());
        assert!(app.quit().success());
        let app = start(denied.path());
        begin_link(&app, &f.stats);
        consent(&app);
        deny(&app);
        assert_eq!(finish(&app)["error"]["code"], "refused");
        empty(denied.path());
        assert!(app.quit().success());
    }
    #[test]
    fn ac6_s24_no_fit_opens_no_pick_and_installs_nothing() {
        let mut f = Fixture::new(false);
        let home = f.eve();
        drop(f.carol.site.take());
        let app = start(home.path());
        begin_link(&app, &f.stats);
        let result = finish(&app);
        assert_eq!(result["error"]["code"], "missing_providers");
        let error = result["error"]["message"].as_str().unwrap();
        assert!(
            error.contains("ratings by Carol") && error.contains("no mod in your indexes fits"),
            "{error}"
        );
        assert!(!app.windows().iter().any(|w| w["label"] == "consent"));
        assert!(
            !app.log()["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["cmd"] == "consent_get")
        );
        empty(home.path());
        assert!(app.quit().success());
    }
    #[test]
    fn ac7_s25_origin_and_derived_mods() {
        let alice = Publisher::new("Alice");
        let reading = alice.dist.publish(update_package("0.1.0", false)).unwrap();
        let carol = Publisher::new("Carol");
        let ratings = carol.publish(
            "ratings",
            "Ratings",
            None,
            Some(&reading.info.reference.to_string()),
        );
        let home = app_home();
        {
            let eve = dist(home.path());
            eve.follow(&alice.link()).unwrap();
            eve.follow(&carol.link()).unwrap()
        };
        let app = start(home.path());
        begin_link(&app, reading.link.as_ref().unwrap());
        consent(&app);
        approve(&app);
        finish(&app);
        shown(&app, "reading-list");
        begin_link(&app, ratings.link.as_ref().unwrap());
        consent(&app);
        let text = body(&app, "consent");
        assert!(
            text.contains("made from")
                && text.contains("(the publisher says so)")
                && text.contains(&reading.info.reference.to_string())
                && text.contains("Alice"),
            "{text}"
        );
        approve(&app);
        finish(&app);
        shown(&app, "ratings");
        details(&app, "ratings");
        details(&app, "reading-list");
        until(
            &app,
            "main",
            "document.querySelector('[data-local=ratings] .technical dd').textContent.includes('Reading list 0.1.0 by Alice')",
        );
        until(
            &app,
            "main",
            "document.querySelector('[data-local=reading-list] .derived').textContent.includes('Ratings')",
        );
        let text = body(&app, "main");
        assert!(
            text.contains("as far as your indexes list them") && text.contains("Made from this:")
        );
        assert_eq!(app.eval("main", "document.querySelectorAll('[data-local=reading-list] .derived .install-release').length"), 0);
        assert!(text.contains("Installed"));
        assert!(app.quit().success());
    }
}
