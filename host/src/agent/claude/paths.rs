//! The third rule of the protected reads (TECHNICAL 19.3): an `allow` to an ask of a file tool
//! is refused when its path, resolved as `claude` would, leaves the launch's workspaces.

use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// The tools whose asks the rule checks (19.3, Protected reads).
const FILE_TOOLS: [&str; 6] = ["Read", "Glob", "Grep", "Edit", "Write", "NotebookEdit"];
/// Symlinks followed in one path, as an operating system does (it stops at 40).
const MAX_LINKS: usize = 40;

/// The path with every symlink resolved, also for a tail that does not exist yet. `None` when
/// a link cannot be followed (a dangling or looping link: `claude` would write through it).
fn resolve(path: &Path, base: &Path, links: &mut usize) -> Option<PathBuf> {
    let parts: Vec<Component<'_>> = path.components().collect();
    let mut cur = base.to_path_buf();
    for (i, part) in parts.iter().enumerate() {
        match part {
            Component::Prefix(_) | Component::RootDir => cur = PathBuf::from("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                cur.pop();
            }
            Component::Normal(name) => {
                cur.push(name);
                let Ok(meta) = std::fs::symlink_metadata(&cur) else {
                    continue;
                };
                if meta.file_type().is_symlink() {
                    *links += 1;
                    if *links > MAX_LINKS {
                        return None;
                    }
                    let target = std::fs::read_link(&cur).ok()?;
                    // The target is relative to the folder of the link; its own `..` and links
                    // resolve in turn, with the rest of the path after it.
                    cur.pop();
                    let rest: PathBuf = parts[i + 1..].iter().collect();
                    let spliced = target.join(rest);
                    // A link whose target is missing is followed to it and found absent: the
                    // target must still lie inside, which `inside` checks on the result.
                    return resolve(&spliced, &cur, links);
                }
            }
        }
    }
    Some(cur)
}

/// `~` and `~/x` are the participant's home, as `claude` expands them; `~user` is not handled.
fn expand(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    match text.strip_prefix('~') {
        None => Some(PathBuf::from(text)),
        Some("") => home.map(Path::to_path_buf),
        Some(rest) => rest.strip_prefix('/').and_then(|r| home.map(|h| h.join(r))),
    }
}

/// The path with each `..` dropping the name before it, as Node's `path.resolve` does before
/// `claude` opens the file.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::Normal(name) => out.push(name),
            Component::ParentDir => {
                out.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::CurDir => {}
        }
    }
    out
}

/// Whether the file path `text` (relative to `cwd`) resolves inside one of `roots`. The links
/// of `cwd` itself are resolved too, and the path must stay inside both as given (a `..` after
/// a link climbs from its target) and with its `..` removed first (from the link's folder).
fn inside(text: &str, cwd: &Path, roots: &[PathBuf], home: Option<&Path>) -> bool {
    let Some(path) = expand(text, home) else {
        return false;
    };
    let joined = cwd.join(path);
    [lexical(&joined), joined].iter().all(|p| {
        resolve(p, Path::new("/"), &mut 0)
            .is_some_and(|real| roots.iter().any(|root| real.starts_with(root)))
    })
}

/// Whether a glob pattern could leave the workspace: absolute, home-relative, or with `..`.
fn pattern_escapes(pattern: &str) -> bool {
    pattern.starts_with('/') || pattern.starts_with('~') || pattern.split('/').any(|s| s == "..")
}

/// Whether an `allow` to an ask of `tool` with `input` is refused: the tool is a file tool and
/// a path input is outside `roots` (canonical folders, `cwd` first among them) or a pattern
/// input is absolute or climbs. An absent path means the working folder; a path or pattern
/// that is not a string is refused (fail closed).
pub(crate) fn refused(tool: &str, input: &Value, cwd: &Path, roots: &[PathBuf]) -> bool {
    if !FILE_TOOLS.contains(&tool) {
        return false;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for key in ["file_path", "path", "notebook_path"] {
        match input.get(key) {
            None => {}
            Some(Value::String(text)) => {
                if !inside(text, cwd, roots, home.as_deref()) {
                    return true;
                }
            }
            Some(_) => return true,
        }
    }
    for (name, key) in [("Glob", "pattern"), ("Grep", "glob")] {
        match input.get(key) {
            Some(Value::String(text)) if tool == name && pattern_escapes(text) => return true,
            Some(v) if tool == name && !v.is_string() => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    struct Folders {
        _dir: tempfile::TempDir,
        work: PathBuf,
        outside: PathBuf,
    }
    fn folders() -> Folders {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let work = base.join("work");
        let outside = base.join("outside");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(work.join("a.txt"), "a").unwrap();
        std::fs::write(outside.join("secret"), "s").unwrap();
        symlink(outside.join("secret"), work.join("link")).unwrap();
        symlink(&outside, work.join("dir")).unwrap();
        symlink(work.join("a.txt"), work.join("fine")).unwrap();
        symlink(base.join("nowhere"), work.join("dangling")).unwrap();
        Folders {
            _dir: dir,
            work,
            outside,
        }
    }
    fn refused_in(f: &Folders, tool: &str, input: Value) -> bool {
        refused(tool, &input, &f.work, std::slice::from_ref(&f.work))
    }

    #[test]
    fn a_path_in_the_workspace_and_an_absent_path_pass() {
        let f = folders();
        let abs = f.work.join("a.txt").display().to_string();
        assert!(!refused_in(&f, "Read", json!({"file_path": abs})));
        assert!(!refused_in(&f, "Read", json!({"file_path": "a.txt"})));
        assert!(!refused_in(&f, "Read", json!({"file_path": "new/b.txt"})));
        assert!(!refused_in(&f, "Read", json!({"file_path": "fine"})));
        assert!(!refused_in(&f, "Grep", json!({"pattern": "x"})));
        assert!(!refused_in(&f, "Glob", json!({"pattern": "**/*.rs"})));
        assert!(!refused_in(
            &f,
            "Grep",
            json!({"pattern": "x", "glob": "*.rs"})
        ));
        assert!(!refused_in(
            &f,
            "Bash",
            json!({"command": "cat /etc/passwd"})
        ));
    }

    #[test]
    fn a_path_outside_the_workspace_is_refused() {
        let f = folders();
        for path in [
            f.outside.join("secret").display().to_string(),
            "../outside/secret".to_owned(),
            "/etc/passwd".to_owned(),
            "~/.ssh/id_rsa".to_owned(),
            "~root/x".to_owned(),
            format!("{}/../outside", f.work.display()),
        ] {
            assert!(refused_in(&f, "Read", json!({"file_path": path})), "{path}");
        }
        assert!(refused_in(&f, "Read", json!({"file_path": 5})));
        assert!(refused_in(&f, "Grep", json!({"pattern": "x", "path": "/"})));
    }

    #[test]
    fn a_symlink_out_of_the_workspace_is_refused_however_the_path_names_it() {
        let f = folders();
        for path in [
            "link",
            "dir/secret",
            "dir/new.txt",
            "dir/not/yet/there",
            "dangling",
            "./dir/../dir/secret",
        ] {
            assert!(refused_in(&f, "Read", json!({"file_path": path})), "{path}");
        }
        let abs = f.work.join("link").display().to_string();
        assert!(refused_in(&f, "Edit", json!({"file_path": abs})));
        assert!(refused_in(
            &f,
            "NotebookEdit",
            json!({"notebook_path": "dir/n.ipynb"})
        ));
        assert!(refused_in(
            &f,
            "Grep",
            json!({"pattern": "x", "path": "dir"})
        ));
    }

    #[test]
    fn a_climb_after_a_link_is_refused_when_either_reading_leaves() {
        let f = folders();
        // `up/..` is `deep` as the kernel reads it, `work` as `path.resolve` reads it; `peek`
        // is a plain file in `deep` and a link out in `work`.
        std::fs::create_dir_all(f.work.join("deep/deeper")).unwrap();
        std::fs::write(f.work.join("deep/peek"), "p").unwrap();
        symlink(f.work.join("deep/deeper"), f.work.join("up")).unwrap();
        symlink(f.outside.join("secret"), f.work.join("peek")).unwrap();
        assert!(refused_in(&f, "Read", json!({"file_path": "up/../peek"})));
        assert!(refused_in(
            &f,
            "Read",
            json!({"file_path": "up/../../a.txt"})
        ));
        assert!(!refused_in(&f, "Read", json!({"file_path": "up/../a.txt"})));
    }

    #[test]
    fn a_working_folder_reached_through_a_link_is_resolved() {
        let f = folders();
        let alias = f.outside.join("alias");
        symlink(&f.work, &alias).unwrap();
        let roots = std::slice::from_ref(&f.work);
        assert!(!refused(
            "Read",
            &json!({"file_path": "a.txt"}),
            &alias,
            roots
        ));
        assert!(!refused(
            "Grep",
            &json!({"pattern": "x", "path": "."}),
            &alias,
            roots
        ));
        assert!(refused(
            "Read",
            &json!({"file_path": "../secret"}),
            &alias,
            roots
        ));
    }

    #[test]
    fn a_pattern_that_is_absolute_home_relative_or_climbs_is_refused() {
        let f = folders();
        for pattern in ["/etc/*", "../x", "a/../../x", "~/x"] {
            assert!(
                refused_in(&f, "Glob", json!({"pattern": pattern})),
                "{pattern}"
            );
            assert!(
                refused_in(&f, "Grep", json!({"pattern": "x", "glob": pattern})),
                "{pattern}"
            );
        }
        assert!(!refused_in(&f, "Glob", json!({"pattern": "src/**/*.rs"})));
    }
}
