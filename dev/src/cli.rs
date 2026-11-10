use crate::{
    check,
    env::Env,
    outcome::{Outcome, Problem},
    preview, template,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
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
    Serve {
        #[arg(long)]
        stop: bool,
    },
}
pub fn run(cli: Cli) -> Outcome {
    let name = match cli.command {
        Command::New { .. } => "new",
        Command::Check => "check",
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
        Command::New { slug } => {
            template::new_mod(&dir, &slug)
                .map_err(|e| vec![e])
                .inspect(|_| {
                    outcome.next = Some("Edit ui/, then run: watane-dev serve --json".into());
                })
        }
        Command::Check => dir
            .canonicalize()
            .map_err(|e| vec![Problem::internal(e)])
            .and_then(|dir| {
                outcome.dir = Some(dir.clone());
                check::check(&dir).map(|(_, v)| v)
            }),
        Command::Serve { stop } => dir
            .canonicalize()
            .map_err(|e| vec![Problem::internal(e)])
            .and_then(|dir| {
                outcome.dir = Some(dir.clone());
                let env = Env::from_process().map_err(|e| vec![e])?;
                if stop {
                    preview::stop(&dir, &env)
                } else {
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
