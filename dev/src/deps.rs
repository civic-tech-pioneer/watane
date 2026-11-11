use crate::outcome::{Code, Problem};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use watane_pkg::{Deps, Mod};

#[derive(Serialize)]
pub struct AddReport {
    pub slug: String,
    pub version: String,
    pub path: String,
    pub wit_dir: String,
    pub imports: Vec<String>,
    pub subscriptions: Vec<String>,
}

#[derive(Default, Deserialize, Serialize)]
struct Local {
    #[serde(default)]
    local_providers: BTreeMap<String, String>,
}
pub fn read_local(dir: &Path) -> Result<BTreeMap<String, String>, Problem> {
    let file = dir.join(".watane-dev.json");
    match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice::<Local>(&bytes)
            .map(|local| local.local_providers)
            .map_err(|e| Problem::internal(e).at(file.to_string_lossy(), None, None)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(Problem::internal(e).at(file.to_string_lossy(), None, None)),
    }
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
) -> Result<(), Problem> {
    if let Some(start) = stack.iter().position(|p| p == dir) {
        return Err(cycle(&stack[start..], dir));
    }
    if seen.contains(dir) {
        return Ok(());
    }
    stack.push(dir.to_path_buf());
    let deps = Deps::read(dir).map_err(|e| crate::check::mod_problems(e).remove(0))?;
    let local = read_local(dir)?;
    for (slug, publisher) in deps.iter() {
        if publisher.is_some() {
            continue;
        }
        let path = local
            .get(&slug.to_string())
            .ok_or_else(|| missing(slug.as_str()))?;
        let provider = dir
            .join(path)
            .canonicalize()
            .map_err(|_| missing(slug.as_str()))?;
        if !provider.join("mod.toml").is_file() {
            return Err(missing(slug.as_str()));
        }
        visit(&provider, stack, seen, order)?;
    }
    stack.pop();
    seen.insert(dir.to_path_buf());
    order.push(dir.to_path_buf());
    Ok(())
}
pub fn local_order(dir: &Path) -> Result<Vec<PathBuf>, Problem> {
    let dir = dir.canonicalize().map_err(Problem::internal)?;
    let mut order = Vec::new();
    visit(&dir, &mut Vec::new(), &mut BTreeSet::new(), &mut order)?;
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
    let dir = dir.canonicalize().map_err(|e| vec![Problem::internal(e)])?;
    if !dir.join("mod.toml").is_file() {
        return Err(vec![Problem::new(Code::UsageNotAMod, format!("{} is not a mod folder: it has no mod.toml", dir.display()))
            .fix("Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>")]);
    }
    let input = if provider.is_absolute() {
        provider.to_path_buf()
    } else {
        dir.join(provider)
    };
    let usage = || {
        vec![Problem::new(
            Code::UsageProvider,
            format!("{} is not a mod folder", provider.display()),
        )]
    };
    let provider = input.canonicalize().map_err(|_| usage())?;
    if provider == dir {
        return Err(vec![Problem::new(
            Code::ModSelfDependency,
            "a mod cannot depend on itself",
        )]);
    }
    if !provider.join("mod.toml").is_file() {
        return Err(usage());
    }
    // Include the consumer on the active path before traversing the proposed edge.
    visit(
        &provider,
        &mut vec![dir.clone()],
        &mut BTreeSet::new(),
        &mut Vec::new(),
    )
    .map_err(|e| vec![e])?;
    let m = Mod::read(&provider).map_err(|_| usage())?;
    let path = relative(&dir, &provider);
    let mut local = read_local(&dir).map_err(|e| vec![e])?;
    local.insert(m.slug().to_string(), path.clone());
    let mut text = serde_json::to_string_pretty(&Local {
        local_providers: local,
    })
    .map_err(|e| vec![Problem::internal(e)])?;
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
