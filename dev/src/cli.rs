//! The command line: clap types and one function per command, each giving an
//! `Outcome` (TECHNICAL 2).

use crate::allowlist::{self, Allowlist};
use crate::builder::{self, BuildCtx};
use crate::env::Env;
use crate::outcome::{Outcome, Problem};
use crate::{check, contract, deps, preview, template, toolchain, unit};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
#[derive(Parser, Debug)]
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
#[derive(Subcommand, Debug)]
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
    Test {
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
    },
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
        #[arg(
            long,
            help = "Push the site and consented source with your own git and gh"
        )]
        push: bool,
        #[arg(long, requires = "push", allow_hyphen_values = true, value_parser = github_account, help = "GitHub account or organisation for the repositories dev creates [default: the gh login]")]
        owner: Option<String>,
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
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
    },
}
fn github_account(account: &str) -> Result<String, String> {
    if (1..=39).contains(&account.len())
        && account
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && !account.starts_with('-')
        && !account.ends_with('-')
        && !account.contains("--")
    {
        Ok(account.into())
    } else {
        Err(format!(
            "{account:?} is not a valid GitHub account: use 1..=39 ASCII letters or digits with single inner hyphens"
        ))
    }
}

/// What `main` does with its process (2.5), with the process given.
///
/// Parses `args` (the program name first), runs the command with `vars` over
/// the process's environment and `cwd` as the current folder (None: the
/// process's), writes the result to `out` and returns the exit status.
pub fn main_with(
    args: &[std::ffi::OsString],
    vars: &[(&str, Option<&std::ffi::OsStr>)],
    cwd: Option<&Path>,
    out: &mut dyn std::io::Write,
) -> u8 {
    use crate::outcome::Code;
    // A closed stdout (`| head -1`) ends the output, not the program with a panic.
    let mut emit = |text: &str| {
        let _ = out.write_all(text.as_bytes()).and_then(|()| out.flush());
    };
    let json = args.iter().any(|s| s == "--json");
    let outcome = match Cli::try_parse_from(args) {
        Ok(cli) => crate::env::with_vars(vars, || {
            run_in(
                cli,
                cwd.map_or_else(std::env::current_dir, |cwd| Ok(cwd.to_path_buf())),
            )
        }),
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            emit(&e.to_string());
            return 0;
        }
        Err(e) => {
            let command = args
                .iter()
                .filter_map(|s| s.to_str())
                .find_map(|s| match s {
                    "new" => Some("new"),
                    "check" => Some("check"),
                    "serve" => Some("serve"),
                    "doctor" => Some("doctor"),
                    "test" => Some("test"),
                    "add" => Some("add"),
                    "install" => Some("install"),
                    "publish" => Some("publish"),
                    "search" => Some("search"),
                    "help" => Some("help"),
                    _ => None,
                })
                .unwrap_or("watane-dev");
            let message = if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    | clap::error::ErrorKind::MissingSubcommand
            ) {
                "no command given: watane-dev needs one of new, check, test, serve, doctor, add, install, publish, search".to_string()
            } else {
                let text = e.to_string();
                let body: Vec<&str> = text
                    .lines()
                    .take_while(|line| !line.trim().is_empty())
                    .map(str::trim)
                    .collect();
                let joined = body.join(" ");
                joined
                    .strip_prefix("error: ")
                    .unwrap_or(&joined)
                    .to_string()
            };
            Outcome::new(command, None).fail(vec![
                Problem::new(Code::UsageArgs, message).fix("watane-dev --help"),
            ])
        }
    };
    if json {
        emit(&format!("{}\n", outcome.to_json()));
    } else {
        emit(&outcome.to_text());
    }
    outcome.exit_code()
}

pub fn run(cli: Cli) -> Outcome {
    run_in(cli, std::env::current_dir())
}
fn run_in(cli: Cli, cwd: std::io::Result<PathBuf>) -> Outcome {
    let rerun = fallback_rerun(&cli, cwd.as_ref().ok().map(PathBuf::as_path));
    end_in_rerun(run_command(cli, cwd), &rerun)
}

/// The command as it was given, as a rerun (`--dir` and `--home` only as given).
fn fallback_rerun(cli: &Cli, cwd: Option<&Path>) -> String {
    let word = |value: &str| quote(Path::new(value));
    let path = |path: &Path| match cwd {
        Some(cwd) => quote(&resolved_path(&cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())),
        None => quote(path),
    };
    let dir = cli
        .dir
        .as_deref()
        .map_or_else(String::new, |dir| format!(" --dir {}", path(dir)));
    let home_of = |home: &Option<PathBuf>| {
        home.as_deref()
            .map_or_else(String::new, |home| format!(" --home {}", path(home)))
    };
    match &cli.command {
        Command::New { slug, from, home } => format!(
            "watane-dev new{}{} --json{dir}{}",
            slug.as_deref()
                .map_or_else(String::new, |s| format!(" {}", word(s))),
            from.as_deref()
                .map_or_else(String::new, |f| format!(" --from {}", word(f))),
            home_of(home)
        ),
        Command::Check => format!("watane-dev check --json{dir}"),
        Command::Doctor => format!("watane-dev doctor --json{dir}"),
        Command::Test { home } => format!("watane-dev test --json{dir}{}", home_of(home)),
        Command::Serve { stop, home } => format!(
            "watane-dev serve{} --json{dir}{}",
            if *stop { " --stop" } else { "" },
            home_of(home)
        ),
        Command::Add {
            provider,
            release,
            home,
        } => format!(
            "watane-dev add{}{} --json{dir}{}",
            provider
                .as_deref()
                .map_or_else(String::new, |p| format!(" {}", quote(p))),
            release
                .as_deref()
                .map_or_else(String::new, |r| format!(" --release {}", word(r))),
            home_of(home)
        ),
        Command::Install { home, wait } => format!(
            "watane-dev install --json{dir}{}{}",
            home_of(home),
            wait.map_or_else(String::new, |w| format!(" --wait {w}"))
        ),
        Command::Publish {
            push,
            owner,
            no_source,
            home,
            wait,
        } => format!(
            "watane-dev publish --json{dir}{}{}{}{}{}",
            home_of(home),
            if *push { " --push" } else { "" },
            owner
                .as_deref()
                .filter(|_| *push)
                .map_or_else(String::new, |o| format!(" --owner {o}")),
            if *no_source { " --no-source" } else { "" },
            wait.map_or_else(String::new, |w| format!(" --wait {w}"))
        ),
        Command::Search {
            text,
            publisher,
            interface,
            home,
        } => search_command(
            text,
            publisher.as_deref(),
            interface.as_deref(),
            &home_of(home),
        ),
    }
}

/// Fixes that end in the command's rerun where the code that made them does not
/// know it (`build.compile`, `build.reads_outside`, `internal`): `<fix>, then run: <rerun>`.
fn end_in_rerun(mut outcome: Outcome, rerun: &str) -> Outcome {
    use crate::outcome::Code;
    for error in &mut outcome.errors {
        if !matches!(
            error.code,
            Code::Internal | Code::BuildCompile | Code::BuildReadsOutside
        ) {
            continue;
        }
        let Some(old) = error.fix.clone().filter(|fix| !fix.contains(", then run:")) else {
            continue;
        };
        let new = format!("{old}, then run: {rerun}");
        if outcome.next.as_ref() == Some(&old) {
            outcome.next = Some(new.clone());
        }
        error.fix = Some(new);
    }
    outcome
}

fn run_command(cli: Cli, cwd: std::io::Result<PathBuf>) -> Outcome {
    let name = match cli.command {
        Command::New { .. } => "new",
        Command::Check => "check",
        Command::Doctor => "doctor",
        Command::Test { .. } => "test",
        Command::Add { .. } => "add",
        Command::Serve { .. } => "serve",
        Command::Install { .. } => "install",
        Command::Publish { .. } => "publish",
        Command::Search { .. } => "search",
    };
    let cwd = match cwd {
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
    let dir_given = cli.dir.is_some();
    let home_given = match &cli.command {
        Command::Test { home }
        | Command::Serve { home, .. }
        | Command::Install { home, .. }
        | Command::Publish { home, .. } => home.clone(),
        _ => None,
    };
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
    // ` --dir <mod>` and ` --home <home>` for the reruns of test and serve, only as given.
    let args = rerun_args(dir_given, &dir, home_given);
    let mut outcome = Outcome::new(name, Some(dir.clone()));
    let result = match cli.command {
        Command::Search { .. } => unreachable!("search needs no mod folder"),
        Command::Doctor => return crate::doctor::doctor(&dir),
        Command::Install { home, wait } => return install(&dir, home, wait, &args),
        Command::Publish { no_source, push, owner, home, wait } => return publish(&dir, no_source, push, owner.as_deref(), home, wait, &args),
        Command::New { slug, .. } => template::new_mod(&dir, slug.as_deref().expect("clap requires a slug"))
            .map_err(|e| vec![e])
            .and_then(|report| serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)]))
            .inspect(|_| {
                outcome.next = Some(format!("Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev serve --json{args}"));
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
        Command::Test { .. } => {
            let mut tested = test_with(&dir, &args);
            end_prose_in_rerun(&mut tested.errors, &mut tested.next, "test", &args);
            return tested;
        }
        Command::Add { release: Some(target), home, .. } => {
            let result = (|| {
                let given = home.is_some();
                let rerun = |home: &str| format!("watane-dev add --release {} --json --dir {}{home}", quote(Path::new(&target)), quote(&dir));
                let home = app_home(home, &rerun)?;
                let shown = home_arg(given, &home);
                let client = crate::api::Client::for_app(&home, &rerun(&shown)).map_err(|e| {
                    if e.code == crate::outcome::Code::EnvHostNotRunning {
                        outcome.next = Some(rerun(&shown));
                    }
                    vec![e]
                })?;
                let env = Env::from_process().map_err(|e|vec![e])?;
                crate::build_on::add_release(&dir, &target, &client, &env, &shown)
            })();
            // A failed fetch has no lines to copy: its fix is the next step.
            return match result {
                Ok(v) => {
                    outcome.next = Some(format!("Copy the lines that the mod needs from result.imports and result.subscriptions into the world in wit/world.wit (result.subscriptions holds `export` lines: they are how the mod answers the provider's events and collects; each line is a grant that the participant approves; contract tests use a fake of it, see AGENTS.md), then run: watane-dev test --json --dir {}", quote(&dir)));
                    outcome.result = Some(v);
                    outcome
                }
                Err(e) => {
                    if outcome.next.is_none() {
                        outcome.next = e.first().and_then(|p| p.fix.clone());
                    }
                    outcome.fail(e)
                }
            };
        }
        Command::Add { provider, .. } => deps::add_local(&dir, provider.as_deref().expect("clap requires a provider"))
            .and_then(|report| serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)]))
            .inspect(|_| {
                outcome.dir = dir.canonicalize().ok();
                outcome.next = Some(format!("Copy the lines that the mod needs from result.imports and result.subscriptions into the world in wit/world.wit (result.subscriptions holds `export` lines: they are how the mod answers the provider's events and collects; each line is a grant that the participant approves), then run: watane-dev test --json --dir {}", quote(outcome.dir.as_deref().unwrap_or(&dir))));
            }),
        Command::Serve { stop, .. } => dir
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
                    let mut tested = test_local(&dir);
                    if !tested.ok {
                        outcome.next = if tested.dir.as_deref() == Some(dir.as_path()) {
                            let next = failed_test_next(&tested.errors, "serve", &args);
                            for error in &mut tested.errors {
                                error.fix = rewritten_fix(error, "serve", &args);
                            }
                            let mut next = Some(next);
                            end_prose_in_rerun(&mut tested.errors, &mut next, "serve", &args);
                            next
                        } else {
                            tested.next
                        };
                        if let Some(warning) = preview::previous_warning(&dir, &env, &args) {
                            outcome.warnings.push(warning);
                        }
                        return Err(tested.errors);
                    }
                    preview::serve(&dir, &env, &format!("watane-dev serve --json{args}"))
                }
            }),
    };
    match result {
        Ok(v) => {
            if name == "serve" {
                outcome.next = serve_next(&v, cli.json, &args);
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
                outcome.next = Some(format!(
                    "Fix the errors, then run: watane-dev check --json{args}"
                ));
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
        let given = home.is_some();
        let rerun = |home: &str| {
            let dir = destination
                .as_deref()
                .map_or_else(String::new, |dir| format!(" --dir {}", quote(dir)));
            let slug = slug.map_or_else(String::new, |slug| format!(" {slug}"));
            format!(
                "watane-dev new{slug} --from {} --json{dir}{home}",
                quote(Path::new(target))
            )
        };
        let home = app_home(home, &rerun)?;
        let shown = home_arg(given, &home);
        let client = crate::api::Client::for_app(&home, &rerun(&shown)).map_err(|e| {
            if e.code == crate::outcome::Code::EnvHostNotRunning {
                outcome.next = Some(rerun(&shown));
            }
            vec![e]
        })?;
        let env = Env::from_process().map_err(|e| vec![e])?;
        crate::build_on::new_from(destination.as_deref(), slug, target, &client, &env, &shown)
    })();
    match result {
        Ok((value, warnings)) => {
            outcome.warnings.extend(warnings);
            let from = value["from_slug"].as_str().expect("from slug");
            let slug = value["slug"].as_str().expect("slug");
            outcome.dir = value["dir"].as_str().map(PathBuf::from);
            let prebuilt = value["files"].as_array().is_some_and(|files| {
                files.contains(&serde_json::json!("logic.wasm"))
                    && !files.contains(&serde_json::json!("logic/Cargo.toml"))
            });
            if prebuilt && from != slug {
                outcome.warnings.push(Problem::new(
                    crate::outcome::Code::NewPrebuiltRenamed,
                    format!("logic.wasm was built for {from}, not {slug}: it will fail the contract tests until the mod has logic source"),
                ).fix(format!("Write the logic source under logic/ (logic/Cargo.toml and logic/src) for {slug}, or run new --from again with the slug {from}")));
            }

            let mentions = value["mentions"].as_array().is_some_and(|v| !v.is_empty());
            let replace = replace_text(from, slug);
            let step = match (prebuilt, from != slug, mentions) {
                (false, _, true) => format!("{replace}, then run"),
                (false, _, false) => {
                    "Edit wit/world.wit, logic/src/lib.rs and ui/, then run".into()
                }
                (true, false, _) => "Edit wit/world.wit and ui/ (logic.wasm is prebuilt and keeps its exports), then run".into(),
                (true, true, true) => format!("{replace}, write the logic source under logic/ (logic/Cargo.toml and logic/src) for {slug}, then run"),
                (true, true, false) => format!("Write the logic source under logic/ (logic/Cargo.toml and logic/src) for {slug}, then run"),
            };
            let dir = outcome.dir.clone().unwrap_or_default();
            outcome.next = Some(format!(
                "{step}: watane-dev test --json --dir {}",
                quote(&dir)
            ));
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

/// The rename step; the snake form is named only when it differs from the slug.
fn replace_text(from: &str, slug: &str) -> String {
    let (from_, slug_) = (from.replace('-', "_"), slug.replace('-', "_"));
    let snake = if from_ == from && slug_ == slug {
        String::new()
    } else {
        format!(" (and {from_} with {slug_})")
    };
    format!("Replace {from} with {slug}{snake} at each result.mentions entry")
}

fn serve_next(result: &serde_json::Value, json: bool, args: &str) -> Option<String> {
    if result["stopped"] == true {
        return Some(format!(
            "The preview and its data are gone. To start a new one, run: watane-dev serve --json{args}"
        ));
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

/// ` --dir <mod>` (when the command was given `--dir`) and ` --home <home>`
/// (when given `--home`), which end every `test` and `serve` rerun.
fn rerun_args(dir_given: bool, dir: &Path, home: Option<PathBuf>) -> String {
    let dir = if dir_given {
        format!(" --dir {}", quote(dir))
    } else {
        String::new()
    };
    let home = home.map_or_else(String::new, |home| {
        let home = resolved_path(&home).unwrap_or(home);
        home_arg(true, &home)
    });
    format!("{dir}{home}")
}

/// The fix of a failing test error, rerun with `command` instead of `test`,
/// and with the `args` of the command line (`rerun_args`).
fn rewritten_fix(error: &Problem, command: &str, args: &str) -> Option<String> {
    use crate::outcome::Code;
    let fix = error.fix.as_ref()?;
    Some(match error.code {
        Code::TestUnit | Code::TestUnitLimit | Code::TestContract => {
            fix.strip_suffix("watane-dev test --json").map_or_else(
                || fix.clone(),
                |prefix| format!("{prefix}watane-dev {command} --json{args}"),
            )
        }
        // The fix names the app's release add: it goes to this mod and this home.
        Code::ModReleaseProviderMissing => format!("{fix}{args}"),
        _ => fix.clone(),
    })
}

/// A failed `test` or `serve` whose fix is prose (a check error before any build)
/// ends in the command's own rerun, as its build failures do.
fn end_prose_in_rerun(
    errors: &mut [Problem],
    next: &mut Option<String>,
    command: &str,
    args: &str,
) {
    if errors
        .first()
        .is_some_and(|e| e.code.class() == crate::outcome::Class::Env)
    {
        return;
    }
    for error in errors {
        let Some(old) = error.fix.clone().filter(|fix| !fix.contains("then run:")) else {
            continue;
        };
        let new = format!("{old}, then run: watane-dev {command} --json{args}");
        if next.as_ref() == Some(&old) {
            *next = Some(new.clone());
        }
        error.fix = Some(new);
    }
}

fn failed_test_next(errors: &[Problem], command: &str, args: &str) -> String {
    errors
        .first()
        .and_then(|error| {
            let fix = rewritten_fix(error, command, args)?;
            // Without the app, `next` is the command alone: the fix names the step, then it.
            match (
                error.code,
                fix.strip_prefix("Start the watane app, then run: "),
            ) {
                (crate::outcome::Code::ModReleaseProviderMissing, Some(rerun)) => {
                    Some(rerun.to_string())
                }
                _ => Some(fix),
            }
        })
        .unwrap_or_else(|| format!("watane-dev {command} --json{args}"))
}

/// Check, build and run unit and contract tests, retaining completed parts on failure.
pub fn test(dir: &Path) -> Outcome {
    test_with(dir, "")
}

fn test_with(dir: &Path, args: &str) -> Outcome {
    let mut outcome = Outcome::new("test", Some(dir.to_path_buf()));
    outcome.next = Some(format!("watane-dev test --json{args}"));
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
        let mut errors = Vec::new();
        if dir.join("logic/Cargo.toml").is_file() {
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
            result["build"] =
                serde_json::to_value(built).map_err(|e| vec![Problem::internal(e)])?;
            let report = unit::unit_tests(&dir, m.slug(), &ctx)?;
            errors.extend(report.problems());
            result["unit"] =
                serde_json::to_value(report).map_err(|e| vec![Problem::internal(e)])?;
        }
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
    let mut errors = pipeline().unwrap_or_else(|errors| errors);
    if !result["check"].is_null() {
        outcome.result = Some(result);
    }
    if errors.is_empty() {
        outcome.next = Some(format!("watane-dev serve --json{args}"));
        outcome
    } else {
        if errors
            .first()
            .is_some_and(|p| p.code.class() == crate::outcome::Class::Env)
        {
            outcome.next = Some("watane-dev doctor --json".into());
        } else {
            outcome.next = Some(failed_test_next(&errors, "test", args));
            for error in &mut errors {
                error.fix = rewritten_fix(error, "test", args);
            }
        }
        outcome.fail(errors)
    }
}

/// The search command as a rerun: words, filters, and `--home` only when given (TECHNICAL 2.4).
fn search_command(
    words: &[String],
    publisher: Option<&str>,
    interface: Option<&str>,
    home: &str,
) -> String {
    let mut next = String::from("watane-dev search --json");
    // A word that starts with `-` would read as a flag: filters first, then `--`.
    let dashed = words.iter().any(|w| w.starts_with('-'));
    let words_text: String = words
        .iter()
        .map(|w| format!(" {}", quote(Path::new(w))))
        .collect();
    if !dashed {
        next.push_str(&words_text);
    }
    for (flag, value) in [("publisher", publisher), ("interface", interface)] {
        if let Some(value) = value {
            // A value that starts with `-` would read as a flag: the `=` form binds it.
            let join = if value.starts_with('-') { '=' } else { ' ' };
            next.push_str(&format!(" --{flag}{join}{}", quote(Path::new(value))));
        }
    }
    next.push_str(home);
    if dashed {
        next.push_str(" --");
        next.push_str(&words_text);
    }
    next
}

fn search(
    words: &[String],
    publisher: Option<&str>,
    interface: Option<&str>,
    home: Option<PathBuf>,
) -> Outcome {
    let mut outcome = Outcome::new("search", None);
    let given = home.is_some();
    let result: Result<serde_json::Value, Vec<Problem>> = (|| {
        let rerun = |home: &str| search_command(words, publisher, interface, home);
        let home = app_home(home, &rerun)?;
        let shown = home_arg(given, &home);
        let rerun = rerun(&shown);
        let client = crate::api::Client::for_app(&home, &rerun).map_err(|e| {
            outcome.next = Some(rerun.clone());
            vec![e]
        })?;
        let answer = client
            .search(&words.join(" "), publisher, interface)
            .map_err(|e| vec![e])?;
        // A quoted argument of two words is two words.
        let split: Vec<&str> = words.iter().flat_map(|w| w.split_whitespace()).collect();
        if answer["results"].as_array().is_some_and(Vec::is_empty) {
            outcome.next = Some(if answer["indexes"] == 0 {
                format!(
                    "Ask the person to add an index in the watane app, then run: {}",
                    rerun
                )
            } else if split.len() >= 2 {
                format!(
                    "Try fewer words: {}",
                    search_command(&[split[0].to_string()], publisher, interface, &shown)
                )
            } else {
                "Try other words, or search with --interface <name>: watane-dev search --json <other words>".into()
            });
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

fn install(dir: &Path, home: Option<PathBuf>, wait: Option<u64>, args: &str) -> Outcome {
    let mut outcome = Outcome::new("install", Some(dir.to_path_buf()));
    let result = (|| {
        let shown_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let rerun = |home: &str, wait: Option<u64>| {
            let mut next = format!(
                "watane-dev install --json --dir {}{home}",
                quote(&shown_dir)
            );
            if let Some(wait) = wait {
                next.push_str(&format!(" --wait {wait}"));
            }
            next
        };
        let (home, client) = app_client(&mut outcome, home, &rerun, wait)?;
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
            .install_wait(&folders, wait.unwrap_or(DEFAULT_WAIT))
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
                    .or_else(|| Some(format!("watane-dev test --json{args}")));
            }
            outcome.fail(errors)
        }
    }
}

/// ` --home <resolved>` for a rerun, only when the command was given `--home`.
fn home_arg(given: bool, home: &Path) -> String {
    if given {
        format!(" --home {}", quote(home))
    } else {
        String::new()
    }
}

/// Quotes a path for a POSIX shell `next` line.
pub(crate) fn quote(path: &Path) -> String {
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

/// The command's rerun for a given ` --home <h>` argument (or none).
type Rerun<'a> = &'a dyn Fn(&str) -> String;

/// The fix of a missing home: pass the folder, then run the rerun with a placeholder.
fn app_home(home: Option<PathBuf>, rerun: Rerun) -> Result<PathBuf, Vec<Problem>> {
    let home = home
        .map(Ok)
        .unwrap_or_else(watane_host::default_home)
        .map_err(|e| vec![Problem::internal(e)])?;
    resolved_path(&home).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            let missing = home.ancestors().find(|p| {
                p.file_name().is_some()
                    && !p.exists()
                    && p.parent()
                        .is_some_and(|p| p.as_os_str().is_empty() || p.exists())
            });
            let detail = missing.map_or_else(
                || e.to_string(),
                |p| format!("{} does not exist", p.display()),
            );
            vec![
                Problem::new(
                    crate::outcome::Code::EnvHostNotRunning,
                    format!(
                        "no watane app runs for the home {} ({detail})",
                        home.display()
                    ),
                )
                .fix(format!(
                    "Pass --home the folder that the watane app uses ({detail}), then run: {}",
                    rerun(" --home <folder>")
                )),
            ]
        } else {
            vec![Problem::internal(e)]
        }
    })
}

/// The command's rerun for a given ` --home <h>` argument and `--wait` (or none).
type AppRerun<'a> = &'a dyn Fn(&str, Option<u64>) -> String;
/// The seconds that `install` and `publish` wait for the person without `--wait`.
const DEFAULT_WAIT: u64 = 600;

/// One rule for a longer wait: double the wait in force (no `--wait` counts as 600).
fn longer_wait(wait: Option<u64>) -> u64 {
    wait.unwrap_or(DEFAULT_WAIT).saturating_mul(2)
}

fn app_client(
    outcome: &mut Outcome,
    home: Option<PathBuf>,
    rerun: AppRerun,
    wait: Option<u64>,
) -> Result<(PathBuf, crate::api::Client), Vec<Problem>> {
    let given = home.is_some();
    let home = app_home(home, &|home: &str| rerun(home, wait))?;
    let shown = home_arg(given, &home);
    let plain = rerun(&shown, wait);
    let longer = rerun(&shown, Some(longer_wait(wait)));
    let client = crate::api::Client::for_app(&home, &plain)
        .map_err(|e| {
            if e.code == crate::outcome::Code::EnvHostNotRunning {
                outcome.next = Some(plain.clone());
            }
            vec![e]
        })?
        .with_longer_wait(&longer);
    Ok((home, client))
}

fn publish(
    dir: &Path,
    no_source: bool,
    push: bool,
    requested_owner: Option<&str>,
    home: Option<PathBuf>,
    wait: Option<u64>,
    args: &str,
) -> Outcome {
    use crate::outcome::{Class, Code};
    let mut outcome = Outcome::new("publish", Some(dir.to_path_buf()));
    let home_given = home.is_some();
    let result = (|| {
        let providers = watane_pkg::Deps::read(dir).map_err(check::mod_problems)?;
        let local = deps::read_local(dir).unwrap_or_default();
        let errors: Vec<_> = providers
            .iter()
            .filter(|(_, publisher)| publisher.is_none())
            .map(|(slug, _)| {
                let names = local.get(&slug.to_string()).map_or_else(
                    || "no release for it".to_string(),
                    |path| {
                        let path = dir.join(path);
                        let path = path.canonicalize().unwrap_or(path);
                        format!("its folder {}, not a release", path.display())
                    },
                );
                Problem::new(
                    Code::PublishLocalDependency,
                    format!("mods:{slug} is a local provider (deps.json names {names}), and a published mod can depend only on released providers"),
                )
                .at("deps.json", None, None)
                .fix(format!("Publish {slug} first (if it is already published, that run reports its release), then depend on its release"))
            })
            .collect();
        if !errors.is_empty() {
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
                    if let Some(owner) = requested_owner {
                        next.push_str(&format!(" --push --owner {owner}"));
                    }
                    next
                });
            return Err(errors);
        }
        let rerun_for = |home: &str, wait: Option<u64>| {
            let mut rerun = format!("watane-dev publish --json --dir {}{home}", quote(dir));
            if push {
                rerun.push_str(" --push");
                if let Some(owner) = requested_owner {
                    rerun.push_str(&format!(" --owner {owner}"));
                }
            }
            if no_source {
                rerun.push_str(" --no-source");
            }
            if let Some(wait) = wait {
                rerun.push_str(&format!(" --wait {wait}"));
            }
            rerun
        };
        let (home, client) = app_client(&mut outcome, home, &rerun_for, wait)?;
        let rerun = rerun_for(&home_arg(home_given, &home), wait);
        let retry_next = |code| match code {
            Code::HostNoAnswer => Some(format!(
                "Run again with a longer --wait: {}",
                rerun_for(&home_arg(home_given, &home), Some(longer_wait(wait)))
            )),
            Code::PublishVersionExists => Some(format!(
                "Raise the version in wit/world.wit, then run: {rerun}"
            )),
            _ => None,
        };
        let releases = client.releases().map_err(|e| {
            outcome.next = retry_next(e.code).or_else(|| e.fix.clone());
            vec![e]
        })?;
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
            outcome.next = retry_next(Code::PublishVersionExists);
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
        let login = if push {
            Some(crate::share::owner(dir, &rerun).map_err(|e| vec![e])?)
        } else {
            None
        };
        let owner = login.as_deref().map(|login| {
            requested_owner
                .filter(|owner| !owner.eq_ignore_ascii_case(login))
                .unwrap_or(login)
        });
        let prepared = if let Some(owner) = &owner
            && !no_source
        {
            let (prepared, warnings) = crate::share::prepare_mod(
                dir,
                owner,
                login.as_deref().expect("an owner implies a login"),
                &rerun,
            )?;
            outcome.warnings.extend(warnings);
            Some(prepared)
        } else {
            None
        };
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
        let timeout = publish_timeout(&providers, &releases, wait.unwrap_or(DEFAULT_WAIT));
        let answer = client.publish_with_links(&package, timeout, &links)
            .map_err(|mut e| {
                if e.code == Code::HostDeclined {
                    let slug = deps::label(dir);
                    e.message = format!("The person declined publishing {slug} from their watane app");
                    e.fix = Some(format!("Ask the person whether they want {slug} published, then run publish again with --wait <seconds>"));
                }
                outcome.next = retry_next(e.code).or_else(|| e.fix.clone());
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
        outcome.next = Some(match answer["link"].as_str() {
            None => format!(
                "Ask the person to set your site's URL in the watane app, then run: {rerun}"
            ),
            Some(link) => format!(
                "Put the site folder {} on your static web host, then give the person this link: {link}",
                answer["site"].as_str().unwrap_or_default()
            ),
        });
        let mut value = serde_json::json!({"home": home, "release": answer["release"], "site": answer["site"], "link": answer["link"], "package": package.id(), "source": package.has_source(), "providers": listed});
        if let Some(owner) = owner {
            let (pushed, errors) = crate::share::push(
                dir,
                Path::new(answer["site"].as_str().unwrap_or_default()),
                &package,
                owner,
                answer["release"].as_str().unwrap_or_default(),
                prepared,
            );
            value["push"] = pushed;
            if !errors.is_empty() {
                outcome.next = Some(
                    errors
                        .iter()
                        .filter_map(|e| e.fix.as_deref())
                        .collect::<Vec<_>>()
                        .join(" && "),
                );
                outcome.result = Some(value);
                return Err(errors);
            }
            if let Some(link) = answer["link"].as_str() {
                outcome.next = Some(format!("Give the person this link to share: {link}"));
            } else if let Some(raw) = value["push"]["site"]["raw"].as_str() {
                outcome.next = Some(push_site_next(
                    raw,
                    value["push"]["site"]["pages"].as_str(),
                    &rerun,
                ));
            }
        }
        Ok(value)
    })();
    match result {
        Ok(value) => {
            outcome.result = Some(value);
            outcome
        }
        Err(errors) => {
            if errors.first().is_some_and(|e| {
                e.code.class() == Class::Env
                    && e.code != Code::EnvHostNotRunning
                    && !matches!(
                        e.code,
                        Code::EnvGitMissing
                            | Code::EnvGhMissing
                            | Code::EnvGhAuth
                            | Code::EnvGitIdentity
                    )
            }) {
                outcome.next = Some("watane-dev doctor --json".into());
            }
            if outcome.next.is_none() {
                outcome.next = errors
                    .first()
                    .and_then(|e| e.fix.clone())
                    .or_else(|| Some(format!("watane-dev test --json{args}")));
            }
            outcome.fail(errors)
        }
    }
}

/// The success `next` of `publish --push` without a link (TECHNICAL 10.4.1).
fn push_site_next(raw: &str, pages: Option<&str>, rerun: &str) -> String {
    let pages = pages.map_or_else(String::new, |pages| {
        format!(" (or to {pages} if GitHub Pages serves this repository)")
    });
    format!(
        "Ask the person to set the site URL in the watane app to {raw}{pages}, then run: {rerun}"
    )
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
    fn s93_ac7_push_next_offers_pages_beside_raw() {
        assert_eq!(
            push_site_next(
                "https://raw.githubusercontent.com/o/r/main/",
                Some("https://o.github.io/r/"),
                "watane-dev publish --json --push"
            ),
            "Ask the person to set the site URL in the watane app to https://raw.githubusercontent.com/o/r/main/ (or to https://o.github.io/r/ if GitHub Pages serves this repository), then run: watane-dev publish --json --push"
        );
        assert_eq!(
            push_site_next("https://raw.githubusercontent.com/o/r/main/", None, "x"),
            "Ask the person to set the site URL in the watane app to https://raw.githubusercontent.com/o/r/main/, then run: x"
        );
    }

    #[test]
    fn s93_ac2_fixes_that_lacked_a_rerun_end_in_one() {
        use crate::outcome::Code;
        let mut outcome = Outcome::new("test", None).fail(vec![
            Problem::new(Code::BuildCompile, "bad")
                .fix("Correct the Rust error at the reported location"),
            Problem::internal("disk"),
            Problem::new(Code::BuildReadsOutside, "bad")
                .fix("Remove the include_bytes! that names it"),
            Problem::new(Code::TestUnit, "bad").fix("Fix it, then run: watane-dev test --json"),
        ]);
        outcome.next = outcome.errors[0].fix.clone();
        let outcome = end_in_rerun(outcome, "watane-dev test --json --dir /m");
        let fixes: Vec<_> = outcome
            .errors
            .iter()
            .map(|e| e.fix.as_deref().unwrap())
            .collect();
        assert_eq!(
            fixes,
            [
                "Correct the Rust error at the reported location, then run: watane-dev test --json --dir /m",
                "Check the paths and permissions, then run: watane-dev test --json --dir /m",
                "Remove the include_bytes! that names it, then run: watane-dev test --json --dir /m",
                "Fix it, then run: watane-dev test --json",
            ]
        );
        assert_eq!(outcome.next.as_deref(), Some(fixes[0]));
    }

    #[test]
    fn s93_ac2_every_command_has_the_rerun_it_was_given_as() {
        let cwd = std::path::Path::new("/w");
        let rerun = |args: &[&str]| {
            let cli =
                Cli::try_parse_from(std::iter::once("watane-dev").chain(args.iter().copied()))
                    .unwrap();
            fallback_rerun(&cli, Some(cwd))
        };
        assert_eq!(
            rerun(&["check", "--dir", "m x"]),
            "watane-dev check --json --dir '/w/m x'"
        );
        assert_eq!(
            rerun(&["test", "--home", "/h"]),
            "watane-dev test --json --home /h"
        );
        assert_eq!(
            rerun(&["serve", "--stop"]),
            "watane-dev serve --stop --json"
        );
        assert_eq!(
            rerun(&["new", "a", "--from", "k/p@1.0.0"]),
            "watane-dev new a --from 'k/p@1.0.0' --json"
        );
        assert_eq!(
            rerun(&["add", "../p", "--dir", "m"]),
            "watane-dev add ../p --json --dir /w/m"
        );
        assert_eq!(
            rerun(&[
                "publish",
                "--push",
                "--owner",
                "bob",
                "--no-source",
                "--wait",
                "5"
            ]),
            "watane-dev publish --json --push --owner bob --no-source --wait 5"
        );
        assert_eq!(
            rerun(&["install", "--wait", "5"]),
            "watane-dev install --json --wait 5"
        );
        assert_eq!(
            rerun(&["search", "--publisher=-x", "w"]),
            "watane-dev search --json w --publisher=-x"
        );
    }

    #[test]
    fn s93_ac2_check_time_failures_of_test_and_serve_end_in_their_rerun() {
        use crate::outcome::Code;
        for (command, args) in [("test", ""), ("serve", " --dir /m --home /h")] {
            let mut errors = vec![
                Problem::new(Code::ModInvalid, "bad")
                    .fix("Correct mod.toml according to the reported rule"),
            ];
            let mut next = errors[0].fix.clone();
            end_prose_in_rerun(&mut errors, &mut next, command, args);
            let fix = format!(
                "Correct mod.toml according to the reported rule, then run: watane-dev {command} --json{args}"
            );
            assert_eq!(errors[0].fix.as_deref(), Some(fix.as_str()));
            assert_eq!(next, Some(fix));
        }
    }

    #[test]
    fn s93_ac2_no_app_next_is_the_rerun_for_add_and_new_from() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().canonicalize().unwrap();
        let dir = home.join("m");
        template::new_mod(&dir, "m").unwrap();
        let rerun = format!(
            "watane-dev add --release 'k/p@0.1.0' --json --dir {} --home {}",
            dir.display(),
            home.display()
        );
        let outcome = run(Cli {
            json: true,
            dir: Some(dir.clone()),
            command: Command::Add {
                provider: None,
                release: Some("k/p@0.1.0".into()),
                home: Some(home.clone()),
            },
        });
        assert_eq!(
            outcome.errors[0].fix,
            Some(format!("Start the watane app, then run: {rerun}"))
        );
        assert_eq!(outcome.next, Some(rerun));
        let target = home.join("fresh");
        let rerun = format!(
            "watane-dev new --from 'k/p@0.1.0' --json --dir {} --home {}",
            target.display(),
            home.display()
        );
        let outcome = run(Cli {
            json: true,
            dir: Some(target),
            command: Command::New {
                slug: None,
                from: Some("k/p@0.1.0".into()),
                home: Some(home),
            },
        });
        assert_eq!(
            outcome.errors[0].fix,
            Some(format!("Start the watane app, then run: {rerun}"))
        );
        assert_eq!(outcome.next, Some(rerun));
    }

    #[test]
    fn s93_ac2_new_next_keeps_the_dir_it_was_given() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().canonicalize().unwrap().join("made");
        let outcome = run(Cli {
            json: true,
            dir: Some(dir.clone()),
            command: Command::New {
                slug: Some("made".into()),
                from: None,
                home: None,
            },
        });
        assert!(outcome.ok, "{:?}", outcome.errors);
        assert_eq!(
            outcome.next,
            Some(format!(
                "Edit wit/world.wit, logic/src/lib.rs and ui/, then run: watane-dev serve --json --dir {}",
                dir.display()
            ))
        );
    }

    #[test]
    fn s93_longer_wait_doubles_the_wait_in_force() {
        assert_eq!(longer_wait(Some(17)), 34);
        assert_eq!(longer_wait(None), 1200);
        assert_eq!(longer_wait(Some(u64::MAX)), u64::MAX);
    }

    #[test]
    fn s93_ac3_search_rerun_parses_a_filter_value_that_starts_with_a_dash() {
        for (publisher, interface) in [("-x", None), ("-x", Some("-v")), ("ok", Some("-v"))] {
            let line = search_command(&["word".to_string()], Some(publisher), interface, "");
            let expected_publisher = format!(
                " --publisher{}{publisher}",
                if publisher.starts_with('-') { "=" } else { " " }
            );
            assert!(line.contains(&expected_publisher), "{line}");
            let argv = line.replace('\'', "");
            let cli = Cli::try_parse_from(argv.split(' ').filter(|a| !a.is_empty()));
            let cli = cli.unwrap_or_else(|e| panic!("{line}: {e}"));
            match cli.command {
                Command::Search {
                    publisher: p,
                    interface: i,
                    text,
                    ..
                } => {
                    assert_eq!(p.as_deref(), Some(publisher));
                    assert_eq!(i.as_deref(), interface);
                    assert_eq!(text, ["word"]);
                }
                _ => panic!("not search"),
            }
        }
        assert_eq!(
            search_command(&[], Some("-x"), None, ""),
            "watane-dev search --json --publisher=-x"
        );
    }

    #[test]
    fn ac1_search_rerun_guards_dash_words_with_double_dash() {
        let line = search_command(&["--home=/x".to_string()], None, None, "");
        assert_eq!(line, "watane-dev search --json -- '--home=/x'");
        let argv = line.replace('\'', "");
        let cli = Cli::try_parse_from(argv.split(' ')).unwrap();
        match cli.command {
            Command::Search { text, home, .. } => {
                assert_eq!(text, ["--home=/x"]);
                assert!(home.is_none());
            }
            _ => panic!("not search"),
        }
        let words = ["cats".to_string(), "-x".to_string()];
        let line = search_command(&words, Some("p"), None, " --home /h");
        assert_eq!(
            line,
            "watane-dev search --json --publisher p --home /h -- cats -x"
        );
        let argv = line.replace('\'', "");
        let cli = Cli::try_parse_from(argv.split(' ')).unwrap();
        match cli.command {
            Command::Search {
                text,
                home,
                publisher,
                ..
            } => {
                assert_eq!(text, ["cats", "-x"]);
                assert_eq!(home, Some(PathBuf::from("/h")));
                assert_eq!(publisher.as_deref(), Some("p"));
            }
            _ => panic!("not search"),
        }
        let plain = ["cats".to_string()];
        assert_eq!(
            search_command(&plain, Some("p"), None, " --home /h"),
            "watane-dev search --json cats --publisher p --home /h"
        );
    }

    #[test]
    fn ac5_owner_documentation_and_ac6_cached_timing_contract() {
        let guide = include_str!("../template/AGENTS.md");
        assert!(guide.contains("--owner <org>"));
        let functional = include_str!("../FUNCTIONAL.md");
        assert!(functional.contains("--owner <account>"));
        let technical = include_str!("../TECHNICAL.md");
        let publishing = technical.split("#### 10.4.1").nth(1).unwrap();
        assert!(publishing.contains("--owner <account>"));
        assert!(publishing.contains("usage.args"));
        let fork = technical.split("#### 10.4.2").nth(1).unwrap();
        assert!(fork.contains("organization=<owner>"));
        let row = technical
            .lines()
            .find(|line| line.starts_with("| DEV-S-16 |"))
            .unwrap();
        assert!(row.contains("second, build-cached run"));
    }

    #[test]
    fn ac1_publish_owner_cli_validation_and_help() {
        use clap::CommandFactory;
        for account in ["a", "Acme-123", &"a".repeat(39)] {
            Cli::try_parse_from(["watane-dev", "publish", "--push", "--owner", account]).unwrap();
        }
        let err = Cli::try_parse_from(["watane-dev", "publish", "--owner", "acme"])
            .err()
            .unwrap();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("--push"));
        for account in ["", "-acme", "acme-", "ac--me", "a_b", "ä", &"a".repeat(40)] {
            let err = Cli::try_parse_from(["watane-dev", "publish", "--push", "--owner", account])
                .err()
                .unwrap();
            assert_eq!(err.exit_code(), 2);
            assert!(err.to_string().contains(account));
        }
        let help = Cli::command()
            .find_subcommand_mut("publish")
            .unwrap()
            .render_help()
            .to_string();
        assert!(help.contains("GitHub account or organisation for the repositories dev creates [default: the gh login]"));
    }

    #[test]
    fn ac6_serve_recovery_preserves_command_text_inside_contract_filename() {
        let error = Problem::new(crate::outcome::Code::TestContract, "mismatch")
            .fix("Fix tests/watane-dev test --json.json step 1 (api.get), then run: watane-dev test --json");
        assert_eq!(
            failed_test_next(&[error], "serve", ""),
            "Fix tests/watane-dev test --json.json step 1 (api.get), then run: watane-dev serve --json"
        );
    }

    #[test]
    fn ac2_serve_rewrites_the_fix_of_test_errors_only() {
        use crate::outcome::Code;
        for code in [Code::TestUnit, Code::TestUnitLimit, Code::TestContract] {
            let error = Problem::new(code, "x").fix("Fix it, then run: watane-dev test --json");
            assert_eq!(
                rewritten_fix(&error, "serve", "").unwrap(),
                "Fix it, then run: watane-dev serve --json"
            );
        }
        let other =
            Problem::new(Code::ModInvalid, "x").fix("Fix it, then run: watane-dev test --json");
        assert_eq!(
            rewritten_fix(&other, "serve", "").unwrap(),
            "Fix it, then run: watane-dev test --json"
        );
    }

    #[test]
    fn ac6_failed_test_and_serve_name_first_failure_and_fallback() {
        use crate::outcome::Code;
        let unit = crate::unit::parse_libtest(
            "running 1 test\ntest tests::fails ... FAILED\n",
            "",
            "demo",
        )
        .problems();
        let limit =
            crate::unit::parse_libtest("running 1 test\ntest tests::endless ... ", "", "demo");
        // A limit has the same recovery action, regardless of its diagnostic message.
        let mut limit_errors = limit.problems();
        limit_errors[0].code = Code::TestUnitLimit;
        let contract = Problem::new(
            Code::TestContract,
            "tests/counter.json step 2 (api.get): expected 2, got 1",
        )
        .fix("Fix tests/counter.json step 2 (api.get), then run: watane-dev test --json");
        for command in ["test", "serve"] {
            assert_eq!(
                failed_test_next(&unit, command, ""),
                format!("Fix the unit test tests::fails, then run: watane-dev {command} --json")
            );
            assert_eq!(
                failed_test_next(&limit_errors, command, ""),
                format!("Fix the unit test tests::endless, then run: watane-dev {command} --json")
            );
            assert_eq!(
                failed_test_next(std::slice::from_ref(&contract), command, ""),
                format!(
                    "Fix tests/counter.json step 2 (api.get), then run: watane-dev {command} --json"
                )
            );
            assert_eq!(
                failed_test_next(
                    &[Problem::new(Code::ModInvalid, "bad").fix("Correct the mod")],
                    command,
                    ""
                ),
                "Correct the mod"
            );
            assert_eq!(
                failed_test_next(&[Problem::new(Code::BuildCompile, "bad")], command, ""),
                format!("watane-dev {command} --json")
            );
        }
    }

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
            app_home(Some(link.join("new/nested")), &|_| String::new()).unwrap(),
            root.path().canonicalize().unwrap().join("new/nested")
        );
        assert!(!root.path().join("new").exists());
    }

    #[test]
    fn ac2_serve_next_url_is_last_and_mcp_keys_match_mode() {
        let result = json!({"reused": false, "pages": [{"url": "http://demo.localhost:7/"}]});
        for json in [false, true] {
            let next = serve_next(&result, json, "").unwrap();
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
            "",
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
                true,
                ""
            )
            .as_deref(),
            Some("Give the person this page: http://demo.localhost:7/")
        );
    }

    #[test]
    fn ac2_new_preview_without_page_still_gives_mcp_setup() {
        let pageless = |reused| json!({"reused": reused, "pages": [{"slug": "demo", "url": null}], "mcp": {"url": "u", "token": "t"}});
        assert!(
            serve_next(&pageless(false), true, "")
                .unwrap()
                .starts_with("The MCP url and token are new.")
        );
        assert_eq!(serve_next(&pageless(true), true, ""), None);
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
                checked.next,
                Some(format!(
                    "Fix the errors, then run: watane-dev check --json --dir {}",
                    dir.canonicalize().unwrap().display()
                ))
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
    fn ac1_template_explains_publish_push_and_person_login() {
        let guide = include_str!("../template/AGENTS.md");
        for text in [
            "watane-dev publish --push --json",
            "env.gh_auth",
            "ask the person to run `gh auth login`",
            "finish a failed push",
        ] {
            assert!(guide.contains(text), "missing {text}");
        }
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
            serve_next(&json!({"stopped":true}), true, "").as_deref(),
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
        assert_eq!(tested.next.as_deref(), Some("Remove logic/build.rs"));
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
                true,
                ""
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
                true,
                ""
            ),
            None
        );
    }

    #[test]
    fn s86_ac1_test_and_serve_take_home() {
        use clap::CommandFactory;
        Cli::try_parse_from(["watane-dev", "test", "--json", "--home", "/x", "--dir", "m"])
            .unwrap();
        Cli::try_parse_from(["watane-dev", "serve", "--stop", "--home", "/x"]).unwrap();
        for name in ["test", "serve"] {
            let help = Cli::command()
                .find_subcommand_mut(name)
                .unwrap()
                .render_help()
                .to_string();
            assert!(help.contains("--home <HOME>"), "{help}");
            assert!(help.contains("Home of the running watane app"), "{help}");
        }
    }

    #[test]
    fn s86_ac2_reruns_end_in_dir_and_home_only_as_given() {
        use crate::outcome::Code;
        let dir = Path::new("/w/my mod");
        assert_eq!(rerun_args(false, dir, None), "");
        assert_eq!(rerun_args(true, dir, None), " --dir '/w/my mod'");
        assert_eq!(
            rerun_args(true, dir, Some("/h".into())),
            " --dir '/w/my mod' --home /h"
        );
        let args = " --dir /w/m --home /h";
        let unit =
            Problem::new(Code::TestUnit, "x").fix("Fix it, then run: watane-dev test --json");
        assert_eq!(
            rewritten_fix(&unit, "test", args).unwrap(),
            "Fix it, then run: watane-dev test --json --dir /w/m --home /h"
        );
        assert_eq!(
            rewritten_fix(&unit, "serve", args).unwrap(),
            "Fix it, then run: watane-dev serve --json --dir /w/m --home /h"
        );
        let missing = Problem::new(Code::ModReleaseProviderMissing, "x")
            .fix("Start the watane app, then run: watane-dev add --release k/p@0.1.0");
        assert_eq!(
            rewritten_fix(&missing, "serve", args).unwrap(),
            "Start the watane app, then run: watane-dev add --release k/p@0.1.0 --dir /w/m --home /h"
        );
        assert_eq!(
            failed_test_next(&[], "serve", args),
            "watane-dev serve --json --dir /w/m --home /h"
        );
        assert_eq!(
            serve_next(&json!({"stopped":true}), true, " --dir /w/m").unwrap(),
            "The preview and its data are gone. To start a new one, run: watane-dev serve --json --dir /w/m"
        );
    }

    #[test]
    fn s86_ac3_missing_home_fix_ends_in_the_rerun_with_a_placeholder() {
        let rerun = |home: &str| format!("watane-dev search --json x{home}");
        let missing = PathBuf::from("x/../h");
        let e = app_home(Some(missing), &rerun).unwrap_err();
        assert_eq!(
            e[0].fix.as_deref(),
            Some(
                "Pass --home the folder that the watane app uses (x does not exist), then run: watane-dev search --json x --home <folder>"
            )
        );
    }

    #[test]
    fn s86_ac4_rename_names_the_snake_form_only_when_it_differs() {
        assert_eq!(
            replace_text("counter", "tally"),
            "Replace counter with tally at each result.mentions entry"
        );
        assert_eq!(
            replace_text("reading-list", "ratings"),
            "Replace reading-list with ratings (and reading_list with ratings) at each result.mentions entry"
        );
        assert_eq!(
            replace_text("ratings", "my-ratings"),
            "Replace ratings with my-ratings (and ratings with my_ratings) at each result.mentions entry"
        );
    }
}
