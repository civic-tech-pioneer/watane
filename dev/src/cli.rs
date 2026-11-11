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
        slug: String,
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
        provider: PathBuf,
    },
    /// Test local mods and ask the person to install them in their app
    Install {
        #[arg(long, help = "Home of the running watane app")]
        home: Option<PathBuf>,
        #[arg(
            long,
            default_value_t = 600,
            help = "Seconds to wait for the person to answer"
        )]
        wait: u64,
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
    };
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => return Outcome::new(name, None).fail(vec![Problem::internal(e)]),
    };
    let dir = cli.dir.unwrap_or_else(|| match &cli.command {
        Command::New { slug } => cwd.join(slug),
        _ => cwd.clone(),
    });
    let dir = if dir.is_absolute() {
        dir
    } else {
        cwd.join(dir)
    };
    let mut outcome = Outcome::new(name, Some(dir.clone()));
    let result = match cli.command {
        Command::Doctor => return crate::doctor::doctor(),
        Command::Install { home, wait } => return install(&dir, home, wait),
        Command::New { slug } => template::new_mod(&dir, &slug)
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
        Command::Add { provider } => deps::add_local(&dir, &cwd.join(provider))
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
                outcome.next = serve_next(&v);
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
            }
            outcome.fail(e)
        }
    }
}

fn serve_next(result: &serde_json::Value) -> Option<String> {
    if result["stopped"] == true {
        return Some(
            "The preview and its data are gone. To start a new one, run: watane-dev serve --json"
                .into(),
        );
    }
    result["pages"].as_array()?.last()?["url"]
        .as_str()
        .map(|url| format!("Give the person this page: {url}"))
}

fn test_local(dir: &Path) -> Outcome {
    let order = match deps::local_order(dir) {
        Ok(order) => order,
        Err(e) => return Outcome::new("test", Some(dir.to_path_buf())).fail(vec![e]),
    };
    for provider in order {
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

fn install(dir: &Path, home: Option<PathBuf>, wait: u64) -> Outcome {
    let mut outcome = Outcome::new("install", Some(dir.to_path_buf()));
    let result = (|| {
        let home = home
            .map(Ok)
            .unwrap_or_else(watane_host::default_home)
            .map_err(|e| vec![Problem::internal(e)])?;
        let home = home.canonicalize().unwrap_or(home);
        let client = crate::api::Client::for_app(&home).map_err(|e| vec![e])?;
        let dir = dir.canonicalize().map_err(|e| vec![Problem::internal(e)])?;
        outcome.dir = Some(dir.clone());
        let tested = test_local(&dir);
        if !tested.ok {
            outcome.next = tested.next;
            return Err(tested.errors);
        }
        let folders = deps::local_order(&dir).map_err(|e| vec![e])?;
        let installed = client.install_wait(&folders, wait).map_err(|e| vec![e])?;
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
            if errors
                .first()
                .is_some_and(|p| p.code.class() == crate::outcome::Class::Env)
            {
                outcome.next = Some("watane-dev doctor --json".into());
            }
            outcome.fail(errors)
        }
    }
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
                wait: 600
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
            serve_next(&json!({"stopped":true})).as_deref(),
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
            serve_next(&json!({"pages":[
                {"slug":"a","url":"http://a.localhost:123/"},
                {"slug":"b","url":"http://b.localhost:123/"}
            ]}))
            .as_deref(),
            Some("Give the person this page: http://b.localhost:123/")
        );
        assert_eq!(
            serve_next(&json!({"pages":[
                {"slug":"a","url":"http://a.localhost:123/"},
                {"slug":"b","url":null}
            ]})),
            None
        );
    }
}
