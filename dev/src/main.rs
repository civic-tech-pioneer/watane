//! The `watane-dev` program: runs `cli::main_with` on its own arguments,
//! environment, folder and stdout, and exits with its status (TECHNICAL 2.2).

fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    let status = watane_dev::cli::main_with(&args, &[], None, &mut std::io::stdout().lock());
    std::process::exit(i32::from(status));
}
