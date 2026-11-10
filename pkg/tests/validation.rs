use std::{
    fs,
    path::{Path, PathBuf},
};
use watane_pkg::*;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn temporary(name: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy(&fixture(name), dir.path());
    dir
}
fn put(dir: &Path, file: &str, text: impl AsRef<[u8]>) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn refused(dir: &Path, rule: Rule, file: &str) -> Vec<Violation> {
    let Err(ModError::Invalid(v)) = Mod::read(dir) else {
        panic!("expected {rule:?} in {file}: {:?}", Mod::read(dir))
    };
    assert!(
        v.iter().any(|v| v.rule == rule && v.file == file),
        "expected {rule:?} in {file}: {v:?}"
    );
    v
}

#[test]
fn ac3_rules_collect_independent_failures() {
    let dir = temporary("hello");
    put(dir.path(), "mod.toml", "unknown = 1");
    put(dir.path(), "deps.json", "[]");
    put(dir.path(), "wit/world.wit", "not WIT");
    fs::remove_file(dir.path().join("ui/index.html")).unwrap();
    let v = refused(dir.path(), Rule::ModToml, "mod.toml");
    for (rule, file) in [
        (Rule::DepsJson, "deps.json"),
        (Rule::Wit, "wit/world.wit"),
        (Rule::Parts, "."),
    ] {
        assert!(v.iter().any(|v| v.rule == rule && v.file == file), "{v:?}");
    }
    assert!(
        !v.iter()
            .any(|v| matches!(v.rule, Rule::Provider | Rule::Http))
    );
    let text = Mod::read(dir.path()).unwrap_err().to_string();
    assert!(text.contains("[mod-toml] mod.toml:"));
    assert_eq!(serde_json::to_string(&v[0].rule).unwrap(), "\"mod-toml\"");
    let io_path = dir.path().join("mod.toml");
    assert!(matches!(Mod::read(&io_path), Err(ModError::Io { .. })));
}

#[test]
fn ac3_rules_paths() {
    for file in [
        "wit/other/x.wit",
        "wit/deps/tags/deeper/x.wit",
        "wit/.hidden/x.wit",
        "wit/.hidden.wit",
        "wit/bad name.wit",
        "wit/deps/x.wasm",
        "wit/deps/x.wat",
    ] {
        let dir = temporary("hello");
        put(dir.path(), file, "");
        let path = match file {
            "wit/other/x.wit" => "wit/other",
            "wit/deps/tags/deeper/x.wit" => "wit/deps/tags/deeper",
            "wit/.hidden/x.wit" => "wit/.hidden",
            _ => file,
        };
        refused(dir.path(), Rule::Path, path);
    }
    let dir = temporary("hello");
    put(dir.path(), "wit/.DS_Store", "ignored");
    put(dir.path(), "wit/notes.txt", "ignored");
    Mod::read(dir.path()).unwrap();
}

#[cfg(unix)]
#[test]
fn ac3_rules_symlinks_never_followed() {
    use std::os::unix::fs::symlink;
    for file in [
        "mod.toml",
        "deps.json",
        "logic.wasm",
        "ui",
        "wit",
        "ui/app.js",
        "wit/deps/tags/world.wit",
        "wit/ignored.txt",
    ] {
        let dir = temporary("hello");
        let path = dir.path().join(file);
        if path.is_dir() {
            fs::remove_dir_all(&path).unwrap();
        } else if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink("missing-target", &path).unwrap();
        refused(dir.path(), Rule::Path, file);
    }
    let dir = temporary("hello");
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "sentinel", "unchanged");
    symlink(
        outside.path().join("sentinel"),
        dir.path().join("deps.json.tmp"),
    )
    .unwrap();
    assert!(Deps::default().write(dir.path()).is_err());
    assert_eq!(
        fs::read_to_string(outside.path().join("sentinel")).unwrap(),
        "unchanged"
    );
}

#[test]
fn ac3_rules_wit_byte_and_nesting_limits() {
    let dir = temporary("hello");
    fs::File::create(dir.path().join("wit/large.wit"))
        .unwrap()
        .set_len(1_048_577)
        .unwrap();
    refused(dir.path(), Rule::Limit, "wit/large.wit");
    let dir = temporary("hello");
    for file in ["a", "b", "c", "d"] {
        put(
            dir.path(),
            &format!("wit/{file}.wit"),
            vec![b' '; 1_048_576],
        );
    }
    refused(dir.path(), Rule::Limit, "wit/world.wit");
    // One brace plus 31 lists is depth 32; comments and arrows do not affect it.
    let dir = temporary("hello");
    let world = format!(
        "package mods:hello@0.1.0;\n/* {{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{ /* nested */ */\n// <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<\ninterface api {{ f: func() -> {}u8{}; }}\nworld hello {{ export api; }}",
        "list<".repeat(31),
        ">".repeat(31)
    );
    put(dir.path(), "wit/world.wit", &world);
    put(dir.path(), "logic/Cargo.toml", "");
    Mod::read(dir.path()).unwrap();
    put(
        dir.path(),
        "wit/world.wit",
        world
            .replace("list<list<", "list<list<list<")
            .replace("u8>", "u8>>"),
    );
    refused(dir.path(), Rule::Limit, "wit/world.wit");
}

#[test]
fn ac3_rules_wit_contracts() {
    for world in [
        "package mods:hello@0.1.0;",
        "package mods:hello@0.1.0; world a {} world b {}",
        "package wasi:hello@0.1.0; world hello {}",
        "package mods:hello; world hello {}",
        "package mods:%world@0.1.0; world hello {}",
        "package mods:hello@0.1.0-pre; world hello {}",
        "package mods:hello@0.1.0; world hello { import f: func(); }",
        "package mods:hello@0.1.0; world hello { export f: func(); }",
        "package mods:hello@0.1.0; world hello { import x: interface {}; }",
        "package mods:hello@0.1.0; interface api {} world hello { export x: api; }",
    ] {
        let dir = temporary("hello");
        put(dir.path(), "wit/world.wit", world);
        refused(dir.path(), Rule::Wit, "wit/world.wit");
    }
    let dir = temporary("hello");
    fs::remove_file(dir.path().join("wit/world.wit")).unwrap();
    refused(dir.path(), Rule::Wit, "wit/world.wit");
    for (name, text) in [
        ("watane-host", "package watane:host@0.2.0;"),
        ("alien", "package alien:pkg@0.1.0;"),
        ("unversioned", "package wasi:cli;"),
        ("wrong", "package mods:tags@0.1.0; world tags {}"),
    ] {
        let dir = temporary("hello");
        let file = format!("wit/deps/{name}/dep.wit");
        put(dir.path(), &file, text);
        refused(dir.path(), Rule::Wit, &file);
    }
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/deps/cli-a/cli.wit",
        "package wasi:cli@0.2.0; interface run {}",
    );
    put(
        dir.path(),
        "wit/deps/cli-b/cli.wit",
        "package wasi:cli@0.2.1; interface run {}",
    );
    let Err(ModError::Invalid(v)) = Mod::read(dir.path()) else {
        panic!("two versions must be refused")
    };
    assert!(v.iter().any(|v| v.rule == Rule::Wit));
}

#[test]
fn ac3_rules_provider_contracts() {
    for (world, wanted) in [
        (
            "package mods:reading-list@0.1.0; interface books {} interface events {}",
            Rule::Provider,
        ),
        (
            "package mods:reading-list@0.1.0; interface books {} interface events {} world a {} world b {}",
            Rule::Provider,
        ),
        (
            "package mods:reading-list@0.1.0; interface books {} interface events {} world a { import events; }",
            Rule::Provider,
        ),
        (
            "package mods:reading-list@0.1.0; interface books {} interface events {} world a { export books; }",
            Rule::Provider,
        ),
    ] {
        let dir = temporary("recommendations");
        put(dir.path(), "wit/deps/reading-list/world.wit", world);
        refused(dir.path(), wanted, "deps.json");
    }
    for deps in [
        "{}",
        "{\"mods:reading-list\":null,\"mods:tags\":null}",
        "{\"mods:reading-list\":null,\"mods:recommendations\":null}",
    ] {
        let dir = temporary("recommendations");
        put(dir.path(), "deps.json", deps);
        refused(dir.path(), Rule::Provider, "deps.json");
    }
    let dir = temporary("recommendations");
    put(
        dir.path(),
        "deps.json",
        "{\"mods:reading-list\":\"yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo\"}",
    );
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(m.requires().len(), 1);
    assert_eq!(
        m.requires()[0].provider.to_string(),
        "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list"
    );
    assert_eq!(
        m.grants()[1].to_string(),
        "call mods:reading-list/books@0.1.0 from yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list"
    );
}

#[test]
fn ac3_grants_elaborated_type_imports() {
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:hello@0.1.0; interface api { use mods:tags/types@0.1.0.{tag}; f: func(t: tag); } world hello { export api; }",
    );
    put(
        dir.path(),
        "wit/deps/tags/tags.wit",
        "package mods:tags@0.1.0; interface types { type tag = string; } world tags { export types; }",
    );
    put(dir.path(), "logic/Cargo.toml", "");
    put(dir.path(), "deps.json", "{\"mods:tags\":null}");
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(
        m.grants()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["call mods:tags/types@0.1.0 from local/tags"]
    );
}

#[test]
fn ac3_grants_base_wasi_and_caller() {
    let dir = temporary("reading-list");
    let mut imports = String::from("import watane:host/caller@0.1.0;");
    for pkg in ["cli", "clocks", "filesystem", "io", "random", "sockets"] {
        put(
            dir.path(),
            &format!("wit/deps/{pkg}/pkg.wit"),
            format!("package wasi:{pkg}@0.2.7; interface api {{}}"),
        );
        imports.push_str(&format!("import wasi:{pkg}/api@0.2.7;"));
    }
    put(
        dir.path(),
        "wit/world.wit",
        format!(
            "package mods:reading-list@0.1.0; interface api {{}} world reading-list {{ {imports} export api; }}"
        ),
    );
    assert!(Mod::read(dir.path()).unwrap().grants().is_empty());
    put(
        dir.path(),
        "wit/deps/http/http.wit",
        "package wasi:http@0.3.0; interface outgoing-handler {}",
    );
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:reading-list@0.1.0; interface api {} world reading-list { import wasi:http/outgoing-handler@0.3.0; export api; }",
    );
    refused(dir.path(), Rule::Import, "wit/world.wit");
}

#[test]
fn ac3_rules_all_world_list_limits() {
    for (own, import) in [(true, true), (true, false), (false, true), (false, false)] {
        for count in [32, 33] {
            let dir = temporary("hello");
            let definitions = (0..count)
                .map(|n| format!("interface i{n} {{}}"))
                .collect::<String>();
            let items = (0..count)
                .map(|n| {
                    format!(
                        "{} {}i{n}{};",
                        if import { "import" } else { "export" },
                        if own { "" } else { "mods:tags/" },
                        if own { "" } else { "@0.1.0" }
                    )
                })
                .collect::<String>();
            let mut text = String::from("package mods:hello@0.1.0;");
            if own {
                text.push_str(&definitions);
            }
            text.push_str("interface api {} world hello {");
            text.push_str(&items);
            if import {
                text.push_str("export api;");
            }
            text.push('}');
            put(dir.path(), "wit/world.wit", text);
            put(dir.path(), "logic/Cargo.toml", "");
            if !own {
                let provider_items = (0..count)
                    .map(|n| format!("{} i{n};", if import { "export" } else { "import" }))
                    .collect::<String>();
                put(
                    dir.path(),
                    "wit/deps/tags/tags.wit",
                    format!(
                        "package mods:tags@0.1.0; {definitions} world tags {{ {provider_items} }}"
                    ),
                );
                put(dir.path(), "deps.json", "{\"mods:tags\":null}");
            }
            if count == 33 {
                refused(dir.path(), Rule::Limit, "wit/world.wit");
            } else {
                Mod::read(dir.path()).unwrap();
            }
        }
    }
}

#[test]
fn ac5_parts_match_world_and_files() {
    for (case, file, text) in [
        ("hello", "logic.wasm", "unbuilt"),
        ("hello", "logic/Cargo.toml", ""),
        (
            "hello",
            "wit/world.wit",
            "package mods:hello@0.1.0; interface events {} world hello { import events; }",
        ),
        (
            "hello",
            "wit/world.wit",
            "package mods:hello@0.1.0; interface api {} world hello { export api; }",
        ),
    ] {
        let dir = temporary(case);
        put(dir.path(), file, text);
        refused(dir.path(), Rule::Parts, ".");
    }
    let dir = temporary("hello");
    fs::create_dir_all(dir.path().join("logic/Cargo.toml")).unwrap();
    refused(dir.path(), Rule::Parts, ".");
}

#[test]
fn ac3_identity_package_line_is_in_world_file() {
    let dir = temporary("hello");
    put(dir.path(), "wit/world.wit", "world hello {}");
    put(dir.path(), "wit/identity.wit", "package mods:hello@0.1.0;");
    refused(dir.path(), Rule::Wit, "wit/world.wit");
}

#[test]
fn ac2_mod_toml_additional_schema_and_origin() {
    let dir = temporary("hello");
    let good = "name = 'Hello'\nsummary = 'Says hello.'\n";
    for extra in [
        "http_hosts = []",
        "origin_table = {}",
        "[origin]\nrelease = 'bad'\nunknown = 1",
        "[http]\nhosts = ['a']\nunknown = 1",
    ] {
        put(dir.path(), "mod.toml", format!("{good}{extra}"));
        refused(dir.path(), Rule::ModToml, "mod.toml");
    }
    let hosts = (0..17)
        .map(|n| format!("'a{n}.example'"))
        .collect::<Vec<_>>()
        .join(",");
    put(
        dir.path(),
        "mod.toml",
        format!("{good}[http]\nhosts = [{hosts}]"),
    );
    refused(dir.path(), Rule::ModToml, "mod.toml");
    put(
        dir.path(),
        "mod.toml",
        format!(
            "{good}[origin]\nrelease = 'yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list@0.1.0'"
        ),
    );
    assert_eq!(
        Mod::read(dir.path()).unwrap().origin().unwrap().to_string(),
        "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list@0.1.0"
    );
    put(
        dir.path(),
        "mod.toml",
        format!("{good}[http]\nhosts = ['a.example']"),
    );
    refused(dir.path(), Rule::Http, "mod.toml");
}

#[test]
fn ac3_world_spans_files_and_accepts_types() {
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/world.wit",
        "// ; ignored\n/* ; /* nested ; */ */ package mods:hello@0.1.0;\nworld hello { type number = u32; export api; }",
    );
    put(
        dir.path(),
        "wit/api.wit",
        "package mods:hello@0.1.0; interface api { count: func() -> u32; }",
    );
    put(dir.path(), "logic/Cargo.toml", "");
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(
        m.world()
            .exports
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["mods:hello/api@0.1.0"]
    );
    assert!(m.grants().is_empty());
    // Built bytes are checked only by stage 4, but their presence selects the same part.
    fs::remove_file(dir.path().join("logic/Cargo.toml")).unwrap();
    put(dir.path(), "logic.wasm", b"not yet checked");
    assert_eq!(
        Mod::read(dir.path()).unwrap().logic(),
        Some(dir.path().join("logic.wasm"))
    );
}

#[test]
fn ac3_rules_multiple_semantic_failures() {
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:hello@0.1.0; world hello { import wasi:keyvalue/store@0.2.0; export wasi:keyvalue/store@0.2.0; }",
    );
    put(
        dir.path(),
        "wit/deps/keyvalue/keyvalue.wit",
        "package wasi:keyvalue@0.2.0; interface store {}",
    );
    put(dir.path(), "deps.json", "{\"mods:unused\":null}");
    put(
        dir.path(),
        "mod.toml",
        "name = 'Hello'\nsummary = 'Hello.'\n[http]\nhosts = ['a.example']",
    );
    let v = refused(dir.path(), Rule::Import, "wit/world.wit");
    for (rule, file) in [
        (Rule::Export, "wit/world.wit"),
        (Rule::Provider, "deps.json"),
        (Rule::Http, "mod.toml"),
        (Rule::Parts, "."),
    ] {
        assert!(v.iter().any(|v| v.rule == rule && v.file == file), "{v:?}");
    }
}

#[test]
fn ac2_schema_boundaries_and_duplicates() {
    let dir = temporary("hello");
    let mut text = "name = 'Hello'\nsummary = 'Hello.'\n".to_string();
    text.push_str(&" ".repeat(16384 - text.len()));
    put(dir.path(), "mod.toml", text);
    Mod::read(dir.path()).unwrap();
    let mut json = "{}".to_string();
    json.push_str(&" ".repeat(65536 - json.len()));
    put(dir.path(), "deps.json", json);
    Deps::read(dir.path()).unwrap();
    put(
        dir.path(),
        "deps.json",
        r#"{"mods:tags":null,"mods:tags":null}"#,
    );
    assert!(
        matches!(Deps::read(dir.path()), Err(ModError::Invalid(v)) if v[0].rule == Rule::DepsJson)
    );
    let mut deps = Deps::default();
    for i in 0..33 {
        deps.set(format!("p{i}").parse().unwrap(), None);
    }
    assert!(deps.write(dir.path()).is_err());
}
