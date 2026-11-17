//! The `watane-app` program: arguments to `watane_app::run`; on an error it
//! prints `watane-app: <error>` and exits 1 (TECHNICAL 1).

fn main() {
    if let Err(error) =
        watane_app::Config::from_args(std::env::args_os().skip(1)).and_then(watane_app::run)
    {
        eprintln!("watane-app: {error}");
        std::process::exit(1);
    }
}
