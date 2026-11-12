use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum ModError {
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the mod breaks {} rule(s): {}", .0.len(), list(.0))]
    Invalid(Vec<Violation>),
}
fn list(violations: &[Violation]) -> String {
    violations
        .iter()
        .map(|v| format!("[{}] {}: {}", v.rule.code(), v.file, v.message))
        .collect::<Vec<_>>()
        .join("; ")
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub rule: Rule,
    pub file: String,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    Path,
    Limit,
    ModToml,
    DepsJson,
    Wit,
    Import,
    Export,
    Provider,
    Http,
    Parts,
    Component,
}
impl Rule {
    fn code(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Limit => "limit",
            Self::ModToml => "mod-toml",
            Self::DepsJson => "deps-json",
            Self::Wit => "wit",
            Self::Import => "import",
            Self::Export => "export",
            Self::Provider => "provider",
            Self::Http => "http",
            Self::Parts => "parts",
            Self::Component => "component",
        }
    }
}
pub(crate) fn invalid(rule: Rule, file: &str, message: impl Into<String>) -> ModError {
    ModError::Invalid(vec![Violation {
        rule,
        file: file.into(),
        message: message.into(),
    }])
}
pub(crate) fn io(path: &Path, source: std::io::Error) -> ModError {
    ModError::Io {
        path: path.into(),
        source,
    }
}

// Bound the read as well as checking metadata, so a growing file cannot bypass the cap.
pub(crate) fn read_file(
    dir: &Path,
    file: &str,
    limit: u64,
    rule: Rule,
    optional: bool,
) -> Result<Option<Vec<u8>>, ModError> {
    let path = dir.join(file);
    let opened = match crate::open::file(dir, file) {
        Ok(file) => file,
        Err((part, e)) if crate::open::is_link(&e) => {
            return Err(invalid(
                Rule::Path,
                &part,
                "replace the symbolic link with a regular file",
            ));
        }
        Err((_, e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return if optional {
                Ok(None)
            } else {
                Err(invalid(rule, file, "required file is missing"))
            };
        }
        Err((_, e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::InvalidInput | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Err(invalid(rule, file, "expected a regular file"));
        }
        Err((part, e)) => return Err(io(&dir.join(part), e)),
    };
    let metadata = opened.metadata().map_err(|e| io(&path, e))?;
    if metadata.len() > limit {
        return Err(invalid(rule, file, format!("file exceeds {limit} bytes")));
    }
    let mut bytes = Vec::new();
    opened
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io(&path, e))?;
    if bytes.len() as u64 > limit {
        return Err(invalid(rule, file, format!("file exceeds {limit} bytes")));
    }
    Ok(Some(bytes))
}

use crate::{
    Deps, Grant, HttpHost, Provider, ReleaseRef, Requirement, Slug, Version, World, meta, world,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Mod {
    dir: PathBuf,
    meta: meta::Meta,
    deps: Deps,
    contract: world::Contract,
    page: bool,
    logic: bool,
}

fn collect<T>(
    result: Result<T, ModError>,
    violations: &mut Vec<Violation>,
) -> Result<Option<T>, ModError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(ModError::Invalid(v)) => {
            violations.extend(v);
            Ok(None)
        }
        Err(e) => Err(e),
    }
}
fn add(violations: &mut Vec<Violation>, rule: Rule, file: &str, message: impl Into<String>) {
    violations.push(Violation {
        rule,
        file: file.into(),
        message: message.into(),
    });
}
fn metadata(path: &Path) -> Result<Option<fs::Metadata>, ModError> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path, e)),
    }
}
fn segment(s: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().next().is_some_and(first)
        && s.bytes().all(|b| first(b) || b == b'.')
}
// Never recurse through links; use an explicit stack for hostile directory depths.
fn inspect(dir: &Path, violations: &mut Vec<Violation>) -> Result<Vec<String>, ModError> {
    let mut files = Vec::new();
    for file in ["mod.toml", "deps.json", "logic.wasm"] {
        if metadata(&dir.join(file))?.is_some_and(|m| m.file_type().is_symlink()) {
            add(
                violations,
                Rule::Path,
                file,
                "replace the symbolic link with a regular file",
            );
        }
    }
    let mut stack = vec!["wit".to_string(), "ui".to_string()];
    while let Some(relative) = stack.pop() {
        let path = dir.join(&relative);
        let Some(m) = metadata(&path)? else {
            continue;
        };
        if m.file_type().is_symlink() {
            add(
                violations,
                Rule::Path,
                &relative,
                "replace the symbolic link with a regular entry",
            );
            continue;
        }
        let wit = relative == "wit" || relative.starts_with("wit/");
        if m.is_dir() {
            if wit
                && relative != "wit"
                && relative != "wit/deps"
                && !(relative.starts_with("wit/deps/")
                    && relative.split('/').count() == 3
                    && relative.rsplit('/').next().is_some_and(segment))
            {
                add(
                    violations,
                    Rule::Path,
                    &relative,
                    "WIT directories must be wit/deps/<segment>",
                );
                continue;
            }
            let mut entries = fs::read_dir(&path)
                .map_err(|e| io(&path, e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| io(&path, e))?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries.into_iter().rev() {
                let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                    add(
                        violations,
                        Rule::Path,
                        &format!("{relative}/{}", entry.file_name().to_string_lossy()),
                        "use UTF-8 path names",
                    );
                    continue;
                };
                stack.push(format!("{relative}/{name}"));
            }
        } else if wit {
            if relative == "wit" || relative == "wit/deps" {
                add(violations, Rule::Wit, &relative, "expected a WIT directory");
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if relative.starts_with("wit/deps/")
                && (name.ends_with(".wasm") || name.ends_with(".wat"))
            {
                add(
                    violations,
                    Rule::Path,
                    &relative,
                    "WIT dependencies must be text .wit files",
                );
            } else if name.ends_with(".wit") {
                if !segment(name) || !m.is_file() {
                    add(
                        violations,
                        Rule::Path,
                        &relative,
                        "use a regular .wit file with a valid segment name",
                    );
                } else {
                    files.push(relative);
                }
            }
        }
    }
    files.sort();
    Ok(files)
}
impl Mod {
    pub fn read(dir: &Path) -> Result<Mod, ModError> {
        let mut violations = Vec::new();
        let files = inspect(dir, &mut violations)?;
        let meta = collect(meta::read(dir), &mut violations)?;
        let deps = collect(Deps::read(dir), &mut violations)?;
        let unsafe_wit = violations
            .iter()
            .any(|v| v.rule == Rule::Path && (v.file == "wit" || v.file.starts_with("wit/")));
        let contract = if unsafe_wit {
            None
        } else {
            world::read(dir, &files, &mut violations)?
        };
        let page = metadata(&dir.join("ui"))?.is_some();
        if page
            && (!metadata(&dir.join("ui"))?.is_some_and(|m| m.is_dir())
                || !metadata(&dir.join("ui/index.html"))?.is_some_and(|m| m.is_file()))
        {
            add(
                &mut violations,
                Rule::Parts,
                ".",
                "ui/ must be a directory containing a regular ui/index.html",
            );
        }
        let source = metadata(&dir.join("logic/Cargo.toml"))?.is_some_and(|m| m.is_file());
        let built = metadata(&dir.join("logic.wasm"))?.is_some_and(|m| m.is_file());
        let any_logic_file = metadata(&dir.join("logic/Cargo.toml"))?.is_some()
            || metadata(&dir.join("logic.wasm"))?.is_some();
        let mut logic = false;
        if let Some(c) = &contract {
            logic = !c.world.exports.is_empty() || !c.world.subscriptions.is_empty();
            if logic && !source && !built {
                add(
                    &mut violations,
                    Rule::Parts,
                    ".",
                    "provide logic.wasm or logic/Cargo.toml for the world's exports",
                );
            }
            if !logic && (c.has_imports || any_logic_file) {
                add(
                    &mut violations,
                    Rule::Parts,
                    ".",
                    "a mod without logic exports must have no imports or logic files",
                );
            }
            if !logic && !page {
                add(
                    &mut violations,
                    Rule::Parts,
                    ".",
                    "provide a page or a logic part",
                );
            }
            if let Some(meta) = &meta
                && c.world.http == meta.http_hosts.is_empty()
            {
                add(
                    &mut violations,
                    Rule::Http,
                    "mod.toml",
                    "import wasi:http/outgoing-handler and list http.hosts together",
                );
            }
            if let Some(deps) = &deps {
                let used: BTreeSet<_> = c
                    .world
                    .calls
                    .iter()
                    .chain(&c.world.subscriptions)
                    .map(|i| i.package())
                    .collect();
                for (slug, _) in deps.iter() {
                    let message = if slug == &c.slug {
                        Some(format!("deps.json lists this mod itself as mods:{slug}"))
                    } else if !used.contains(slug.as_str()) {
                        Some(format!(
                            "deps.json lists mods:{slug} but wit/world.wit imports none of its interfaces"
                        ))
                    } else {
                        None
                    };
                    if let Some(message) = message {
                        add(&mut violations, Rule::Provider, "deps.json", message);
                    }
                }
                for (interfaces, subscribe) in
                    [(&c.world.calls, false), (&c.world.subscriptions, true)]
                {
                    for interface in interfaces {
                        let Ok(slug) = interface.package().parse::<Slug>() else {
                            continue;
                        };
                        if deps.get(&slug).is_none() {
                            add(
                                &mut violations,
                                Rule::Provider,
                                "deps.json",
                                format!("add a publisher or null for mods:{slug}"),
                            );
                        }
                        if let Some(p) = c.providers.get(&slug) {
                            if !p.one_world {
                                add(
                                    &mut violations,
                                    Rule::Provider,
                                    "deps.json",
                                    format!("mods:{slug} must have exactly one world"),
                                );
                            } else if !(if subscribe { &p.events } else { &p.exports })
                                .contains(interface)
                            {
                                add(
                                    &mut violations,
                                    Rule::Provider,
                                    "deps.json",
                                    format!(
                                        "provider mods:{slug} does not {} {interface}",
                                        if subscribe { "import" } else { "export" }
                                    ),
                                );
                            }
                        } else {
                            add(
                                &mut violations,
                                Rule::Provider,
                                "deps.json",
                                format!("provide the WIT package mods:{slug}"),
                            );
                        }
                    }
                }
            }
        }
        let mut unique = Vec::new();
        for v in violations {
            if !unique.contains(&v) {
                unique.push(v);
            }
        }
        if !unique.is_empty() {
            return Err(ModError::Invalid(unique));
        }
        let (Some(meta), Some(deps), Some(contract)) = (meta, deps, contract) else {
            return Err(invalid(
                Rule::Wit,
                "wit/world.wit",
                "could not read the contract",
            ));
        };
        Ok(Self {
            dir: dir.into(),
            meta,
            deps,
            contract,
            page,
            logic,
        })
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn slug(&self) -> &Slug {
        &self.contract.slug
    }
    pub fn version(&self) -> &Version {
        &self.contract.version
    }
    pub fn name(&self) -> &str {
        &self.meta.name
    }
    pub fn summary(&self) -> &str {
        &self.meta.summary
    }
    pub fn http_hosts(&self) -> &[HttpHost] {
        &self.meta.http_hosts
    }
    pub fn origin(&self) -> Option<&ReleaseRef> {
        self.meta.origin.as_ref()
    }
    pub fn deps(&self) -> &Deps {
        &self.deps
    }
    pub fn world(&self) -> &World {
        &self.contract.world
    }
    pub fn page(&self) -> Option<PathBuf> {
        self.page.then(|| self.dir.join("ui"))
    }
    pub fn logic(&self) -> Option<PathBuf> {
        self.logic.then(|| self.dir.join("logic.wasm"))
    }
    // World's grant classification omits caller and type-only host/WASI names.
    // Reuse the WIT reader's path, size and nesting checks before resolving them.
    pub(crate) fn component_imports(&self) -> Result<BTreeSet<String>, ModError> {
        let mut violations = Vec::new();
        let files = inspect(&self.dir, &mut violations)?;
        if !violations.is_empty() {
            return Err(ModError::Invalid(violations));
        }
        let contract = world::read(&self.dir, &files, &mut violations)?;
        if !violations.is_empty() {
            return Err(ModError::Invalid(violations));
        }
        contract
            .map(|c| c.imports)
            .ok_or_else(|| invalid(Rule::Wit, "wit/world.wit", "could not read the contract"))
    }

    pub fn grants(&self) -> Vec<Grant> {
        let w = self.world();
        let mut grants = Vec::new();
        if w.storage {
            grants.push(Grant::Storage);
        }
        grants.extend(
            self.http_hosts()
                .iter()
                .cloned()
                .map(|host| Grant::Http { host }),
        );
        for (interfaces, subscribe) in [(&w.calls, false), (&w.subscriptions, true)] {
            for interface in interfaces {
                if let Ok(slug) = interface.package().parse::<Slug>() {
                    let provider = Provider {
                        publisher: self.deps.get(&slug).flatten().cloned(),
                        slug,
                    };
                    grants.push(if subscribe {
                        Grant::Subscribe {
                            interface: interface.clone(),
                            provider,
                        }
                    } else {
                        Grant::Call {
                            interface: interface.clone(),
                            provider,
                        }
                    });
                }
            }
        }
        grants.sort();
        grants.dedup();
        grants
    }
    pub fn requires(&self) -> Vec<Requirement> {
        self.deps
            .iter()
            .filter_map(|(slug, publisher)| {
                self.contract.providers.get(slug).map(|p| Requirement {
                    provider: Provider {
                        slug: slug.clone(),
                        publisher: publisher.cloned(),
                    },
                    version: p.version.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac3_s22_missing_optional_file() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_file(dir.path(), "deps.json", 100, Rule::DepsJson, true).unwrap(),
            None
        );
    }
    #[cfg(unix)]
    #[test]
    fn ac3_s22_checked_handle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod.toml");
        std::os::unix::fs::symlink("missing", &path).unwrap();
        let Err(ModError::Invalid(v)) =
            read_file(dir.path(), "mod.toml", 100, Rule::ModToml, false)
        else {
            panic!("expected invalid")
        };
        assert_eq!(v[0].rule, Rule::Path);
        assert_eq!(
            v[0].message,
            "replace the symbolic link with a regular file"
        );
        fs::remove_file(&path).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
        let Err(ModError::Invalid(v)) =
            read_file(dir.path(), "mod.toml", 100, Rule::ModToml, false)
        else {
            panic!("expected invalid")
        };
        assert_eq!(v[0].message, "expected a regular file");
    }
}
