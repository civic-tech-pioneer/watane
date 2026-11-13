use serde::Serialize;
use std::{
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
// Stat below the mod root through no-follow directory handles; a missing or
// link/non-directory parent means the entry is not there.
fn lstat(root: &Path, relative: &str) -> Result<Option<crate::open::Stat>, ModError> {
    let (parent, name) = relative.rsplit_once('/').unwrap_or(("", relative));
    let dir = match crate::open::dir(root, parent) {
        Ok(dir) => dir,
        Err((_, e)) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err((part, e))
            if !part.is_empty()
                && (crate::open::is_link(&e) || e.kind() == std::io::ErrorKind::NotADirectory) =>
        {
            return Ok(None);
        }
        Err((part, e)) => return Err(io(&root.join(part), e)),
    };
    match crate::open::stat(&dir, name) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(&root.join(relative), e)),
    }
}
// The links below a directory the WIT rule refuses, hidden ones included.
fn links_below(
    root: &Path,
    relative: &str,
    violations: &mut Vec<Violation>,
) -> Result<bool, ModError> {
    use crate::open::Kind;
    let mut found = false;
    let mut stack = vec![relative.to_string()];
    while let Some(relative) = stack.pop() {
        let handle = match crate::open::dir(root, &relative) {
            Ok(handle) => handle,
            Err((part, e)) if crate::open::is_link(&e) => {
                found = true;
                add(violations, Rule::Path, &part, LINK);
                continue;
            }
            Err((part, e)) => return Err(io(&root.join(part), e)),
        };
        let list = crate::open::entries(&handle).map_err(|e| io(&root.join(&relative), e))?;
        for (name, m) in list.into_iter().rev() {
            let path = format!("{relative}/{}", name.to_string_lossy());
            match m.kind {
                Kind::Link => {
                    found = true;
                    add(violations, Rule::Path, &path, LINK);
                }
                Kind::Dir => stack.push(path),
                _ => {}
            }
        }
    }
    Ok(found)
}
const LINK: &str = "replace the symbolic link with a regular entry";
fn segment(s: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().next().is_some_and(first)
        && s.bytes().all(|b| first(b) || b == b'.')
}
// Never recurse through links; use an explicit stack for hostile directory depths.
fn inspect(dir: &Path, violations: &mut Vec<Violation>) -> Result<Vec<String>, ModError> {
    use crate::open::Kind;
    let mut files = Vec::new();
    for file in ["mod.toml", "deps.json", "logic.wasm"] {
        if lstat(dir, file)?.is_some_and(|m| m.kind == Kind::Link) {
            add(
                violations,
                Rule::Path,
                file,
                "replace the symbolic link with a regular file",
            );
        }
    }
    let mut stack = Vec::new();
    for top in ["ui", "wit"] {
        if let Some(m) = lstat(dir, top)? {
            stack.push((top.to_string(), m));
        }
    }
    while let Some((relative, m)) = stack.pop() {
        if m.kind == Kind::Link {
            add(violations, Rule::Path, &relative, LINK);
            continue;
        }
        let wit = relative == "wit" || relative.starts_with("wit/");
        if m.kind == Kind::Dir {
            if wit
                && relative != "wit"
                && relative != "wit/deps"
                && !(relative.starts_with("wit/deps/")
                    && relative.split('/').count() == 3
                    && relative.rsplit('/').next().is_some_and(segment))
            {
                if !links_below(dir, &relative, violations)? {
                    add(
                        violations,
                        Rule::Path,
                        &relative,
                        "WIT directories must be wit/deps/<segment>",
                    );
                }
                continue;
            }
            let handle = match crate::open::dir(dir, &relative) {
                Ok(handle) => handle,
                Err((part, e)) if crate::open::is_link(&e) => {
                    add(violations, Rule::Path, &part, LINK);
                    continue;
                }
                Err((part, e)) => return Err(io(&dir.join(part), e)),
            };
            let list = crate::open::entries(&handle).map_err(|e| io(&dir.join(&relative), e))?;
            for (name, child) in list.into_iter().rev() {
                let Some(name) = name.to_str() else {
                    add(
                        violations,
                        Rule::Path,
                        &format!("{relative}/{}", name.to_string_lossy()),
                        "use UTF-8 path names",
                    );
                    continue;
                };
                let path = format!("{relative}/{name}");
                // Hidden content is not part of a package: only its links matter.
                let hidden = !wit && path.split('/').any(|s| s.starts_with('.'));
                if hidden && !matches!(child.kind, Kind::Link | Kind::Dir) {
                    continue;
                }
                stack.push((path, child));
            }
        } else if wit {
            if relative == "wit" || relative == "wit/deps" {
                add(violations, Rule::Wit, &relative, "expected a WIT directory");
            }
            let name = relative.rsplit('/').next().unwrap_or_default();
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
                if !segment(name) || m.kind != Kind::File {
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
        } else if m.kind == Kind::Other {
            add(
                violations,
                Rule::Path,
                &relative,
                "expected a regular file or a directory",
            );
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
        use crate::open::Kind;
        let ui = lstat(dir, "ui")?;
        let page = ui.is_some();
        // A linked `ui` already has its `path` violation; the parts rule adds nothing for it.
        if page
            && !ui.is_some_and(|m| m.kind == Kind::Link)
            && (!lstat(dir, "ui")?.is_some_and(|m| m.kind == Kind::Dir)
                || !lstat(dir, "ui/index.html")?.is_some_and(|m| m.kind == Kind::File))
        {
            add(
                &mut violations,
                Rule::Parts,
                ".",
                "ui/ must be a directory containing a regular ui/index.html",
            );
        }
        let source = lstat(dir, "logic/Cargo.toml")?.is_some_and(|m| m.kind == Kind::File);
        let built = lstat(dir, "logic.wasm")?.is_some_and(|m| m.kind == Kind::File);
        let any_logic_file =
            lstat(dir, "logic/Cargo.toml")?.is_some() || lstat(dir, "logic.wasm")?.is_some();
        let mut logic = false;
        if let Some(c) = &contract {
            if c.own_package_copy
                && !deps
                    .as_ref()
                    .is_some_and(|deps| deps.get(&c.slug).is_some())
            {
                let file = format!("wit/deps/{}", c.slug);
                add(
                    &mut violations,
                    Rule::Provider,
                    &file,
                    format!("{file} holds this mod's own package mods:{}", c.slug),
                );
            }
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
    pub fn repository(&self) -> Option<&str> {
        self.meta.repository.as_deref()
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
    use std::fs;
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
