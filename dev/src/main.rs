use clap::Parser;
use watane_dev::{
    cli::Cli,
    outcome::{Code, Outcome, Problem},
};
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
            print!("{e}");
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
                    "help" => Some("help"),
                    _ => None,
                })
                .unwrap_or("watane-dev");
            Outcome::new(command, None).fail(vec![
                Problem::new(Code::UsageArgs, e.to_string()).fix("watane-dev --help"),
            ])
        }
    };
    if json {
        println!("{}", outcome.to_json());
    } else {
        print!("{}", outcome.to_text());
    }
    std::process::exit(i32::from(outcome.exit_code()));
}
