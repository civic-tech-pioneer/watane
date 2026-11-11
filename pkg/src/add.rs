use crate::{
    Deps, Mod, ModError, PublisherKey, Rule,
    folder::{invalid, io, read_file},
};
use std::{collections::BTreeMap, fs, path::Path};

type Files = BTreeMap<String, Vec<u8>>;
fn metadata(path: &Path) -> Result<Option<fs::Metadata>, ModError> {
    match fs::symlink_metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path, e)),
    }
}
fn directory(root: &Path, relative: &str) -> Result<bool, ModError> {
    let Some(m) = metadata(&root.join(relative))? else {
        return Ok(false);
    };
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err(invalid(
            Rule::Path,
            relative,
            "expected a directory without symbolic links",
        ));
    }
    Ok(true)
}
// Only top-level .wit files participate, just as in the mod WIT walk.
fn files(root: &Path, relative: &str) -> Result<Files, ModError> {
    let mut result = Files::new();
    if !directory(root, relative)? {
        return Ok(result);
    }
    let path = root.join(relative);
    for entry in fs::read_dir(&path).map_err(|e| io(&path, e))? {
        let entry = entry.map_err(|e| io(&path, e))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let rel = format!("{relative}/{name}");
        let m = entry.file_type().map_err(|e| io(&entry.path(), e))?;
        if relative == "wit" && name == "deps" && m.is_dir() {
            continue;
        }
        if m.is_symlink() || m.is_dir() {
            return Err(invalid(Rule::Path, &rel, "expected a regular WIT entry"));
        }
        if name.ends_with(".wit") {
            let bytes = read_file(root, &rel, 1024 * 1024, Rule::Limit, false)?
                .ok_or_else(|| invalid(Rule::Wit, &rel, "missing WIT file"))?;
            result.insert(name.into_owned(), bytes);
        }
    }
    Ok(result)
}
fn write_files(root: &Path, relative: &str, files: &Files) -> Result<(), ModError> {
    let path = root.join(relative);
    fs::create_dir_all(&path).map_err(|e| io(&path, e))?;
    for (name, bytes) in files {
        let path = path.join(name);
        fs::write(&path, bytes).map_err(|e| io(&path, e))?;
    }
    Ok(())
}

/// Copy a provider's WIT and record its publisher without reading the consumer world.
pub fn add_provider(
    dir: &Path,
    provider: &Mod,
    publisher: Option<PublisherKey>,
) -> Result<(), ModError> {
    directory(dir, "wit")?;
    directory(dir, "wit/deps")?;
    directory(provider.dir(), "wit")?;
    let main = files(provider.dir(), "wit")?;
    let mut copies = BTreeMap::new();
    if directory(provider.dir(), "wit/deps")? {
        let path = provider.dir().join("wit/deps");
        for entry in fs::read_dir(&path).map_err(|e| io(&path, e))? {
            let entry = entry.map_err(|e| io(&path, e))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let relative = format!("wit/deps/{name}");
            // Non-WIT files in deps are ignored by the WIT walk.
            if !entry
                .file_type()
                .map_err(|e| io(&entry.path(), e))?
                .is_dir()
            {
                if entry
                    .file_type()
                    .map_err(|e| io(&entry.path(), e))?
                    .is_symlink()
                {
                    return Err(invalid(Rule::Path, &relative, "symbolic link"));
                }
                continue;
            }
            let source = files(provider.dir(), &relative)?;
            if metadata(&dir.join(&relative))?.is_some() {
                if files(dir, &relative)? != source {
                    return Err(invalid(
                        Rule::Provider,
                        &relative,
                        "provider dependency differs",
                    ));
                }
            } else {
                copies.insert(relative, source);
            }
        }
    }
    let relative = format!("wit/deps/{}", provider.slug());
    directory(dir, &relative)?;
    let mut deps = Deps::read(dir)?;
    deps.set(provider.slug().clone(), publisher);
    if deps.iter().count() > 32 {
        return Err(invalid(Rule::DepsJson, "deps.json", "at most 32 providers"));
    }
    // Finish all conflict and input checks before the first write.
    let target = dir.join(&relative);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|e| io(&target, e))?;
    }
    write_files(dir, &relative, &main)?;
    for (relative, files) in copies {
        write_files(dir, &relative, &files)?;
    }
    deps.write(dir)
}
