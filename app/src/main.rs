fn main() {
    if let Err(error) =
        watane_app::Config::from_args(std::env::args_os().skip(1)).and_then(watane_app::run)
    {
        eprintln!("watane-app: {error}");
        std::process::exit(1);
    }
}
