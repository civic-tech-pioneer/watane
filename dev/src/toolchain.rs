//! Finds cargo, rustc and the sysroot, and checks that the `wasm32-wasip2`
//! target is installed (TECHNICAL 6.1).

use crate::env::Env;
use crate::outcome::{Code, Problem};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub struct Toolchain {
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    pub sysroot: PathBuf,
    pub version: String,
}
#[cfg(unix)]
pub(crate) fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
#[cfg(windows)]
pub(crate) fn executable(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

pub(crate) fn program_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn problem(detail: impl std::fmt::Display) -> Problem {
    Problem::new(
        Code::EnvToolchain,
        format!("no Rust toolchain found: {detail}"),
    )
    .fix("Install rustup with your package manager, then run: rustup default stable")
}
fn query(command: &mut Command) -> Result<String, Problem> {
    let output = command.output().map_err(problem)?;
    if !output.status.success() {
        return Err(problem(String::from_utf8_lossy(&output.stderr)));
    }
    let text = String::from_utf8(output.stdout).map_err(problem)?;
    if text.trim().is_empty() {
        return Err(problem("empty toolchain response"));
    }
    Ok(text.trim().to_string())
}
pub fn find(env: &Env) -> Result<Toolchain, Problem> {
    let toolchain = discover(env)?;
    toolchain.check_target()?;
    Ok(toolchain)
}

/// Discover the toolchain without requiring an installed Wasm target.
pub fn discover(env: &Env) -> Result<Toolchain, Problem> {
    let (cargo, rustc) = if let Some(root) = crate::env::var_os("WATANE_DEV_TOOLCHAIN") {
        let root = PathBuf::from(root).canonicalize().map_err(problem)?;
        (
            root.join("bin").join(program_name("cargo")),
            root.join("bin").join(program_name("rustc")),
        )
    } else {
        let mut candidates = crate::env::var_os("PATH")
            .map(|p| {
                std::env::split_paths(&p)
                    .map(|p| p.join(program_name("rustup")))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(home) = crate::env::var_os("CARGO_HOME") {
            candidates.push(PathBuf::from(home).join("bin").join(program_name("rustup")));
        }
        if let Some(home) = crate::env::var_os("HOME") {
            candidates.push(
                PathBuf::from(home)
                    .join(".cargo/bin")
                    .join(program_name("rustup")),
            );
        }
        let rustup = candidates
            .into_iter()
            .find(|p| executable(p))
            .ok_or_else(|| problem("rustup is missing"))?;
        let cargo = query(
            Command::new(&rustup)
                .current_dir(&env.cache)
                .args(["which", "cargo"]),
        )?;
        let rustc = query(
            Command::new(&rustup)
                .current_dir(&env.cache)
                .args(["which", "rustc"]),
        )?;
        (PathBuf::from(cargo), PathBuf::from(rustc))
    };
    if !executable(&cargo) || !executable(&rustc) {
        return Err(problem(format!(
            "missing cargo or rustc in {}",
            cargo.display()
        )));
    }
    let sysroot = PathBuf::from(query(
        Command::new(&rustc)
            .current_dir(&env.cache)
            .args(["--print", "sysroot"]),
    )?);
    let version = query(
        Command::new(&rustc)
            .current_dir(&env.cache)
            .arg("--version"),
    )?;
    Ok(Toolchain {
        cargo,
        rustc,
        sysroot: sysroot.canonicalize().map_err(problem)?,
        version,
    })
}

impl Toolchain {
    pub fn check_target(&self) -> Result<(), Problem> {
        if !self.sysroot.join("lib/rustlib/wasm32-wasip2/lib").is_dir() {
            return Err(Problem::new(
                Code::EnvNoWasmTarget,
                format!(
                    "the Rust toolchain at {} has no wasm32-wasip2 target",
                    self.sysroot.display()
                ),
            )
            .fix("rustup target add wasm32-wasip2"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac3_doctor_toolchain_gap_texts_and_fixes() {
        let error = problem("rustup is missing");
        assert_eq!(error.code, Code::EnvToolchain);
        assert_eq!(error.message, "no Rust toolchain found: rustup is missing");
        assert_eq!(
            error.fix.as_deref(),
            Some("Install rustup with your package manager, then run: rustup default stable")
        );
        let toolchain = Toolchain {
            cargo: "/absent/cargo".into(),
            rustc: "/absent/rustc".into(),
            sysroot: "/absent/sysroot".into(),
            version: "rustc fake".into(),
        };
        let error = toolchain.check_target().unwrap_err();
        assert_eq!(error.code, Code::EnvNoWasmTarget);
        assert_eq!(
            error.message,
            "the Rust toolchain at /absent/sysroot has no wasm32-wasip2 target"
        );
        assert_eq!(
            error.fix.as_deref(),
            Some("rustup target add wasm32-wasip2")
        );
    }
}
