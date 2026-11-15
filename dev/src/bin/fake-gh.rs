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
        ["api", "-X", "POST", endpoint, "-f", name, rest @ ..]
            if endpoint.starts_with("repos/")
                && endpoint.ends_with("/forks")
                && name.starts_with("name=") =>
        {
            let user = env::var("FAKE_GH_USER").map_err(|e| (4, e.to_string()))?;
            let account = match rest {
                ["-F", "default_branch_only=true", "--jq", ".full_name"] => user.as_str(),
                [
                    "-f",
                    organization,
                    "-F",
                    "default_branch_only=true",
                    "--jq",
                    ".full_name",
                ] if organization.starts_with("organization=") => {
                    let org = &organization[13..];
                    if org.eq_ignore_ascii_case(&user) {
                        return Err((1, format!("organization={org} must not name the login")));
                    }
                    org
                }
                _ => return Err((2, format!("fake-gh: unsupported: {}", args.join(" ")))),
            };
            let upstream = &endpoint[6..endpoint.len() - 6];
            let source = root.join(upstream);
            if !source.is_dir() {
                return Err((1, "gh: Not Found (HTTP 404)".into()));
            }
            let owner = root.join(account);
            fs::create_dir_all(&owner).map_err(|e| (1, e.to_string()))?;
            for entry in fs::read_dir(&owner).map_err(|e| (1, e.to_string()))? {
                let entry = entry.map_err(|e| (1, e.to_string()))?;
                if fs::read_to_string(entry.path().join("fake-gh-parent"))
                    .is_ok_and(|p| p.trim() == upstream)
                {
                    println!("{account}/{}", entry.file_name().to_string_lossy());
                    return Ok(());
                }
            }
            let path = owner.join(&name[5..]);
            git_output(&["clone", "-q", "--bare"], &[&source, &path])?;
            let branch = git_output(&["-C", path.to_str().unwrap(), "symbolic-ref", "HEAD"], &[])?;
            let branches = git_output(
                &[
                    "-C",
                    path.to_str().unwrap(),
                    "for-each-ref",
                    "--format=%(refname)",
                    "refs/heads",
                ],
                &[],
            )?;
            for other in branches.lines().filter(|b| *b != branch.trim()) {
                git_output(
                    &["-C", path.to_str().unwrap(), "update-ref", "-d", other],
                    &[],
                )?;
            }
            fs::write(path.join("fake-gh-parent"), upstream).map_err(|e| (1, e.to_string()))?;
            println!("{account}/{}", &name[5..]);
            Ok(())
        }
        ["repo", "edit", name, "--default-branch", branch] => {
            let path = root.join(name);
            let reference = format!("refs/heads/{branch}");
            git_output(
                &[
                    "-C",
                    path.to_str().unwrap(),
                    "show-ref",
                    "--verify",
                    &reference,
                ],
                &[],
            )?;
            git_output(
                &[
                    "-C",
                    path.to_str().unwrap(),
                    "symbolic-ref",
                    "HEAD",
                    &reference,
                ],
                &[],
            )?;
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
fn git_output(args: &[&str], paths: &[&std::path::Path]) -> Result<String, (u8, String)> {
    let out = Command::new("git")
        .args(args)
        .args(paths)
        .output()
        .map_err(|e| (1, e.to_string()))?;
    if !out.status.success() {
        return Err((1, String::from_utf8_lossy(&out.stderr).into()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().into())
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
