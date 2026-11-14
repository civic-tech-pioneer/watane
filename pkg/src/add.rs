use crate::{
    Deps, Mod, ModError, PublisherKey, Rule,
    folder::{invalid, io, read_file},
    open::{self, Dir, Kind},
};
use std::{collections::BTreeMap, io::ErrorKind, path::Path};

type Files = BTreeMap<String, Vec<u8>>;
const NOT_A_DIRECTORY: &str = "expected a directory without symbolic links";
// A part that is a link or not a directory is the `path` violation at that part.
fn refused(root: &Path, part: &str, e: std::io::Error) -> ModError {
    if open::is_link(&e) || e.kind() == ErrorKind::NotADirectory {
        invalid(Rule::Path, part, NOT_A_DIRECTORY)
    } else {
        io(&root.join(part), e)
    }
}
fn directory(root: &Path, relative: &str) -> Result<Option<Dir>, ModError> {
    match open::dir(root, relative) {
        Ok(dir) => Ok(Some(dir)),
        Err((_, e)) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err((part, e)) => Err(refused(root, &part, e)),
    }
}
// Open `name` below `parent` (reached by `relative`), creating it when absent.
fn below(root: &Path, parent: &Dir, relative: &str, create: bool) -> Result<Option<Dir>, ModError> {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    let opened = if create {
        open::child_or_create(parent, name)
    } else {
        open::child(parent, name)
    };
    match opened {
        Ok(dir) => Ok(Some(dir)),
        Err(e) if !create && e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(refused(root, relative, e)),
    }
}
// Only top-level .wit files participate, just as in the mod WIT walk.
fn files(root: &Path, relative: &str) -> Result<Files, ModError> {
    let mut result = Files::new();
    let Some(handle) = directory(root, relative)? else {
        return Ok(result);
    };
    let path = root.join(relative);
    for (name, kind) in open::entries(&handle).map_err(|e| io(&path, e))? {
        let name = name.to_string_lossy();
        let rel = format!("{relative}/{name}");
        if relative == "wit" && name == "deps" && kind.kind == Kind::Dir {
            continue;
        }
        if matches!(kind.kind, Kind::Link | Kind::Dir) {
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
// Write below the handle of the destination directory; nothing is addressed by a path string.
fn write_files(
    root: &Path,
    parent: &Dir,
    relative: &str,
    target: Option<Dir>,
    files: &Files,
) -> Result<(), ModError> {
    let path = root.join(relative);
    let target = match target {
        Some(dir) => {
            open::clear(&dir).map_err(|e| io(&path, e))?;
            dir
        }
        None => below(root, parent, relative, true)?.expect("created"),
    };
    for (name, bytes) in files {
        open::create_file(&target, name, bytes).map_err(|e| io(&path.join(name), e))?;
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
    let source_root = provider.dir();
    directory(source_root, "wit")?;
    let main = files(source_root, "wit")?;
    let mut copies = BTreeMap::new();
    if let Some(handle) = directory(source_root, "wit/deps")? {
        let path = source_root.join("wit/deps");
        for (name, kind) in open::entries(&handle).map_err(|e| io(&path, e))? {
            let name = name.to_string_lossy();
            let relative = format!("wit/deps/{name}");
            // Non-WIT files in deps are ignored by the WIT walk.
            if kind.kind == Kind::Link {
                return Err(invalid(Rule::Path, &relative, "symbolic link"));
            }
            if kind.kind != Kind::Dir {
                continue;
            }
            let source = files(source_root, &relative)?;
            if directory(dir, &relative)?.is_some() {
                if files(dir, &relative)? != source {
                    return Err(invalid(
                        Rule::Provider,
                        &relative,
                        "provider dependency differs",
                    ));
                }
            } else {
                copies.insert(name.into_owned(), source);
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
    // Open the destinations once; every write below is relative to these handles.
    let top = open::dir(dir, "").map_err(|(_, e)| io(dir, e))?;
    let wit = below(dir, &top, "wit", true)?.expect("created");
    let handle = below(dir, &wit, "wit/deps", true)?.expect("created");
    let target = below(dir, &handle, &relative, false)?;
    // Finish all conflict and input checks before the first write.
    #[cfg(test)]
    hook::run();
    write_files(dir, &handle, &relative, target, &main)?;
    for (name, files) in copies {
        write_files(dir, &handle, &format!("wit/deps/{name}"), None, &files)?;
    }
    deps.write(dir)
}

#[cfg(test)]
pub(crate) mod hook {
    use std::cell::RefCell;
    type Hook = Option<Box<dyn FnOnce()>>;
    thread_local!(static HOOK: RefCell<Hook> = const { RefCell::new(None) });
    pub(crate) fn set(f: impl FnOnce() + 'static) {
        HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    }
    pub(super) fn run() {
        if let Some(f) = HOOK.with(|h| h.borrow_mut().take()) {
            f();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

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
    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }
    fn setup() -> (tempfile::TempDir, tempfile::TempDir, Mod) {
        let p = tempfile::tempdir().unwrap();
        copy(&fixture("reading-list"), p.path());
        let provider = Mod::read(p.path()).unwrap();
        let c = tempfile::tempdir().unwrap();
        copy(&fixture("hello"), c.path());
        (p, c, provider)
    }
    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }
    fn not_outside(result: Result<(), ModError>) {
        match result {
            Ok(()) => {}
            Err(ModError::Invalid(v)) => assert_eq!(v[0].rule, Rule::Path, "{v:?}"),
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn ac1_s57_deps_swapped_after_preflight() {
        let (_p, c, m) = setup();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir_all(c.path().join("wit/deps")).unwrap();
        let (root, out) = (c.path().to_path_buf(), outside.path().to_path_buf());
        hook::set(move || {
            fs::rename(root.join("wit/deps"), root.join("wit/deps.moved")).unwrap();
            symlink(&out, root.join("wit/deps")).unwrap();
        });
        not_outside(add_provider(c.path(), &m, None));
        assert!(names(outside.path()).is_empty());
    }
    #[test]
    fn ac2_s57_slug_swapped_after_preflight() {
        let (_p, c, m) = setup();
        add_provider(c.path(), &m, None).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep.wit"), "keep").unwrap();
        let (root, out) = (c.path().to_path_buf(), outside.path().to_path_buf());
        hook::set(move || {
            fs::rename(
                root.join("wit/deps/reading-list"),
                root.join("wit/deps/reading-list.moved"),
            )
            .unwrap();
            symlink(&out, root.join("wit/deps/reading-list")).unwrap();
        });
        not_outside(add_provider(c.path(), &m, None));
        assert_eq!(names(outside.path()), ["keep.wit"]);
        assert_eq!(fs::read(outside.path().join("keep.wit")).unwrap(), b"keep");
    }
    #[test]
    fn deps_json_swapped_after_preflight_is_not_followed() {
        let (_p, c, m) = setup();
        let outside = tempfile::tempdir().unwrap();
        let secret = outside.path().join("secret");
        fs::write(&secret, "keep").unwrap();
        let (root, out) = (c.path().to_path_buf(), secret.clone());
        hook::set(move || symlink(&out, root.join("deps.json")).unwrap());
        not_outside(add_provider(c.path(), &m, None));
        assert_eq!(fs::read(&secret).unwrap(), b"keep");
        assert!(
            !fs::symlink_metadata(c.path().join("deps.json"))
                .unwrap()
                .is_symlink()
        );
    }
    #[test]
    fn ac3_s57_provider_dep_link() {
        let (p, c, m) = setup();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("x.wit"), "x").unwrap();
        // Linked after the provider was read, as add_provider sees it.
        fs::create_dir_all(p.path().join("wit/deps")).unwrap();
        symlink(outside.path(), p.path().join("wit/deps/E")).unwrap();
        let Err(ModError::Invalid(v)) = add_provider(c.path(), &m, None) else {
            panic!("expected invalid")
        };
        assert_eq!(v[0].rule, Rule::Path);
        assert_eq!(v[0].file, "wit/deps/E");
        assert!(!c.path().join("wit/deps/E").exists());
    }
}
