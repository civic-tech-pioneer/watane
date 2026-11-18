#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::{Path, PathBuf};
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
    Deps::default().write(dir.path()).unwrap();
    assert_eq!(
        fs::read_to_string(outside.path().join("sentinel")).unwrap(),
        "unchanged"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("deps.json")).unwrap(),
        "{}\n"
    );
    fs::symlink_metadata(dir.path().join("deps.json.tmp")).unwrap_err();
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
fn ac1_s33_own_provider_copy_reports_deps_once() {
    let dir = temporary("hello");
    let world = "package mods:s@0.1.0; world s {}";
    put(dir.path(), "wit/world.wit", world);
    put(dir.path(), "wit/deps/s/world.wit", world);
    put(dir.path(), "deps.json", r#"{"mods:s":null}"#);
    let v = refused(dir.path(), Rule::Provider, "deps.json");
    assert_eq!(
        v,
        [Violation {
            rule: Rule::Provider,
            file: "deps.json".into(),
            message: "deps.json lists this mod itself as mods:s".into(),
        }]
    );
}

#[test]
fn ac2_s33_own_provider_copy_without_deps_reports_folder() {
    for deps in [None, Some("{}")] {
        let dir = temporary("hello");
        let world = "package mods:s@0.1.0; world s {}";
        put(dir.path(), "wit/world.wit", world);
        put(dir.path(), "wit/deps/s/world.wit", world);
        if let Some(deps) = deps {
            put(dir.path(), "deps.json", deps);
        }
        let v = refused(dir.path(), Rule::Provider, "wit/deps/s");
        assert_eq!(
            v,
            [Violation {
                rule: Rule::Provider,
                file: "wit/deps/s".into(),
                message: "wit/deps/s holds this mod's own package mods:s".into(),
            }]
        );
    }
}

#[test]
fn regress_s33_own_copy_other_version() {
    // A hand-made wit/deps/<own slug>/ copied from another version of the mod
    // is still the mod's own package; it must not fall through to the WIT
    // "repeated package" error (which dev answers with the `add` fix).
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:s@0.2.0; world s {}",
    );
    put(
        dir.path(),
        "wit/deps/s/world.wit",
        "package mods:s@0.1.0; world s {}",
    );
    let v = refused(dir.path(), Rule::Provider, "wit/deps/s");
    assert_eq!(
        v,
        [Violation {
            rule: Rule::Provider,
            file: "wit/deps/s".into(),
            message: "wit/deps/s holds this mod's own package mods:s".into(),
        }]
    );
}

#[test]
fn ac1_s31_unused_provider_message() {
    let dir = temporary("hello");
    put(dir.path(), "deps.json", r#"{"mods:unused":null}"#);
    let v = refused(dir.path(), Rule::Provider, "deps.json");
    assert_eq!(
        v,
        [Violation {
            rule: Rule::Provider,
            file: "deps.json".into(),
            message: "deps.json lists mods:unused but wit/world.wit imports none of its interfaces"
                .into(),
        }]
    );
}

#[test]
fn ac2_s31_own_provider_message_once() {
    for fixture in ["hello", "reading-list"] {
        let dir = temporary(fixture);
        put(
            dir.path(),
            "deps.json",
            format!(r#"{{"mods:{fixture}":null}}"#),
        );
        let v = refused(dir.path(), Rule::Provider, "deps.json");
        assert_eq!(
            v,
            [Violation {
                rule: Rule::Provider,
                file: "deps.json".into(),
                message: format!("deps.json lists this mod itself as mods:{fixture}"),
            }]
        );
    }
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

#[test]
fn ac2_http_host_count_limit_is_inclusive() {
    // TECHNICAL.md §4.2: [http] hosts holds 1..=16 hosts.
    let dir = temporary("http-valid");
    let hosts = (0..16)
        .map(|n| format!("'a{n:02}.example'"))
        .collect::<Vec<_>>()
        .join(",");
    put(
        dir.path(),
        "mod.toml",
        format!("name = 'Hello'\nsummary = 'Says hello.'\n[http]\nhosts = [{hosts}]"),
    );
    let m = Mod::read(dir.path()).unwrap();
    assert_eq!(m.http_hosts().len(), 16);
    assert_eq!(m.http_hosts()[15].as_str(), "a15.example");
}

#[test]
fn ac3_rules_wit_total_limit_is_inclusive() {
    // TECHNICAL.md §4.7: all .wit files together are at most 4 MiB; each at most 1 MiB.
    let dir = temporary("hello");
    let world = fs::read(dir.path().join("wit/world.wit")).unwrap();
    for file in ["a", "b", "c"] {
        put(
            dir.path(),
            &format!("wit/{file}.wit"),
            vec![b' '; 1_048_576],
        );
    }
    put(dir.path(), "wit/d.wit", vec![b' '; 1_048_576 - world.len()]);
    Mod::read(dir.path()).unwrap();
    put(
        dir.path(),
        "wit/d.wit",
        vec![b' '; 1_048_576 - world.len() + 1],
    );
    refused(dir.path(), Rule::Limit, "wit/world.wit");
}

// The message of the `wit` violation for a world that wit-parser itself refuses.
fn wit_message(dir: &Path) -> String {
    let v = refused(dir, Rule::Wit, "wit/world.wit");
    v.into_iter().find(|v| v.rule == Rule::Wit).unwrap().message
}
fn parser_message(dir: &Path) -> String {
    let error = wit_parser::Resolve::default()
        .push_dir(dir.join("wit"))
        .unwrap_err();
    format!("{error:#}")
}

#[test]
fn ac3_rules_wit_message_is_first_2000_bytes() {
    // TECHNICAL.md §4.4 step 2: the violation carries the first 2000 bytes of the
    // parser message. The oracle is wit-parser itself, called directly.
    let dir = temporary("hello");
    put(
        dir.path(),
        "wit/world.wit",
        "package mods:hello@0.1.0;\ninterface api { type t = missing; }\nworld hello {}\n",
    );
    let expected = parser_message(dir.path());
    assert!(expected.len() < 2000);
    let message = wit_message(dir.path());
    assert!(message.starts_with(&expected), "{message}");
    let arrow = format!(" --> {}:2:26", dir.path().join("wit/world.wit").display());
    assert!(message.contains(&arrow), "{message}");

    let name = "a".repeat(3000);
    put(
        dir.path(),
        "wit/world.wit",
        format!(
            "package mods:hello@0.1.0;\ninterface api {{ type t = {name}; }}\nworld hello {{}}\n"
        ),
    );
    let expected = parser_message(dir.path());
    assert!(expected.len() > 2000 && expected.is_char_boundary(2000));
    let message = wit_message(dir.path());
    assert!(message.len() <= 2000 && message.is_char_boundary(message.len()));
    // A snippet line over 500 bytes makes wit-parser print `path:line:col: error` (no
    // `-->` form); the location sits after the oracle's prefix and the cut keeps 2000 bytes.
    let prefix = format!(
        "failed to parse package: {}: ",
        dir.path().join("wit").display()
    );
    assert!(expected.starts_with(&prefix));
    assert!(message.starts_with(&prefix), "{message}");
    let location = format!(
        "{}:2:26: type `aaaa",
        dir.path().join("wit/world.wit").display()
    );
    assert!(message[prefix.len()..].starts_with(&location), "{message}");
    assert_eq!(message.len(), 2000, "{}", message.len());
}

#[test]
fn ac3_rules_wit_skipped_only_for_wit_path_violations() {
    // TECHNICAL.md §4.4: the WIT is walked (rule `path`) before any parse, so a
    // path violation inside wit/ stops the parse...
    let world = "package mods:hello@0.1.0; world hello { import other:x/y@1.0.0; }";
    let dir = temporary("hello");
    put(dir.path(), "wit/world.wit", world);
    put(
        dir.path(),
        "wit/x/y.wit",
        "package other:x@1.0.0; interface y {}",
    );
    let v = refused(dir.path(), Rule::Path, "wit/x");
    assert!(
        !v.iter().any(|v| matches!(v.rule, Rule::Wit | Rule::Import)),
        "{v:?}"
    );
    // ...while a path violation outside wit/ leaves the WIT checks running (§4.7:
    // every violation that can be found is reported).
    #[cfg(unix)]
    {
        let dir = temporary("hello");
        put(dir.path(), "wit/world.wit", world);
        put(
            dir.path(),
            "wit/deps/x/y.wit",
            "package other:x@1.0.0; interface y {}",
        );
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("ui/link")).unwrap();
        let v = refused(dir.path(), Rule::Path, "ui/link");
        assert!(
            v.iter()
                .any(|v| v.rule == Rule::Wit || v.rule == Rule::Import),
            "{v:?}"
        );
    }
}

#[test]
fn ac3_rules_nesting_scan_comments() {
    // TECHNICAL.md §4.4 nesting scan: comments (which nest) are skipped; depth above 32 is `limit`.
    let open = "{".repeat(33);
    let dir = temporary("hello");
    for world in [
        // Braces inside the outer comment, after a nested one has closed.
        format!("package mods:hello@0.1.0;\n/* /* */ {open} */\nworld hello {{}}\n"),
        // One brace in each of 33 separate comments.
        format!(
            "package mods:hello@0.1.0;\n{}\nworld hello {{}}\n",
            "/*{*/ ".repeat(33)
        ),
        // A line comment at the very end, with no newline after it.
        "package mods:hello@0.1.0;\nworld hello {}\n// end".to_string(),
    ] {
        put(dir.path(), "wit/world.wit", &world);
        Mod::read(dir.path()).unwrap_or_else(|e| panic!("{world}: {e}"));
    }
    // A comment at the very start ends at its first */; the braces after it count.
    put(
        dir.path(),
        "wit/world.wit",
        format!("/* c */ package mods:hello@0.1.0;\n{open}"),
    );
    refused(dir.path(), Rule::Limit, "wit/world.wit");
}

#[test]
fn ac3_identity_package_line_after_comments() {
    // TECHNICAL.md §4.1: the package line is in wit/world.wit; comments before it
    // may hold `;`, and a `;` after it in the file is not part of it.
    let dir = temporary("hello");
    let rest = "interface api { f: func(); }\nworld hello {}\n";
    for head in [
        "/* /* */ ; */ package mods:hello@0.1.0;\n",
        "/* a */ package mods:hello@0.1.0;\n// */\n",
        "          //\n/* x */ package mods:hello@0.1.0;\n",
    ] {
        let world = format!("{head}{rest}");
        put(dir.path(), "wit/world.wit", &world);
        let m = Mod::read(dir.path()).unwrap_or_else(|e| panic!("{world}: {e}"));
        assert_eq!(m.slug().as_str(), "hello");
    }
    // Without a package line the refusal is the parser's, never a panic (§4.4, §10).
    for world in ["world hello {}", "world hello {} // end"] {
        put(dir.path(), "wit/world.wit", world);
        let message = wit_message(dir.path());
        assert!(!message.contains("panicked"), "{world}: {message}");
    }
}

#[test]
fn ac2_mod_toml_at_limit_is_read_whole() {
    // TECHNICAL.md §4.2: mod.toml is at most 16 KiB; a file of exactly 16384 bytes
    // is read to its last byte (here the closing quote of `summary`).
    let dir = temporary("hello");
    let tail = "\nname = 'Hello'\nsummary = 'Says hello.'";
    let text = format!("#{}{tail}", "-".repeat(16384 - 1 - tail.len()));
    assert_eq!(text.len(), 16384);
    put(dir.path(), "mod.toml", &text);
    assert_eq!(Mod::read(dir.path()).unwrap().summary(), "Says hello.");
    put(dir.path(), "mod.toml", format!("-{text}"));
    refused(dir.path(), Rule::ModToml, "mod.toml");
}

#[test]
fn regress_s33_other_slug_is_not_own_copy() {
    // Ignoring the version must not let another slug pass as the own copy:
    // the folder and the package name must both be the mod's own slug.
    for (file, package) in [
        ("wit/deps/t/world.wit", "package mods:s@0.1.0; world s {}"),
        ("wit/deps/s/world.wit", "package mods:t@0.1.0; world t {}"),
    ] {
        let dir = temporary("hello");
        put(
            dir.path(),
            "wit/world.wit",
            "package mods:s@0.2.0; world s {}",
        );
        put(dir.path(), file, package);
        let v = match Mod::read(dir.path()) {
            Err(ModError::Invalid(v)) => v,
            other => panic!("{file}: {other:?}"),
        };
        assert!(
            !v.iter()
                .any(|v| v.message.contains("this mod's own package")),
            "{file}: {v:?}"
        );
    }
}

fn broken_message(file: &str) -> (String, PathBuf) {
    let dir = temporary("hello");
    let text = "package mods:broken@0.1.0;\n\nworld broken {\n  @\n}\n";
    put(dir.path(), file, text);
    let path = dir.path().join(file);
    let v = refused(dir.path(), Rule::Wit, file);
    let wit: Vec<_> = v.iter().filter(|v| v.rule == Rule::Wit).collect();
    assert_eq!(wit.len(), 1, "{v:?}");
    (wit[0].message.clone(), path)
}

#[test]
fn ac1_s86_wit_syntax_error_carries_location() {
    let (message, path) = broken_message("wit/world.wit");
    let arrow = format!(" --> {}:5:1", path.display());
    assert!(message.contains(&arrow), "{message}");
}

#[test]
fn ac2_s86_provider_copy_error_names_its_file() {
    let (message, path) = broken_message("wit/deps/broken/world.wit");
    let arrow = format!(" --> {}:5:1", path.display());
    assert!(message.contains(&arrow), "{message}");
}

#[test]
fn ac1_s86_header_and_resolve_errors_carry_location() {
    // The package header is parsed on its own first; resolution is a later step.
    for (text, at) in [
        ("not WIT", "1:1"),
        (
            "package mods:hello@0.1.0;\nworld hello {\n  import foo:bar/baz;\n}\n",
            "3:10",
        ),
    ] {
        let dir = temporary("hello");
        put(dir.path(), "wit/world.wit", text);
        let message = wit_message(dir.path());
        let arrow = format!(" --> {}:{at}", dir.path().join("wit/world.wit").display());
        assert!(message.contains(&arrow), "{message}");
    }
}
