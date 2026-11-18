//! The `watane-dev` program: parses the command line, runs the command, prints
//! its result and exits with the status of its class (TECHNICAL 2.2).

use clap::Parser;
use std::io::Write;
use watane_dev::cli::Cli;
use watane_dev::outcome::{Code, Outcome, Problem};
/// Writes to stdout; a closed stdout (`| head -1`) ends the output, not the program with a panic.
fn emit(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes()).and_then(|()| out.flush());
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let json = args.iter().any(|s| s == "--json");
    let outcome = match Cli::try_parse_from(&args) {
        Ok(cli) => watane_dev::cli::run(cli),
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            emit(&e.to_string());
            return;
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
    std::process::exit(i32::from(outcome.exit_code()));
}
