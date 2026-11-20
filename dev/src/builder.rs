//! Builds a checked mod to `logic.wasm` in a cache copy of its sources
//! (TECHNICAL 6). No mod code runs natively; every cargo run keeps `env_clear()`.

use crate::allowlist::{self, Allowlist};
use crate::env::Env;
use crate::outcome::{Code, Problem};
use crate::toolchain::Toolchain;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};
use watane_pkg::{Mod, ModError, Slug};

#[derive(Debug)]
pub struct BuildCtx<'a> {
    pub env: &'a Env,
    pub toolchain: &'a Toolchain,
    pub vendor: &'a Path,
    pub allow: &'a Allowlist,
}
#[derive(Serialize, Debug)]
pub struct Built {
    pub wasm: String,
    pub bytes: u64,
    pub lock_updated: bool,
    pub seconds: f64,
}
fn quoted(path: &Path) -> String {
    toml::Value::String(path.to_string_lossy().into_owned()).to_string()
}
pub fn cargo(ctx: &BuildCtx<'_>, copy: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(&ctx.toolchain.cargo);
    command
        .current_dir(copy.join("logic"))
        .env_clear()
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin",
                ctx.toolchain
                    .cargo
                    .parent()
                    .expect("the cargo binary sits in a directory")
                    .display()
            ),
        )
        .env("RUSTC", &ctx.toolchain.rustc)
        .env("CARGO_HOME", ctx.env.cache.join("cargo-home"))
        .env("HOME", ctx.env.cache.join("cargo-home"))
        .env("TMPDIR", ctx.env.cache.join("tmp"))
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0");
    for key in ["SDKROOT", "DEVELOPER_DIR"] {
        if let Some(value) = crate::env::var_os(key) {
            command.env(key, value);
        }
    }
    let flags = toml::Value::Array(
        [
            (ctx.env.cache.as_path(), "/watane-dev"),
            (ctx.vendor, "/vendor"),
            (ctx.toolchain.sysroot.as_path(), "/toolchain"),
        ]
        .into_iter()
        .map(|(path, mapped)| {
            toml::Value::String(format!("--remap-path-prefix={}={mapped}", path.display()))
        })
        .collect(),
    );
    command
        .args(args)
        .arg("--manifest-path")
        .arg(copy.join("logic/Cargo.toml"))
        .arg("--offline");
    for config in [
        format!("build.target-dir={}", quoted(&ctx.env.cache.join("target"))),
        "source.crates-io.replace-with=\"watane-allowlist\"".into(),
        format!("source.watane-allowlist.directory={}", quoted(ctx.vendor)),
        format!("target.wasm32-wasip2.rustflags={flags}"),
        "profile.release.debug=false".into(),
        "profile.release.codegen-units=1".into(),
    ] {
        command.arg("--config").arg(config);
    }
    command
}
fn copy_tree(from: &Path, to: &Path, rust_only: bool) -> Result<(), Problem> {
    fs::create_dir_all(to).map_err(Problem::internal)?;
    for entry in fs::read_dir(from).map_err(Problem::internal)? {
        let entry = entry.map_err(Problem::internal)?;
        let kind = entry.file_type().map_err(Problem::internal)?;
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&source, &destination, rust_only)?;
        } else if kind.is_file() && (!rust_only || source.extension().is_some_and(|e| e == "rs")) {
            fs::copy(&source, &destination).map_err(Problem::internal)?;
        } else {
            return Err(Problem::new(
                Code::ModLogicFile,
                format!("{} is not a regular allowed source file", source.display()),
            )
            .at(source.to_string_lossy(), None, None)
            .fix("Use only regular WIT files and Rust sources"));
        }
    }
    Ok(())
}
fn resolve_problem(stderr: &[u8], allow: &Allowlist) -> Problem {
    let text = String::from_utf8_lossy(stderr);
    let mut lines = text.lines().rev().take(40).collect::<Vec<_>>();
    lines.reverse();
    Problem::new(Code::BuildResolve, lines.join("\n"))
        .at("logic/Cargo.toml", None, None)
        .fix(format!(
            "Use only the crates and features of the allowlist: {}",
            allow.crates()
        ))
}
fn check_lock(text: &str, manifest: &str, allow: &Allowlist) -> Result<(), Problem> {
    let lock: toml::Table = toml::from_str(text).map_err(Problem::internal)?;
    let manifest: toml::Table = toml::from_str(manifest).map_err(Problem::internal)?;
    for package in lock["package"]
        .as_array()
        .ok_or_else(|| Problem::internal("cargo generated a lock without packages"))?
    {
        let name = package["name"].as_str().unwrap_or_default();
        let version = package["version"].as_str().unwrap_or_default();
        if package.get("source").is_none()
            && Some(name)
                == manifest["package"]
                    .get("name")
                    .and_then(toml::Value::as_str)
            && Some(version)
                == manifest["package"]
                    .get("version")
                    .and_then(toml::Value::as_str)
        {
            continue;
        }
        let checksum = package
            .get("checksum")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        if !allow.contains(name, version, checksum) {
            return Err(Problem::new(
                Code::EnvAllowlist,
                format!(
                    "resolved crate {name} {version} differs from the allowlist of this watane-dev"
                ),
            )
            .at("logic/Cargo.lock", None, None)
            .fix("Reinstall watane-dev, or set WATANE_DEV_VENDOR"));
        }
    }
    Ok(())
}
fn compiler_problem(message: &Value, copy: &Path, slug: &Slug) -> Problem {
    let mut problem = Problem::new(
        Code::BuildCompile,
        message["message"]
            .as_str()
            .unwrap_or("Rust compilation failed"),
    )
    .fix("Correct the Rust error at the reported location");
    if let Some(span) = message["spans"]
        .as_array()
        .and_then(|spans| spans.iter().find(|s| s["is_primary"] == true))
        && let Some(file) = span["file_name"].as_str()
    {
        let path = Path::new(file);
        let remapped = PathBuf::from(format!("/watane-dev/build/{slug}"));
        let file = if let Ok(relative) = path
            .strip_prefix(copy)
            .or_else(|_| path.strip_prefix(&remapped))
        {
            relative.to_string_lossy().into_owned()
        } else if path.is_relative() {
            Path::new("logic").join(path).to_string_lossy().into_owned()
        } else {
            file.into()
        };
        problem = problem.at(
            file,
            span["line_start"].as_u64().and_then(|n| n.try_into().ok()),
            span["column_start"]
                .as_u64()
                .and_then(|n| n.try_into().ok()),
        );
    }
    problem
}
pub(crate) fn build_lock(slug: &Slug, ctx: &BuildCtx<'_>) -> Result<fs::File, Vec<Problem>> {
    for folder in ["build", "target", "cargo-home", "tmp"] {
        fs::create_dir_all(ctx.env.cache.join(folder)).map_err(|e| vec![Problem::internal(e)])?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(ctx.env.cache.join(format!("build/{slug}.lock")))
        .map_err(|e| vec![Problem::internal(e)])?;
    lock.lock().map_err(|e| vec![Problem::internal(e)])?;
    Ok(lock)
}

// Called under build_lock by both build and unit compilation. Only checked sources enter here.
pub(crate) fn prepare(
    dir: &Path,
    slug: &Slug,
    ctx: &BuildCtx<'_>,
) -> Result<(PathBuf, bool), Vec<Problem>> {
    allowlist::verify_vendor(ctx.vendor, ctx.allow).map_err(|e| vec![e])?;
    let copy = ctx.env.cache.join("build").join(slug.as_str());
    if copy.exists() {
        fs::remove_dir_all(&copy).map_err(|e| vec![Problem::internal(e)])?;
    }
    fs::create_dir_all(copy.join("logic")).map_err(|e| vec![Problem::internal(e)])?;
    copy_tree(&dir.join("wit"), &copy.join("wit"), false).map_err(|e| vec![e])?;
    copy_tree(&dir.join("logic/src"), &copy.join("logic/src"), true).map_err(|e| vec![e])?;
    fs::copy(dir.join("logic/Cargo.toml"), copy.join("logic/Cargo.toml"))
        .map_err(|e| vec![Problem::internal(e)])?;
    let generated = cargo(ctx, &copy, &["generate-lockfile"])
        .output()
        .map_err(|e| vec![Problem::internal(e)])?;
    if !generated.status.success() {
        return Err(vec![resolve_problem(&generated.stderr, ctx.allow)]);
    }
    let lock_text = fs::read_to_string(copy.join("logic/Cargo.lock"))
        .map_err(|e| vec![Problem::internal(e)])?;
    let manifest = fs::read_to_string(copy.join("logic/Cargo.toml"))
        .map_err(|e| vec![Problem::internal(e)])?;
    check_lock(&lock_text, &manifest, ctx.allow).map_err(|e| vec![e])?;
    let lock_updated = fs::read_to_string(dir.join("logic/Cargo.lock"))
        .ok()
        .as_deref()
        != Some(&lock_text);
    if lock_updated {
        fs::write(dir.join("logic/Cargo.lock"), &lock_text)
            .map_err(|e| vec![Problem::internal(e)])?;
    }
    Ok((copy, lock_updated))
}

pub(crate) fn compile(
    ctx: &BuildCtx<'_>,
    copy: &Path,
    slug: &Slug,
    args: &[&str],
) -> Result<Vec<PathBuf>, Vec<Problem>> {
    let manifest: toml::Table = toml::from_str(
        &fs::read_to_string(copy.join("logic/Cargo.toml"))
            .map_err(|e| vec![Problem::internal(e)])?,
    )
    .map_err(|e| vec![Problem::internal(e)])?;
    let package = manifest["package"]["name"]
        .as_str()
        .ok_or_else(|| vec![Problem::internal("missing crate name")])?;
    let crate_name = package.replace('-', "_");
    let profile = if args[0] == "test" {
        "debug"
    } else {
        "release"
    };
    let profile_dir = ctx.env.cache.join("target/wasm32-wasip2").join(profile);
    // rustc's dep-info of the mod crate: in `deps/` up to Cargo 1.99, in
    // `build/<package>/<hash>/out/` from Cargo 1.100's build-dir layout (6.4).
    let dep_files = || -> Result<Vec<PathBuf>, Vec<Problem>> {
        let read = |dir: &Path| -> std::io::Result<Vec<PathBuf>> {
            match fs::read_dir(dir) {
                Ok(entries) => entries.map(|e| e.map(|e| e.path())).collect(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
                Err(e) => Err(e),
            }
        };
        let internal = |e: std::io::Error| vec![Problem::internal(e)];
        let mut dirs = vec![profile_dir.join("deps")];
        for unit in read(&profile_dir.join("build").join(package)).map_err(internal)? {
            dirs.push(unit.join("out"));
        }
        let mut files = Vec::new();
        for dir in dirs {
            for path in read(&dir).map_err(internal)? {
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if path.extension().is_some_and(|e| e == "d")
                    && (stem == crate_name || stem.starts_with(&format!("{crate_name}-")))
                {
                    files.push(path);
                }
            }
        }
        Ok(files)
    };
    for path in dep_files()? {
        fs::remove_file(path).map_err(|e| vec![Problem::internal(e)])?;
    }
    // Cargo's binary dep-info can still consider a build fresh after the text .d
    // is removed. Refresh only the copied crate root so rustc emits new dep-info.
    fs::File::open(copy.join("logic/src/lib.rs"))
        .and_then(|file| file.set_modified(SystemTime::now()))
        .map_err(|e| vec![Problem::internal(e)])?;
    if args[0] == "build" {
        eprintln!("building {slug}");
    }
    let output = cargo(ctx, copy, args)
        .output()
        .map_err(|e| vec![Problem::internal(e)])?;
    let mut errors = Vec::new();
    let mut siblings = BTreeSet::new();
    let mut artifacts = Vec::new();
    for line in output.stdout.split(|b| *b == b'\n') {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if value["reason"] == "compiler-message" && value["message"]["level"] == "error" {
            errors.push(compiler_problem(&value["message"], copy, slug));
            siblings.extend(sibling_paths(
                value["message"]["rendered"].as_str().unwrap_or_default(),
                &ctx.env.cache.join("build"),
                copy,
            ));
        }
        if value["reason"] == "compiler-artifact"
            && if args[0] == "test" {
                value["profile"]["test"] == true
            } else {
                value["target"]["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "cdylib"))
            }
            && value["manifest_path"]
                .as_str()
                .is_some_and(|p| Path::new(p) == copy.join("logic/Cargo.toml"))
        {
            let artifact = if args[0] == "test" {
                value["executable"].as_str()
            } else {
                value["filenames"].as_array().and_then(|files| {
                    files
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|p| p.ends_with(".wasm"))
                })
            };
            if let Some(path) = artifact {
                artifacts.push(PathBuf::from(path));
            }
        }
    }
    // A relative path that leaves the copy lands in another mod's copy in the cache: the
    // compiler's errors are about that stale sibling, not about the mod (6.4).
    if !siblings.is_empty() {
        return Err(reads_outside_problems(siblings.into_iter().collect()));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if !output.status.success() {
        return Err(vec![resolve_problem(&output.stderr, ctx.allow)]);
    }
    let target = ctx.env.cache.join("target");
    let allowed = [
        copy,
        ctx.vendor,
        ctx.toolchain.sysroot.as_path(),
        target.as_path(),
    ];
    let files = dep_files()?;
    if files.is_empty() {
        return Err(vec![Problem::internal(
            "cargo produced no mod dependency file",
        )]);
    }
    let mut outside = BTreeSet::new();
    for path in files {
        let text = fs::read_to_string(path).map_err(|e| vec![Problem::internal(e)])?;
        outside.extend(outside_reads(&text, &allowed));
    }
    if !outside.is_empty() {
        return Err(reads_outside_problems(outside.into_iter().collect()));
    }
    Ok(artifacts)
}

/// The caller runs check first. Only checked sources enter the cache copy.
pub fn build(dir: &Path, slug: &Slug, ctx: &BuildCtx<'_>) -> Result<Built, Vec<Problem>> {
    let started = Instant::now();
    let destination = dir.join("logic.wasm");
    let _lock = build_lock(slug, ctx)?;
    let work = || -> Result<Built, Vec<Problem>> {
        match fs::remove_file(&destination) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(vec![Problem::internal(e)]),
        }
        let (copy, lock_updated) = prepare(dir, slug, ctx)?;
        let artifacts = compile(
            ctx,
            &copy,
            slug,
            &[
                "build",
                "--locked",
                "--release",
                "--target",
                "wasm32-wasip2",
                "--message-format",
                "json",
            ],
        )?;
        let artifact = artifacts.first().ok_or_else(|| {
            vec![Problem::internal(
                "cargo produced no cdylib WebAssembly artifact",
            )]
        })?;
        let bytes = fs::copy(artifact, &destination).map_err(|e| vec![Problem::internal(e)])?;
        Mod::read(dir)
            .and_then(|m| m.check_component())
            .map_err(|e| match e {
                ModError::Invalid(violations) => violations
                    .into_iter()
                    .map(|v| {
                        Problem::new(Code::BuildImports, format!("[component] {}", v.message))
                            .at(v.file, None, None)
                            .fix("Make logic exports and imports match wit/world.wit")
                    })
                    .collect(),
                ModError::Io { path, source } => {
                    vec![Problem::internal(source).at(path.to_string_lossy(), None, None)]
                }
            })?;
        Ok(Built {
            wasm: "logic.wasm".into(),
            bytes,
            lock_updated,
            seconds: started.elapsed().as_secs_f64(),
        })
    };
    let result = work();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}

// Resolve real files (including symlinks); lexical normalization also supports literal dep-info tests.
fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        let mut result = PathBuf::new();
        for part in path.components() {
            match part {
                Component::ParentDir => {
                    result.pop();
                }
                Component::CurDir => {}
                part => result.push(part),
            }
        }
        result
    })
}

/// One problem for all outside `.wit` paths, then one for each other path.
fn reads_outside_problems(paths: Vec<PathBuf>) -> Vec<Problem> {
    let (wit, other): (Vec<_>, Vec<_>) = paths
        .into_iter()
        .partition(|p| p.extension().is_some_and(|e| e == "wit"));
    let mut problems = Vec::new();
    match wit.as_slice() {
        [] => {}
        [one] => problems.push(reads_outside_problem(one)),
        many => {
            let mut folder = many[0].parent().unwrap_or(Path::new("")).to_path_buf();
            for path in &many[1..] {
                while !path.starts_with(&folder) && folder.pop() {}
            }
            let mut problem = reads_outside_problem(&folder);
            problem.message = format!(
                "the build read {} .wit files under {}, outside the mod",
                many.len(),
                folder.display()
            );
            problem.fix = Some(wit_fix(&folder));
            problems.push(problem);
        }
    }
    problems.extend(other.iter().map(|p| reads_outside_problem(p)));
    problems
}

/// The paths in `text` that lie in the cache's `build` folder but not in this mod's `copy`.
/// `build` and `copy` are compared canonicalized (a symlinked cache path names the same
/// folder); found paths are normalized lexically and must stay under `build`. A path stops at
/// whitespace, a control character or a bracket or quote.
fn sibling_paths(text: &str, build: &Path, copy: &Path) -> Vec<PathBuf> {
    let real_build = build.canonicalize().unwrap_or_else(|_| build.to_path_buf());
    let real_copy = copy.canonicalize().unwrap_or_else(|_| copy.to_path_buf());
    let mut prefixes = vec![build.to_path_buf()];
    if real_build != build {
        prefixes.push(real_build.clone());
    }
    let mut found = Vec::new();
    for base in prefixes {
        let prefix = format!("{}/", base.display());
        let mut rest = text;
        while let Some(at) = rest.find(&prefix) {
            let tail = &rest[at..];
            // The path ends after the prefix (which may hold a space) at a space or a bracket.
            let end = tail[prefix.len()..]
                .find(|c: char| c.is_whitespace() || c.is_control() || "[]'\"`()".contains(c))
                .map_or(tail.len(), |n| n + prefix.len());
            let mut path = &tail[..end];
            // `world.wit:19:10` names a line and a column.
            while let Some((head, last)) = path.rsplit_once(':')
                && last.chars().all(|c| c.is_ascii_digit())
            {
                path = head;
            }
            let path = normalized(Path::new(path.trim_end_matches(':')));
            let path = match path.strip_prefix(build) {
                Ok(inner) => real_build.join(inner),
                _ => path,
            };
            if path.starts_with(&real_build)
                && !path.starts_with(&real_copy)
                && !found.contains(&path)
            {
                found.push(path);
            }
            rest = &tail[end..];
        }
    }
    // The folder of a read file is the one thing to name.
    let all = found.clone();
    found.retain(|p| !all.iter().any(|a| a != p && p.starts_with(a)));
    found
}

fn wit_fix(path: &Path) -> String {
    format!(
        "Point wit_bindgen::generate! in logic/src at this mod's own wit/ folder (path: \"../wit\"), not at {}",
        path.display()
    )
}

fn reads_outside_problem(path: &Path) -> Problem {
    let fix = if path.extension().is_some_and(|e| e == "wit")
        || path.file_name().is_some_and(|n| n == "wit")
    {
        wit_fix(path)
    } else {
        "Remove the include_bytes!, include_str!, include! or #[path] that names it".into()
    };
    Problem::new(
        Code::BuildReadsOutside,
        format!("the build read {}, outside the mod", path.display()),
    )
    .at(path.to_string_lossy(), None, None)
    .fix(fix)
}

fn outside_reads(dep_info: &str, allowed: &[&Path]) -> Vec<PathBuf> {
    let roots: Vec<_> = allowed.iter().map(|p| normalized(p)).collect();
    let mut outside = BTreeSet::new();
    for line in dep_info.lines().filter(|line| !line.starts_with('#')) {
        let Some((_, dependencies)) = line.split_once(": ") else {
            continue;
        };
        let mut chars = dependencies.chars().peekable();
        let mut word = String::new();
        let mut paths = Vec::new();
        while let Some(ch) = chars.next() {
            if ch == '\\'
                && chars
                    .peek()
                    .is_some_and(|c| c.is_whitespace() || *c == '\\' || *c == '#')
            {
                word.push(chars.next().expect("the condition peeked this char"));
            } else if ch.is_whitespace() {
                if !word.is_empty() {
                    paths.push(std::mem::take(&mut word));
                }
            } else {
                word.push(ch);
            }
        }
        if !word.is_empty() {
            paths.push(word);
        }
        for path in paths {
            let path = PathBuf::from(path);
            let path = normalized(&if path.is_absolute() {
                path
            } else {
                allowed[0].join("logic").join(path)
            });
            if !roots.iter().any(|root| path.starts_with(root)) {
                outside.insert(path);
            }
        }
    }
    outside.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac2_outside_wit_paths_are_one_problem() {
        let paths = [
            "/x/other/wit/deps/a/b.wit",
            "/x/other/wit/deps/c.wit",
            "/x/other/wit/world.wit",
            "/x/other/data.rs",
        ]
        .map(PathBuf::from)
        .to_vec();
        let problems = reads_outside_problems(paths);
        assert_eq!(problems.len(), 2);
        assert_eq!(
            problems[0].message,
            "the build read 3 .wit files under /x/other/wit, outside the mod"
        );
        assert_eq!(
            problems[0].fix.as_deref(),
            Some(
                "Point wit_bindgen::generate! in logic/src at this mod's own wit/ folder (path: \"../wit\"), not at /x/other/wit"
            )
        );
        assert_eq!(
            problems[1].message,
            "the build read /x/other/data.rs, outside the mod"
        );
        assert_eq!(
            problems[1].fix.as_deref(),
            Some("Remove the include_bytes!, include_str!, include! or #[path] that names it")
        );
        let one = reads_outside_problems(vec![PathBuf::from("/x/other/wit/world.wit")]);
        assert_eq!(one.len(), 1);
        assert_eq!(
            one[0].message,
            "the build read /x/other/wit/world.wit, outside the mod"
        );
    }
    #[test]
    fn ac6_reads_outside_fix_depends_on_the_path() {
        let wit = reads_outside_problem(Path::new("/x/other/wit/world.wit"));
        assert_eq!(
            wit.fix.as_deref(),
            Some(
                "Point wit_bindgen::generate! in logic/src at this mod's own wit/ folder (path: \"../wit\"), not at /x/other/wit/world.wit"
            )
        );
        let data = reads_outside_problem(Path::new("/x/data.bin"));
        assert!(data.fix.unwrap().starts_with("Remove the include_bytes!,"));
    }
    #[test]
    fn ac1_release_and_unit_build_progress_stays_on_stderr() {
        const CHILD: &str = "WATANE_TEST_BUILD_PROGRESS";
        if std::env::var_os(CHILD).is_some() {
            let root = tempfile::tempdir().unwrap();
            let copy = root.path().join("copy");
            {
                fs::create_dir_all(copy.join("logic/src")).unwrap();
                fs::write(
                    copy.join("logic/Cargo.toml"),
                    "[package]\nname = 'counter'\n",
                )
                .unwrap();
                fs::write(copy.join("logic/src/lib.rs"), "").unwrap();
                let env = Env {
                    cache: root.path().join("cache"),
                    unit_seconds: 60,
                };
                let toolchain = Toolchain {
                    cargo: "/usr/bin/false".into(),
                    rustc: "/unused/rustc".into(),
                    sysroot: "/unused/sysroot".into(),
                    version: "test".into(),
                };
                let allow = Allowlist::embedded();
                let ctx = BuildCtx {
                    env: &env,
                    toolchain: &toolchain,
                    vendor: root.path(),
                    allow: &allow,
                };
                for args in [&["build", "--release"][..], &["test", "--no-run"][..]] {
                    compile(&ctx, &copy, &"counter".parse().unwrap(), args).unwrap_err();
                }
            }
            // `exit` runs no destructors: drop the temp folder first.
            drop(root);
            println!("{{\"ok\":false}}");
            std::process::exit(0);
        }
        let tmp = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "builder::tests::ac1_release_and_unit_build_progress_stays_on_stderr",
                "--nocapture",
                "--quiet",
            ])
            .env(CHILD, "1")
            .env("TMPDIR", tmp.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let left: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(left.is_empty(), "temp folders left by the child: {left:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "building counter\n"
        );
        // libtest writes its own preamble; the rest of stdout is the one result object.
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.ends_with("\n{\"ok\":false}\n"), "{stdout}");
        assert!(!stdout.contains("building"), "{stdout}");
    }
    #[test]
    fn ac3_dep_info_parses_escaped_spaces_and_checks_all_allowed_roots() {
        let allowed = [
            Path::new("/copy"),
            Path::new("/vendor"),
            Path::new("/sysroot"),
            Path::new("/cache/target"),
        ];
        let text = "/cache/target/mod.wasm: /copy/logic/src/lib.rs /copy/wit/world.wit /vendor/a/src/lib.rs /sysroot/lib/std.rs /cache/target/generated.rs /outside/secret\\ file.txt /copy/../escaped.txt\n\n/copy/logic/src/lib.rs:\n# env-dep:SECRET=abc\n";
        assert_eq!(
            outside_reads(text, &allowed),
            vec![
                PathBuf::from("/escaped.txt"),
                PathBuf::from("/outside/secret file.txt")
            ]
        );
    }
    #[test]
    fn ac6_cargo_config_quotes_paths_and_restricts_environment() {
        let env = Env {
            cache: PathBuf::from("/cache with \"quotes\""),
            unit_seconds: 60,
        };
        let toolchain = Toolchain {
            cargo: "/rust/bin/cargo".into(),
            rustc: "/rust/bin/rustc".into(),
            sysroot: "/rust".into(),
            version: "test".into(),
        };
        let allow = Allowlist::embedded();
        let ctx = BuildCtx {
            env: &env,
            toolchain: &toolchain,
            vendor: Path::new("/vendor with \"quotes\""),
            allow: &allow,
        };
        let command = cargo(&ctx, Path::new("/copy"), &["generate-lockfile"]);
        let args: Vec<_> = command.get_args().map(|a| a.to_str().unwrap()).collect();
        let value = args
            .windows(2)
            .find(|w| w[0] == "--config" && w[1].starts_with("source.watane-allowlist.directory="))
            .unwrap()[1];
        let config: toml::Table = toml::from_str(value).unwrap();
        assert_eq!(
            config["source"]["watane-allowlist"]["directory"].as_str(),
            Some("/vendor with \"quotes\"")
        );
        let keys: Vec<_> = command
            .get_envs()
            .filter_map(|(k, v)| v.map(|_| k.to_str().unwrap()))
            .collect();
        assert!(keys.iter().all(|k| {
            [
                "PATH",
                "RUSTC",
                "CARGO_HOME",
                "HOME",
                "TMPDIR",
                "CARGO_TERM_COLOR",
                "CARGO_INCREMENTAL",
                "SDKROOT",
                "DEVELOPER_DIR",
            ]
            .contains(k)
        }));
        assert!(args.contains(&"--offline"));
    }

    #[test]
    fn s93_ac5_sibling_cache_paths_in_compiler_text() {
        let text = "failed to resolve directory while parsing WIT for path [/c/build/other/wit]\n\nCaused by:\n  package 'watane:host@0.1.0' not found\n     --> /c/build/other/wit/world.wit:19:10\n  also /c/build/hello/logic/src/lib.rs:3:1 and /elsewhere/x.wit";
        assert_eq!(
            sibling_paths(text, Path::new("/c/build"), Path::new("/c/build/hello")),
            [PathBuf::from("/c/build/other/wit")]
        );
        assert_eq!(
            sibling_paths(text, Path::new("/c/build"), Path::new("/c/build/other")),
            [PathBuf::from("/c/build/hello/logic/src/lib.rs")]
        );
        // A cache folder with a space: the copy's own path is not a sibling.
        let text = "failed to read [/a b/build/hello/wits] and (/a b/build/other/wit/w.wit:1:2)";
        assert_eq!(
            sibling_paths(text, Path::new("/a b/build"), Path::new("/a b/build/hello")),
            [PathBuf::from("/a b/build/other/wit/w.wit")]
        );
    }
    #[test]
    fn s93_fix2_sibling_paths_are_real_siblings() {
        let (b, c) = (Path::new("/c/build"), Path::new("/c/build/me"));
        // AC1
        assert!(sibling_paths("error at /c/build/hello/../../../etc/passwd:1:2", b, c).is_empty());
        // AC2
        assert_eq!(
            sibling_paths("/c/build/a\x1b[2J", b, c),
            [PathBuf::from("/c/build/a")]
        );
        // AC3
        let dir = std::env::temp_dir().join(format!("watane-fix2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("real/build/me")).unwrap();
        std::os::unix::fs::symlink(dir.join("real"), dir.join("link")).unwrap();
        let text = format!("{}/link/build/me/logic/missing.rs", dir.display());
        assert!(
            sibling_paths(&text, &dir.join("link/build"), &dir.join("real/build/me")).is_empty()
        );
        // A sibling named through the link or by its real path is one sibling, in real form.
        let real = dir.join("real").canonicalize().unwrap();
        let linked = format!("{}/link/build/other/wit", dir.display());
        let canonical = format!("{}/build/other/wit", real.display());
        for text in [&linked, &canonical, &format!("{linked} and {canonical}")] {
            assert_eq!(
                sibling_paths(text, &dir.join("link/build"), &dir.join("real/build/me")),
                [real.join("build/other/wit")]
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn ac3_compiler_failure_code_location_and_fix() {
        let message = serde_json::json!({"message":"test-only compile failure", "spans":[{
            "is_primary":true,"file_name":"/watane-dev/build/counter/logic/src/lib.rs", "line_start":71,"column_start":1
        }]});
        let error = compiler_problem(
            &message,
            Path::new("/cache/build/counter"),
            &"counter".parse().unwrap(),
        );
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!({
                "code":"build.compile","message":"test-only compile failure","file":"logic/src/lib.rs","line":71,"column":1,
                "fix":"Correct the Rust error at the reported location"
            })
        );
    }
}
