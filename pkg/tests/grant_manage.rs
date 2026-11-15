use std::{fs, path::Path};
use watane_pkg::*;
#[path = "support/release.rs"]
#[allow(dead_code)]
mod support;

const HOST: &str = "package watane:host@0.1.0;
interface state { get: func(key: string) -> option<list<u8>>; }
interface manage { names: func() -> list<string>; }";

fn hex(s: &str) -> Vec<u8> {
    data_encoding::HEXLOWER.decode(s.as_bytes()).unwrap()
}
fn put(dir: &Path, file: &str, bytes: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn a30() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "mod.toml",
        "name = 'Hello'\nsummary = 'Manages.'\n",
    );
    put(dir.path(), "ui/index.html", "<p>hi</p>\n");
    put(dir.path(), "logic/Cargo.toml", "");
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:hello@0.1.0;\ninterface api { count: func() -> u32; }\nworld hello { import watane:host/state@0.1.0; import watane:host/manage@0.1.0; export api; }",
    );
    put(dir.path(), "wit/deps/watane-host/host.wit", HOST);
    dir
}

#[test]
fn ac1_world_importing_manage_reads() {
    let dir = a30();
    let m = Mod::read(dir.path()).unwrap();
    assert!(m.world().manage);
    assert!(m.world().storage);
    assert_eq!(m.grants(), vec![Grant::Storage, Grant::Manage]);
}

#[test]
fn ac2_manage_json_and_text() {
    assert_eq!(
        serde_json::to_string(&Grant::Manage).unwrap(),
        r#"{"kind":"manage"}"#
    );
    assert_eq!(
        serde_json::from_str::<Grant>(r#"{"kind":"manage"}"#).unwrap(),
        Grant::Manage
    );
    assert_eq!(Grant::Manage.to_string(), "manage");
}

#[test]
fn ac3_manage_sorts_last_and_same_authority() {
    let subscribe = Grant::Subscribe {
        interface: "mods:reading-list/events@0.1.0".parse().unwrap(),
        provider: Provider {
            slug: "reading-list".parse().unwrap(),
            publisher: None,
        },
    };
    assert!(subscribe < Grant::Manage);
    assert!(Grant::Manage.same_authority(&Grant::Manage));
}

#[test]
fn ac4_release_body_carries_manage() {
    let golden = concat!(
        "8c016568656c6c6f65302e312e306548656c6c6f6b536179732068656c6c6f2e5820",
        "0866e9df433265eaf0d959c589d82f1c2020d5b07faf7a530b012ec9f62ff87b",
        "189ff4808080f6"
    );
    let mut r = Release::decode(&hex(golden)).unwrap();
    r.grants = vec![Grant::Storage, Grant::Manage];
    let mut expected = hex(golden);
    expected.splice(71..72, hex("8281008104"));
    assert_eq!(r.encode(), expected);
    assert_eq!(Release::decode(&expected).unwrap().grants, r.grants);

    // [4] before [3, ...] is refused as not ascending.
    let mut bad = hex(golden);
    let subscribe = format!(
        "8303781e6d6f64733a72656164696e672d6c6973742f6576656e747340302e312e305820{}",
        "01".repeat(32)
    );
    bad.splice(71..72, hex(&format!("828104{subscribe}")));
    let e = Release::decode(&bad).unwrap_err();
    assert!(e.to_string().contains("ascending"), "{e}");
}
