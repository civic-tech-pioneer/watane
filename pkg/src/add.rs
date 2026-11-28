//! `add_provider` (TECHNICAL 4.8): adds a provider's WIT and `deps.json` entry
//! to a mod folder without following any link that appears during the write.

use crate::folder::{invalid, io};
use crate::open::{self, Dir, Kind};
use crate::refusal::{self, CHANGED, DIRECTORY, SPECIAL};
use crate::{Deps, Mod, ModError, PublisherKey, Rule};
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read};
use std::path::Path;

type Files = BTreeMap<String, Vec<u8>>;
const NOT_A_DIRECTORY: &str = "expected a directory without symbolic links";
// A part that is a link or not a directory is the `path` violation at that part.
fn refused(root: &Path, part: &str, e: std::io::Error) -> ModError {
    if open::is_link(&e) || e.kind() == ErrorKind::NotADirectory {
        invalid(Rule::Path, part, NOT_A_DIRECTORY)
    } else {
        failed(root, part, e)
    }
}
// An entry created, removed or replaced under the write is the CHANGED cause; other errors stay I/O.
fn failed(root: &Path, relative: &str, e: std::io::Error) -> ModError {
    if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::AlreadyExists) {
        invalid(Rule::Path, relative, CHANGED)
    } else {
        io(&root.join(relative), e)
    }
}
// A vanished entry is named at the entry; any other listing error names the folder.
fn listing(root: &Path, relative: &str, e: open::ListingError) -> ModError {
    if e.source.kind() == ErrorKind::NotFound {
        invalid(Rule::Path, &e.relative_path(relative), CHANGED)
    } else {
        failed(root, relative, e.source)
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
    match directory(root, relative)? {
        Some(handle) => files_in(root, relative, &handle),
        None => Ok(Files::new()),
    }
}
// A listed dependency folder opened again: gone or no longer a directory is CHANGED at the entry.
fn listed_files(root: &Path, relative: &str) -> Result<Files, ModError> {
    match open::dir(root, relative) {
        Ok(handle) => files_in(root, relative, &handle),
        Err((part, e))
            if part == relative
                && !open::is_link(&e)
                && matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) =>
        {
            Err(invalid(Rule::Path, relative, CHANGED))
        }
        Err((part, e)) => Err(refused(root, &part, e)),
    }
}
fn files_in(root: &Path, relative: &str, handle: &Dir) -> Result<Files, ModError> {
    let mut result = Files::new();
    for open::Entry { name, stat, .. } in
        open::entries(handle).map_err(|e| listing(root, relative, e))?
    {
        let name = name.to_string_lossy();
        let rel = format!("{relative}/{name}");
        if relative == "wit" && name == "deps" && stat.kind == Kind::Dir {
            continue;
        }
        if matches!(stat.kind, Kind::Link | Kind::Dir) {
            return Err(invalid(Rule::Path, &rel, "expected a regular WIT entry"));
        }
        if name.ends_with(".wit") {
            result.insert(name.into_owned(), wit(root, &rel)?);
        }
    }
    Ok(result)
}
// A listed `.wit` read again: what it became since the listing is named by its `path` cause.
fn wit(root: &Path, rel: &str) -> Result<Vec<u8>, ModError> {
    const LIMIT: u64 = 1024 * 1024;
    let file = open::file(root, rel).map_err(|(part, e)| {
        let cause = if open::is_link(&e) {
            refusal::link(&part)
        } else if open::is_socket(&e) || e.kind() == ErrorKind::InvalidInput {
            SPECIAL
        } else if e.kind() == ErrorKind::IsADirectory {
            DIRECTORY
        } else if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) {
            CHANGED
        } else {
            return io(&root.join(&part), e);
        };
        invalid(Rule::Path, &part, cause)
    })?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io(&root.join(rel), e))?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid(
            Rule::Limit,
            rel,
            format!("file exceeds {LIMIT} bytes"),
        ));
    }
    Ok(bytes)
}
// Write below the handle of the destination directory; nothing is addressed by a path string.
fn write_files(
    root: &Path,
    parent: &Dir,
    relative: &str,
    target: Option<Dir>,
    files: &Files,
) -> Result<(), ModError> {
    let target = match target {
        Some(dir) => {
            open::clear(&dir).map_err(|e| failed(root, relative, e))?;
            dir
        }
        None => below(root, parent, relative, true)?.expect("created"),
    };
    for (name, bytes) in files {
        open::create_file(&target, name, bytes)
            .map_err(|e| failed(root, &format!("{relative}/{name}"), e))?;
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
        for open::Entry { name, stat, .. } in
            open::entries(&handle).map_err(|e| listing(source_root, "wit/deps", e))?
        {
            let name = name.to_string_lossy();
            let relative = format!("wit/deps/{name}");
            // Non-WIT files in deps are ignored by the WIT walk.
            if stat.kind == Kind::Link {
                return Err(invalid(Rule::Path, &relative, "symbolic link"));
            }
            if stat.kind != Kind::Dir {
                continue;
            }
            #[cfg(all(test, unix))]
            hook::run_listed();
            let source = listed_files(source_root, &relative)?;
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
    #[cfg(all(test, unix))]
    hook::run();
    write_files(dir, &handle, &relative, target, &main)?;
    for (name, files) in copies {
        write_files(dir, &handle, &format!("wit/deps/{name}"), None, &files)?;
    }
    deps.write(dir)
}

#[cfg(all(test, unix))]
pub(crate) mod hook {
    use std::cell::RefCell;
    type Hook = Option<Box<dyn FnOnce()>>;
    thread_local!(static HOOK: RefCell<Hook> = const { RefCell::new(None) });
    thread_local!(static LISTED: RefCell<Hook> = const { RefCell::new(None) });
    // Runs once a provider dependency folder is listed, before it is opened.
    pub(crate) fn set_listed(f: impl FnOnce() + 'static) {
        LISTED.with(|h| *h.borrow_mut() = Some(Box::new(f)));
    }
    pub(super) fn run_listed() {
        if let Some(f) = LISTED.with(|h| h.borrow_mut().take()) {
            f();
        }
    }
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
#[allow(clippy::unwrap_used, reason = "tests may panic")]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

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
    fn listing_error(name: Option<&str>, kind: ErrorKind) -> open::ListingError {
        open::ListingError {
            name: name.map(Into::into),
            source: kind.into(),
        }
    }
    #[test]
    fn ac1_vanished_entry_is_changed_at_the_entry() {
        let e = listing(
            Path::new("/m"),
            "wit/deps",
            listing_error(Some("x"), ErrorKind::NotFound),
        );
        assert!(
            matches!(&e, ModError::Invalid(v) if v.len() == 1 && v[0].rule == Rule::Path
                && v[0].file == "wit/deps/x" && v[0].message == "changed while the mod was read; read it again"),
            "{e:?}"
        );
    }
    #[test]
    fn ac2_unnamed_listing_error_names_the_folder() {
        let e = listing(
            Path::new("/m"),
            "wit/deps",
            listing_error(None, ErrorKind::NotFound),
        );
        assert!(
            matches!(&e, ModError::Invalid(v) if v[0].file == "wit/deps" && v[0].message == "changed while the mod was read; read it again"),
            "{e:?}"
        );
        let e = listing(
            Path::new("/m"),
            "wit/deps",
            listing_error(Some("x"), ErrorKind::PermissionDenied),
        );
        assert!(
            matches!(&e, ModError::Io { path, .. } if path == Path::new("/m/wit/deps")),
            "{e:?}"
        );
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
    fn changed(result: Result<(), ModError>, file: &str) {
        let Err(ModError::Invalid(v)) = result else {
            panic!("expected invalid, got {result:?}")
        };
        assert_eq!((v[0].rule, v[0].file.as_str()), (Rule::Path, file), "{v:?}");
        assert_eq!(v[0].message, CHANGED);
    }
    // The listing saw a regular `.wit`; the read finds what it became since.
    fn reread(root: &Path, rel: &str) -> (Rule, String, String) {
        let Err(ModError::Invalid(v)) = wit(root, rel) else {
            panic!("expected invalid")
        };
        (v[0].rule, v[0].file.clone(), v[0].message.clone())
    }
    #[test]
    fn ac2_s91_fix1_listed_wit_swapped_for_directory() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("wit/world.wit")).unwrap();
        let expected = (Rule::Path, "wit/world.wit".into(), DIRECTORY.into());
        assert_eq!(reread(t.path(), "wit/world.wit"), expected);
    }
    #[test]
    fn ac2_s91_fix1_listed_wit_other_swaps_keep_their_cause() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir(t.path().join("wit")).unwrap();
        let made = std::process::Command::new("mkfifo")
            .arg(t.path().join("wit/f.wit"))
            .status()
            .unwrap();
        assert!(made.success());
        let special = (Rule::Path, "wit/f.wit".into(), SPECIAL.into());
        assert_eq!(reread(t.path(), "wit/f.wit"), special);
        let gone = (Rule::Path, "wit/g.wit".into(), CHANGED.into());
        assert_eq!(reread(t.path(), "wit/g.wit"), gone);
        fs::write(t.path().join("deps"), "x").unwrap();
        let parent = (Rule::Path, "deps".into(), CHANGED.into());
        assert_eq!(reread(t.path(), "deps/a.wit"), parent);
        symlink("f.wit", t.path().join("wit/l.wit")).unwrap();
        let link = (
            Rule::Path,
            "wit/l.wit".into(),
            refusal::link("wit/l.wit").into(),
        );
        assert_eq!(reread(t.path(), "wit/l.wit"), link);
        fs::write(t.path().join("wit/big.wit"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
        let big = (
            Rule::Limit,
            "wit/big.wit".into(),
            "file exceeds 1048576 bytes".into(),
        );
        assert_eq!(reread(t.path(), "wit/big.wit"), big);
        fs::write(t.path().join("wit/ok.wit"), vec![b'x'; 1024 * 1024]).unwrap();
        assert_eq!(wit(t.path(), "wit/ok.wit").unwrap().len(), 1024 * 1024);
    }
    #[test]
    fn ac1_s91_name_created_at_target_after_preflight() {
        let (_p, c, m) = setup();
        let root = c.path().to_path_buf();
        hook::set(move || {
            fs::create_dir_all(root.join("wit/deps/reading-list")).unwrap();
            fs::write(root.join("wit/deps/reading-list/world.wit"), "x").unwrap();
        });
        changed(
            add_provider(c.path(), &m, None),
            "wit/deps/reading-list/world.wit",
        );
    }
    #[test]
    fn ac1_s91_target_removed_after_preflight() {
        let (_p, c, m) = setup();
        add_provider(c.path(), &m, None).unwrap();
        let root = c.path().to_path_buf();
        hook::set(move || fs::remove_dir_all(root.join("wit/deps/reading-list")).unwrap());
        changed(
            add_provider(c.path(), &m, None),
            "wit/deps/reading-list/world.wit",
        );
    }
    #[test]
    fn ac1_s91_deps_removed_after_preflight() {
        let (_p, c, m) = setup();
        fs::create_dir_all(c.path().join("wit/deps")).unwrap();
        let root = c.path().to_path_buf();
        hook::set(move || fs::remove_dir_all(root.join("wit/deps")).unwrap());
        changed(add_provider(c.path(), &m, None), "wit/deps/reading-list");
    }
    fn provider_dep() -> (tempfile::TempDir, tempfile::TempDir, Mod) {
        let (p, c, m) = setup();
        fs::create_dir_all(p.path().join("wit/deps/x")).unwrap();
        fs::write(p.path().join("wit/deps/x/x.wit"), "x").unwrap();
        (p, c, m)
    }
    fn untouched(c: &Path) {
        assert!(!c.join("wit/deps/x").exists());
        assert!(!c.join("deps.json").exists());
    }
    #[test]
    fn ac1_listed_dep_folder_gone_is_changed() {
        let (p, c, m) = provider_dep();
        let root = p.path().to_path_buf();
        hook::set_listed(move || fs::remove_dir_all(root.join("wit/deps/x")).unwrap());
        changed(add_provider(c.path(), &m, None), "wit/deps/x");
        untouched(c.path());
    }
    #[test]
    fn ac2_listed_dep_folder_now_a_file_is_changed() {
        let (p, c, m) = provider_dep();
        let root = p.path().to_path_buf();
        hook::set_listed(move || {
            fs::remove_dir_all(root.join("wit/deps/x")).unwrap();
            fs::write(root.join("wit/deps/x"), "f").unwrap();
        });
        changed(add_provider(c.path(), &m, None), "wit/deps/x");
        untouched(c.path());
    }
}
