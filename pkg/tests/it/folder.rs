use std::path::{Path, PathBuf};
use watane_pkg::*;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn ac4_hello_page_only() {
    let dir = fixture("hello");
    let m = Mod::read(&dir).unwrap();
    assert_eq!(m.dir(), dir);
    assert_eq!(m.slug().as_str(), "hello");
    assert_eq!(m.version().to_string(), "0.1.0");
    assert_eq!(m.name(), "Hello");
    assert_eq!(m.summary(), "Says hello.");
    assert_eq!(m.page(), Some(dir.join("ui")));
    assert_eq!(m.logic(), None);
    assert_eq!(
        m.world(),
        &World {
            exports: vec![],
            events: vec![],
            collects: vec![],
            calls: vec![],
            subscriptions: vec![],
            storage: false,
            manage: false,
            http: false
        }
    );
    assert!(m.grants().is_empty());
    assert!(m.requires().is_empty());
    assert!(m.http_hosts().is_empty());
    assert!(m.origin().is_none());
    assert_eq!(m.deps(), &Deps::default());
}

#[test]
fn ac5_empty_and_page_without_index_are_parts() {
    for name in ["empty", "page-no-index"] {
        let Err(ModError::Invalid(v)) = Mod::read(&fixture(name)) else {
            panic!("{name} must be invalid")
        };
        assert!(
            v.iter().any(|v| v.rule == Rule::Parts && v.file == "."),
            "{v:?}"
        );
    }
}

#[test]
fn ac3_rules_fixtures() {
    for (case, rule, file) in [
        ("path", Rule::Path, "wit/deps/evil.wat"),
        ("limit", Rule::Limit, "wit/world.wit"),
        ("mod-toml", Rule::ModToml, "mod.toml"),
        ("deps-json", Rule::DepsJson, "deps.json"),
        ("wit", Rule::Wit, "wit/world.wit"),
        ("import", Rule::Import, "wit/world.wit"),
        ("export", Rule::Export, "wit/world.wit"),
        ("provider", Rule::Provider, "deps.json"),
        ("http", Rule::Http, "mod.toml"),
        ("empty", Rule::Parts, "."),
    ] {
        let Err(ModError::Invalid(v)) = Mod::read(&fixture(case)) else {
            panic!("{case} must be invalid")
        };
        assert!(
            v.iter().any(|v| v.rule == rule && v.file == file),
            "{case}: {v:?}"
        );
    }
    let Err(ModError::Invalid(v)) = Mod::read(&fixture("import")) else {
        unreachable!()
    };
    assert!(
        v.iter()
            .any(|v| v.message.contains("wasi:keyvalue/store@0.2.0"))
    );
}

#[test]
fn ac3_grants_from_world() {
    let reading = Mod::read(&fixture("reading-list")).unwrap();
    assert_eq!(reading.slug().as_str(), "reading-list");
    assert_eq!(reading.version().to_string(), "0.1.0");
    assert_eq!(reading.grants(), [Grant::Storage]);
    assert_eq!(
        reading
            .world()
            .exports
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["mods:reading-list/books@0.1.0"]
    );
    assert_eq!(
        reading
            .world()
            .events
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["mods:reading-list/events@0.1.0"]
    );
    assert_eq!(reading.page(), Some(fixture("reading-list").join("ui")));
    assert_eq!(
        reading.logic(),
        Some(fixture("reading-list").join("logic.wasm"))
    );
    let recommendations = Mod::read(&fixture("recommendations")).unwrap();
    assert_eq!(
        recommendations
            .grants()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [
            "storage",
            "call mods:reading-list/books@0.1.0 from local/reading-list",
            "subscribe mods:reading-list/events@0.1.0 from local/reading-list"
        ]
    );
    // deps.json of the fixture is {"mods:reading-list": null} (TECHNICAL.md §9).
    assert_eq!(
        recommendations
            .deps()
            .iter()
            .map(|(s, p)| (s.as_str(), p))
            .collect::<Vec<_>>(),
        [("reading-list", None)]
    );
    assert_eq!(
        recommendations.requires(),
        [Requirement {
            provider: Provider {
                slug: "reading-list".parse().unwrap(),
                publisher: None
            },
            version: "0.1.0".parse().unwrap()
        }]
    );
    let http = Mod::read(&fixture("http-valid")).unwrap();
    assert!(http.world().http);
    assert_eq!(
        http.http_hosts()
            .iter()
            .map(HttpHost::as_str)
            .collect::<Vec<_>>(),
        ["a.example", "b.example"]
    );
    assert_eq!(
        http.grants()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["http a.example", "http b.example"]
    );
}
