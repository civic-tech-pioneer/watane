use crate::outcome::{Code, Problem};
use serde::Serialize;
use std::{fs, path::Path};
use watane_pkg::{Mod, ModError};
fn symlinks(root: &Path) -> Result<Vec<Problem>, Problem> {
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
            } else if kind.is_dir()
                && !matches!(path.strip_prefix(root), Ok(p) if p == Path::new(".git") || p == Path::new("logic/target"))
            {
                stack.push(path);
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
pub fn check(dir: &Path) -> Result<(Mod, CheckReport), Vec<Problem>> {
    if fs::symlink_metadata(dir.join("mod.toml"))
        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
    {
        return Err(vec![Problem::new(Code::UsageNotAMod,format!("{} is not a mod folder: it has no mod.toml",dir.display())).fix("Run the command in a mod folder, or pass --dir <folder>. Create a mod with: watane-dev new <slug>")]);
    }
    let errors = symlinks(dir).map_err(|e| vec![e])?;
    if !errors.is_empty() {
        return Err(errors);
    }
    let m = Mod::read(dir).map_err(|e| match e {
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
    })?;
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
