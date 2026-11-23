//! Checks a mod folder against the pkg rules plus the builder's constraints,
//! before anything is built (TECHNICAL 5).

use crate::allowlist::Allowlist;
use crate::outcome::{Code, Problem};
use serde::Serialize;
use std::fs;
use std::path::Path;
use watane_pkg::{Mod, ModError};
fn files(root: &Path) -> Result<Vec<Problem>, Problem> {
    let mut stack = vec![root.to_path_buf()];
    let mut errors = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir).map_err(Problem::internal)? {
            let entry = entry.map_err(Problem::internal)?;
            let kind = entry.file_type().map_err(Problem::internal)?;
            let path = entry.path();
            if kind.is_symlink() {
                let file = path
                    .strip_prefix(root)
                    .expect("walked paths are under the root")
                    .to_string_lossy();
                errors.push(
                    Problem::new(Code::ModSymlink, format!("{file} is a symbolic link"))
                        .at(file.to_string(), None, None)
                        .fix("Replace it with a file"),
                );
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("walked paths are under the root");
                if relative.starts_with("logic")
                    && relative != Path::new("logic")
                    && !relative.starts_with("logic/target")
                {
                    let allowed = if kind.is_dir() {
                        relative.starts_with("logic/src")
                    } else {
                        kind.is_file()
                            && (matches!(
                                relative.to_str(),
                                Some("logic/Cargo.toml" | "logic/Cargo.lock")
                            ) || (relative.starts_with("logic/src")
                                && relative.extension().is_some_and(|e| e == "rs")))
                    };
                    if !allowed {
                        let file = relative.to_string_lossy();
                        let reason = if relative == Path::new("logic/build.rs") {
                            "the builder runs no build script"
                        } else if relative.starts_with("logic/.cargo") {
                            "the builder uses no cargo configuration of the mod"
                        } else if matches!(
                            relative.to_str(),
                            Some("logic/rust-toolchain" | "logic/rust-toolchain.toml")
                        ) {
                            "the builder chooses the toolchain"
                        } else {
                            "only Cargo.toml, Cargo.lock and src/**/*.rs are built"
                        };
                        errors.push(
                            Problem::new(
                                Code::ModLogicFile,
                                format!("{file} is not allowed: {reason}"),
                            )
                            .at(file.to_string(), None, None)
                            .fix(format!("Remove {file}")),
                        );
                    }
                }
                if kind.is_dir()
                    && !matches!(path.strip_prefix(root), Ok(p) if p == Path::new(".git") || p == Path::new("logic/target"))
                {
                    stack.push(path);
                }
            }
        }
    }
    errors.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(errors)
}
#[derive(Serialize, Debug)]
pub struct ProviderReport {
    pub slug: String,
    pub publisher: Option<String>,
    pub path: Option<String>,
}
#[derive(Serialize, Debug)]
pub struct CheckReport {
    pub slug: String,
    pub version: String,
    pub name: String,
    pub page: bool,
    pub logic: bool,
    pub grants: Vec<String>,
    pub exports: Vec<String>,
    pub events: Vec<String>,
    pub collects: Vec<String>,
    pub calls: Vec<String>,
    pub subscriptions: Vec<String>,
    pub providers: Vec<ProviderReport>,
}
fn wit_location(message: &str) -> Option<(String, u32, u32)> {
    message.lines().find_map(|line| {
        let location = line.trim().strip_prefix("-->")?.trim();
        let mut parts = location.rsplitn(3, ':');
        let column = parts.next()?.parse().ok()?;
        let line = parts.next()?.parse().ok()?;
        Some((parts.next()?.into(), line, column))
    })
}
/// Map pkg violations, preserving locations supplied in parser diagnostics.
pub(crate) fn mod_problems(e: ModError) -> Vec<Problem> {
    mod_problems_for_slug(e, None, None)
}

fn mod_problems_for_slug(
    e: ModError,
    own_slug: Option<&watane_pkg::Slug>,
    dir: Option<&Path>,
) -> Vec<Problem> {
    // Name wit/deps/<slug>/ only when that folder is really there.
    let self_fix = |slug: &str| {
        if dir.is_some_and(|d| d.join("wit/deps").join(slug).exists()) {
            format!(
                "Ask before deleting wit/deps/{slug}/ and mods:{slug} from deps.json: a mod cannot depend on itself"
            )
        } else {
            format!("Ask before deleting mods:{slug} from deps.json: a mod cannot depend on itself")
        }
    };
    match e {
        ModError::Invalid(v) => v
            .into_iter()
            .map(|v| {
                let location = wit_location(&v.message);
                let (file, line, column) =
                    location.map_or((v.file.clone(), None, None), |(file, line, column)| {
                        // pkg may report the package directory while wit-parser names a file.
                        let file = if let Some(i) = file.rfind("/wit/") {
                            file[i + 1..].into()
                        } else if !file.starts_with("wit/") && Path::new(&file).is_relative() {
                            let base = Path::new(&v.file);
                            let base = if base.extension().is_some() {
                                base.parent().unwrap_or(base)
                            } else {
                                base
                            };
                            base.join(file).to_string_lossy().into_owned()
                        } else {
                            file
                        };
                        (file, Some(line), Some(column))
                    });
                let fix = if file == "deps.json"
                    && let Some(slug) = v.message.strip_prefix("deps.json lists this mod itself as mods:")
                {
                    self_fix(slug)
                } else if file == "deps.json"
                    && let Some(slug) = v.message.strip_prefix("deps.json lists mods:")
                        .and_then(|s| s.strip_suffix(" but wit/world.wit imports none of its interfaces"))
                {
                    if own_slug.is_some_and(|own| own.as_str() == slug) {
                        self_fix(slug)
                    } else {
                        format!("Import the provider's interface in wit/world.wit, or ask before deleting wit/deps/{slug}/ and mods:{slug} from deps.json")
                    }
                } else if let Some(folder) = file.strip_prefix("wit/deps/")
                    && v.message.strip_prefix(&format!("{file} holds this mod's own package mods:"))
                        .is_some_and(|slug| slug.parse::<watane_pkg::Slug>().is_ok())
                {
                    format!("Ask before deleting wit/deps/{folder}/: a mod cannot depend on itself")
                } else if let Some(name) = v.message.split("duplicate item named `").nth(1)
                    .and_then(|rest| rest.split('`').next())
                    && file.ends_with("world.wit")
                {
                    format!("Two items of wit/world.wit are named `{name}`. If they are the world and an interface, rename the world in wit/world.wit (for example `world {name}-mod`): the package line keeps the slug, and the world name is free. Otherwise rename one of them")
                } else if matches!(file.as_str(), "deps.json" | ".watane-dev.json")
                    || Path::new(&file).starts_with("wit/deps")
                {
                    "Run: watane-dev add <provider folder> to refresh the dependency files, then check imports in wit/world.wit".into()
                } else if v.rule == watane_pkg::Rule::ModToml
                    && v.message.starts_with("`repository`")
                {
                    format!("Set repository in mod.toml to the https URL of the mod's git repository (for example https://github.com/<owner>/{}), or remove the line", own_slug.map_or("<slug>", |s| s.as_str()))
                } else {
                    format!("Correct {} according to the reported rule", v.file)
                };
                Problem::new(
                    Code::ModInvalid,
                    format!(
                        "[{}] {}",
                        serde_json::to_value(v.rule).expect("a rule serializes").as_str().expect("a rule serializes as a string"),
                        v.message
                    ),
                )
                .at(file, line, column)
                .fix(fix)
            })
            .collect(),
        ModError::Io { path, source } => {
            vec![Problem::internal(source).at(path.to_string_lossy(), None, None)]
        }
    }
}
pub fn check(dir: &Path) -> Result<(Mod, CheckReport), Vec<Problem>> {
    if fs::symlink_metadata(dir.join("mod.toml"))
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(vec![Problem::new(Code::UsageNotAMod,format!("{} is not a mod folder: it has no mod.toml",dir.display())).fix("Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>")]);
    }
    for path in [
        "wit",
        "wit/deps",
        "wit/deps/watane-host",
        "wit/deps/watane-host/host.wit",
    ] {
        if fs::symlink_metadata(dir.join(path)).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(vec![
                Problem::new(Code::ModSymlink, format!("{path} is a symbolic link"))
                    .at(path, None, None)
                    .fix("Replace it with a file"),
            ]);
        }
    }
    let host_wit = dir.join("wit/deps/watane-host/host.wit");
    if fs::read(&host_wit).ok().as_deref() != Some(watane_host::HOST_WIT.as_bytes()) {
        fs::create_dir_all(host_wit.parent().expect("the host WIT path has a parent"))
            .map_err(|e| vec![Problem::internal(e)])?;
        fs::write(host_wit, watane_host::HOST_WIT).map_err(|e| vec![Problem::internal(e)])?;
    }
    let errors = files(dir).map_err(|e| vec![e])?;
    if !errors.is_empty() {
        return Err(errors);
    }
    let m = Mod::read(dir).map_err(|error| {
        let slug_of = |path: &Path| {
            fs::read_to_string(path).ok().and_then(|text| {
                crate::build_on::declaration(&text)
                    .ok()
                    .map(|(_, slug, _)| slug)
            })
        };
        let own_slug = slug_of(&dir.join("wit/world.wit"));
        let error = match (error, &own_slug) {
            // pkg flags only wit/deps/<own slug>; a copy under another folder is a
            // duplicate package for wit-parser. Name that folder as the own-package copy.
            (ModError::Invalid(violations), Some(own)) => ModError::Invalid(
                violations
                    .into_iter()
                    .map(|v| {
                        let duplicate = v.message.contains(&format!("package `mods:{own}@"))
                            && v.message.contains("is defined in two different locations");
                        let folder =
                            duplicate
                                .then(|| fs::read_dir(dir.join("wit/deps")).ok())
                                .flatten()
                                .into_iter()
                                .flatten()
                                .flatten()
                                .map(|entry| entry.path())
                                .filter(|path| path.is_dir())
                                .find(|path| {
                                    fs::read_dir(path).into_iter().flatten().flatten().any(|f| {
                                        slug_of(&f.path()).is_some_and(|slug| slug == *own)
                                    })
                                });
                        match folder.and_then(|f| f.file_name()?.to_str().map(String::from)) {
                            Some(folder) => watane_pkg::Violation {
                                rule: watane_pkg::Rule::Provider,
                                file: format!("wit/deps/{folder}"),
                                message: format!(
                                    "wit/deps/{folder} holds this mod's own package mods:{own}"
                                ),
                            },
                            None => v,
                        }
                    })
                    .collect(),
            ),
            (error, _) => error,
        };
        mod_problems_for_slug(error, own_slug.as_ref(), Some(dir))
    })?;
    if dir.join("logic").exists() {
        let path = dir.join("logic/Cargo.toml");
        let text = fs::read_to_string(&path)
            .map_err(|e| vec![Problem::internal(e).at("logic/Cargo.toml", None, None)])?;
        let errors = check_manifest(&text, &Allowlist::embedded());
        if !errors.is_empty() {
            return Err(errors);
        }
    }
    if m.logic().is_some() {
        let has_tests = match fs::read_dir(dir.join("tests")) {
            Ok(entries) => {
                let mut found = false;
                for entry in entries {
                    let entry = entry.map_err(|e| vec![Problem::internal(e)])?;
                    if entry.path().extension().is_some_and(|e| e == "json")
                        && entry
                            .file_type()
                            .map_err(|e| vec![Problem::internal(e)])?
                            .is_file()
                    {
                        found = true;
                    }
                }
                found
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(vec![Problem::internal(e)]),
        };
        if !has_tests {
            return Err(vec![
                Problem::new(
                    Code::ModNoContractTests,
                    "the mod has a logic part and no contract test",
                )
                .fix("Add tests/<name>.json (see AGENTS.md)"),
            ]);
        }
    }
    let w = m.world();
    let strings = |items: &[watane_pkg::InterfaceName]| {
        items.iter().map(ToString::to_string).collect::<Vec<_>>()
    };
    let local = crate::deps::read_local(dir).map_err(|e| vec![e])?;
    let report = CheckReport {
        slug: m.slug().to_string(),
        version: m.version().to_string(),
        name: m.name().to_string(),
        page: m.page().is_some(),
        logic: m.logic().is_some(),
        grants: m.grants().iter().map(ToString::to_string).collect(),
        exports: strings(&w.exports),
        events: strings(&w.events),
        collects: strings(&w.collects),
        calls: strings(&w.calls),
        subscriptions: strings(&w.subscriptions),
        providers: m
            .deps()
            .iter()
            .map(|(slug, publisher)| ProviderReport {
                slug: slug.to_string(),
                publisher: publisher.map(ToString::to_string),
                path: local.get(slug.as_str()).cloned(),
            })
            .collect(),
    };
    Ok((m, report))
}

fn manifest_line(text: &str, key: &str) -> Option<u32> {
    let root = toml::de::DeTable::parse(text).ok()?;
    let mut table = root.get_ref();
    let mut parts = key.split('.').peekable();
    while let Some(part) = parts.next() {
        let (key, value) = table
            .iter()
            .find(|(key, _)| key.get_ref().as_ref() == part)?;
        if parts.peek().is_none() {
            return (text[..key.span().start]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1)
            .try_into()
            .ok();
        }
        table = value.get_ref().as_table()?;
    }
    None
}
fn cargo_key(text: &str, key: &str) -> Problem {
    Problem::new(Code::ModCargoKey, format!("logic/Cargo.toml: `{key}` is not allowed or has an invalid value"))
        .at("logic/Cargo.toml", manifest_line(text, key), None)
        .fix("Remove it or correct its value. Allowed: package.{name,version,edition,publish,description,license,authors}, lib.crate-type = [\"cdylib\"], dependencies, dev-dependencies, empty workspace")
}
pub fn check_manifest(text: &str, allow: &Allowlist) -> Vec<Problem> {
    let table: toml::Table = match toml::from_str(text) {
        Ok(table) => table,
        Err(e) => {
            let mut error = cargo_key(text, "manifest").fix(format!("Correct the TOML: {e}"));
            if let Some(prefix) = e.span().and_then(|span| text.get(..span.start)) {
                error.line = (prefix.bytes().filter(|b| *b == b'\n').count() + 1)
                    .try_into()
                    .ok();
                error.column = (prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1)
                    .try_into()
                    .ok();
            }
            return vec![error];
        }
    };
    let mut errors = Vec::new();
    for (section, value) in &table {
        match section.as_str() {
            "package" => {
                let Some(package) = value.as_table() else {
                    errors.push(cargo_key(text, section));
                    continue;
                };
                for (key, value) in package {
                    let valid = match key.as_str() {
                        "name" | "version" => value.is_str(),
                        "edition" => matches!(value.as_str(), Some("2021" | "2024")),
                        "publish" | "description" | "license" | "authors" => true,
                        _ => false,
                    };
                    if !valid {
                        errors.push(cargo_key(text, &format!("package.{key}")));
                    }
                }
            }
            "lib" => {
                let Some(lib) = value.as_table() else {
                    errors.push(cargo_key(text, section));
                    continue;
                };
                for key in lib.keys().filter(|k| k.as_str() != "crate-type") {
                    errors.push(cargo_key(text, &format!("lib.{key}")));
                }
            }
            "workspace" => {
                if !value.as_table().is_some_and(|t| t.is_empty()) {
                    errors.push(cargo_key(text, section));
                }
            }
            "dependencies" | "dev-dependencies" => {
                let Some(deps) = value.as_table() else {
                    errors.push(cargo_key(text, section));
                    continue;
                };
                for (name, value) in deps {
                    let requirement = if let Some(dep) = value.as_table() {
                        for (key, value) in dep {
                            let valid = match key.as_str() {
                                "version" => true,
                                "features" => value
                                    .as_array()
                                    .is_some_and(|a| a.iter().all(toml::Value::is_str)),
                                "default-features" => value.is_bool(),
                                _ => false,
                            };
                            if !valid {
                                errors.push(cargo_key(text, &format!("{section}.{name}.{key}")));
                            }
                        }
                        dep.get("version").and_then(toml::Value::as_str)
                    } else {
                        value.as_str()
                    };
                    let version = allow.version_of(name);
                    let valid = version.zip(requirement).is_some_and(|(version, req)| {
                        semver::VersionReq::parse(req).is_ok_and(|req| req.matches(version))
                    });
                    if !valid {
                        let detail = match (version, requirement) {
                            (None, _) => "is not in the allowlist".to_string(),
                            (_, None) => "must be a version requirement".to_string(),
                            (Some(version), Some(req)) => {
                                format!("requires {req}, the allowlist has {version}")
                            }
                        };
                        errors.push(
                            Problem::new(
                                Code::ModDependency,
                                format!("dependency `{name}` {detail}"),
                            )
                            .at(
                                "logic/Cargo.toml",
                                manifest_line(text, &format!("{section}.{name}")),
                                None,
                            )
                            .fix(format!("Use only: {}", allow.crates())),
                        );
                    }
                }
            }
            _ => errors.push(cargo_key(text, section)),
        }
    }
    let cdylib = table
        .get("lib")
        .and_then(|l| l.get("crate-type"))
        .and_then(toml::Value::as_array);
    if !cdylib.is_some_and(|a| a.len() == 1 && a[0].as_str() == Some("cdylib")) {
        errors.push(cargo_key(text, "lib.crate-type"));
    }
    if !table.contains_key("workspace") {
        errors.push(cargo_key(text, "workspace"));
    }
    errors
}

#[cfg(test)]
mod manifest_tests {
    use super::*;
    #[test]
    fn ac3_managed_violation_fixes_name_the_add_action() {
        for file in ["deps.json", "wit/deps/books/world.wit", ".watane-dev.json"] {
            let problems = mod_problems(ModError::Invalid(vec![watane_pkg::Violation {
                rule: watane_pkg::Rule::Provider,
                file: file.into(),
                message: "invalid dependency".into(),
            }]));
            assert_eq!(
                problems[0].fix.as_deref(),
                Some(
                    "Run: watane-dev add <provider folder> to refresh the dependency files, then check imports in wit/world.wit"
                )
            );
        }
    }

    #[test]
    fn ac5_wit_location_in_message_is_retained() {
        assert_eq!(
            wit_location("bad WIT\n --> wit/deps/books/world.wit:14:7\n |"),
            Some(("wit/deps/books/world.wit".into(), 14, 7))
        );
        let problems = mod_problems(ModError::Invalid(vec![watane_pkg::Violation {
            rule: watane_pkg::Rule::Wit,
            file: "wit".into(),
            message: "bad WIT\n --> /mod/wit/deps/books/world.wit:14:7".into(),
        }]));
        assert_eq!(
            problems[0].file.as_deref(),
            Some("wit/deps/books/world.wit")
        );
        assert_eq!(problems[0].line, Some(14));
        assert_eq!(problems[0].column, Some(7));
        let relative = mod_problems(ModError::Invalid(vec![watane_pkg::Violation {
            rule: watane_pkg::Rule::Wit,
            file: "wit".into(),
            message: "bad WIT\n --> world.wit:4:3".into(),
        }]));
        assert_eq!(relative[0].file.as_deref(), Some("wit/world.wit"));
        assert_eq!(wit_location("no location"), None);
    }

    #[test]
    fn ac5_wit_syntax_error_retains_parser_location() {
        let root = tempfile::tempdir().unwrap();
        crate::template::new_mod(root.path().join("broken").as_path(), "broken").unwrap();
        let dir = root.path().join("broken");
        fs::write(
            dir.join("wit/world.wit"),
            "package mods:broken@0.1.0;\n\nworld broken {\n  @\n}\n",
        )
        .unwrap();
        let errors = match check(&dir) {
            Err(errors) => errors,
            Ok(_) => panic!("accepted syntax error"),
        };
        assert_eq!(errors[0].code, Code::ModInvalid);
        assert_eq!(errors[0].file.as_deref(), Some("wit/world.wit"));
        assert_eq!(errors[0].line, Some(5), "{}", errors[0].message);
        assert_eq!(errors[0].column, Some(1));
    }

    #[test]
    fn ac3_invalid_toml_names_file_and_line() {
        let allow = crate::allowlist::Allowlist::embedded();
        let errors = check_manifest("[package]\nname = @\n", &allow);
        assert_eq!(errors[0].code.as_str(), "mod.cargo_key");
        assert_eq!(errors[0].file.as_deref(), Some("logic/Cargo.toml"));
        assert_eq!(errors[0].line, Some(2));
        assert_eq!(errors[0].column, Some(8));
    }
    #[test]
    fn ac3_twenty_literal_manifests() {
        let allow = crate::allowlist::Allowlist::embedded();
        let cases: [(&str, &[(&str, &str)]); 20] = [
            (
                r#"[package]
name = "hello"
version = "0.1.0"
edition = "2024"
publish = false
[lib]
crate-type = ["cdylib"]
[workspace]
"#,
                &[],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
edition = "2021"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde_json = "1"
[workspace]
"#,
                &[],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde = {version = "=1.0.229", features = ["derive"], default-features = false}
[workspace]
"#,
                &[],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dev-dependencies]
serde_json = {version = "^1.0.154"}
[workspace]
"#,
                &[],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
build = false
links = "native"
[lib]
crate-type = ["cdylib"]
[workspace]
"#,
                &[
                    ("mod.cargo_key", "package.build"),
                    ("mod.cargo_key", "package.links"),
                ],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[build-dependencies]
[workspace]
"#,
                &[("mod.cargo_key", "build-dependencies")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
proc-macro = true
[workspace]
"#,
                &[("mod.cargo_key", "lib.proc-macro")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[patch.crates-io]
[replace]
[workspace]
"#,
                &[("mod.cargo_key", "patch"), ("mod.cargo_key", "replace")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[profile.release]
opt-level = 2
[features]
custom = []
[workspace]
"#,
                &[("mod.cargo_key", "features"), ("mod.cargo_key", "profile")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[target.'cfg(unix)'.dependencies]
[workspace]
"#,
                &[("mod.cargo_key", "target")],
            ),
            (
                r#"cargo-features = []
[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[workspace]
"#,
                &[("mod.cargo_key", "cargo-features")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde_json = {version = "1", path = "../evil", git = "https://example.invalid", registry = "elsewhere", package = "evil", optional = true}
[workspace]
"#,
                &[
                    ("mod.cargo_key", "dependencies.serde_json.git"),
                    ("mod.cargo_key", "dependencies.serde_json.optional"),
                    ("mod.cargo_key", "dependencies.serde_json.package"),
                    ("mod.cargo_key", "dependencies.serde_json.path"),
                    ("mod.cargo_key", "dependencies.serde_json.registry"),
                ],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
rand = "0.9"
[workspace]
"#,
                &[("mod.dependency", "rand")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde_json = "2"
[workspace]
"#,
                &[("mod.dependency", "serde_json")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde = {features = ["derive"]}
[workspace]
"#,
                &[("mod.dependency", "serde")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["rlib"]
[workspace]
"#,
                &[("mod.cargo_key", "lib.crate-type")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[workspace]
members = []
"#,
                &[("mod.cargo_key", "workspace")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
edition = "2018"
[lib]
crate-type = ["cdylib"]
[workspace]
"#,
                &[("mod.cargo_key", "package.edition")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
[dependencies]
serde_json = 1
[workspace]
"#,
                &[("mod.dependency", "serde_json")],
            ),
            (
                r#"[package]
name = "hello"
version = "0.1.0"
[lib]
crate-type = ["cdylib"]
"#,
                &[("mod.cargo_key", "workspace")],
            ),
        ];
        for (manifest, expected) in cases {
            let errors = check_manifest(manifest, &allow);
            let actual: Vec<_> = errors.iter().map(|e| e.code.as_str()).collect();
            let codes: Vec<_> = expected.iter().map(|(code, _)| *code).collect();
            assert_eq!(actual, codes, "{manifest}");
            for (error, (_, key)) in errors.iter().zip(expected) {
                assert!(
                    error.message.contains(key),
                    "{} should name {key}",
                    error.message
                );
                assert_eq!(error.file.as_deref(), Some("logic/Cargo.toml"));
                assert!(error.fix.is_some());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ac1_ac2_repository_violation_gets_its_own_fix() {
        use watane_pkg::{Rule, Violation};
        let fix = |rule, message: &str| {
            let p = mod_problems(ModError::Invalid(vec![Violation {
                rule,
                file: "mod.toml".into(),
                message: message.into(),
            }]));
            assert_eq!(p.len(), 1);
            assert_eq!(p[0].code, Code::ModInvalid);
            assert_eq!(p[0].file.as_deref(), Some("mod.toml"));
            (p[0].message.clone(), p[0].fix.clone().unwrap())
        };
        let want = "Set repository in mod.toml to the https URL of the mod's git repository (for example https://github.com/<owner>/<slug>), or remove the line";
        for m in [
            "`repository` must start with https://",
            "`repository` is longer than 256 bytes",
            "`repository` must not hold a user name or password",
            "`repository` has no host",
            "`repository` may hold only letters, digits and - . _ ~ / % after the host",
        ] {
            let (msg, f) = fix(Rule::ModToml, m);
            assert_eq!(msg, format!("[mod-toml] {m}"));
            assert_eq!(f, want);
        }
        let generic = "Correct mod.toml according to the reported rule";
        assert_eq!(
            fix(
                Rule::ModToml,
                "name must have 1..=64 characters, no control characters or surrounding whitespace"
            )
            .1,
            generic
        );
        assert_eq!(fix(Rule::Path, "`repository` odd").1, generic);
    }

    #[test]
    fn ac7_own_provider_diagnostics_use_identity_and_actual_folder() {
        use watane_pkg::{Rule, Violation};
        let own = "consumer".parse().unwrap();
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("wit/deps/consumer")).unwrap();
        for (file, message, expected) in [
            (
                "deps.json",
                "deps.json lists mods:consumer but wit/world.wit imports none of its interfaces",
                "Ask before deleting wit/deps/consumer/ and mods:consumer from deps.json: a mod cannot depend on itself",
            ),
            (
                "wit/deps/renamed",
                "wit/deps/renamed holds this mod's own package mods:consumer",
                "Ask before deleting wit/deps/renamed/: a mod cannot depend on itself",
            ),
        ] {
            let errors = mod_problems_for_slug(
                ModError::Invalid(vec![Violation {
                    rule: Rule::Provider,
                    file: file.into(),
                    message: message.into(),
                }]),
                Some(&own),
                Some(t.path()),
            );
            assert_eq!(errors[0].fix.as_deref(), Some(expected));
            assert_eq!(errors[0].file.as_deref(), Some(file));
        }
        let empty = tempfile::tempdir().unwrap();
        let errors = mod_problems_for_slug(
            ModError::Invalid(vec![Violation {
                rule: Rule::Provider,
                file: "deps.json".into(),
                message: "deps.json lists this mod itself as mods:consumer".into(),
            }]),
            Some(&own),
            Some(empty.path()),
        );
        assert_eq!(
            errors[0].fix.as_deref(),
            Some("Ask before deleting mods:consumer from deps.json: a mod cannot depend on itself")
        );
    }

    #[test]
    fn ac1_duplicate_item_fix_renames_the_world() {
        use watane_pkg::{Rule, Violation};
        let errors = mod_problems(ModError::Invalid(vec![Violation {
            rule: Rule::Wit,
            file: "wit/world.wit".into(),
            message: "duplicate item named `menu`\n --> wit/world.wit:3:1".into(),
        }]));
        let fix = errors[0].fix.as_deref().unwrap();
        assert!(fix.contains("rename the world in wit/world.wit (for example `world menu-mod`)"));
        assert!(fix.contains("package line keeps the slug") && fix.contains("world name is free"));
    }

    #[test]
    fn ac3_repository_fix_names_the_own_slug() {
        use watane_pkg::{Rule, Violation};
        let fix = |own: Option<&watane_pkg::Slug>| {
            mod_problems_for_slug(
                ModError::Invalid(vec![Violation {
                    rule: Rule::ModToml,
                    file: "mod.toml".into(),
                    message: "`repository` has no host".into(),
                }]),
                own,
                None,
            )[0]
            .fix
            .clone()
            .unwrap()
        };
        let ratings = "ratings".parse().unwrap();
        assert!(fix(Some(&ratings)).contains("https://github.com/<owner>/ratings)"));
        assert!(fix(None).contains("https://github.com/<owner>/<slug>)"));
    }

    #[test]
    fn ac3_check_reports_recorded_provider_path() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        let b = t.path().join("b");
        crate::template::new_mod(&a, "a").unwrap();
        crate::template::new_mod(&b, "b").unwrap();
        crate::deps::add_local(&b, &a).unwrap();
        let world = fs::read_to_string(b.join("wit/world.wit")).unwrap();
        fs::write(
            b.join("wit/world.wit"),
            world.replace("world b {", "world b {\n    import mods:a/api@0.1.0;"),
        )
        .unwrap();
        let (_, report) = check(&b).unwrap();
        assert_eq!(
            serde_json::to_value(report).unwrap()["providers"][0]["path"],
            "../a"
        );
        fs::remove_file(b.join(".watane-dev.json")).unwrap();
        let (_, report) = check(&b).unwrap();
        assert_eq!(
            serde_json::to_value(report).unwrap()["providers"][0]["path"],
            serde_json::Value::Null
        );
    }

    #[cfg(unix)] // Uses a POSIX symlink fixture.
    #[test]
    fn ac3_dev_s5_and_s8_file_codes_locations_and_fixes() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("logic/src")).unwrap();
        fs::write(temp.path().join("logic/build.rs"), "fn main() {}").unwrap();
        std::os::unix::fs::symlink("missing", temp.path().join("logic/src/link.rs")).unwrap();
        assert_eq!(
            serde_json::to_value(files(temp.path()).unwrap()).unwrap(),
            json!([
                {"code":"mod.logic_file","message":"logic/build.rs is not allowed: the builder runs no build script",
                 "file":"logic/build.rs","line":null,"column":null,"fix":"Remove logic/build.rs"},
                {"code":"mod.symlink","message":"logic/src/link.rs is a symbolic link",
                 "file":"logic/src/link.rs","line":null,"column":null,"fix":"Replace it with a file"}
            ])
        );
    }

    #[test]
    fn ac3_dev_s6_and_s7_manifest_codes_locations_and_fixes() {
        let base = "[lib]\ncrate-type = [\"cdylib\"]\n[workspace]\n";
        for (extra, code, line, message, fix) in [
            (
                "[dependencies]\nrand = \"0.9\"\n",
                "mod.dependency",
                5,
                "dependency `rand` is not in the allowlist",
                "Use only: serde 1.0.229, serde_json 1.0.154, wit-bindgen 0.63.0",
            ),
            (
                "[build-dependencies]\n",
                "mod.cargo_key",
                4,
                "logic/Cargo.toml: `build-dependencies` is not allowed or has an invalid value",
                "Remove it or correct its value. Allowed: package.{name,version,edition,publish,description,license,authors}, lib.crate-type = [\"cdylib\"], dependencies, dev-dependencies, empty workspace",
            ),
        ] {
            let errors = check_manifest(&format!("{base}{extra}"), &Allowlist::embedded());
            assert_eq!(
                serde_json::to_value(errors).unwrap(),
                json!([
                    {"code":code,"message":message,"file":"logic/Cargo.toml","line":line,"column":null,"fix":fix}
                ])
            );
        }
        let errors = check_manifest(
            "[lib]\ncrate-type = [\"cdylib\"]\nproc-macro = true\n[workspace]\n",
            &Allowlist::embedded(),
        );
        assert_eq!(
            serde_json::to_value(errors).unwrap(),
            json!([{
                "code":"mod.cargo_key", "message":"logic/Cargo.toml: `lib.proc-macro` is not allowed or has an invalid value",
                "file":"logic/Cargo.toml","line":3,"column":null,
                "fix":"Remove it or correct its value. Allowed: package.{name,version,edition,publish,description,license,authors}, lib.crate-type = [\"cdylib\"], dependencies, dev-dependencies, empty workspace"
            }])
        );
    }

    #[cfg(unix)] // Uses a POSIX symlink fixture.
    #[test]
    fn ac3_check_repairs_host_wit_and_skips_git_links() {
        let root = tempfile::tempdir().unwrap();
        let temp = root.path().join("counter");
        crate::template::new_mod(temp.as_path(), "counter").unwrap();
        fs::create_dir(temp.as_path().join(".git")).unwrap();
        std::os::unix::fs::symlink("missing", temp.as_path().join(".git/link")).unwrap();
        fs::write(
            temp.as_path().join("wit/deps/watane-host/host.wit"),
            "changed",
        )
        .unwrap();
        check(temp.as_path()).unwrap();
        assert_eq!(
            fs::read_to_string(temp.as_path().join("wit/deps/watane-host/host.wit")).unwrap(),
            watane_host::HOST_WIT
        );
    }

    #[test]
    fn ac3_logic_needs_contract_but_page_only_does_not() {
        let root = tempfile::tempdir().unwrap();
        let temp = root.path().join("counter");
        crate::template::new_mod(temp.as_path(), "counter").unwrap();
        fs::remove_file(temp.as_path().join("tests/counter.json")).unwrap();
        let errors = check(temp.as_path()).err().unwrap();
        assert_eq!(
            serde_json::to_value(errors).unwrap(),
            json!([{
                "code":"mod.no_contract_tests","message":"the mod has a logic part and no contract test",
                "file":null,"line":null,"column":null,"fix":"Add tests/<name>.json (see AGENTS.md)"
            }])
        );
        fs::remove_dir_all(temp.as_path().join("logic")).unwrap();
        fs::write(
            temp.as_path().join("wit/world.wit"),
            "package mods:counter@0.1.0;\nworld counter {}\n",
        )
        .unwrap();
        check(temp.as_path()).unwrap();
    }
}
