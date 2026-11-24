//! The third rule of the protected reads (TECHNICAL 19.3): an `allow` to an ask of a file tool
//! is refused when its path, resolved as `claude` would, leaves the launch's workspaces.

use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use unicode_normalization::is_nfc;

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
                // Only a missing component lets the path go on (a prospective write); any
                // other error (`ENOTDIR`, `EACCES`, ...) leaves it unresolved (19.3).
                let meta = match std::fs::symlink_metadata(&cur) {
                    Ok(meta) => meta,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => return None,
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

/// Read as `claude` reads it (19.3): a path with white space (JavaScript's `trim` set) at an end
/// is trimmed by `claude`, and one not in NFC is normalized; the host resolves neither, so
/// it refuses both.
fn misread(path: &str) -> bool {
    path.starts_with(space) || path.ends_with(space) || !is_nfc(path)
}

/// White space as JavaScript's `trim` and `\s` read it: Rust's set (which adds U+0085) and U+FEFF.
fn space(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

/// A glob that `claude` may split into several or expand with braces; refused whole.
fn glob_ambiguous(pattern: &str) -> bool {
    pattern.contains(['{', '}', ',']) || pattern.contains(space)
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
                if misread(text) || !inside(text, cwd, roots, home.as_deref()) {
                    return true;
                }
            }
            Some(_) => return true,
        }
    }
    for (name, key) in [("Glob", "pattern"), ("Grep", "glob")] {
        match input.get(key) {
            Some(Value::String(text))
                if tool == name && (pattern_escapes(text) || glob_ambiguous(text)) =>
            {
                return true;
            }
            Some(v) if tool == name && !v.is_string() => return true,
            _ => {}
        }
    }
    false
}

/// The absolute path that an ask or check of `tool` with `input` reaches, symlinks resolved, as
/// the item's `path` (19.3, step 4): the tool's own key (`file_path` for Read, Edit and Write,
/// `path` for Glob and Grep, `notebook_path` for NotebookEdit), resolved as `claude` does. Absent
/// (Glob and Grep only): the working folder; Read, Edit, Write and NotebookEdit without their key,
/// or another path key present: `None`. `None` for a
/// tool that is not a file tool, a path that the answer rule refuses, or one that cannot be
/// resolved or is not UTF-8.
pub(crate) fn resolved(
    tool: &str,
    input: &Value,
    cwd: &Path,
    roots: &[PathBuf],
    run: &Path,
) -> Option<String> {
    if !FILE_TOOLS.contains(&tool) || refused_for_answer(tool, input, cwd, roots, run) {
        return None;
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let own = match tool {
        "Glob" | "Grep" => "path",
        "NotebookEdit" => "notebook_path",
        _ => "file_path",
    };
    let keys = ["file_path", "path", "notebook_path"];
    if keys.iter().any(|k| *k != own && input.get(*k).is_some()) {
        return None;
    }
    let text = match input.get(own) {
        None => None,
        Some(v) => Some(v.as_str()?),
    };
    let real = match text {
        None if own == "path" => resolve(cwd, Path::new("/"), &mut 0)?,
        None => return None,
        Some(text) => {
            let joined = cwd.join(expand(text, home.as_deref())?);
            resolve(&lexical(&joined), Path::new("/"), &mut 0)?
        }
    };
    real.to_str().map(str::to_owned)
}

/// `refused` for an answer: the run folder `run` (one of `roots`) is read-only, so it counts
/// for `Read`, `Glob` and `Grep` only (19.3), also when a workspace holds it or a link leads in.
pub(crate) fn refused_for_answer(
    tool: &str,
    input: &Value,
    cwd: &Path,
    roots: &[PathBuf],
    run: &Path,
) -> bool {
    refused(tool, input, cwd, roots)
        || matches!(tool, "Edit" | "Write" | "NotebookEdit") && reaches(input, cwd, run)
}

/// Whether a path input, as given or with its `..` removed first, resolves inside `run`.
fn reaches(input: &Value, cwd: &Path, run: &Path) -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    ["file_path", "path", "notebook_path"]
        .iter()
        .filter_map(|key| input.get(*key)?.as_str())
        .any(|text| {
            expand(text, home.as_deref()).is_some_and(|path| {
                let joined = cwd.join(path);
                [lexical(&joined), joined].iter().any(|p| {
                    resolve(p, Path::new("/"), &mut 0).is_some_and(|real| real.starts_with(run))
                })
            })
        })
}

#[cfg(all(test, unix))] // Fixtures use Unix symlinks or permission bits.
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
    fn ac2_white_space_and_non_nfc_paths_and_ambiguous_globs_are_refused() {
        let f = folders();
        for path in [
            "a.txt ",
            " a.txt",
            "\na.txt",
            "a.txt\u{a0}",
            "\u{feff}a.txt",
            "cafe\u{301}",
        ] {
            assert!(
                refused_in(&f, "Read", json!({"file_path": path})),
                "{path:?}"
            );
        }
        assert!(!refused_in(&f, "Read", json!({"file_path": "caf\u{e9}"})));
        for glob in ["{a,b}", "a}", "a,b", "a b", "a\tb", "a\u{feff}../b"] {
            assert!(refused_in(&f, "Glob", json!({"pattern": glob})), "{glob:?}");
            assert!(
                refused_in(&f, "Grep", json!({"pattern": "x", "glob": glob})),
                "{glob:?}"
            );
        }
    }

    #[test]
    fn ac3_the_run_folder_is_read_only_for_an_answer() {
        let f = folders();
        let run = f.outside.join("run");
        std::fs::create_dir_all(run.join("watane-run/skills/x")).unwrap();
        let roots = [f.work.clone(), run.clone()];
        let skill = run
            .join("watane-run/skills/x/SKILL.md")
            .display()
            .to_string();
        let check =
            |tool: &str, input: Value| refused_for_answer(tool, &input, &f.work, &roots, &run);
        assert!(check("Edit", json!({"file_path": skill})));
        assert!(check("Write", json!({"file_path": skill})));
        assert!(check("NotebookEdit", json!({"notebook_path": skill})));
        assert!(!check("Read", json!({"file_path": skill})));
        assert!(!check(
            "Grep",
            json!({"pattern": "x", "path": run.display().to_string()})
        ));
        assert!(!check("Write", json!({"file_path": "new.txt"})));
        // A link from a workspace into the run folder, and a run folder a workspace holds.
        symlink(&run, f.work.join("r")).unwrap();
        assert!(check(
            "Edit",
            json!({"file_path": "r/watane-run/skills/x/SKILL.md"})
        ));
        assert!(!check(
            "Read",
            json!({"file_path": "r/watane-run/skills/x/SKILL.md"})
        ));
        let held = f.work.join("home/runs/1/watane-run");
        std::fs::create_dir_all(held.join("skills/x")).unwrap();
        let roots = [f.work.clone(), held.clone()];
        let skill = held.join("skills/x/SKILL.md").display().to_string();
        assert!(refused_for_answer(
            "Write",
            &json!({"file_path": skill}),
            &f.work,
            &roots,
            &held
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

/// Adversary step of S100 (red): allows that pass the third rule but that `claude` reads
/// outside the workspaces.
#[cfg(all(test, unix))] // Fixtures use Unix symlinks or permission bits.
mod adv_s100 {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let work = base.join("work");
        let outside = base.join("outside");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), "s").unwrap();
        (dir, work, outside)
    }

    /// `claude`'s `expandPath` trims the path before it resolves it: ` /etc/passwd` is read
    /// as `/etc/passwd`, but the host joins it to the working folder as a relative name.
    #[test]
    fn adv_s100_path_with_leading_space_is_read_as_absolute() {
        let (_d, work, outside) = setup();
        let roots = std::slice::from_ref(&work);
        let secret = format!(" {}", outside.join("secret").display());
        for path in [secret.as_str(), " /etc/passwd", "\t~/.bash_history"] {
            assert!(
                refused("Read", &json!({ "file_path": path }), &work, roots),
                "{path:?} passes the host, and claude trims it to a path outside"
            );
        }
    }

    /// `claude` NFC-normalizes the path it opens; the host stats the bytes as given. A link
    /// named in NFC (a picked folder may hold one) is missed when the path names it in NFD.
    #[test]
    fn adv_s100_nfd_path_misses_an_nfc_link_out() {
        let (_d, work, outside) = setup();
        symlink(outside.join("secret"), work.join("caf\u{e9}")).unwrap();
        let roots = std::slice::from_ref(&work);
        assert!(refused(
            "Read",
            &json!({"file_path": "caf\u{e9}"}),
            &work,
            roots
        ));
        assert!(
            refused("Read", &json!({"file_path": "cafe\u{301}"}), &work, roots),
            "the NFD name passes the host; claude opens the NFC link to the outside file"
        );
    }

    /// `claude`'s Grep splits `glob` on white space and on commas (outside braces) into
    /// several globs; the host checks the whole text, so a second glob may climb or be
    /// absolute.
    #[test]
    fn adv_s100_grep_glob_list_hides_a_climb() {
        let (_d, work, _o) = setup();
        let roots = std::slice::from_ref(&work);
        for glob in [
            "*.rs ../outside/*",
            "*.rs,../outside/*",
            "*.rs /etc/*",
            "*.rs,~/x",
        ] {
            assert!(
                refused("Grep", &json!({"pattern": "x", "glob": glob}), &work, roots),
                "{glob:?} passes the host; claude reads a second glob that leaves"
            );
        }
    }

    /// A brace alternative expands to a pattern that starts with `/` or `~` or climbs; the
    /// host only looks at the first character and the `/`-separated segments.
    #[test]
    fn adv_s100_glob_brace_hides_an_absolute_or_climbing_pattern() {
        let (_d, work, _o) = setup();
        let roots = std::slice::from_ref(&work);
        for pattern in ["{/etc,x}/*", "{..,x}/outside/*", "{~,x}/.bash_history"] {
            assert!(
                refused("Glob", &json!({ "pattern": pattern }), &work, roots),
                "{pattern:?} passes the host; the glob expands to one that leaves"
            );
        }
    }
}

#[cfg(all(test, unix))] // Fixtures use Unix symlinks or permission bits.
mod resolved_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ac2_the_path_of_an_ask_is_the_one_it_reaches_with_links_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().canonicalize().unwrap().join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("real.txt"), "r").unwrap();
        std::os::unix::fs::symlink(work.join("real.txt"), work.join("link.txt")).unwrap();
        let roots = [work.clone()];
        let run = PathBuf::from("/nonexistent-run");
        let got = |tool: &str, input: Value| resolved(tool, &input, &work, &roots, &run);
        let real = work.join("real.txt").to_str().unwrap().to_owned();
        assert_eq!(got("Read", json!({"file_path":"link.txt"})), Some(real));
        assert_eq!(
            got("Grep", json!({"pattern":"x"})),
            Some(work.to_str().unwrap().to_owned())
        );
        assert_eq!(got("Read", json!({"file_path":"/etc/passwd"})), None);
        assert_eq!(got("Glob", json!({"pattern":"/etc/*"})), None);
        assert_eq!(got("Bash", json!({"command":"ls"})), None);
    }
}

#[cfg(test)]
mod ac2_own_key {
    use super::*;
    use serde_json::json;

    #[test]
    fn ac2_only_the_tools_own_key_gives_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().canonicalize().unwrap().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let roots = [work.clone()];
        let run = PathBuf::from("/nonexistent-run");
        let got = |tool: &str, input: Value| resolved(tool, &input, &work, &roots, &run);
        let w = work.to_str().unwrap().to_owned();
        assert_eq!(got("Read", json!({"path": "a"})), None);
        assert_eq!(got("Grep", json!({"pattern": "x", "file_path": "a"})), None);
        assert_eq!(
            got(
                "Grep",
                json!({"pattern": "x", "file_path": "a", "path": "b"})
            ),
            None
        );
        assert_eq!(
            got(
                "NotebookEdit",
                json!({"file_path": "a", "notebook_path": "b"})
            ),
            None
        );
        assert_eq!(got("Glob", json!({"pattern": "x"})), Some(w.clone()));
        for tool in ["Read", "Edit", "Write"] {
            assert_eq!(got(tool, json!({"file_path": "a"})), Some(format!("{w}/a")));
        }
        assert_eq!(
            got("NotebookEdit", json!({"notebook_path": "n.ipynb"})),
            Some(format!("{w}/n.ipynb"))
        );
        assert_eq!(
            got("Grep", json!({"pattern": "x", "path": "d"})),
            Some(format!("{w}/d"))
        );
        assert_eq!(got("Read", json!({"file_path": 1})), None);
    }

    #[test]
    fn s100_fix8_ac1_only_glob_and_grep_fall_back_to_the_working_folder() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().canonicalize().unwrap().join("work");
        std::fs::create_dir_all(&work).unwrap();
        let roots = [work.clone()];
        let run = PathBuf::from("/nonexistent-run");
        let got = |tool: &str, input: Value| resolved(tool, &input, &work, &roots, &run);
        for tool in ["Read", "Edit", "Write", "NotebookEdit"] {
            assert_eq!(got(tool, json!({})), None, "{tool}");
        }
        assert_eq!(got("Read", json!({"offset": 1})), None);
        assert_eq!(got("NotebookEdit", json!({"new_source": ""})), None);
        let w = work.to_str().unwrap().to_owned();
        assert_eq!(got("Glob", json!({"pattern": "x"})), Some(w.clone()));
        assert_eq!(got("Grep", json!({"pattern": "x"})), Some(w));
    }
}

/// Adversary step of S100, third pass (red): the item's `path` must be the path the tool
/// touches. `resolved` takes the first of `file_path`, `path`, `notebook_path` whatever the
/// tool, so a Grep or Glob ask that also carries a `file_path`, or a NotebookEdit ask that
/// carries one beside its `notebook_path`, shows the kernel another path than the one the
/// tool reaches, and the kernel scopes a session allow on it.
#[cfg(test)]
mod adv_s100_path_key {
    use super::*;
    use serde_json::json;

    #[test]
    fn adv_s100_resolved_path_is_the_key_the_tool_reads() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().canonicalize().unwrap().join("work");
        std::fs::create_dir_all(work.join("secrets")).unwrap();
        std::fs::write(work.join("README.md"), "r").unwrap();
        let roots = [work.clone()];
        let run = PathBuf::from("/nonexistent-run");
        let got = |tool: &str, input: Value| resolved(tool, &input, &work, &roots, &run);
        let secrets = work.join("secrets").to_str().unwrap().to_owned();
        // Host TECHNICAL 19.3 step 4: an input that holds another path key beside the tool's own
        // gives no `path`, so the kernel cannot scope an allow on a path the tool does not reach.
        for tool in ["Grep", "Glob"] {
            assert_eq!(
                got(
                    tool,
                    json!({"pattern": "*", "file_path": "README.md", "path": "secrets"})
                ),
                None,
                "{tool} searches `path`, the item must not show `file_path`"
            );
            assert_eq!(
                got(tool, json!({"pattern": "*", "path": "secrets"})),
                Some(secrets.clone()),
                "{tool} reads `path`"
            );
        }
        assert_eq!(
            got(
                "NotebookEdit",
                json!({"file_path": "README.md", "notebook_path": "secrets/n.ipynb", "new_source": ""})
            ),
            None,
            "NotebookEdit writes `notebook_path`, the item must not show `file_path`"
        );
    }
}

/// Review r2 of S100, F2.1: only a missing component lets a path resolve through; any other
/// error of the filesystem makes it unresolvable (TECHNICAL 19.3, the resolve rule).
#[cfg(all(test, unix))] // Fixtures use Unix symlinks or permission bits.
mod fix9_resolve_errors {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn rig() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().canonicalize().unwrap().join("work");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("a.txt"), "a").unwrap();
        (dir, work)
    }
    fn answer(work: &Path, tool: &str, input: &Value) -> (bool, Option<String>) {
        let roots = [work.to_path_buf()];
        let run = PathBuf::from("/nonexistent-run");
        (
            refused_for_answer(tool, input, work, &roots, &run),
            resolved(tool, input, work, &roots, &run),
        )
    }

    #[test]
    fn s100_fix9_ac1_a_file_used_as_a_folder_is_refused_and_gives_no_path() {
        let (_d, work) = rig();
        let input = json!({"file_path": "a.txt/child"});
        assert_eq!(answer(&work, "Read", &input), (true, None));
        let input = json!({"file_path": "a.txt/child/deeper"});
        assert_eq!(answer(&work, "Write", &input), (true, None));
    }

    #[test]
    fn s100_fix9_ac1_a_symlink_loop_is_refused_and_gives_no_path() {
        let (_d, work) = rig();
        symlink(work.join("b"), work.join("a")).unwrap();
        symlink(work.join("a"), work.join("b")).unwrap();
        for path in ["a", "a/x", "b/x/y"] {
            let input = json!({"file_path": path});
            assert_eq!(answer(&work, "Read", &input), (true, None), "{path}");
        }
    }

    #[test]
    fn s100_fix9_ac1_an_unreadable_folder_is_refused_and_gives_no_path() {
        let (_d, work) = rig();
        let locked = work.join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let unreadable = std::fs::symlink_metadata(locked.join("x"))
            .is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied);
        let input = json!({"file_path": "locked/x"});
        let got = answer(&work, "Read", &input);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !unreadable {
            eprintln!("skipped: this user (root) can search a mode 000 folder");
            return;
        }
        assert_eq!(got, (true, None));
    }

    #[test]
    fn s100_fix9_ac1_a_missing_tail_under_an_existing_folder_still_resolves() {
        let (_d, work) = rig();
        std::fs::create_dir(work.join("sub")).unwrap();
        let w = work.to_str().unwrap();
        let input = json!({"file_path": "sub/new/deep/b.txt"});
        assert_eq!(
            answer(&work, "Write", &input),
            (false, Some(format!("{w}/sub/new/deep/b.txt")))
        );
        let input = json!({"file_path": "missing"});
        assert_eq!(
            answer(&work, "Read", &input),
            (false, Some(format!("{w}/missing")))
        );
    }
}
