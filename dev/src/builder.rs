use crate::{
    allowlist::{self, Allowlist},
    env::Env,
    outcome::{Code, Problem},
    toolchain::Toolchain,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime},
};
use watane_pkg::{Mod, ModError, Slug};

pub struct BuildCtx<'a> {
    pub env: &'a Env,
    pub toolchain: &'a Toolchain,
    pub vendor: &'a Path,
    pub allow: &'a Allowlist,
}
#[derive(Serialize)]
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
                ctx.toolchain.cargo.parent().unwrap().display()
            ),
        )
        .env("RUSTC", &ctx.toolchain.rustc)
        .env("CARGO_HOME", ctx.env.cache.join("cargo-home"))
        .env("HOME", ctx.env.cache.join("cargo-home"))
        .env("TMPDIR", ctx.env.cache.join("tmp"))
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_INCREMENTAL", "0");
    for key in ["SDKROOT", "DEVELOPER_DIR"] {
        if let Some(value) = std::env::var_os(key) {
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
    .fix("Correct the Rust error at the reported location and run the command again");
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
    let crate_name = manifest["package"]["name"]
        .as_str()
        .ok_or_else(|| vec![Problem::internal("missing crate name")])?
        .replace('-', "_");
    let profile = if args[0] == "test" {
        "debug"
    } else {
        "release"
    };
    let deps = ctx
        .env
        .cache
        .join("target/wasm32-wasip2")
        .join(profile)
        .join("deps");
    let dep_files = || -> Result<Vec<PathBuf>, Vec<Problem>> {
        if !deps.exists() {
            return Ok(Vec::new());
        }
        fs::read_dir(&deps)
            .map_err(|e| vec![Problem::internal(e)])?
            .filter_map(|entry| match entry {
                Ok(entry) => {
                    let path = entry.path();
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    (path.extension().is_some_and(|e| e == "d")
                        && (stem == crate_name || stem.starts_with(&format!("{crate_name}-"))))
                    .then_some(Ok(path))
                }
                Err(e) => Some(Err(vec![Problem::internal(e)])),
            })
            .collect()
    };
    for path in dep_files()? {
        fs::remove_file(path).map_err(|e| vec![Problem::internal(e)])?;
    }
    // Cargo's binary dep-info can still consider a build fresh after the text .d
    // is removed. Refresh only the copied crate root so rustc emits new dep-info.
    fs::File::open(copy.join("logic/src/lib.rs"))
        .and_then(|file| file.set_modified(SystemTime::now()))
        .map_err(|e| vec![Problem::internal(e)])?;
    eprintln!("building {slug} (the first build takes a minute or more)");
    let output = cargo(ctx, copy, args)
        .output()
        .map_err(|e| vec![Problem::internal(e)])?;
    let mut errors = Vec::new();
    let mut artifacts = Vec::new();
    for line in output.stdout.split(|b| *b == b'\n') {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        if value["reason"] == "compiler-message" && value["message"]["level"] == "error" {
            errors.push(compiler_problem(&value["message"], copy, slug));
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
        return Err(outside
            .into_iter()
            .map(|path| {
                Problem::new(
                    Code::BuildReadsOutside,
                    format!("the build read {}, outside the mod", path.display()),
                )
                .at(path.to_string_lossy(), None, None)
                .fix("Remove the include_bytes!, include_str!, include! or #[path] that names it")
            })
            .collect());
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
                word.push(chars.next().unwrap());
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
    fn ac1_release_and_unit_build_progress_stays_on_stderr() {
        const CHILD: &str = "WATANE_TEST_BUILD_PROGRESS";
        if std::env::var_os(CHILD).is_some() {
            let root = tempfile::tempdir().unwrap();
            let copy = root.path().join("copy");
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
                assert!(compile(&ctx, &copy, &"counter".parse().unwrap(), args).is_err());
            }
            println!("{{\"ok\":false}}");
            std::process::exit(0);
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "builder::tests::ac1_release_and_unit_build_progress_stays_on_stderr",
                "--nocapture",
                "--quiet",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "building counter (the first build takes a minute or more)\nbuilding counter (the first build takes a minute or more)\n"
        );
        // libtest writes its own leading newline and test count before the child exits.
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "\nrunning 1 test\n{\"ok\":false}\n"
        );
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
                "fix":"Correct the Rust error at the reported location and run the command again"
            })
        );
    }
}
