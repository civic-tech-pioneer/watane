#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::Path;
use watane_pkg::*;

const HOST: &str = "package watane:host@0.1.0;
interface state { get: func(key: string) -> option<list<u8>>; }
interface streams { open: func() -> u32; }
interface claude-types { type id = u32; }
interface claude-session { say: func(text: string); }
interface agent-tools { open: func() -> u32; }
interface timers { now: func() -> u32; }
interface caller { identity: func() -> string; }
interface https-stream { start: func() -> u32; }
interface unknown { f: func(); }
interface stream-sink { push: func(); }
interface stream-producer { signal: func(); }
interface timer-sink { fired: func(); }";

fn put(dir: &Path, file: &str, bytes: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn build(items: &str, toml: &str, page: bool, own_export: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "mod.toml",
        &format!("name = 'Hello'\nsummary = 'Agent.'\n{toml}"),
    );
    if page {
        put(dir.path(), "ui/index.html", "<p>hi</p>\n");
    }
    put(dir.path(), "logic/Cargo.toml", "");
    let own = if own_export { "export api; " } else { "" };
    put(
        dir.path(),
        "wit/world.wit",
        &format!(
            "package mods:hello@0.1.0;\ninterface api {{ count: func() -> u32; }}\nworld hello {{ {items} {own} }}"
        ),
    );
    put(dir.path(), "wit/deps/watane-host/host.wit", HOST);
    dir
}
fn rules(dir: &Path) -> Vec<Rule> {
    match Mod::read(dir) {
        Err(ModError::Invalid(v)) => v.into_iter().map(|v| v.rule).collect(),
        Ok(_) => vec![],
        Err(e) => panic!("{e}"),
    }
}
fn i(s: &str) -> InterfaceName {
    s.parse().unwrap()
}

#[test]
fn ac2_full_agent_world_gives_storage_only() {
    let items = "import watane:host/state@0.1.0; import watane:host/streams@0.1.0;
        import watane:host/claude-types@0.1.0; import watane:host/claude-session@0.1.0;
        import watane:host/agent-tools@0.1.0; import watane:host/timers@0.1.0;
        import watane:host/caller@0.1.0; export watane:host/stream-sink@0.1.0;
        export watane:host/stream-producer@0.1.0; export watane:host/timer-sink@0.1.0;";
    let dir = build(items, "", true, true);
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(m.grants(), vec![Grant::Storage]);
    assert_eq!(
        m.world().host_exports,
        vec![
            i("watane:host/stream-producer@0.1.0"),
            i("watane:host/stream-sink@0.1.0"),
            i("watane:host/timer-sink@0.1.0"),
        ]
    );
    assert_eq!(m.world().exports, vec![i("mods:hello/api@0.1.0")]);
}

#[test]
fn ac2_https_stream_falls_under_http() {
    let items = "import watane:host/state@0.1.0; import watane:host/https-stream@0.1.0;";
    let dir = build(items, "[http]\nhosts = [\"api.openai.com\"]\n", true, true);
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(
        m.grants(),
        vec![
            Grant::Storage,
            Grant::Http {
                host: "api.openai.com".parse().unwrap()
            }
        ]
    );
    assert_eq!(rules(build(items, "", true, true).path()), vec![Rule::Http]);
    let no_import = build(
        "import watane:host/state@0.1.0;",
        "[http]\nhosts = [\"api.openai.com\"]\n",
        true,
        true,
    );
    assert_eq!(rules(no_import.path()), vec![Rule::Http]);
}

#[test]
fn ac2_wrong_direction_and_unknown_are_refused() {
    for item in [
        "export watane:host/state@0.1.0;",
        "export watane:host/timers@0.1.0;",
    ] {
        assert_eq!(
            rules(build(item, "", true, true).path()),
            vec![Rule::Export]
        );
    }
    let dir = build("import watane:host/unknown@0.1.0;", "", true, true);
    assert_eq!(rules(dir.path()), vec![Rule::Import]);
}

#[test]
fn ac2_host_exports_alone_are_a_logic_part() {
    let items = "export watane:host/stream-sink@0.1.0; export watane:host/timer-sink@0.1.0;";
    let dir = build(items, "", false, false);
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(
        m.world().host_exports,
        vec![
            i("watane:host/stream-sink@0.1.0"),
            i("watane:host/timer-sink@0.1.0")
        ]
    );
    assert!(m.world().exports.is_empty());
    assert!(m.page().is_none());
    fs::remove_dir_all(dir.path().join("logic")).unwrap();
    assert!(rules(dir.path()).iter().all(|r| *r == Rule::Parts));
    assert!(!rules(dir.path()).is_empty());
    let none = build("", "", false, false);
    assert!(rules(none.path()).iter().all(|r| *r == Rule::Parts));
    assert!(!rules(none.path()).is_empty());
}
