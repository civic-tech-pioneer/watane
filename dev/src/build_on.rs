//! Build on published packages without executing their sources or trusting their paths.
use crate::api::Client;
use crate::env::Env;
use crate::outcome::{Code, Problem};
use crate::{check, deps, template};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use watane_pkg::{Deps, Mod, Package, PackageId, PublisherKey, ReleaseRef, Slug};

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct ReleaseProvider {
    pub release: ReleaseRef,
    pub package: PackageId,
    pub link: Option<String>,
}

/// Read a dependency's version from its copied WIT package declaration.
pub fn provider_release(
    dir: &Path,
    slug: &Slug,
    publisher: &PublisherKey,
) -> Result<ReleaseRef, Problem> {
    let wit = dir.join("wit/deps").join(slug.as_str());
    for file in files(&wit)? {
        if file.extension().is_some_and(|e| e == "wit") {
            let text = fs::read_to_string(wit.join(&file)).map_err(Problem::internal)?;
            if let Ok((_, found, version)) = declaration(&text)
                && found == *slug
            {
                return Ok(ReleaseRef {
                    publisher: publisher.clone(),
                    slug: slug.clone(),
                    version,
                });
            }
        }
    }
    Err(Problem::new(
        Code::ModInvalid,
        format!("wit/deps/{slug} has no package declaration"),
    )
    .fix(format!(
        "Run: watane-dev add --release {publisher}/{slug}@<version>"
    )))
}

// Only the package declaration is edited; strings and comments elsewhere stay byte-identical.
pub(crate) fn declaration(
    text: &str,
) -> Result<(std::ops::Range<usize>, Slug, watane_pkg::Version), Problem> {
    let mut clean = text.as_bytes().to_vec();
    let (mut i, mut depth) = (0, 0usize);
    while i < clean.len() {
        if depth > 0 {
            if text.as_bytes().get(i..i + 2) == Some(b"/*") {
                clean[i..i + 2].fill(b' ');
                depth += 1;
                i += 2;
            } else if text.as_bytes().get(i..i + 2) == Some(b"*/") {
                clean[i..i + 2].fill(b' ');
                depth -= 1;
                i += 2;
            } else {
                clean[i] = b' ';
                i += 1;
            }
        } else if clean.get(i..i + 2) == Some(b"/*") {
            clean[i..i + 2].fill(b' ');
            depth = 1;
            i += 2;
        } else if clean.get(i..i + 2) == Some(b"//") {
            while i < clean.len() && clean[i] != b'\n' {
                clean[i] = b' ';
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    let clean = String::from_utf8(clean).map_err(Problem::internal)?;
    let bad = || {
        Problem::new(Code::ModInvalid, "missing WIT package declaration").at(
            "wit/world.wit",
            None,
            None,
        )
    };
    let start = clean.find("package").ok_or_else(bad)? + "package".len();
    let end = start + clean[start..].find(';').ok_or_else(bad)?;
    let name = clean[start..end].trim();
    let compact: String = name.chars().filter(|c| !c.is_whitespace()).collect();
    let (slug, version) = compact
        .strip_prefix("mods:")
        .and_then(|s| s.split_once('@'))
        .ok_or_else(bad)?;
    let begin = start + clean[start..end].find(name).ok_or_else(bad)?;
    Ok((
        begin..begin + name.len(),
        slug.parse().map_err(Problem::internal)?,
        version.parse().map_err(Problem::internal)?,
    ))
}

fn files(dir: &Path) -> Result<Vec<PathBuf>, Problem> {
    fn walk(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in fs::read_dir(root.join(relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            let ty = entry.file_type()?;
            if path == Path::new(".git") {
                continue;
            }
            if ty.is_dir() {
                walk(root, &path, out)?;
            } else if ty.is_file() {
                out.push(path);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, Path::new(""), &mut out).map_err(Problem::internal)?;
    out.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    Ok(out)
}

/// Store only validated bytes, naming both cache paths by their computed id.
pub fn store(package: &Package, env: &Env) -> Result<PathBuf, Problem> {
    let id = package.id();
    let packages = env.cache.join("packages");
    let providers = env.cache.join("providers");
    fs::create_dir_all(&packages).map_err(Problem::internal)?;
    fs::create_dir_all(&providers).map_err(Problem::internal)?;
    let folder = providers.join(id.to_string());
    let provider_temp = providers.join(format!("{id}.{}.tmp", std::process::id()));
    if provider_temp.exists() {
        fs::remove_dir_all(&provider_temp).map_err(Problem::internal)?;
    }
    if !folder.is_dir() {
        package.unpack(&provider_temp).map_err(Problem::internal)?;
        if let Err(error) = fs::rename(&provider_temp, &folder) {
            fs::remove_dir_all(&provider_temp).map_err(Problem::internal)?;
            if !folder.is_dir() {
                return Err(Problem::internal(error));
            }
        }
    }
    let temp = packages.join(format!("{id}.{}.tmp", std::process::id()));
    fs::write(&temp, package.as_bytes()).map_err(Problem::internal)?;
    fs::rename(&temp, packages.join(id.to_string())).map_err(Problem::internal)?;
    Ok(folder)
}

/// Collect all released dependencies, in fetch order, recording transitive packages too.
pub fn fetch_providers(
    dir: &Path,
    client: &Client,
    env: &Env,
) -> Result<Vec<ReleaseRef>, Vec<Problem>> {
    fn visit(
        dir: &Path,
        client: &Client,
        env: &Env,
        records: &mut BTreeMap<String, ReleaseProvider>,
        seen: &mut BTreeSet<ReleaseRef>,
        order: &mut Vec<ReleaseRef>,
    ) -> Result<(), Vec<Problem>> {
        let providers = Deps::read(dir).map_err(check::mod_problems)?;
        for (slug, publisher) in providers.iter() {
            let Some(publisher) = publisher else {
                continue;
            };
            let release = provider_release(dir, slug, publisher).map_err(|e| vec![e])?;
            if !seen.insert(release.clone()) {
                continue;
            }
            if seen.len() > 32 {
                return Err(vec![Problem::new(
                    Code::HostRefused,
                    "too many released providers",
                )]);
            }
            let existing = records
                .get(slug.as_str())
                .filter(|r| {
                    r.release == release
                        && env
                            .cache
                            .join("providers")
                            .join(r.package.to_string())
                            .is_dir()
                })
                .cloned();
            let record = if let Some(record) = existing {
                record
            } else {
                let (fetched, package) = client.fetch(&release.to_string()).map_err(|e| vec![e])?;
                store(&package, env).map_err(|e| vec![e])?;
                ReleaseProvider {
                    release: fetched,
                    package: package.id(),
                    link: None,
                }
            };
            let folder = env.cache.join("providers").join(record.package.to_string());
            records.insert(slug.to_string(), record);
            order.push(release);
            visit(&folder, client, env, records, seen, order)?;
        }
        Ok(())
    }
    let mut config = deps::read_config(dir).map_err(|e| vec![e])?;
    let mut order = Vec::new();
    visit(
        dir,
        client,
        env,
        &mut config.release_providers,
        &mut BTreeSet::new(),
        &mut order,
    )?;
    if !config.release_providers.is_empty() || dir.join(".watane-dev.json").exists() {
        deps::write_config(dir, &config).map_err(|e| vec![e])?;
    }
    Ok(order)
}

pub fn add_release(
    dir: &Path,
    target: &str,
    client: &Client,
    env: &Env,
    home: &str,
) -> Result<Value, Vec<Problem>> {
    if !dir.join("mod.toml").is_file() {
        return Err(vec![Problem::new(Code::UsageNotAMod, format!("{} is not a mod folder: it has no mod.toml",dir.display()))
            .fix("Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>")]);
    }
    let (release, package) = client.fetch(target).map_err(|e| vec![e])?;
    let folder = store(&package, env).map_err(|e| vec![e])?;
    let provider = Mod::read(&folder).map_err(check::mod_problems)?;
    // Read the consumer's package declaration even while an earlier add awaits an import.
    let text =
        fs::read_to_string(dir.join("wit/world.wit")).map_err(|e| vec![Problem::internal(e)])?;
    let (_, slug, _) = declaration(&text).map_err(|e| vec![e])?;
    if slug == *provider.slug() {
        return Err(vec![
            Problem::new(Code::ModSelfDependency, "a mod cannot depend on itself")
                .fix(format!(
                    "Pick the link or release of another mod, then run: watane-dev add --release <link> --dir {}{home}",
                    crate::cli::quote(dir)
                )),
        ]);
    }
    watane_pkg::add_provider(dir, &provider, Some(release.publisher.clone()))
        .map_err(check::mod_problems)?;
    let link = target.starts_with("watane:").then(|| target.to_string());
    let mut config = deps::read_config(dir).map_err(|e| vec![e])?;
    config.local_providers.remove(provider.slug().as_str());
    config.release_providers.insert(
        provider.slug().to_string(),
        ReleaseProvider {
            release: release.clone(),
            package: package.id(),
            link: link.clone(),
        },
    );
    deps::write_config(dir, &config).map_err(|e| vec![e])?;
    let providers = fetch_providers(dir, client, env)?;
    Ok(
        json!({"slug":provider.slug(),"version":provider.version(),"path":null,"wit_dir":format!("wit/deps/{}",provider.slug()),"publisher":release.publisher,"release":release,"link":link,"providers":providers,
        "imports":provider.world().exports.iter().map(|i|format!("import {i};")).collect::<Vec<_>>(),
        "subscriptions":provider.world().events.iter().map(|i|format!("export {i};")).collect::<Vec<_>>() }),
    )
}

pub fn new_from(
    destination: Option<&Path>,
    slug: Option<&str>,
    target: &str,
    client: &Client,
    env: &Env,
    home: &str,
) -> Result<(Value, Vec<Problem>), Vec<Problem>> {
    let (release, package) = client.fetch(target).map_err(|e| vec![e])?;
    if !package.has_source() {
        // The mod that would depend on it is the one this run would have created.
        let mod_dir = destination.map(Path::to_path_buf).or_else(|| {
            let slug = slug.unwrap_or(release.slug.as_str());
            std::env::current_dir().ok().map(|cwd| cwd.join(slug))
        });
        let mod_dir = mod_dir.map_or_else(String::new, |dir| {
            format!(" --dir {}", crate::cli::quote(&dir))
        });
        return Err(vec![Problem::new(Code::UsageNoSource, format!("the release {release} holds no source (no logic/Cargo.toml, logic/src/ or tests/)"))
        .fix(format!("Build on a release that includes its source, or depend on this one with: watane-dev add --release {}{mod_dir}{home}", crate::cli::quote(Path::new(target))))]);
    }
    let slug: Slug = slug
        .unwrap_or(release.slug.as_str())
        .parse()
        .map_err(|e| vec![Problem::internal(e)])?;
    let dir = destination.map(Path::to_path_buf).unwrap_or(
        std::env::current_dir()
            .map_err(|e| vec![Problem::internal(e)])?
            .join(slug.as_str()),
    );
    let dir = dir.as_path();
    let setup = || -> Result<(), Problem> {
        template::write_guides(dir, &slug)?;
        if dir.join("logic/Cargo.toml").is_file() {
            match fs::remove_file(dir.join("logic.wasm")) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Problem::internal(e)),
            }
        }
        let mut meta: toml::Table =
            toml::from_str(&fs::read_to_string(dir.join("mod.toml")).map_err(Problem::internal)?)
                .map_err(Problem::internal)?;
        meta.remove("repository");
        meta.insert(
            "name".into(),
            toml::Value::String(template::render("{{title}}", &slug)),
        );
        meta.insert(
            "origin".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "release".into(),
                toml::Value::String(release.to_string()),
            )])),
        );
        fs::write(
            dir.join("mod.toml"),
            toml::to_string(&meta).map_err(Problem::internal)?,
        )
        .map_err(Problem::internal)?;
        if slug != release.slug {
            let path = dir.join("wit/world.wit");
            let mut text = fs::read_to_string(&path).map_err(Problem::internal)?;
            let (range, _, _) = declaration(&text)?;
            text.replace_range(range, &format!("mods:{slug}@{}", release.version));
            fs::write(path, text).map_err(Problem::internal)?;
        }
        Ok(())
    };
    // The caller has already checked existence; never delete a pre-existing target.
    if dir.symlink_metadata().is_ok() {
        return Err(vec![exists(dir)]);
    }
    let cached = store(&package, env).map_err(|e| vec![e])?;
    let source = Mod::read(&cached).map_err(check::mod_problems)?;
    let mut warnings = Vec::new();
    let mut fork = Value::Null;
    if let Some(repository) = source.repository() {
        match crate::share::fork(dir, repository, &release, &package) {
            Ok(()) => fork = json!({"repository":repository,"tag":release}),
            Err(warning) => warnings.push(warning),
        }
    }
    if fork.is_null() {
        package.unpack(dir).map_err(|e| {
            vec![match e {
                watane_pkg::PackageError::DestExists { .. } => exists(dir),
                e => Problem::internal(e),
            }]
        })?;
    }
    if let Err(e) = setup() {
        let _ = fs::remove_dir_all(dir);
        return Err(vec![e]);
    }
    let providers = fetch_providers(dir, client, env)?;
    Mod::read(dir).map_err(check::mod_problems)?;
    let paths = files(dir).map_err(|e| vec![e])?;
    let old = release.slug.as_str();
    let snake = old.replace('-', "_");
    let boundary = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-';
    let mut mentions = Vec::new();
    if slug != release.slug {
        for path in &paths {
            let name = path.to_string_lossy();
            if !(name.starts_with("wit/")
                && !name.starts_with("wit/deps/")
                && name.ends_with(".wit")
                || ["logic/", "ui/", "tests/"]
                    .iter()
                    .any(|p| name.starts_with(p)))
            {
                continue;
            }
            let Ok(text) = fs::read_to_string(dir.join(path)) else {
                continue;
            };
            for (line, text) in text.lines().enumerate() {
                if [old, &snake].iter().any(|word| {
                    text.match_indices(word).any(|(i, word)| {
                        (i == 0 || !boundary(text.as_bytes()[i - 1]))
                            && (i + word.len() == text.len()
                                || !boundary(text.as_bytes()[i + word.len()]))
                    })
                }) && mentions.len() < 200
                {
                    mentions.push(json!({"file":name,"line":line+1}));
                }
            }
        }
    }
    Ok((
        json!({"slug":slug,"dir":dir,"files":files(dir).map_err(|e|vec![e])?,"origin":release,"from_slug":old,"providers":providers,"mentions":mentions,"fork":fork}),
        warnings,
    ))
}

pub(crate) fn exists(dir: &Path) -> Problem {
    Problem::new(Code::UsageExists, format!("{} exists", dir.display()))
        .fix("Choose another slug, or pass --dir <new folder>")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac1_store_recovers_partial_temps_and_keeps_existing_provider() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir_all(source.join("wit")).unwrap();
        fs::create_dir_all(source.join("ui")).unwrap();
        fs::write(
            source.join("mod.toml"),
            "name = \"Demo\"\nsummary = \"page\"",
        )
        .unwrap();
        fs::write(
            source.join("wit/world.wit"),
            "package mods:demo@0.1.0; world demo {}",
        )
        .unwrap();
        fs::write(source.join("ui/index.html"), "complete").unwrap();
        let package = watane_pkg::pack(&source, false).unwrap();
        let env = Env {
            cache: root.path().join("cache"),
            unit_seconds: 60,
        };
        let providers = env.cache.join("providers");
        let temp = providers.join(format!("{}.{}.tmp", package.id(), std::process::id()));
        let killed = providers.join(format!("{}.{}.tmp", package.id(), std::process::id() + 1));
        for folder in [&temp, &killed] {
            fs::create_dir_all(folder).unwrap();
            fs::write(folder.join("partial"), "partial").unwrap();
        }
        let folder = store(&package, &env).unwrap();
        assert_eq!(
            fs::read_to_string(folder.join("ui/index.html")).unwrap(),
            "complete"
        );
        assert!(!folder.join("partial").exists());
        assert!(!temp.exists());
        assert!(killed.exists());
        fs::write(folder.join("keep"), "existing").unwrap();
        fs::create_dir_all(&temp).unwrap();
        fs::write(temp.join("partial"), "partial").unwrap();
        assert_eq!(store(&package, &env).unwrap(), folder);
        assert_eq!(fs::read_to_string(folder.join("keep")).unwrap(), "existing");
        assert!(!temp.exists());
    }

    #[test]
    fn ac1_files_are_sorted_by_relative_path_text() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("logic/src/a")).unwrap();
        fs::write(root.path().join("logic/src/a-b.rs"), "").unwrap();
        fs::write(root.path().join("logic/src/a/inner.rs"), "").unwrap();
        let names: Vec<_> = files(root.path())
            .unwrap()
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["logic/src/a-b.rs", "logic/src/a/inner.rs"]);
    }

    #[test]
    fn ac1_package_declaration_accepts_comments_and_whitespace() {
        let text = "// package mods:decoy@9.9.9;\npackage /* parent */ mods : reading-list @ 0.1.0;\nworld reading-list {}";
        let (_, slug, version) = declaration(text).unwrap();
        assert_eq!(slug.as_str(), "reading-list");
        assert_eq!(version.to_string(), "0.1.0");
    }
}
