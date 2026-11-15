use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
fn run(dir: &Path, args: &[&str]) -> (i32, Value) {
    let o = Command::new(env!("CARGO_BIN_EXE_watane-dev"))
        .current_dir(dir)
        .env("WATANE_DEV_CACHE", dir.join("cache"))
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}
#[test]
fn ac2_check_counter_mod_and_rejections() {
    let t = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("missing", t.path().join("outside-link")).unwrap();
    let (c, v) = run(t.path(), &["check", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(v["errors"][0]["code"], "usage.not_a_mod");
    fs::remove_file(t.path().join("outside-link")).unwrap();
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 0);
    assert_eq!(
        v["result"],
        json!({"slug":"hello","version":"0.1.0","name":"Hello","page":true,"logic":true,"grants":["storage"],"exports":["mods:hello/api@0.1.0"],"events":["mods:hello/changes@0.1.0"],"collects":[],"calls":[],"subscriptions":[],"providers":[]})
    );
    fs::write(dir.join("mod.toml"), "name = \"Hello\"\n").unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 1);
    assert_eq!(v["errors"][0]["code"], "mod.invalid");
    assert_eq!(v["errors"][0]["file"], "mod.toml");
    fs::write(
        dir.join("mod.toml"),
        "name = \"Hello\"\nsummary = \"Hello page\"\n",
    )
    .unwrap();
    std::os::unix::fs::symlink("missing", dir.join("link")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 1);
    assert_eq!(v["errors"][0]["code"], "mod.symlink");
    assert_eq!(v["errors"][0]["file"], "link");
}
#[test]
fn check_missing_dir_and_git_symlink() {
    let t = tempfile::tempdir().unwrap();
    let (c, v) = run(t.path(), &["--dir", "nowhere", "check", "--json"]);
    assert_eq!(c, 2);
    assert_eq!(v["errors"][0]["code"], "usage.not_a_mod");
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    fs::create_dir(dir.join(".git")).unwrap();
    std::os::unix::fs::symlink("missing", dir.join(".git/link")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 0, "{v}");
}

#[test]
fn ac2_check_repairs_host_wit() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &["new", "hello", "--json"]).0, 0);
    let dir = t.path().join("hello");
    fs::write(dir.join("wit/deps/watane-host/host.wit"), "changed").unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 0, "{value}");
    assert_eq!(value["ok"], true);
    assert_eq!(
        fs::read_to_string(dir.join("wit/deps/watane-host/host.wit")).unwrap(),
        watane_host::HOST_WIT
    );
    assert!(watane_host::HOST_WIT.contains("interface manage"));
}

#[test]
fn ac4_unused_provider_message_and_fix_respect_managed_files() {
    let t = tempfile::tempdir().unwrap();
    for slug in ["consumer", "provider"] {
        assert_eq!(run(t.path(), &["new", slug, "--json"]).0, 0);
    }
    let dir = t.path().join("consumer");
    assert_eq!(run(&dir, &["add", "../provider", "--json"]).0, 0);
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|error| error["file"] == "deps.json")
        .unwrap();
    assert_eq!(
        error["message"],
        "[provider] deps.json lists mods:provider but wit/world.wit imports none of its interfaces"
    );
    assert_eq!(
        error["fix"],
        "Import the provider's interface in wit/world.wit, or ask before deleting wit/deps/provider/ and mods:provider from deps.json"
    );
}

#[test]
fn ac4_own_provider_message_does_not_claim_own_events_are_unused() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &["new", "consumer", "--json"]).0, 0);
    let dir = t.path().join("consumer");
    let world = fs::read_to_string(dir.join("wit/world.wit")).unwrap();
    fs::write(
        dir.join("wit/world.wit"),
        world.replace(
            "package mods:",
            "/* package mods:wrong@1.0.0; /* nested */ */\npackage /* identity */ mods:",
        ),
    )
    .unwrap();
    fs::write(dir.join("deps.json"), r#"{"mods:consumer":null}"#).unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|error| error["file"] == "deps.json")
        .unwrap();
    assert_eq!(
        error["message"],
        "[provider] deps.json lists this mod itself as mods:consumer"
    );
    assert_eq!(
        error["fix"],
        "Ask before deleting mods:consumer from deps.json: a mod cannot depend on itself"
    );
}

#[test]
fn ac6_own_package_copy_in_wit_deps_gets_its_own_fix() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &["new", "consumer", "--json"]).0, 0);
    let dir = t.path().join("consumer");
    fs::create_dir(dir.join("wit/deps/consumer")).unwrap();
    fs::copy(
        dir.join("wit/world.wit"),
        dir.join("wit/deps/consumer/world.wit"),
    )
    .unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|error| error["file"] == "wit/deps/consumer")
        .unwrap_or_else(|| panic!("{value}"));
    assert_eq!(
        error["message"],
        "[provider] wit/deps/consumer holds this mod's own package mods:consumer"
    );
    assert_eq!(
        error["fix"],
        "Ask before deleting wit/deps/consumer/: a mod cannot depend on itself"
    );
}

#[test]
fn ac7_deps_only_own_slug_never_suggests_importing_itself() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("consumer");
    watane_dev::template::new_mod(&dir, "consumer").unwrap();
    fs::remove_dir_all(dir.join("logic")).unwrap();
    fs::write(
        dir.join("wit/world.wit"),
        "package mods:consumer@0.1.0; world consumer {}",
    )
    .unwrap();
    fs::write(dir.join("deps.json"), r#"{"mods:consumer":null}"#).unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["file"] == "deps.json")
        .unwrap();
    assert_eq!(
        error["fix"],
        "Ask before deleting mods:consumer from deps.json: a mod cannot depend on itself"
    );
}

#[test]
fn ac7_own_package_in_different_folder_names_actual_folder() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("consumer");
    watane_dev::template::new_mod(&dir, "consumer").unwrap();
    fs::create_dir_all(dir.join("wit/deps/renamed")).unwrap();
    fs::copy(
        dir.join("wit/world.wit"),
        dir.join("wit/deps/renamed/world.wit"),
    )
    .unwrap();
    let (code, value) = run(&dir, &["check", "--json"]);
    assert_eq!(code, 1, "{value}");
    let error = value["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["file"] == "wit/deps/renamed")
        .unwrap_or_else(|| panic!("{value}"));
    assert_eq!(
        error["message"],
        "[provider] wit/deps/renamed holds this mod's own package mods:consumer"
    );
    assert_eq!(
        error["fix"],
        "Ask before deleting wit/deps/renamed/: a mod cannot depend on itself"
    );
}

#[test]
fn ac1_ac4_collect_outlet_named_like_slug() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(run(t.path(), &["new", "menu", "--json"]).0, 0);
    let dir = t.path().join("menu");
    let world = "package mods:menu@0.1.0;\ninterface menu { ask: func() -> list<string>; }\ninterface api { items: func() -> list<string>; }\nworld WORLD { import menu; export api; }\n";
    fs::write(dir.join("wit/world.wit"), world.replace("WORLD", "menu")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 1, "{v}");
    assert_eq!(v["errors"][0]["code"], "mod.invalid");
    assert!(
        v["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("duplicate item named `menu`"),
        "{v}"
    );
    let fix = v["errors"][0]["fix"].as_str().unwrap();
    assert!(
        fix.contains("rename the world")
            && fix.contains("world menu-mod")
            && fix.contains("package line keeps the slug"),
        "{fix}"
    );
    fs::write(dir.join("wit/world.wit"), world.replace("WORLD", "shell")).unwrap();
    let (c, v) = run(&dir, &["check", "--json"]);
    assert_eq!(c, 0, "{v}");
    assert_eq!(v["result"]["collects"], json!(["mods:menu/menu@0.1.0"]));
    assert_eq!(v["result"]["events"], json!(["mods:menu/menu@0.1.0"]));
}
