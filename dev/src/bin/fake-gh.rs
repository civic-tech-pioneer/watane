//! The GitHub stand-in: all repositories are local bare git repositories.
use std::{
    env, fs,
    path::PathBuf,
    process::{Command, ExitCode},
};
fn run(args: &[String]) -> Result<(), (u8, String)> {
    let root =
        PathBuf::from(env::var_os("FAKE_GH_ROOT").ok_or((2, "FAKE_GH_ROOT is not set".into()))?);
    let a = args.iter().map(String::as_str).collect::<Vec<_>>();
    match a.as_slice() {
        ["api", "user", "--jq", ".login"] => {
            let user = env::var("FAKE_GH_USER").map_err(|_| {
                (
                    4,
                    "To get started with GitHub CLI, please run:  gh auth login".into(),
                )
            })?;
            println!("{user}");
            Ok(())
        }
        ["api", repo, "--jq", ".parent.full_name"] if repo.starts_with("repos/") => {
            let path = root.join(&repo[6..]);
            if !path.is_dir() {
                return Err((1, "gh: Not Found (HTTP 404)".into()));
            }
            println!(
                "{}",
                fs::read_to_string(path.join("fake-gh-parent"))
                    .unwrap_or_else(|_| "null".into())
                    .trim()
            );
            Ok(())
        }
        ["repo", "create", name, "--public"] => {
            let path = root.join(name);
            if path.exists() {
                return Err((
                    1,
                    "GraphQL: Name already exists on this account (createRepository)".into(),
                ));
            }
            fs::create_dir_all(&path).map_err(|e| (1, e.to_string()))?;
            let output = Command::new("git")
                .args(["init", "-q", "--bare", "-b", "main"])
                .arg(&path)
                .output()
                .map_err(|e| (1, e.to_string()))?;
            if !output.status.success() {
                return Err((1, String::from_utf8_lossy(&output.stderr).into()));
            }
            println!("https://github.com/{name}");
            Ok(())
        }
        _ => Err((2, format!("fake-gh: unsupported: {}", args.join(" ")))),
    }
}
fn main() -> ExitCode {
    match run(&env::args().skip(1).collect::<Vec<_>>()) {
        Ok(()) => ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
