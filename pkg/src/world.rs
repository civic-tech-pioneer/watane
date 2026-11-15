use crate::{InterfaceName, ModError, Rule, Slug, Version, Violation, folder::read_file};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use wit_parser::{
    Resolve, SourceMap, Type, TypeDefKind, UnresolvedPackageGroup, WorldItem, WorldKey,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct World {
    pub exports: Vec<InterfaceName>,
    pub events: Vec<InterfaceName>,
    pub collects: Vec<InterfaceName>,
    pub calls: Vec<InterfaceName>,
    pub subscriptions: Vec<InterfaceName>,
    pub storage: bool,
    pub manage: bool,
    pub http: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Contract {
    pub slug: Slug,
    pub version: Version,
    pub world: World,
    pub has_imports: bool,
    pub imports: BTreeSet<String>,
    pub providers: BTreeMap<Slug, ProviderWorld>,
    pub own_package_copy: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct ProviderWorld {
    pub version: Version,
    pub exports: BTreeSet<InterfaceName>,
    pub events: BTreeSet<InterfaceName>,
    pub one_world: bool,
}
fn violation(v: &mut Vec<Violation>, rule: Rule, file: &str, message: impl Into<String>) {
    v.push(Violation {
        rule,
        file: file.into(),
        message: message.into(),
    });
}

fn is_list(resolve: &Resolve, t: &Type) -> bool {
    let mut t = *t;
    // Follow `type x = y` aliases; the chain is finite because WIT forbids cycles.
    while let Type::Id(id) = t {
        match &resolve.types[id].kind {
            TypeDefKind::List(_) => return true,
            TypeDefKind::Type(next) => t = *next,
            _ => return false,
        }
    }
    false
}

// This scan runs before handing any WIT to the parser. Comments may nest.
fn too_deep(bytes: &[u8]) -> bool {
    let (mut i, mut depth, mut comments) = (0, 0usize, 0usize);
    while i < bytes.len() {
        let pair = bytes.get(i..i + 2);
        if comments > 0 {
            if pair == Some(b"/*") {
                comments += 1;
                i += 2;
            } else if pair == Some(b"*/") {
                comments -= 1;
                i += 2;
            } else {
                i += 1;
            }
        } else if pair == Some(b"//") {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if pair == Some(b"/*") {
            comments = 1;
            i += 2;
        } else if pair == Some(b"->") {
            i += 2;
        } else {
            match bytes[i] {
                b'<' | b'(' | b'{' => {
                    depth += 1;
                    if depth > 32 {
                        return true;
                    }
                }
                b'>' | b')' | b'}' => depth = depth.saturating_sub(1),
                _ => (),
            }
            i += 1;
        }
    }
    false
}

// Parse just the package declaration independently: local interfaces may live in other files.
fn package_header(text: &str) -> &str {
    let bytes = text.as_bytes();
    let (mut i, mut comments) = (0, 0usize);
    while i < bytes.len() {
        let pair = bytes.get(i..i + 2);
        if comments > 0 {
            if pair == Some(b"/*") {
                comments += 1;
                i += 2;
            } else if pair == Some(b"*/") {
                comments -= 1;
                i += 2;
            } else {
                i += 1;
            }
        } else if pair == Some(b"/*") {
            comments = 1;
            i += 2;
        } else if pair == Some(b"//") {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b';' {
            return &text[..i + 1];
        } else {
            i += 1;
        }
    }
    text
}

pub(crate) fn read(
    dir: &Path,
    files: &[String],
    violations: &mut Vec<Violation>,
) -> Result<Option<Contract>, ModError> {
    let mut total = 0usize;
    let mut world_bytes = Vec::new();
    let mut checked = BTreeMap::new();
    let mut safe = true;
    if !files.iter().any(|f| f == "wit/world.wit") {
        violation(
            violations,
            Rule::Wit,
            "wit/world.wit",
            "add wit/world.wit with a package line and one world",
        );
        safe = false;
    }
    for file in files {
        let bytes = match read_file(dir, file, 1_048_576, Rule::Limit, false) {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(ModError::Invalid(v)) => {
                violations.extend(v);
                safe = false;
                continue;
            }
            Err(e) => return Err(e),
        };
        total += bytes.len();
        if total > 4_194_304 {
            violation(
                violations,
                Rule::Limit,
                file,
                "all WIT files must total at most 4 MiB",
            );
            safe = false;
        }
        if file == "wit/world.wit" {
            world_bytes.clone_from(&bytes);
        }
        checked.insert(file.clone(), bytes.clone());
        if too_deep(&bytes) {
            violation(
                violations,
                Rule::Limit,
                file,
                "WIT nesting must be at most 32",
            );
            safe = false;
        }
    }
    if !safe {
        return Ok(None);
    }
    let parsed = std::panic::catch_unwind(|| -> Result<_, String> {
        // Feed only bytes read and bounded above to the parser, never a path to reopen.
        let text = std::str::from_utf8(&world_bytes).unwrap_or("");
        let own_name =
            UnresolvedPackageGroup::parse(dir.join("wit/world.wit"), package_header(text))
                .map_err(|(_, e)| e.to_string())?
                .main
                .name;
        let mut maps: BTreeMap<std::path::PathBuf, SourceMap> = BTreeMap::new();
        for (file, bytes) in &checked {
            let path = dir.join(file);
            let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            // As push_dir: each wit/deps/<s>.wit is its own package; a directory is one.
            let parent = path.parent().unwrap_or(dir);
            let key = if parent == dir.join("wit/deps") {
                path.clone()
            } else {
                parent.to_path_buf()
            };
            maps.entry(key).or_default().push(&path, text);
        }
        let mut groups = Vec::new();
        let mut sources = BTreeMap::new();
        let mut main = None;
        let mut own_package_copy = false;
        for (parent, map) in maps {
            let paths: Vec<_> = map.source_names().map(std::path::PathBuf::from).collect();
            let group = map
                .parse()
                .map_err(|(_, e)| format!("failed to parse package: {}: {e}", parent.display()))?;
            // Diagnose the own-provider copy before resolution can obscure it
            // with a duplicate-package error. Its checked bytes were parsed above.
            if own_name.namespace == "mods"
                && parent == dir.join("wit/deps").join(&own_name.name)
                && group.main.name.namespace == own_name.namespace
                && group.main.name.name == own_name.name
            {
                own_package_copy = true;
                continue;
            }
            for package in std::iter::once(&group.main).chain(&group.nested) {
                sources.insert(package.name.to_string(), paths.clone());
            }
            if parent == dir.join("wit") {
                main = Some(group);
            } else {
                groups.push(group);
            }
        }
        let mut resolve = Resolve::default();
        let main = resolve
            .push_groups(main.ok_or("missing WIT package")?, groups)
            .map_err(|e| e.to_string())?;
        Ok((resolve, main, sources, own_package_copy))
    });
    let (resolve, main, sources, own_package_copy) = match parsed {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => {
            let message = format!("{e:#}");
            let file = files
                .iter()
                .find(|f| message.contains(&dir.join(f).to_string_lossy().to_string()))
                .map_or("wit/world.wit", String::as_str);
            // Take at a UTF-8 boundary, at most 2000 bytes.
            let end = message
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 2000)
                .last()
                .unwrap_or(0);
            let message = if message.len() <= 2000 {
                &message
            } else {
                &message[..end]
            };
            violation(violations, Rule::Wit, file, message);
            return Ok(None);
        }
        Err(_) => {
            violation(
                violations,
                Rule::Wit,
                "wit/world.wit",
                "WIT parser panicked; simplify or repair the contract",
            );
            return Ok(None);
        }
    };
    let mut packages_valid = true;
    let mut seen = BTreeSet::new();
    for (id, package) in resolve.packages.iter() {
        let name = &package.name;
        let file = sources
            .get(&name.to_string())
            .and_then(|p| p.first())
            .and_then(|p| p.strip_prefix(dir).ok())
            .and_then(Path::to_str)
            .unwrap_or("wit/world.wit");
        let version = name
            .version
            .as_ref()
            .and_then(|v| v.to_string().parse::<Version>().ok());
        let allowed = match name.namespace.as_str() {
            "mods" => name.name.parse::<Slug>().is_ok(),
            "wasi" => true,
            "watane" => {
                name.name == "host"
                    && version
                        .as_ref()
                        .is_some_and(|v| v.major == 0 && v.minor == 1 && v.patch == 0)
            }
            _ => false,
        };
        if !allowed
            || version.is_none()
            || !seen.insert((name.namespace.clone(), name.name.clone()))
        {
            violation(
                violations,
                Rule::Wit,
                file,
                format!("invalid, unversioned or repeated package {name}"),
            );
            packages_valid = false;
        }
        if name.namespace == "mods" && id != main {
            let parent = dir.join("wit/deps").join(&name.name);
            if sources
                .get(&name.to_string())
                .is_none_or(|paths| !paths.iter().all(|p| p.parent() == Some(parent.as_path())))
            {
                violation(
                    violations,
                    Rule::Wit,
                    file,
                    format!("put mods:{} in wit/deps/{}/", name.name, name.name),
                );
                packages_valid = false;
            }
        }
    }
    let main_package = &resolve.packages[main];
    let name = &main_package.name;
    if name.namespace != "mods" || main_package.worlds.len() != 1 {
        violation(
            violations,
            Rule::Wit,
            "wit/world.wit",
            "use package mods:<slug>@<version> and exactly one world",
        );
        return Ok(None);
    }
    if !packages_valid {
        return Ok(None);
    }
    let (Ok(slug), Some(Ok(version)), Some(&world_id)) = (
        name.name.parse::<Slug>(),
        name.version
            .as_ref()
            .map(|v| v.to_string().parse::<Version>()),
        main_package.worlds.values().next(),
    ) else {
        return Ok(None);
    };
    let wit_world = &resolve.worlds[world_id];
    let mut world = World {
        exports: vec![],
        events: vec![],
        collects: vec![],
        calls: vec![],
        subscriptions: vec![],
        storage: false,
        manage: false,
        http: false,
    };
    for (import, items) in [(true, &wit_world.imports), (false, &wit_world.exports)] {
        for (key, item) in items {
            if matches!(item, WorldItem::Type { .. }) {
                continue;
            }
            let (WorldKey::Interface(_), WorldItem::Interface { id, .. }) = (key, item) else {
                violation(
                    violations,
                    Rule::Wit,
                    "wit/world.wit",
                    "use package-qualified interfaces, not world functions or plain-named interfaces",
                );
                continue;
            };
            let Some(text) = resolve.id_of(*id) else {
                violation(
                    violations,
                    Rule::Wit,
                    "wit/world.wit",
                    "interface must belong to a named, versioned package",
                );
                continue;
            };
            let interface: InterfaceName = match text.parse() {
                Ok(i) => i,
                Err(e) => {
                    violation(violations, Rule::Wit, "wit/world.wit", e.to_string());
                    continue;
                }
            };
            let (ns, pkg) = (interface.namespace(), interface.package());
            if ns == "mods" {
                match (import, pkg == slug.as_str()) {
                    (true, true) => {
                        let mut collect = false;
                        for (fname, f) in &resolve.interfaces[*id].functions {
                            match &f.result {
                                None => {}
                                Some(t) if is_list(&resolve, t) => collect = true,
                                Some(_) => violation(
                                    violations,
                                    Rule::Import,
                                    "wit/world.wit",
                                    format!(
                                        "{}.{fname}: a function of an own interface that the world imports returns nothing (an event) or a list (a collect)",
                                        interface.interface()
                                    ),
                                ),
                            }
                        }
                        if collect {
                            world.collects.push(interface.clone());
                        }
                        world.events.push(interface);
                    }
                    (true, false) => world.calls.push(interface),
                    (false, true) => world.exports.push(interface),
                    (false, false) => world.subscriptions.push(interface),
                }
                continue;
            }
            let v = interface.version();
            let accepted = import
                && match ns {
                    "watane" => match text.as_str() {
                        "watane:host/state@0.1.0" => {
                            world.storage = true;
                            true
                        }
                        "watane:host/manage@0.1.0" => {
                            world.manage = true;
                            true
                        }
                        "watane:host/caller@0.1.0" => true,
                        _ => false,
                    },
                    "wasi" if v.major == 0 && v.minor == 2 => match pkg {
                        "cli" | "clocks" | "filesystem" | "io" | "random" | "sockets" => true,
                        "http" => match interface.interface() {
                            "outgoing-handler" => {
                                world.http = true;
                                true
                            }
                            "types" => true,
                            _ => false,
                        },
                        _ => false,
                    },
                    _ => false,
                };
            if !accepted {
                violation(
                    violations,
                    if import { Rule::Import } else { Rule::Export },
                    "wit/world.wit",
                    format!("remove the unsupported interface {text}"),
                );
            }
        }
    }
    for list in [
        &mut world.exports,
        &mut world.events,
        &mut world.collects,
        &mut world.calls,
        &mut world.subscriptions,
    ] {
        list.sort();
        list.dedup();
        if list.len() > 32 {
            violation(
                violations,
                Rule::Limit,
                "wit/world.wit",
                "each world interface list is limited to 32",
            );
        }
    }
    let mut providers = BTreeMap::new();
    for (id, package) in resolve.packages.iter() {
        if id == main || package.name.namespace != "mods" {
            continue;
        }
        let Ok(slug) = package.name.name.parse() else {
            continue;
        };
        let Some(Ok(version)) = package.name.version.as_ref().map(|v| v.to_string().parse()) else {
            continue;
        };
        let mut provider = ProviderWorld {
            version,
            exports: BTreeSet::new(),
            events: BTreeSet::new(),
            one_world: package.worlds.len() == 1,
        };
        if let Some(w) = package.worlds.values().next() {
            let w = &resolve.worlds[*w];
            for (set, items) in [
                (&mut provider.exports, &w.exports),
                (&mut provider.events, &w.imports),
            ] {
                for (key, item) in items {
                    if !matches!(key, WorldKey::Interface(_)) {
                        continue;
                    }
                    if let WorldItem::Interface { id, .. } = item
                        && let Some(Ok(name)) =
                            resolve.id_of(*id).map(|s| s.parse::<InterfaceName>())
                    {
                        set.insert(name);
                    }
                }
            }
        }
        providers.insert(slug, provider);
    }
    Ok(Some(Contract {
        slug,
        version,
        world,
        has_imports: !wit_world.imports.is_empty(),
        imports: wit_world
            .imports
            .keys()
            .map(|key| resolve.name_world_key(key))
            .collect(),
        providers,
        own_package_copy,
    }))
}
