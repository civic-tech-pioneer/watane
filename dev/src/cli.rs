use crate::{
    allowlist::{self, Allowlist},
    builder::{self, BuildCtx},
    check, contract, deps,
    env::Env,
    outcome::{Outcome, Problem},
    preview, template, toolchain, unit,
};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
#[derive(Parser)]
#[command(version, about = "Make and preview watane mods")]
pub struct Cli {
    #[arg(long, global = true, help = "Print one JSON result object")]
    pub json: bool,
    #[arg(
        long,
        global = true,
        help = "Use this mod folder (new: the destination folder)"
    )]
    pub dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Create a mod from the counter template
    New {
        /// Lowercase mod slug, such as reading-list
        #[arg(required_unless_present = "from")]
        slug: Option<String>,
        #[arg(long, help = "Create from the source of a published mod link")]
        from: Option<String>,
        #[arg(long, requires = "from", help = "Home of the running watane app")]
        home: Option<PathBuf>,
    },
    /// Check the mod layout, WIT and builder constraints
    Check,
    /// Report the toolchain, allowlist and host environment
    Doctor,
    /// Check, build and run sandboxed unit and contract tests
    Test,
    /// Add a local provider and report its WIT imports
    Add {
        /// Folder of the provider mod
        #[arg(required_unless_present = "release", conflicts_with = "release")]
        provider: Option<PathBuf>,
        #[arg(long, help = "Published provider link or ReleaseRef")]
        release: Option<String>,
        #[arg(long, requires = "release", help = "Home of the running watane app")]
        home: Option<PathBuf>,
    },
    /// Test local mods and ask the person to install them in their app
    Install {
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
        #[arg(long, help = "Seconds to wait for the person to answer [default: 600]")]
        wait: Option<u64>,
    },
    /// Test and package the mod, then ask the person to sign a release in their app
    Publish {
        #[arg(long, help = "Exclude the source from the package")]
        no_source: bool,
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
        #[arg(long, help = "Seconds to wait for the person to answer [default: 600]")]
        wait: Option<u64>,
    },
    /// Find published mods in the person's chosen indexes
    // Shadow the inherited global argument so search can hide it without
    // restructuring every mod command. It is accepted and ignored for search.
    #[command(arg(clap::Arg::new("dir").long("dir").value_parser(clap::value_parser!(PathBuf)).help("Ignored by search").hide(true)))]
    Search {
        /// Words to search for (joined with one space; empty searches all mods)
        text: Vec<String>,
        #[arg(long, help = "Publisher key to filter by (52 characters)")]
        publisher: Option<String>,
        #[arg(long, help = "Interface name to filter by, optionally with a version")]
        interface: Option<String>,
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
    },
    /// Test the mod and serve it in a preview
    Serve {
        #[arg(long, help = "Stops the preview and deletes its data")]
        stop: bool,
    },
}
pub fn run(cli: Cli) -> Outcome {
    let name = match cli.command {
        Command::New { .. } => "new",
        Command::Check => "check",
        Command::Doctor => "doctor",
        Command::Test => "test",
        Command::Add { .. } => "add",
        Command::Serve { .. } => "serve",
        Command::Install { .. } => "install",
        Command::Publish { .. } => "publish",
        Command::Search { .. } => "search",
    };
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => return Outcome::new(name, None).fail(vec![Problem::internal(e)]),
    };
    if let Command::Search {
        text,
        publisher,
        interface,
        home,
    } = cli.command
    {
        return search(&text, publisher.as_deref(), interface.as_deref(), home);
    }
    if let Command::New {
        slug,
        from: Some(target),
        home,
    } = &cli.command
    {
        return derive(
            cli.dir.as_deref(),
            slug.as_deref(),
            target,
            home.clone(),
            &cwd,
        );
    }
    let dir = cli.dir.unwrap_or_else(|| match &cli.command {
        Command::New { slug, .. } => cwd.join(slug.as_deref().expect("clap requires a slug")),
        _ => cwd.clone(),
    });
    let dir = if dir.is_absolute() {
        dir
    } else {
        cwd.join(dir)
    };
    let dir = match resolved_path(&dir) {
        Ok(dir) => dir,
        Err(e) => return Outcome::new(name, Some(dir)).fail(vec![Problem::internal(e)]),
    };
    let mut outcome = Outcome::new(name, Some(dir.clone()));
    let result = match cli.command {
        Command::Search { .. } => unreachable!("search needs no mod folder"),
        Command::Doctor => return crate::doctor::doctor(&dir),
        Command::Install { home, wait } => return install(&dir, home, wait),
        Command::Publish { no_source, home, wait } => return publish(&dir, no_source, home, wait),
        Command::New { slug, .. } => template::new_mod(&dir, slug.as_deref().expect("clap requires a slug"))
            .map_err(|e| vec![e])
            .and_then(|report| serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)]))
            .inspect(|_| {
                outcome.next = Some("Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev serve --json".into());
            }),
        Command::Check => dir
            .canonicalize()
            .or_else(|e| match e.kind() {
                // A missing folder has no mod.toml: check reports usage.not_a_mod.
                std::io::ErrorKind::NotFound => Ok(dir.clone()),
                _ => Err(vec![Problem::internal(e)]),
            })
            .and_then(|dir| {
                outcome.dir = Some(dir.clone());
                check::check(&dir).and_then(|(_, report)| {
                    serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)])
                })
            }),
        Command::Test => {
            return test(&dir);
        }
        Command::Add { release: Some(target), home, .. } => {
            let result = (|| {
                let home = app_home(home)?;
                let client = crate::api::Client::for_app(&home).map_err(|e|vec![e])?;
                let env = Env::from_process().map_err(|e|vec![e])?;
                crate::build_on::add_release(&dir, &target, &client, &env)
            })();
            // A failed fetch has no lines to copy: its fix is the next step.
            return match result {
                Ok(v) => {
                    outcome.next = Some("Copy the lines that the mod needs into the world in wit/world.wit. Each line is a grant that the participant approves. Contract tests use a fake of it (see AGENTS.md).".into());
                    outcome.result = Some(v);
                    outcome
                }
                Err(e) => {
                    outcome.next = e.first().and_then(|p| p.fix.clone());
                    outcome.fail(e)
                }
            };
        }
        Command::Add { provider, .. } => deps::add_local(&dir, provider.as_deref().expect("clap requires a provider"))
            .and_then(|report| serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)]))
            .inspect(|_| {
                outcome.dir = dir.canonicalize().ok();
                outcome.next = Some("Copy the lines that the mod needs into the world in wit/world.wit. Each line is a grant that the participant approves.".into());
            }),
        Command::Serve { stop } => dir
            .canonicalize()
            .or_else(|e| match e.kind() {
                // A missing folder has no mod.toml: check reports usage.not_a_mod.
                std::io::ErrorKind::NotFound => Ok(dir.clone()),
                _ => Err(vec![Problem::internal(e)]),
            })
            .and_then(|dir| {
                outcome.dir = Some(dir.clone());
                let env = Env::from_process().map_err(|e| vec![e])?;
                if stop {
                    preview::stop(&dir, &env)
                } else {
                    let tested = test_local(&dir);
                    if !tested.ok {
                        outcome.next = tested.next;
                        if let Some(warning) = preview::previous_warning(&dir, &env) {
                            outcome.warnings.push(warning);
                        }
                        return Err(tested.errors);
                    }
                    preview::serve(&dir, &env)
                }
            }),
    };
    match result {
        Ok(v) => {
            if name == "serve" {
                outcome.next = serve_next(&v, cli.json);
            }
            outcome.result = Some(v);
            if name == "new" {
                outcome.dir = std::fs::canonicalize(&dir).ok();
            }
            outcome
        }
        Err(e) => {
            if e.first()
                .is_some_and(|p| p.code.class() == crate::outcome::Class::Env)
            {
                outcome.next = Some("watane-dev doctor --json".into());
            } else if name == "check"
                && e.first()
                    .is_some_and(|p| p.code.class() == crate::outcome::Class::Mod)
            {
                outcome.next = Some("Fix the errors, then run: watane-dev check --json".into());
            }
            outcome.fail(e)
        }
    }
}

fn is_cached_provider(dir: &Path) -> bool {
    Env::from_process().is_ok_and(|env| dir.starts_with(env.cache.join("providers")))
}

fn derive(
    destination: Option<&Path>,
    slug: Option<&str>,
    target: &str,
    home: Option<PathBuf>,
    cwd: &Path,
) -> Outcome {
    let mut outcome = Outcome::new("new", None);
    let result = (|| {
        if let Some(slug) = slug {
            slug.parse::<watane_pkg::Slug>().map_err(|e|vec![Problem::new(crate::outcome::Code::UsageSlug,format!("{slug:?} is not a valid slug: {e}")).fix("Use lowercase words of letters and digits joined by \"-\", each starting with a letter, at most 40 bytes, not a WIT keyword. Example: reading-list")])?;
        }
        let destination = destination
            .map(Path::to_path_buf)
            .or_else(|| slug.map(|s| cwd.join(s)));
        let destination = destination
            .map(|p| resolved_path(&cwd.join(p)))
            .transpose()
            .map_err(|e| vec![Problem::internal(e)])?;
        outcome.dir = destination.clone();
        if let Some(dir) = &destination
            && dir.symlink_metadata().is_ok()
        {
            return Err(vec![crate::build_on::exists(dir)]);
        }
        let home = app_home(home)?;
        let client = crate::api::Client::for_app(&home).map_err(|e| vec![e])?;
        let env = Env::from_process().map_err(|e| vec![e])?;
        crate::build_on::new_from(destination.as_deref(), slug, target, &client, &env)
    })();
    match result {
        Ok(value) => {
            let from = value["from_slug"].as_str().expect("from slug");
            let slug = value["slug"].as_str().expect("slug");
            outcome.dir = value["dir"].as_str().map(PathBuf::from);
            outcome.next = Some(
                if value["mentions"].as_array().is_some_and(|v| !v.is_empty()) {
                    format!(
                        "Replace {from} with {slug} (and {} with {}) at each result.mentions entry, then run: watane-dev test --json",
                        from.replace('-', "_"),
                        slug.replace('-', "_")
                    )
                } else {
                    "Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev test --json"
                        .into()
                },
            );
            outcome.result = Some(value);
            outcome
        }
        Err(errors) => {
            outcome.next = errors.first().and_then(|e| e.fix.clone());
            outcome.fail(errors)
        }
    }
}

fn serve_next(result: &serde_json::Value, json: bool) -> Option<String> {
    if result["stopped"] == true {
        return Some(
            "The preview and its data are gone. To start a new one, run: watane-dev serve --json"
                .into(),
        );
    }
    let page = result["pages"].as_array()?.last()?["url"]
        .as_str()
        .map(|url| format!("Give the person this page: {url}"));
    if result["reused"] == false {
        let prefix = if json { "result." } else { "" };
        let setup = format!(
            "The MCP url and token are new. Set or update your agent's MCP config with {prefix}mcp.url and {prefix}mcp.token (see AGENTS.md, \"MCP tools for your agent\")."
        );
        Some(match page {
            Some(page) => format!("{setup} {page}"),
            None => setup,
        })
    } else {
        page
    }
}

fn test_local(dir: &Path) -> Outcome {
    let order = match deps::local_order(dir) {
        Ok(order) => order,
        Err(e) => return Outcome::new("test", Some(dir.to_path_buf())).fail(vec![e]),
    };
    for provider in order {
        if provider != dir && is_cached_provider(&provider) {
            continue;
        }
        let mut tested = test(&provider);
        if !tested.ok {
            if provider != dir {
                let slug = deps::label(&provider);
                let mut path = provider.display().to_string();
                if !path
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
                {
                    path = format!("'{}'", path.replace('\'', "'\"'\"'"));
                }
                let next = format!("watane-dev test --json --dir {path}");
                tested.next = Some(next.clone());
                for problem in &mut tested.errors {
                    problem.message = format!("{slug}: {}", problem.message);
                    problem.fix = Some(next.clone());
                    let file = problem
                        .file
                        .as_deref()
                        .map_or(provider.clone(), |f| provider.join(f));
                    problem.file = Some(file.to_string_lossy().into_owned());
                }
            }
            return tested;
        }
    }
    Outcome::new("test", Some(dir.to_path_buf()))
}

/// Check, build and run unit and contract tests, retaining completed parts on failure.
pub fn test(dir: &Path) -> Outcome {
    let mut outcome = Outcome::new("test", Some(dir.to_path_buf()));
    outcome.next = Some("watane-dev test --json".into());
    let mut result =
        serde_json::json!({"check": null, "build": null, "unit": null, "contract": null});
    let mut pipeline = || -> Result<Vec<Problem>, Vec<Problem>> {
        let dir = dir.canonicalize().or_else(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Ok(dir.to_path_buf())
            } else {
                Err(vec![Problem::internal(e)])
            }
        })?;
        outcome.dir = Some(dir.clone());
        let (m, report) = check::check(&dir)?;
        result["check"] = serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)])?;
        if m.logic().is_none() {
            return Ok(vec![]);
        }
        let env = Env::from_process().map_err(|e| vec![e])?;
        let toolchain = toolchain::find(&env).map_err(|e| vec![e])?;
        let vendor = allowlist::vendor_dir().map_err(|e| vec![e])?;
        let allow = Allowlist::embedded();
        let ctx = BuildCtx {
            env: &env,
            toolchain: &toolchain,
            vendor: &vendor,
            allow: &allow,
        };
        let built = builder::build(&dir, m.slug(), &ctx)?;
        result["build"] = serde_json::to_value(built).map_err(|e| vec![Problem::internal(e)])?;
        let report = unit::unit_tests(&dir, m.slug(), &ctx)?;
        let mut errors = report.problems();
        result["unit"] = serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)])?;
        let (report, problems) = match contract::contract_tests(&dir, m.slug()) {
            Ok(result) => result,
            Err(problems) => {
                errors.extend(problems);
                return Err(errors);
            }
        };
        errors.extend(problems);
        result["contract"] =
            serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)])?;
        Ok(errors)
    };
    let errors = pipeline().unwrap_or_else(|errors| errors);
    if !result["check"].is_null() {
        outcome.result = Some(result);
    }
    if errors.is_empty() {
        outcome.next = Some("watane-dev serve --json".into());
        outcome
    } else {
        if errors
            .first()
            .is_some_and(|p| p.code.class() == crate::outcome::Class::Env)
        {
            outcome.next = Some("watane-dev doctor --json".into());
        }
        outcome.fail(errors)
    }
}

fn search(
    words: &[String],
    publisher: Option<&str>,
    interface: Option<&str>,
    home: Option<PathBuf>,
) -> Outcome {
    let mut outcome = Outcome::new("search", None);
    let result: Result<serde_json::Value, Vec<Problem>> = (|| {
        let home = app_home(home)?;
        let client = crate::api::Client::for_app(&home).map_err(|e| {
            let mut next = String::from("watane-dev search --json");
            for word in words {
                next.push_str(&format!(" {}", quote(Path::new(word))));
            }
            for (flag, value) in [("publisher", publisher), ("interface", interface)] {
                if let Some(value) = value {
                    next.push_str(&format!(" --{flag} {}", quote(Path::new(value))));
                }
            }
            next.push_str(&format!(" --home {}", quote(&home)));
            outcome.next = Some(next);
            vec![e]
        })?;
        let answer = client
            .search(&words.join(" "), publisher, interface)
            .map_err(|e| vec![e])?;
        if answer["results"].as_array().is_some_and(Vec::is_empty) {
            outcome.next = Some(
                if answer["indexes"] == 0 {
                    "Ask the person to add an index in the watane app, then run search again"
                } else {
                    "Try fewer words, or search with --interface <name>"
                }
                .into(),
            );
        }
        Ok(serde_json::json!({"home":home,"indexes":answer["indexes"],"results":answer["results"]}))
    })();
    match result {
        Ok(value) => {
            outcome.result = Some(value);
            outcome
        }
        Err(errors) => {
            if outcome.next.is_none() {
                outcome.next = errors.first().and_then(|e| e.fix.clone());
            }
            outcome.fail(errors)
        }
    }
}

fn install(dir: &Path, home: Option<PathBuf>, wait: Option<u64>) -> Outcome {
    let mut outcome = Outcome::new("install", Some(dir.to_path_buf()));
    let result = (|| {
        let (home, client) = app_client(&mut outcome, home, wait)?;
        let dir = dir.canonicalize().map_err(|e| vec![Problem::internal(e)])?;
        outcome.dir = Some(dir.clone());
        let tested = test_local(&dir);
        if !tested.ok {
            outcome.next = tested.next;
            return Err(tested.errors);
        }
        let folders: Vec<_> = deps::local_order(&dir)
            .map_err(|e| vec![e])?
            .into_iter()
            .filter(|p| !is_cached_provider(p))
            .collect();
        let installed = client
            .install_wait(&folders, wait.unwrap_or(600))
            .map_err(|e| vec![e])?;
        let installed: Vec<_> = installed.into_iter().map(|m| serde_json::json!({
            "slug":m.slug, "local":m.local, "version":m.version, "package":m.package, "page":m.page
        })).collect();
        outcome.next = install_next(&installed);
        Ok(serde_json::json!({"home":home, "installed":installed}))
    })();
    match result {
        Ok(value) => {
            outcome.result = Some(value);
            outcome
        }
        Err(errors) => {
            if errors.first().is_some_and(|p| {
                p.code.class() == crate::outcome::Class::Env
                    && p.code != crate::outcome::Code::EnvHostNotRunning
            }) {
                outcome.next = Some("watane-dev doctor --json".into());
            }
            if outcome.next.is_none() {
                outcome.next = errors
                    .first()
                    .and_then(|e| e.fix.clone())
                    .or_else(|| Some("watane-dev test --json".into()));
            }
            outcome.fail(errors)
        }
    }
}

/// Quotes a path for a POSIX shell `next` line.
fn quote(path: &Path) -> String {
    let path = path.display().to_string();
    if !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
    {
        path
    } else {
        format!("'{}'", path.replace('\'', "'\"'\"'"))
    }
}

/// Resolve symlinks in the existing ancestor without creating the missing suffix.
fn resolved_path(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match ancestor.canonicalize() {
            Ok(mut resolved) => {
                for part in suffix.into_iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = ancestor.file_name() else {
                    return Err(e);
                };
                suffix.push(name);
                ancestor = ancestor.parent().ok_or(e)?;
            }
            Err(e) => return Err(e),
        }
    }
}

fn app_home(home: Option<PathBuf>) -> Result<PathBuf, Vec<Problem>> {
    let home = home
        .map(Ok)
        .unwrap_or_else(watane_host::default_home)
        .map_err(|e| vec![Problem::internal(e)])?;
    resolved_path(&home).map_err(|e| vec![Problem::internal(e)])
}

fn app_client(
    outcome: &mut Outcome,
    home: Option<PathBuf>,
    wait: Option<u64>,
) -> Result<(PathBuf, crate::api::Client), Vec<Problem>> {
    let home = app_home(home)?;
    let client = crate::api::Client::for_app(&home).map_err(|e| {
        if e.code == crate::outcome::Code::EnvHostNotRunning {
            let dir = outcome.dir.as_ref().expect("command has a mod folder");
            let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
            let mut next = format!(
                "watane-dev {} --json --dir {} --home {}",
                outcome.command,
                quote(&dir),
                quote(&home)
            );
            if let Some(wait) = wait {
                next.push_str(&format!(" --wait {wait}"));
            }
            outcome.next = Some(next);
        }
        vec![e]
    })?;
    Ok((home, client))
}

fn publish(dir: &Path, no_source: bool, home: Option<PathBuf>, wait: Option<u64>) -> Outcome {
    use crate::outcome::{Class, Code};
    let mut outcome = Outcome::new("publish", Some(dir.to_path_buf()));
    let result = (|| {
        let providers = watane_pkg::Deps::read(dir).map_err(check::mod_problems)?;
        let errors: Vec<_> = providers
            .iter()
            .filter(|(_, publisher)| publisher.is_none())
            .map(|(slug, _)| {
                Problem::new(
                    Code::PublishLocalDependency,
                    format!("mods:{slug} is a local provider; it has no release"),
                )
                .at("deps.json", None, None)
                .fix(format!("Publish {slug} first (if it is already published, that run reports its release), then depend on its release"))
            })
            .collect();
        if !errors.is_empty() {
            let local = deps::read_local(dir).unwrap_or_default();
            outcome.next = providers
                .iter()
                .filter(|(_, publisher)| publisher.is_none())
                .find_map(|(slug, _)| local.get(&slug.to_string()))
                .map(|path| {
                    let path = dir.join(path);
                    let path = path.canonicalize().unwrap_or(path);
                    let mut next = format!("watane-dev publish --json --dir {}", quote(&path));
                    if let Some(home) = &home {
                        let home = resolved_path(home).unwrap_or_else(|_| home.clone());
                        next.push_str(&format!(" --home {}", quote(&home)));
                    }
                    next
                });
            return Err(errors);
        }
        let (home, client) = app_client(&mut outcome, home, wait)?;
        let releases = client.releases().map_err(|e| vec![e])?;
        // Mod owns WIT identity parsing. Invalid mods retain the pipeline's diagnostics.
        if let Ok(m) = watane_pkg::Mod::read(dir)
            && let Some(release) = releases["releases"].as_array().and_then(|entries| {
                entries.iter().find(|r| {
                    r["slug"] == m.slug().as_str() && r["version"] == m.version().to_string()
                })
            })
        {
            outcome.result = Some(serde_json::json!({
                "home": home, "release": release["release"], "link": release["link"]
            }));
            let link = release["link"].as_str().map_or_else(
                || "it has no link yet: set your site's URL in the watane app".into(),
                |link| format!("its link is {link}"),
            );
            return Err(vec![
                Problem::new(
                    Code::PublishVersionExists,
                    format!(
                        "{} {} is already published as {}; {link}",
                        m.slug(),
                        m.version(),
                        release["release"].as_str().unwrap_or_default()
                    ),
                )
                .at("wit/world.wit", None, None)
                .fix("This version is already published. To publish changes, raise the version in wit/world.wit, then run publish again"),
            ]);
        }
        let tested = test(dir);
        outcome.dir = tested.dir;
        if !tested.ok {
            outcome.next = tested.next;
            return Err(tested.errors);
        }
        let package = watane_pkg::pack(dir, !no_source).map_err(|e| match e {
            watane_pkg::PackageError::Mod(e) => check::mod_problems(e),
            e => vec![
                Problem::new(Code::ModInvalid, e.to_string())
                    .fix("Fix the package contents, then run publish again"),
            ],
        })?;
        let config = deps::read_config(dir).map_err(|e| vec![e])?;
        let links: Vec<_> = providers
            .iter()
            .filter_map(|(slug, _)| {
                config
                    .release_providers
                    .get(slug.as_str())
                    .and_then(|r| r.link.clone())
            })
            .collect();
        let timeout = publish_timeout(&providers, &releases, wait.unwrap_or(600));
        let answer = client.publish_with_links(&package, timeout, &links)
            .map_err(|mut e| {
                if e.code == Code::HostDeclined {
                    let slug = deps::label(dir);
                    e.message = format!("The person declined publishing {slug} from their watane app");
                    e.fix = Some(format!("Ask the person whether they want {slug} published, then run publish again with --wait <seconds>"));
                }
                outcome.next = e.fix.clone();
                vec![e]
            })?;
        let listed = answer
            .get("providers")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        if let Some(providers) = listed.as_array() {
            for provider in providers.iter().filter(|p| p["listed"] == false) {
                let name = provider["provider"].as_str().unwrap_or_default();
                let slug = deps::label(dir);
                outcome.warnings.push(Problem::new(Code::PublishProviderUnlisted, format!("{name} is not listed in your channel, so people who install {slug} need a channel or an index that names where it is published"))
                    .fix(format!("Ask the person to list {name} in their own index in the watane app")));
            }
        }
        outcome.next = Some(
            if answer["link"].is_null() {
                "Set your site's URL in the watane app, then run publish again to get the link"
            } else {
                "Put the site folder on your static web host"
            }
            .into(),
        );
        Ok(
            serde_json::json!({"home": home, "release": answer["release"], "site": answer["site"], "link": answer["link"], "package": package.id(), "source": package.has_source(), "providers": listed}),
        )
    })();
    match result {
        Ok(value) => {
            outcome.result = Some(value);
            outcome
        }
        Err(errors) => {
            if errors
                .first()
                .is_some_and(|e| e.code.class() == Class::Env && e.code != Code::EnvHostNotRunning)
            {
                outcome.next = Some("watane-dev doctor --json".into());
            }
            if outcome.next.is_none() {
                outcome.next = errors
                    .first()
                    .and_then(|e| e.fix.clone())
                    .or_else(|| Some("watane-dev test --json".into()));
            }
            outcome.fail(errors)
        }
    }
}

fn publish_timeout(
    providers: &watane_pkg::Deps,
    releases: &serde_json::Value,
    wait: u64,
) -> std::time::Duration {
    let own_key = releases["releases"]
        .as_array()
        .and_then(|r| r.first())
        .and_then(|r| r["release"].as_str())
        .and_then(|r| r.split_once('/'))
        .map(|(key, _)| key);
    let foreign = providers
        .iter()
        .filter(|(_, p)| p.is_some_and(|p| Some(p.to_string()).as_deref() != own_key))
        .count() as u64;
    std::time::Duration::from_secs(wait.saturating_mul(1 + foreign))
}

fn install_next(installed: &[serde_json::Value]) -> Option<String> {
    installed
        .iter()
        .rev()
        .find_map(|m| m["page"].as_str())
        .map(|url| format!("Give the person this page: {url}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ac6_publish_timeout_allows_each_foreign_provider_listing() {
        let mut providers = watane_pkg::Deps::default();
        let own: watane_pkg::PublisherKey = "y".repeat(52).parse().unwrap();
        let other = watane_pkg::PublisherKey::from_bytes([1; 32]);
        providers.set("ratings".parse().unwrap(), Some(other));
        providers.set("tags".parse().unwrap(), Some(own.clone()));
        assert_eq!(
            publish_timeout(
                &providers,
                &json!({"releases":[{"release":format!("{own}/stats@0.1.0")}]}),
                2
            ),
            std::time::Duration::from_secs(4)
        );
        assert_eq!(
            publish_timeout(&providers, &json!({"releases":[]}), 2),
            std::time::Duration::from_secs(6)
        );
    }

    #[test]
    fn ac3_only_search_hides_global_dir() {
        use clap::CommandFactory;
        let mut cli = Cli::command();
        cli.build();
        for name in [
            "new", "check", "doctor", "test", "add", "install", "publish", "serve", "search",
        ] {
            let help = cli
                .find_subcommand_mut(name)
                .unwrap()
                .render_long_help()
                .to_string();
            assert_eq!(help.contains("--dir"), name != "search", "{name}: {help}");
        }
        // clap's inherited global flag remains accepted and search ignores it.
        let parsed = Cli::try_parse_from(["watane-dev", "search", "--dir", "x", "foo"]).unwrap();
        assert!(matches!(parsed.command, Command::Search { text, .. } if text == ["foo"]));
    }

    #[test]
    fn ac1_missing_home_resolves_existing_symlink_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("alias");
        std::os::unix::fs::symlink(root.path(), &link).unwrap();
        assert_eq!(
            app_home(Some(link.join("new/nested"))).unwrap(),
            root.path().canonicalize().unwrap().join("new/nested")
        );
        assert!(!root.path().join("new").exists());
    }

    #[test]
    fn ac2_serve_next_url_is_last_and_mcp_keys_match_mode() {
        let result = json!({"reused": false, "pages": [{"url": "http://demo.localhost:7/"}]});
        for json in [false, true] {
            let next = serve_next(&result, json).unwrap();
            let prefix = if json { "result." } else { "" };
            assert_eq!(
                next,
                format!(
                    "The MCP url and token are new. Set or update your agent's MCP config with {prefix}mcp.url and {prefix}mcp.token (see AGENTS.md, \"MCP tools for your agent\"). Give the person this page: http://demo.localhost:7/"
                )
            );
        }
    }

    #[test]
    fn ac2_new_preview_next_includes_page_and_mcp_setup() {
        let next = serve_next(
            &json!({
                "reused": false,
                "pages": [{"url": "http://demo.localhost:7/"}],
                "mcp": {"url": "http://localhost:7/mcp/preview", "token": "wt_new"}
            }),
            true,
        )
        .unwrap();
        for text in [
            "Give the person this page: http://demo.localhost:7/",
            "MCP url and token are new",
            "Set or update your agent's MCP config",
            "result.mcp.url",
            "result.mcp.token",
            "MCP tools for your agent",
        ] {
            assert!(next.contains(text), "missing {text}: {next}");
        }
    }

    #[test]
    fn ac2_reused_preview_next_keeps_page_instruction() {
        assert_eq!(
            serve_next(
                &json!({
                    "reused": true,
                    "pages": [{"url": "http://demo.localhost:7/"}],
                    "mcp": {"url": "http://localhost:7/mcp/preview", "token": "wt_old"}
                }),
                true
            )
            .as_deref(),
            Some("Give the person this page: http://demo.localhost:7/")
        );
    }

    #[test]
    fn ac2_new_preview_without_page_still_gives_mcp_setup() {
        let pageless = |reused| json!({"reused": reused, "pages": [{"slug": "demo", "url": null}], "mcp": {"url": "u", "token": "t"}});
        assert!(
            serve_next(&pageless(false), true)
                .unwrap()
                .starts_with("The MCP url and token are new.")
        );
        assert_eq!(serve_next(&pageless(true), true), None);
    }

    #[test]
    fn ac4_failed_check_mod_errors_give_recovery_next() {
        for (file, contents, expected_code) in [
            (
                "wit/world.wit",
                "package mods:demo@0.1.0; world demo { export interface; }",
                crate::outcome::Code::ModInvalid,
            ),
            (
                "logic/build.rs",
                "fn main() {}",
                crate::outcome::Code::ModLogicFile,
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            let dir = root.path().join("demo");
            template::new_mod(&dir, "demo").unwrap();
            std::fs::write(dir.join(file), contents).unwrap();
            let checked = run(Cli {
                json: true,
                dir: Some(dir.clone()),
                command: Command::Check,
            });
            assert!(!checked.ok);
            assert_eq!(checked.errors[0].code, expected_code);
            assert_eq!(
                checked.next.as_deref(),
                Some("Fix the errors, then run: watane-dev check --json")
            );
        }
    }

    #[test]
    fn ac4_install_next_uses_last_available_page() {
        assert_eq!(
            install_next(&[
                json!({"page":"http://a.localhost:7/"}),
                json!({"page":null})
            ])
            .as_deref(),
            Some("Give the person this page: http://a.localhost:7/")
        );
        assert_eq!(install_next(&[json!({"page":null})]), None);
        assert_eq!(install_next(&[]), None);
    }

    #[test]
    fn ac7_install_help_and_default_wait() {
        let cli = Cli::try_parse_from(["watane-dev", "install"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Install {
                home: None,
                wait: None
            }
        ));
        use clap::CommandFactory;
        let help = Cli::command()
            .find_subcommand_mut("install")
            .unwrap()
            .render_help()
            .to_string();
        assert!(help.contains("--home"));
        assert!(help.contains("--wait"));
    }
    #[test]
    fn ac8_template_explains_home_install() {
        let guide = include_str!("../template/AGENTS.md");
        for text in [
            "watane-dev install --json",
            "approves in the app",
            "env.host_not_running",
            "start the app",
        ] {
            assert!(guide.contains(text), "missing {text}");
        }
    }

    #[test]
    fn ac1_all_commands_and_arguments_have_help() {
        use clap::CommandFactory;
        fn walk(command: &clap::Command) {
            for arg in command.get_arguments() {
                if !matches!(arg.get_id().as_str(), "help" | "version") {
                    assert!(
                        arg.get_help().is_some(),
                        "{}: {}",
                        command.get_name(),
                        arg.get_id()
                    );
                }
            }
            for sub in command.get_subcommands() {
                assert!(sub.get_about().is_some(), "{}", sub.get_name());
                walk(sub);
            }
        }
        walk(&Cli::command());
        assert!(
            Cli::command()
                .find_subcommand_mut("serve")
                .unwrap()
                .render_help()
                .to_string()
                .contains("deletes its data")
        );
        assert_eq!(
            serve_next(&json!({"stopped":true}), true).as_deref(),
            Some(
                "The preview and its data are gone. To start a new one, run: watane-dev serve --json"
            )
        );
    }

    #[test]
    fn ac1_provider_failure_sets_next_and_all_fixes() {
        let root = tempfile::tempdir().unwrap();
        let consumer = root.path().join("consumer");
        let provider = root.path().join("provider");
        template::new_mod(&consumer, "consumer").unwrap();
        template::new_mod(&provider, "provider").unwrap();
        deps::add_local(&consumer, &provider).unwrap();
        std::fs::write(provider.join("logic/build.rs"), "fn main() {}").unwrap();
        let tested = test_local(&consumer.canonicalize().unwrap());
        let expected = format!(
            "watane-dev test --json --dir {}",
            provider.canonicalize().unwrap().display()
        );
        assert!(!tested.ok);
        assert_eq!(tested.next.as_deref(), Some(expected.as_str()));
        assert!(
            tested
                .errors
                .iter()
                .all(|p| p.fix.as_deref() == Some(expected.as_str()))
        );
    }

    #[test]
    fn provider_next_quotes_a_path_with_a_space() {
        let root = tempfile::tempdir().unwrap();
        let consumer = root.path().join("consumer");
        let provider = root.path().join("my provider");
        template::new_mod(&consumer, "consumer").unwrap();
        template::new_mod(&provider, "provider").unwrap();
        deps::add_local(&consumer, &provider).unwrap();
        std::fs::write(provider.join("logic/build.rs"), "fn main() {}").unwrap();
        let tested = test_local(&consumer.canonicalize().unwrap());
        let expected = format!(
            "watane-dev test --json --dir '{}'",
            provider.canonicalize().unwrap().display()
        );
        assert_eq!(tested.next.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn ac2_consumer_failure_keeps_local_next() {
        let root = tempfile::tempdir().unwrap();
        let consumer = root.path().join("consumer");
        template::new_mod(&consumer, "consumer").unwrap();
        std::fs::write(consumer.join("logic/build.rs"), "fn main() {}").unwrap();
        let tested = test_local(&consumer.canonicalize().unwrap());
        assert!(!tested.ok);
        assert_eq!(tested.next.as_deref(), Some("watane-dev test --json"));
    }

    #[test]
    fn ac2_first_failing_provider_decides_next() {
        let root = tempfile::tempdir().unwrap();
        let consumer = root.path().join("consumer");
        template::new_mod(&consumer, "consumer").unwrap();
        for slug in ["alpha", "beta"] {
            let provider = root.path().join(slug);
            template::new_mod(&provider, slug).unwrap();
            deps::add_local(&consumer, &provider).unwrap();
            std::fs::write(provider.join("logic/build.rs"), "fn main() {}").unwrap();
        }
        let tested = test_local(&consumer.canonicalize().unwrap());
        assert!(!tested.ok);
        assert_eq!(
            tested.next,
            Some(format!(
                "watane-dev test --json --dir {}",
                root.path().join("alpha").canonicalize().unwrap().display()
            ))
        );
        assert!(
            tested
                .errors
                .iter()
                .all(|p| p.message.starts_with("alpha: "))
        );
    }

    #[test]
    fn ac7a_next_uses_consumer_page_after_providers() {
        assert_eq!(
            serve_next(
                &json!({"pages":[
                    {"slug":"a","url":"http://a.localhost:123/"},
                    {"slug":"b","url":"http://b.localhost:123/"}
                ]}),
                true
            )
            .as_deref(),
            Some("Give the person this page: http://b.localhost:123/")
        );
        assert_eq!(
            serve_next(
                &json!({"pages":[
                    {"slug":"a","url":"http://a.localhost:123/"},
                    {"slug":"b","url":null}
                ]}),
                true
            ),
            None
        );
    }
}
