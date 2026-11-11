use crate::{
    allowlist::{self, Allowlist},
    builder::{self, BuildCtx},
    check, contract,
    env::Env,
    outcome::{Outcome, Problem},
    preview, template, toolchain, unit,
};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
#[derive(Parser)]
#[command(version, about = "Make and preview watane mods")]
pub struct Cli {
    #[arg(long, global = true)]
    pub json: bool,
    #[arg(long, global = true)]
    pub dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    New {
        slug: String,
    },
    Check,
    Doctor,
    Test,
    Serve {
        #[arg(long)]
        stop: bool,
    },
}
pub fn run(cli: Cli) -> Outcome {
    let name = match cli.command {
        Command::New { .. } => "new",
        Command::Check => "check",
        Command::Doctor => "doctor",
        Command::Test => "test",
        Command::Serve { .. } => "serve",
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
                    let tested = test(&dir);
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
            outcome.result = Some(v);
            if name == "new" {
                outcome.dir = std::fs::canonicalize(&dir).ok();
            }
            outcome
        }
        Err(e) => outcome.fail(e),
    }
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
        outcome.fail(errors)
    }
}
