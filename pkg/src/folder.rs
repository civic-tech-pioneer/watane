//! The mod folder (TECHNICAL 4.6, 4.7): `Mod::read`, its parts, and the rules it
//! enforces. Every refusal is a value that names the rule, never a panic.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};

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
            return Err(invalid(Rule::Path, &part, crate::refusal::LINK_FILE));
        }
        Err((_, e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return if optional {
                Ok(None)
            } else {
                Err(invalid(rule, file, "required file is missing"))
            };
        }
        Err((_, e))
            if crate::open::is_socket(&e)
                || matches!(
                    e.kind(),
                    std::io::ErrorKind::InvalidInput
                        | std::io::ErrorKind::NotADirectory
                        | std::io::ErrorKind::IsADirectory
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
        let before = violations.len();
        let Some(handle) = open_listed(root, &relative, violations)? else {
            found |= violations.len() > before;
            continue;
        };
        let Some(list) = list_entries(&handle, root, &relative, violations)? else {
            found |= violations.len() > before;
            continue;
        };
        for crate::open::Entry { name, stat, .. } in list.into_iter().rev() {
            let path = format!("{relative}/{}", name.to_string_lossy());
            match stat.kind {
                Kind::Link => {
                    found = true;
                    record_link(violations, &path);
                }
                Kind::Dir => stack.push(path),
                _ => {}
            }
        }
    }
    Ok(found)
}
use crate::refusal::{CHANGED, LINK_ENTRY as LINK, LINK_FILE, SPECIAL};
const WIT_DIRECTORY: &str = "WIT directories must be wit/deps/<segment>";

fn record_link(violations: &mut Vec<Violation>, path: &str) {
    let hidden = path
        .match_indices('/')
        .map(|(end, _)| &path[..end])
        .find(|parent| {
            parent
                .rsplit('/')
                .next()
                .is_some_and(|name| name.starts_with('.'))
        });
    let (file, message) = match hidden {
        Some(folder) if folder.starts_with("wit/") => (folder, WIT_DIRECTORY),
        Some(folder) => (
            folder,
            "a hidden directory contains a symbolic link. Remove the link.",
        ),
        None => (path, LINK),
    };
    add(violations, Rule::Path, file, message);
}

#[cfg(test)]
type OpenHook = Option<Box<dyn FnMut(&str)>>;
#[cfg(test)]
thread_local!(static BEFORE_OPEN: std::cell::RefCell<OpenHook> = const { std::cell::RefCell::new(None) });
// Test hook: runs after an entry was listed and before it is opened.
fn before_open(_relative: &str) {
    #[cfg(test)]
    BEFORE_OPEN.with(|h| {
        if let Some(f) = h.borrow_mut().as_mut() {
            f(_relative);
        }
    });
}
// Opening a listed directory: a link keeps its own refusal; a vanished or replaced entry is a change.
fn open_listed(
    root: &Path,
    relative: &str,
    violations: &mut Vec<Violation>,
) -> Result<Option<crate::open::Dir>, ModError> {
    before_open(relative);
    match crate::open::dir(root, relative) {
        Ok(handle) => Ok(Some(handle)),
        Err((part, e)) if crate::open::is_link(&e) => {
            record_link(violations, &part);
            Ok(None)
        }
        Err((_, e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            add(violations, Rule::Path, relative, CHANGED);
            Ok(None)
        }
        Err((part, e)) => Err(io(&root.join(part), e)),
    }
}
// Listing an opened directory: an entry gone before its stat is a change, not a fault of the folder.
fn list_entries(
    handle: &crate::open::Dir,
    root: &Path,
    relative: &str,
    violations: &mut Vec<Violation>,
) -> Result<Option<Vec<crate::open::Entry>>, ModError> {
    match crate::open::entries(handle) {
        Ok(list) => Ok(Some(list)),
        Err(e) if e.source.kind() == std::io::ErrorKind::NotFound => {
            add(violations, Rule::Path, &e.relative_path(relative), CHANGED);
            Ok(None)
        }
        Err(e) if e.source.kind() == std::io::ErrorKind::NotADirectory => {
            add(violations, Rule::Path, relative, CHANGED);
            Ok(None)
        }
        Err(e) => Err(io(&root.join(relative), e.source)),
    }
}
fn segment(s: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().next().is_some_and(first)
        && s.bytes().all(|b| first(b) || b == b'.')
        && !crate::package::windows_device(s)
}
// Never recurse through links; use an explicit stack for hostile directory depths.
fn inspect(dir: &Path, violations: &mut Vec<Violation>) -> Result<Vec<String>, ModError> {
    use crate::open::Kind;
    let mut files = Vec::new();
    for file in ["mod.toml", "deps.json", "logic.wasm"] {
        if lstat(dir, file)?.is_some_and(|m| m.kind == Kind::Link) {
            add(violations, Rule::Path, file, LINK_FILE);
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
            record_link(violations, &relative);
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
                    add(violations, Rule::Path, &relative, WIT_DIRECTORY);
                }
                continue;
            }
            let Some(handle) = open_listed(dir, &relative, violations)? else {
                continue;
            };
            let Some(list) = list_entries(&handle, dir, &relative, violations)? else {
                continue;
            };
            for crate::open::Entry { name, stat, .. } in list.into_iter().rev() {
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
                if hidden && !matches!(stat.kind, Kind::Link | Kind::Dir) {
                    continue;
                }
                stack.push((path, stat));
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
            add(violations, Rule::Path, &relative, SPECIAL);
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
        // A linked part already has its `path` violation; the parts rules add nothing for it.
        let ui = lstat(dir, "ui")?;
        let page = ui.is_some();
        if meta.as_ref().is_some_and(|meta| meta.main)
            && !ui.is_some_and(|m| m.kind == Kind::Link)
            && !lstat(dir, "ui/index.html")?.is_some_and(|m| m.kind == Kind::File)
        {
            add(
                &mut violations,
                Rule::ModToml,
                "mod.toml",
                "`main` needs a page (ui/index.html)",
            );
        }
        if ui.is_some_and(|m| m.kind != Kind::Link && m.kind != Kind::Dir)
            || ui.is_some_and(|m| m.kind == Kind::Dir)
                && !lstat(dir, "ui/index.html")?.is_some_and(|m| m.kind == Kind::File)
        {
            add(
                &mut violations,
                Rule::Parts,
                ".",
                "ui/ must be a directory containing a regular ui/index.html",
            );
        }
        let source = lstat(dir, "logic/Cargo.toml")?.is_some_and(|m| m.kind == Kind::File);
        let wasm = lstat(dir, "logic.wasm")?;
        let wasm_link = wasm.is_some_and(|m| m.kind == Kind::Link);
        let built = wasm.is_some_and(|m| m.kind == Kind::File);
        let any_logic_file =
            lstat(dir, "logic/Cargo.toml")?.is_some() || wasm.is_some() && !wasm_link;
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
            logic = !c.world.exports.is_empty()
                || !c.world.subscriptions.is_empty()
                || !c.world.host_exports.is_empty();
            if logic && !source && !built && !wasm_link {
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
                    "import wasi:http/outgoing-handler or watane:host/https-stream and list http.hosts together",
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
    pub fn main(&self) -> bool {
        self.meta.main
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
        if w.manage {
            grants.push(Grant::Manage);
        }
        if self
            .contract
            .imports
            .contains("watane:host/claude-code@0.1.0")
        {
            grants.push(Grant::ClaudeCode);
        }
        if self
            .contract
            .imports
            .contains("watane:host/claude-code-shell@0.1.0")
        {
            grants.push(Grant::ClaudeCodeShell);
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
            "a symbolic link; replace it with a regular file"
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
    #[cfg(unix)]
    fn listed(
        handle: &crate::open::Dir,
        relative: &str,
    ) -> (
        Result<Option<Vec<crate::open::Entry>>, ModError>,
        Vec<Violation>,
    ) {
        let mut violations = Vec::new();
        let result = list_entries(handle, Path::new("/mod"), relative, &mut violations);
        (result, violations)
    }
    #[cfg(unix)]
    #[test]
    fn ac1_s121_list_entries_vanished_is_changed_at_entry() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("gone"), b"x").unwrap();
        let handle = crate::open::dir(dir.path(), "").map_err(|e| e.1).unwrap();
        let path = dir.path().join("gone");
        crate::open::BEFORE_ENTRY_STAT.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |_| {
                let _ = fs::remove_file(&path);
            }));
        });
        let (result, violations) = listed(&handle, "ui");
        crate::open::BEFORE_ENTRY_STAT.with(|h| *h.borrow_mut() = None);
        assert!(matches!(result, Ok(None)));
        assert_eq!(
            violations,
            vec![Violation {
                rule: Rule::Path,
                file: "ui/gone".into(),
                message: CHANGED.into()
            }]
        );
    }
    #[cfg(unix)]
    #[test]
    fn ac1_s121_list_entries_not_a_directory_is_changed_at_relative() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, b"x").unwrap();
        let handle = crate::open::Dir::from_file(fs::File::open(file).unwrap());
        let (result, violations) = listed(&handle, "ui");
        assert!(matches!(result, Ok(None)));
        assert_eq!(
            violations,
            vec![Violation {
                rule: Rule::Path,
                file: "ui".into(),
                message: CHANGED.into()
            }]
        );
    }
    #[cfg(unix)]
    #[test]
    fn ac1_s121_list_entries_other_error_is_io_naming_directory() {
        use rustix::fs::{Mode, OFlags, open};
        let dir = tempfile::tempdir().unwrap();
        // An O_PATH handle cannot be read as a directory: EBADF, neither class above.
        let fd = open(
            dir.path(),
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        let handle = crate::open::Dir::from_file(fs::File::from(fd));
        let (result, violations) = listed(&handle, "ui");
        assert!(violations.is_empty());
        let Err(ModError::Io { path, .. }) = result else {
            panic!("expected Io")
        };
        assert_eq!(path, Path::new("/mod/ui"));
    }
    fn hello() -> tempfile::TempDir {
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).unwrap();
            for e in fs::read_dir(from).unwrap() {
                let p = e.unwrap().path();
                let dest = to.join(p.file_name().unwrap());
                if p.is_dir() {
                    copy(&p, &dest);
                } else {
                    fs::copy(p, dest).unwrap();
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello"),
            dir.path(),
        );
        dir
    }
    fn changed(dir: &Path, entry: &'static str, swap: impl Fn(&Path) + 'static) {
        let root = dir.to_path_buf();
        BEFORE_OPEN.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |relative| {
                if relative == entry {
                    swap(&root);
                }
            }));
        });
        let result = Mod::read(dir);
        BEFORE_OPEN.with(|h| *h.borrow_mut() = None);
        let Err(ModError::Invalid(v)) = result else {
            panic!("expected invalid")
        };
        assert!(
            v.contains(&Violation {
                rule: Rule::Path,
                file: entry.into(),
                message: "changed while the mod was read; read it again".into()
            }),
            "{v:?}"
        );
    }
    #[test]
    fn ac5_s57_inspect_entry_replaced_by_file() {
        let dir = hello();
        fs::create_dir_all(dir.path().join("ui/sub")).unwrap();
        changed(dir.path(), "ui/sub", |root| {
            fs::remove_dir(root.join("ui/sub")).unwrap();
            fs::write(root.join("ui/sub"), "x").unwrap();
        });
    }
    #[test]
    fn ac5_s57_links_below_entry_removed() {
        let dir = hello();
        fs::create_dir_all(dir.path().join("wit/x/sub")).unwrap();
        changed(dir.path(), "wit/x/sub", |root| {
            fs::remove_dir(root.join("wit/x/sub")).unwrap();
        });
    }
}
