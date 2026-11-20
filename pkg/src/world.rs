//! Reading the WIT (TECHNICAL 4.4, 4.5): parses a mod's world and classifies its
//! imports and exports into `World`.

use crate::folder::read_file;
use crate::{InterfaceName, ModError, Rule, Slug, Version, Violation};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
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
    #[expect(
        clippy::disallowed_methods,
        reason = "defense in depth: wit-parser may panic on untrusted WIT"
    )]
    let parsed = std::panic::catch_unwind(|| -> Result<_, String> {
        // Feed only bytes read and bounded above to the parser, never a path to reopen.
        let text = std::str::from_utf8(&world_bytes).unwrap_or("");
        let own_name =
            UnresolvedPackageGroup::parse(dir.join("wit/world.wit"), package_header(text))
                .map_err(|(map, e)| e.render(&map))?
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
            let group = map.parse().map_err(|(map, e)| {
                format!(
                    "failed to parse package: {}: {}",
                    parent.display(),
                    e.render(&map)
                )
            })?;
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
            .map_err(|e| e.render(&resolve.source_map))?;
        Ok((resolve, main, sources, own_package_copy))
    });
    let (resolve, main, sources, own_package_copy) = match parsed {
        Ok(Ok(value)) => value,
        Ok(Err(e)) => {
            let message = fix_message(&format!("{e:#}"), dir, &checked);
            let file = files
                .iter()
                .find(|f| message.contains(&dir.join(f).to_string_lossy().to_string()))
                .map_or("wit/world.wit", String::as_str);
            let message = cap_message(&escape_message(&message));
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

/// Characters that are Unicode Default_Ignorable_Code_Point or general category Cf
/// (DerivedCoreProperties and DerivedGeneralCategory of Unicode 16.0), the Hangul
/// fillers and the line and paragraph separators, as sorted inclusive ranges.
const INVISIBLE: &[(u32, u32)] = &[
    (0xad, 0xad),
    (0x34f, 0x34f),
    (0x600, 0x605),
    (0x61c, 0x61c),
    (0x6dd, 0x6dd),
    (0x70f, 0x70f),
    (0x890, 0x891),
    (0x8e2, 0x8e2),
    (0x115f, 0x1160),
    (0x17b4, 0x17b5),
    (0x180b, 0x180f),
    (0x200b, 0x200f),
    (0x2028, 0x202e),
    (0x2060, 0x206f),
    (0x3164, 0x3164),
    (0xfe00, 0xfe0f),
    (0xfeff, 0xfeff),
    (0xffa0, 0xffa0),
    (0xfff0, 0xfffb),
    (0x110bd, 0x110bd),
    (0x110cd, 0x110cd),
    (0x13430, 0x1343f),
    (0x1bca0, 0x1bca3),
    (0x1d173, 0x1d17a),
    (0xe0000, 0xe0fff),
];

fn is_invisible(c: char) -> bool {
    let c = c as u32;
    INVISIBLE.iter().any(|&(lo, hi)| (lo..=hi).contains(&c))
}

/// Write every control character other than `\n`, and every invisible character
/// (default-ignorable, format, Hangul filler, line or paragraph separator) of `message`
/// in its escaped form (TECHNICAL 4).
fn escape_message(message: &str) -> String {
    let mut out = String::with_capacity(message.len());
    for c in message.chars() {
        if c == '\n' {
            out.push(c);
        } else if c.is_control() {
            out.extend(c.escape_debug());
        } else if is_invisible(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out
}

/// Make wit-parser's rendered message exact for a person (TECHNICAL 4): its
/// `EscapeUnicode(EscapeIterInner('\u{..}'))` debug text becomes the escaped char,
/// and the byte column of its `path:line:col` location becomes a char column.
/// The source snippet after the location is left as written.
fn fix_message(message: &str, dir: &Path, sources: &BTreeMap<String, Vec<u8>>) -> String {
    const OPEN: &str = "EscapeUnicode(EscapeIterInner('";
    let (head, snippet) = message.split_at(message.find("\n      |").unwrap_or(message.len()));
    let mut text = String::with_capacity(message.len());
    let mut rest = head;
    while let Some(at) = rest.find(OPEN) {
        text.push_str(&rest[..at]);
        let inner = &rest[at + OPEN.len()..];
        match inner.find("'))") {
            Some(end) => {
                text.push_str(&inner[..end]);
                rest = &inner[end + 3..];
            }
            None => {
                rest = &rest[at..];
                break;
            }
        }
    }
    text.push_str(rest);
    for (file, bytes) in sources {
        let path = format!("{}:", dir.join(file).to_string_lossy());
        let Some(at) = text.find(&path).map(|at| at + path.len()) else {
            continue;
        };
        if let Some((len, location)) = char_column(&text[at..], bytes) {
            text.replace_range(at..at + len, &location);
        }
    }
    text + snippet
}

/// `text` starts with wit-parser's `<line>:<byte column>`: its length and
/// `<line>:<char column>` counted in `source`.
fn char_column(text: &str, source: &[u8]) -> Option<(usize, String)> {
    let (row, rest) = text.split_once(':')?;
    let col = &rest[..rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len())];
    let (line, byte): (usize, usize) = (row.parse().ok()?, col.parse().ok()?);
    let before = std::str::from_utf8(source)
        .ok()?
        .split('\n')
        .nth(line.checked_sub(1)?)?
        .get(..byte.checked_sub(1)?)?;
    Some((
        row.len() + 1 + col.len(),
        format!("{row}:{}", before.chars().count() + 1),
    ))
}

/// Cut `message` at a UTF-8 boundary to at most 2000 bytes, keeping the
/// wit-parser ` --> path:line:col` line (TECHNICAL 4).
fn cap_message(message: &str) -> String {
    fn cut(text: &str, max: usize) -> &str {
        let mut end = max.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    }
    if message.len() <= 2000 {
        return message.to_string();
    }
    let Some(arrow) = message.lines().find(|l| l.contains("--> ")) else {
        return cut(message, 2000).to_string();
    };
    let kept = cut(message, 2000);
    if kept.contains(arrow) {
        return kept.to_string();
    }
    if arrow.len() >= 2000 {
        return cut(arrow, 2000).to_string();
    }
    let head = message.replacen(arrow, "", 1);
    format!("{}\n{arrow}", cut(head.trim_end(), 1999 - arrow.len()))
}

#[cfg(test)]
mod tests {
    use super::{cap_message, escape_message, read};
    use crate::{Rule, Violation};

    fn wit_message(world: &str) -> String {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wit")).unwrap();
        std::fs::write(dir.path().join("wit/world.wit"), world).unwrap();
        let mut violations: Vec<Violation> = Vec::new();
        let files = vec!["wit/world.wit".to_string()];
        read(dir.path(), &files, &mut violations).unwrap();
        let v = violations.iter().find(|v| v.rule == Rule::Wit).unwrap();
        v.message
            .replace(&dir.path().to_string_lossy().to_string(), "<dir>")
    }

    #[test]
    fn ac1_s93_wit_column_counts_chars_not_bytes() {
        let m = wit_message("package a:b;\n/* é€ */ bad-token-here\n");
        assert!(m.contains("\n     --> <dir>/wit/world.wit:2:10\n"), "{m}");
    }

    #[test]
    fn ac2_s93_wit_message_has_no_rust_debug_text() {
        let m = wit_message("package a:b;\nworld w {\n  import a\u{202e}b: func();\n}\n");
        assert!(m.contains("\\u{202e}"), "{m}");
        assert!(!m.contains("EscapeUnicode"), "{m}");
        assert!(m.contains("codepoint \\u{202e}\n"), "{m}");
    }

    #[test]
    fn ac1_s93_long_line_location_counts_chars() {
        // A line over 500 bytes: wit-parser writes `path:line:col: message`, no snippet.
        let m = wit_message(&format!(
            "package a:b;\n/* {} */ bad\n",
            "\u{e9}".repeat(300)
        ));
        assert!(m.contains("<dir>/wit/world.wit:2:308: expected"), "{m}");
    }

    #[test]
    fn ac2_s93_snippet_keeps_source_as_written() {
        let m = wit_message("package a:b;\nworld w { @ } // EscapeUnicode(EscapeIterInner('x'))\n");
        assert!(
            m.contains("| world w { @ } // EscapeUnicode(EscapeIterInner('x'))"),
            "{m}"
        );
    }

    #[test]
    fn ac3_s93_escape_message_covers_soft_hyphen_mongolian_hangul_filler() {
        assert_eq!(
            escape_message("a\u{ad}b\u{180e}c\u{3164}d"),
            "a\\u{ad}b\\u{180e}c\\u{3164}d"
        );
    }

    /// Adversary S93: the other Hangul fillers (siblings of U+3164), the invisible math
    /// operators, the deprecated format characters, the combining grapheme joiner and the
    /// Khmer inherent vowels are invisible too; a WIT comment on the error line carries them
    /// raw into the snippet of the `wit` message.
    #[test]
    fn adv_s93_wit_message_other_invisible_chars() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("wit")).unwrap();
        let hidden = "\u{115f}\u{1160}\u{ffa0}\u{2061}\u{2062}\u{2063}\u{2064}\u{206a}\u{206f}\u{34f}\u{17b4}\u{17b5}";
        std::fs::write(
            dir.path().join("wit/world.wit"),
            format!("package a:b;\nworld w {{ @ }} // x{hidden}y\n"),
        )
        .unwrap();
        let mut violations: Vec<Violation> = Vec::new();
        read(dir.path(), &["wit/world.wit".to_string()], &mut violations).unwrap();
        let m = &violations
            .iter()
            .find(|v| v.rule == Rule::Wit)
            .unwrap()
            .message;
        let raw: Vec<char> = m.chars().filter(|c| hidden.contains(*c)).collect();
        assert!(
            raw.is_empty(),
            "raw invisible chars in the message: {raw:?}"
        );
    }

    /// Both ends of every range of the set (Unicode 16.0 Default_Ignorable_Code_Point, Cf,
    /// Hangul fillers, U+2028/U+2029) are escaped; the character just outside each is not.
    #[test]
    fn s93_fix1_invisible_table_boundaries() {
        let inside: &[u32] = &[
            0xad, 0x34f, 0x600, 0x605, 0x61c, 0x6dd, 0x70f, 0x890, 0x891, 0x8e2, 0x115f, 0x1160,
            0x17b4, 0x17b5, 0x180b, 0x180f, 0x200b, 0x200f, 0x2028, 0x202e, 0x2060, 0x2065, 0x206f,
            0x3164, 0xfe00, 0xfe0f, 0xfeff, 0xffa0, 0xfff0, 0xfffb, 0x110bd, 0x110cd, 0x13430,
            0x1343f, 0x1bca0, 0x1bca3, 0x1d173, 0x1d17a, 0xe0000, 0xe0fff,
        ];
        let outside: &[u32] = &[
            0xac, 0xae, 0x34e, 0x350, 0x5ff, 0x606, 0x61b, 0x61d, 0x6dc, 0x6de, 0x70e, 0x710,
            0x88f, 0x892, 0x8e1, 0x8e3, 0x115e, 0x1161, 0x17b3, 0x17b6, 0x180a, 0x1810, 0x200a,
            0x2010, 0x2027, 0x202f, 0x205f, 0x2070, 0x3163, 0x3165, 0xfdff, 0xfe10, 0xfefe, 0xff00,
            0xff9f, 0xffa1, 0xffef, 0xfffc, 0x110bc, 0x110be, 0x110cc, 0x110ce, 0x1342f, 0x13440,
            0x1bc9f, 0x1bca4, 0x1d172, 0x1d17b, 0xdffff, 0xe1000,
        ];
        for &u in inside {
            let c = char::from_u32(u).unwrap();
            assert_eq!(
                escape_message(&c.to_string()),
                format!("\\u{{{u:x}}}"),
                "{u:#x}"
            );
        }
        for &u in outside {
            let c = char::from_u32(u).unwrap();
            assert_eq!(escape_message(&c.to_string()), c.to_string(), "{u:#x}");
        }
    }

    #[test]
    fn ac3_s93_visible_text_passes_through() {
        let text = "é€日本 😀 مرحبا";
        assert_eq!(escape_message(text), text);
    }

    #[test]
    fn ac3_s86_long_message_keeps_arrow_and_utf8_boundary() {
        // "x" then two-byte chars: every limit below falls inside a char.
        let tail = format!("x{}", "é".repeat(3000));
        let arrow = "     --> /m/wit/world.wit:4:3";
        let message = format!("{tail}\n{arrow}\n      |\n    4 | @");
        let cut = cap_message(&message);
        assert!((1998..=2000).contains(&cut.len()), "{}", cut.len());
        assert!(cut.starts_with("xé") && cut.contains(arrow), "{cut}");
        assert_eq!(cap_message(&tail), tail[..1999]);
        // An arrow inside the first 2000 bytes keeps wit-parser's order.
        let early = format!("error\n{arrow}\n{tail}");
        let cut = cap_message(&early);
        assert_eq!(cut, early[..1999]);
    }

    #[test]
    fn ac1_s86_escape_message_covers_every_listed_char() {
        // Oracle written by hand, not via escape_debug/escape_unicode.
        let raw = "a\tb\rc\u{1b}d\u{7}e\u{7f}f\u{85}g\n --> /m/w.wit:1:2\n\
                   \u{61c}\u{200e}\u{200f}\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}\
                   \u{2066}\u{2067}\u{2068}\u{2069}é\\";
        let want = "a\\tb\\rc\\u{1b}d\\u{7}e\\u{7f}f\\u{85}g\n --> /m/w.wit:1:2\n\
                    \\u{61c}\\u{200e}\\u{200f}\\u{202a}\\u{202b}\\u{202c}\\u{202d}\\u{202e}\
                    \\u{2066}\\u{2067}\\u{2068}\\u{2069}é\\";
        assert_eq!(escape_message(raw), want);
    }

    #[test]
    fn ac1_s86_fix3_escape_message_covers_invisible_chars() {
        let raw = "a\u{2028}b\u{2029}c\u{200b}\u{200c}\u{200d}\u{2060}\u{feff}\u{e0000}\u{e0041}\u{e007f}\u{1b}é\\\n --> /m/w.wit:1:2";
        let want = "a\\u{2028}b\\u{2029}c\\u{200b}\\u{200c}\\u{200d}\\u{2060}\\u{feff}\\u{e0000}\\u{e0041}\\u{e007f}\\u{1b}é\\\n --> /m/w.wit:1:2";
        assert_eq!(escape_message(raw), want);
    }
}
