use std::fs;
use watane_pkg::{Mod, ModError, Rule};

fn read(menu: &str) -> Result<Mod, ModError> {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("wit")).unwrap();
    fs::write(
        dir.path().join("mod.toml"),
        "name = \"Shell\"\nsummary = \"A shell.\"\n",
    )
    .unwrap();
    fs::write(dir.path().join("logic.wasm"), b"\0asm").unwrap();
    fs::write(
        dir.path().join("wit/world.wit"),
        format!(
            "package mods:shell@0.1.0;\ninterface menu {{ {menu} }}\ninterface api {{ f: func(); }}\nworld shell {{ import menu; export api; }}\n"
        ),
    )
    .unwrap();
    let m = Mod::read(dir.path());
    drop(dir);
    m
}
fn refusal(menu: &str) -> String {
    let Err(ModError::Invalid(v)) = read(menu) else {
        panic!("{menu} must be refused")
    };
    let v = v.iter().find(|v| v.rule == Rule::Import).unwrap();
    v.message.clone()
}

#[test]
fn ac1_event_and_collect_outlet() {
    let m = read("ask: func() -> list<string>; clicked: func(id: string);").unwrap();
    let menu = vec!["mods:shell/menu@0.1.0".parse().unwrap()];
    assert_eq!(m.world().events, menu);
    assert_eq!(m.world().collects, menu);
    let only = read("clicked: func(id: string);").unwrap();
    assert_eq!(m.grants(), only.grants());
}

#[test]
fn ac2_non_list_result_is_refused() {
    let msg = refusal("ask: func() -> string;");
    assert_eq!(
        msg,
        "menu.ask: a function of an own interface that the world imports returns nothing (an event) or a list (a collect)"
    );
}

#[test]
fn ac3_kinds_of_result() {
    let m = read("clicked: func(id: string);").unwrap();
    assert!(m.world().collects.is_empty());
    assert!(
        read("a: func() -> list<u8>;")
            .unwrap()
            .world()
            .collects
            .len()
            == 1
    );
    let m = read("record r { x: u32 } a: func() -> list<r>;").unwrap();
    assert_eq!(m.world().collects.len(), 1);
    for bad in [
        "a: func() -> result<list<u8>, string>;",
        "a: func() -> option<list<u8>>;",
        "a: func() -> tuple<u8, u8>;",
    ] {
        assert!(refusal(bad).starts_with("menu.a:"), "{bad}");
    }
}
