use crate::{
    allowlist::Allowlist,
    outcome::{Code, Problem},
};
use serde::Serialize;
use std::{fs, path::Path};
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
                let file = path.strip_prefix(root).unwrap().to_string_lossy();
                errors.push(
                    Problem::new(Code::ModSymlink, format!("{file} is a symbolic link"))
                        .at(file.to_string(), None, None)
                        .fix("Replace it with a file"),
                );
            } else {
                let relative = path.strip_prefix(root).unwrap();
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
#[derive(Serialize)]
pub struct ProviderReport {
    pub slug: String,
    pub publisher: Option<String>,
    pub path: Option<String>,
}
#[derive(Serialize)]
pub struct CheckReport {
    pub slug: String,
    pub version: String,
    pub name: String,
    pub page: bool,
    pub logic: bool,
    pub grants: Vec<String>,
    pub exports: Vec<String>,
    pub events: Vec<String>,
    pub calls: Vec<String>,
    pub subscriptions: Vec<String>,
    pub providers: Vec<ProviderReport>,
}
/// Map a pkg `ModError` to `mod.invalid` problems (or an internal one for I/O).
pub(crate) fn mod_problems(e: ModError) -> Vec<Problem> {
    match e {
        ModError::Invalid(v) => v
            .into_iter()
            .map(|v| {
                Problem::new(
                    Code::ModInvalid,
                    format!(
                        "[{}] {}",
                        serde_json::to_value(v.rule).unwrap().as_str().unwrap(),
                        v.message
                    ),
                )
                .at(v.file.clone(), None, None)
                .fix(format!("Correct {} according to the reported rule", v.file))
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
        fs::create_dir_all(host_wit.parent().unwrap()).map_err(|e| vec![Problem::internal(e)])?;
        fs::write(host_wit, watane_host::HOST_WIT).map_err(|e| vec![Problem::internal(e)])?;
    }
    let errors = files(dir).map_err(|e| vec![e])?;
    if !errors.is_empty() {
        return Err(errors);
    }
    let m = Mod::read(dir).map_err(mod_problems)?;
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
    let report = CheckReport {
        slug: m.slug().to_string(),
        version: m.version().to_string(),
        name: m.name().to_string(),
        page: m.page().is_some(),
        logic: m.logic().is_some(),
        grants: m.grants().iter().map(ToString::to_string).collect(),
        exports: strings(&w.exports),
        events: strings(&w.events),
        calls: strings(&w.calls),
        subscriptions: strings(&w.subscriptions),
        providers: m
            .deps()
            .iter()
            .map(|(slug, publisher)| ProviderReport {
                slug: slug.to_string(),
                publisher: publisher.map(ToString::to_string),
                path: None,
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
