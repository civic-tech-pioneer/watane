//! The providers of a mod: `add <mod folder>`, `.watane-dev.json`, and local
//! and released providers in order (TECHNICAL 3, 9.2, 9.5).

use crate::outcome::{Code, Problem};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use watane_pkg::{Deps, Mod};

#[derive(Serialize, Debug)]
pub struct AddReport {
    pub slug: String,
    pub version: String,
    pub path: String,
    pub wit_dir: String,
    pub imports: Vec<String>,
    pub subscriptions: Vec<String>,
}

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct Local {
    #[serde(default)]
    pub local_providers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub release_providers: BTreeMap<String, crate::build_on::ReleaseProvider>,
}
pub(crate) fn read_config(dir: &Path) -> Result<Local, Problem> {
    let file = dir.join(".watane-dev.json");
    match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice::<Local>(&bytes)
            .map_err(|e| Problem::internal(e).at(file.to_string_lossy(), None, None)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Local::default()),
        Err(e) => Err(Problem::internal(e).at(file.to_string_lossy(), None, None)),
    }
}
pub fn read_local(dir: &Path) -> Result<BTreeMap<String, String>, Problem> {
    read_config(dir).map(|c| c.local_providers)
}
pub(crate) fn write_config(dir: &Path, config: &Local) -> Result<(), Problem> {
    let text = serde_json::to_string_pretty(config).map_err(Problem::internal)?;
    fs::write(dir.join(".watane-dev.json"), format!("{text}\n")).map_err(Problem::internal)
}
fn missing(slug: &str) -> Problem {
    Problem::new(
        Code::ModProviderMissing,
        format!("the local provider {slug} has no known folder"),
    )
    .fix(format!("Run: watane-dev add <folder of {slug}>"))
}
pub(crate) fn label(dir: &Path) -> String {
    Mod::read(dir)
        .map(|m| m.slug().to_string())
        .unwrap_or_else(|_| {
            dir.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
}
fn cycle(paths: &[PathBuf], dir: &Path) -> Problem {
    let names = paths
        .iter()
        .map(|p| label(p))
        .chain(std::iter::once(label(dir)))
        .collect::<Vec<_>>();
    Problem::new(
        Code::ModDependencyCycle,
        format!("{} is a cycle", names.join(" -> ")),
    )
}
fn visit(
    dir: &Path,
    stack: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    order: &mut Vec<PathBuf>,
    inherited: &BTreeMap<String, crate::build_on::ReleaseProvider>,
) -> Result<(), Problem> {
    if seen.contains(dir) {
        return Ok(());
    }
    stack.push(dir.to_path_buf());
    let deps = Deps::read(dir).map_err(|e| crate::check::mod_problems(e).remove(0))?;
    let config = read_config(dir)?;
    let mut records = inherited.clone();
    records.extend(config.release_providers);
    for (slug, publisher) in deps.iter() {
        let provider = if let Some(publisher) = publisher {
            let release = crate::build_on::provider_release(dir, slug, publisher)?;
            let missing = || {
                Problem::new(
                    Code::ModReleaseProviderMissing,
                    format!("the provider {release} is not in the cache of watane-dev"),
                )
                .fix(format!(
                    "Start the watane app, then run: watane-dev add --release {release}"
                ))
            };
            let record = records
                .get(slug.as_str())
                .filter(|r| r.release == release)
                .ok_or_else(missing)?;
            let folder = crate::env::Env::from_process()?
                .cache
                .join("providers")
                .join(record.package.to_string());
            let m = Mod::read(&folder).map_err(|_| missing())?;
            if m.slug() != slug || *m.version() != release.version {
                return Err(missing());
            }
            folder
        } else {
            let path = config
                .local_providers
                .get(&slug.to_string())
                .ok_or_else(|| missing(slug.as_str()))?;
            let provider = dir
                .join(path)
                .canonicalize()
                .map_err(|_| missing(slug.as_str()))?;
            if !provider.join("mod.toml").is_file() {
                return Err(missing(slug.as_str()));
            }
            provider
        };
        if let Some(start) = stack.iter().position(|p| p == &provider) {
            let folder = if dir.join("wit/deps").join(slug.as_str()).exists() {
                format!(" and wit/deps/{slug}/")
            } else {
                String::new()
            };
            return Err(cycle(&stack[start..], &provider).fix(format!(
                "Remove mods:{slug} from deps.json{folder} in {}",
                dir.display()
            )));
        }
        visit(&provider, stack, seen, order, &records)?;
    }
    stack.pop();
    seen.insert(dir.to_path_buf());
    order.push(dir.to_path_buf());
    Ok(())
}
pub fn local_order(dir: &Path) -> Result<Vec<PathBuf>, Problem> {
    let dir = dir.canonicalize().map_err(Problem::internal)?;
    let mut order = Vec::new();
    visit(
        &dir,
        &mut Vec::new(),
        &mut BTreeSet::new(),
        &mut order,
        &BTreeMap::new(),
    )?;
    Ok(order)
}
fn relative(dir: &Path, provider: &Path) -> String {
    let base = dir.components().collect::<Vec<_>>();
    let target = provider.components().collect::<Vec<_>>();
    let shared = base.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let mut path = PathBuf::new();
    for _ in shared..base.len() {
        path.push("..");
    }
    for part in &target[shared..] {
        path.push(part);
    }
    path.to_string_lossy().into_owned()
}
pub fn add_local(dir: &Path, provider: &Path) -> Result<AddReport, Vec<Problem>> {
    if !dir.join("mod.toml").is_file() {
        return Err(vec![Problem::new(Code::UsageNotAMod, format!("{} is not a mod folder: it has no mod.toml", dir.display()))
            .fix("Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>")]);
    }
    let dir = dir.canonicalize().map_err(|e| vec![Problem::internal(e)])?;
    let given = provider;
    let usage = |found: Option<&Path>| {
        let mut message = format!("{} is not a mod folder", given.display());
        if let Some(found) = found.filter(|found| *found != given) {
            message = format!("{message} ({} has no mod.toml)", found.display());
        }
        let e = Problem::new(Code::UsageProvider, message);
        vec![if given.is_relative() {
            e.fix("A relative provider folder is read from the current folder, not from --dir. Run add in the provider's folder or pass its absolute path")
        } else {
            e
        }]
    };
    let provider = provider.canonicalize().map_err(|_| usage(None))?;
    if provider == dir {
        return Err(vec![
            Problem::new(Code::ModSelfDependency, "a mod cannot depend on itself")
                .fix("Pass the folder of another mod to add"),
        ]);
    }
    if !provider.join("mod.toml").is_file() {
        return Err(usage(Some(&provider)));
    }
    // Include the consumer on the active path before traversing the proposed edge.
    visit(
        &provider,
        &mut vec![dir.clone()],
        &mut BTreeSet::new(),
        &mut Vec::new(),
        &BTreeMap::new(),
    )
    .map_err(|e| {
        let e = if e.code == Code::ModDependencyCycle {
            e.fix(format!(
                "Do not add {}: it already depends on this mod",
                label(&provider)
            ))
        } else {
            e
        };
        vec![e]
    })?;
    let m = Mod::read(&provider).map_err(|_| usage(None))?;
    let path = relative(&dir, &provider);
    let mut local = read_local(&dir).map_err(|e| vec![e])?;
    local.insert(m.slug().to_string(), path.clone());
    let mut config = read_config(&dir).map_err(|e| vec![e])?;
    config.local_providers = local;
    config.release_providers.remove(m.slug().as_str());
    let mut text = serde_json::to_string_pretty(&config).map_err(|e| vec![Problem::internal(e)])?;
    text.push('\n');
    watane_pkg::add_provider(&dir, &m, None).map_err(crate::check::mod_problems)?;
    fs::write(dir.join(".watane-dev.json"), text).map_err(|e| vec![Problem::internal(e)])?;
    Ok(AddReport {
        slug: m.slug().to_string(),
        version: m.version().to_string(),
        path,
        wit_dir: format!("wit/deps/{}", m.slug()),
        imports: m
            .world()
            .exports
            .iter()
            .map(|i| format!("import {i};"))
            .collect(),
        subscriptions: m
            .world()
            .events
            .iter()
            .map(|i| format!("export {i};"))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac1_self_dependency_fix() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        crate::template::new_mod(&a, "a").unwrap();
        let errors = add_local(&a, &a).err().unwrap();
        assert_eq!(errors[0].code, Code::ModSelfDependency);
        assert_eq!(
            errors[0].fix.as_deref(),
            Some("Pass the folder of another mod to add")
        );
    }

    #[test]
    fn regress_s23_add_missing_dir_is_not_a_mod() {
        // `add . --dir ../typo` from the provider folder: a missing --dir has no mod.toml,
        // as with check; it must not be an internal error without the path.
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a");
        crate::template::new_mod(&a, "a").unwrap();
        let errors = add_local(&t.path().join("typo"), &a).err().unwrap();
        assert_eq!(errors[0].code, Code::UsageNotAMod);
        assert!(errors[0].message.contains("typo"), "{}", errors[0].message);
    }

    #[test]
    fn ac2_cycle_fixes_name_the_edge() {
        let t = tempfile::tempdir().unwrap();
        for slug in ["a", "b", "c"] {
            crate::template::new_mod(&t.path().join(slug), slug).unwrap();
        }
        let a = t.path().join("a");
        let b = t.path().join("b");
        let c = t.path().join("c");
        for (dir, slug, path) in [(&a, "b", "../b"), (&b, "c", "../c")] {
            fs::write(dir.join("deps.json"), format!(r#"{{"mods:{slug}":null}}"#)).unwrap();
            fs::write(
                dir.join(".watane-dev.json"),
                format!(r#"{{"local_providers":{{"{slug}":"{path}"}}}}"#),
            )
            .unwrap();
        }
        let errors = add_local(&c, &a).err().unwrap();
        assert_eq!(errors[0].code, Code::ModDependencyCycle);
        let add_fix = errors[0].fix.clone();
        fs::write(c.join("deps.json"), r#"{"mods:a":null}"#).unwrap();
        fs::write(
            c.join(".watane-dev.json"),
            r#"{"local_providers":{"a":"../a"}}"#,
        )
        .unwrap();
        let error = local_order(&a).unwrap_err();
        assert_eq!(error.code, Code::ModDependencyCycle);
        assert_eq!(
            error.fix.as_deref(),
            Some(
                format!(
                    "Remove mods:a from deps.json in {}",
                    c.canonicalize().unwrap().display()
                )
                .as_str()
            )
        );
        fs::create_dir_all(c.join("wit/deps/a")).unwrap();
        assert_eq!(
            local_order(&a).unwrap_err().fix.as_deref(),
            Some(
                format!(
                    "Remove mods:a from deps.json and wit/deps/a/ in {}",
                    c.canonicalize().unwrap().display()
                )
                .as_str()
            )
        );
        assert_eq!(
            add_fix.as_deref(),
            Some("Do not add a: it already depends on this mod")
        );
    }

    #[test]
    fn ac3_missing_provider_text_and_fix() {
        let error = missing("a");
        assert_eq!(error.code, Code::ModProviderMissing);
        assert_eq!(error.message, "the local provider a has no known folder");
        assert_eq!(
            error.fix.as_deref(),
            Some("Run: watane-dev add <folder of a>")
        );
        assert_eq!(error.file, None);
        assert_eq!(error.line, None);
    }
    #[test]
    fn ac3_local_order_and_missing_folders() {
        let t = tempfile::tempdir().unwrap();
        for slug in ["a", "b", "c"] {
            crate::template::new_mod(&t.path().join(slug), slug).unwrap();
        }
        let a = t.path().join("a");
        let b = t.path().join("b");
        let c = t.path().join("c");
        add_local(&b, &a).unwrap();
        let world = fs::read_to_string(b.join("wit/world.wit")).unwrap();
        fs::write(
            b.join("wit/world.wit"),
            world.replace("world b {", "world b {\n    import mods:a/api@0.1.0;"),
        )
        .unwrap();
        add_local(&c, &b).unwrap();
        assert_eq!(
            local_order(&c).unwrap(),
            vec![
                a.canonicalize().unwrap(),
                b.canonicalize().unwrap(),
                c.canonicalize().unwrap()
            ]
        );
        for missing_entry in [true, false] {
            if missing_entry {
                fs::remove_file(b.join(".watane-dev.json")).unwrap();
            } else {
                fs::write(
                    b.join(".watane-dev.json"),
                    r#"{"local_providers":{"a":"../gone"}}"#,
                )
                .unwrap();
            }
            let e = local_order(&c).unwrap_err();
            assert_eq!(e.code.as_str(), "mod.provider_missing");
            assert_eq!(e.message, "the local provider a has no known folder");
            assert_eq!(e.fix.as_deref(), Some("Run: watane-dev add <folder of a>"));
        }
    }
}
