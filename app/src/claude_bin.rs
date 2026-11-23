//! Checks the participant's Claude Code program before it is stored (TECHNICAL 19.3).

use std::io;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::{
    io::{ErrorKind, Read},
    os::fd::OwnedFd,
    os::unix::{fs::PermissionsExt, net::UnixStream, process::CommandExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crate::{CmdError, words};

const MIN_CLAUDE: (u64, u64, u64) = (2, 1, 295);
#[cfg(unix)]
const PATH: &str = "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin";

#[derive(Clone, Debug, serde::Serialize)]
pub(crate) struct ClaudeStatus {
    pub path: PathBuf,
    pub version: String,
}

pub(crate) fn validate(path: &Path) -> Result<ClaudeStatus, CmdError> {
    let not_program = |path: &Path| {
        CmdError::new(
            "bad_input",
            words::ClaudeBinary::NotProgram(path).to_string(),
        )
    };
    let path = path.canonicalize().map_err(|_| not_program(path))?;
    if !is_program(&path) {
        return Err(not_program(&path));
    }
    let version = version(&path).map_err(|_cause| {
        #[cfg(test)]
        eprintln!("Claude Code version check for {path:?}: {_cause:?}");
        CmdError::new(
            "bad_input",
            words::ClaudeBinary::NoAnswer(&path).to_string(),
        )
    })?;
    let found = dotted(version);
    if version < MIN_CLAUDE {
        return Err(CmdError::new(
            "bad_input",
            words::ClaudeBinary::TooOld {
                found: &found,
                need: &dotted(MIN_CLAUDE),
            }
            .to_string(),
        ));
    }
    Ok(ClaudeStatus {
        path,
        version: found,
    })
}

fn dotted((major, minor, patch): (u64, u64, u64)) -> String {
    format!("{major}.{minor}.{patch}")
}

#[cfg(unix)]
fn is_program(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|entry| entry.is_file() && entry.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_program(_: &Path) -> bool {
    false
}

#[cfg(not(unix))]
fn version(_: &Path) -> io::Result<(u64, u64, u64)> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(unix)]
fn version(path: &Path) -> io::Result<(u64, u64, u64)> {
    let line = probe(path, Duration::from_secs(5))?;
    let line = std::str::from_utf8(&line).map_err(io::Error::other)?;
    line.split(|c: char| !c.is_ascii_digit() && c != '.')
        .find_map(|word| {
            let mut parts = word.trim_matches('.').split('.');
            let found = (
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            );
            parts.next().is_none().then_some(found)
        })
        .ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                "No dotted version on the first stdout line",
            )
        })
}

#[cfg(unix)]
fn probe(path: &Path, timeout: Duration) -> io::Result<Vec<u8>> {
    let env = std::env::vars_os().filter(|(name, _)| {
        name.to_str().is_some_and(|name| {
            matches!(
                name,
                "HOME" | "USER" | "LOGNAME" | "LANG" | "TMPDIR" | "SHELL"
            ) || name.starts_with("LC_")
        })
    });
    // A local socket lets the reader time out without a thread that could stay blocked.
    let (stdout, output) = UnixStream::pair()?;
    let deadline = Instant::now() + timeout;
    let mut child = Command::new(path)
        .arg("--version")
        .env_clear()
        .envs(env)
        .env("PATH", PATH)
        .stdin(Stdio::null())
        .stdout(OwnedFd::from(output))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| io::Error::other(format!("Could not spawn --version: {error}")))?;
    let failure = match read_first_line(stdout, deadline) {
        Ok(line) => loop {
            if Instant::now() >= deadline {
                break ErrorKind::TimedOut.into();
            }
            match child.try_wait() {
                Ok(Some(status)) if status.success() => return Ok(line),
                Ok(Some(status)) => {
                    return Err(io::Error::other(format!("--version exited with {status}")));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(error) => break error,
            }
        },
        Err(error) => io::Error::other(format!("Could not read --version stdout: {error}")),
    };
    // The group leader has not been reaped, so the PID still names this attempt.
    let _ = Command::new("/bin/kill")
        .args(["-s", "KILL", "--", &format!("-{}", child.id())])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
    Err(failure)
}

#[cfg(unix)]
fn read_first_line(mut stdout: UnixStream, deadline: Instant) -> io::Result<Vec<u8>> {
    const LINE_LIMIT: usize = 4096;
    let mut buf = [0; LINE_LIMIT];
    let mut line = Vec::new();
    let mut complete = false;
    loop {
        let time_left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(ErrorKind::TimedOut)?;
        stdout.set_read_timeout(Some(time_left))?;
        let count = match stdout.read(&mut buf) {
            Ok(0) => return Ok(line),
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        // Drain later output so a verbose program can finish normally.
        if !complete {
            let end = buf[..count].iter().position(|&byte| byte == b'\n');
            let bytes = &buf[..end.unwrap_or(count)];
            if line.len() + bytes.len() > LINE_LIMIT {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "First stdout line is too long",
                ));
            }
            line.extend_from_slice(bytes);
            complete = end.is_some();
        }
    }
}

#[cfg(all(test, unix))]
#[expect(clippy::unwrap_used, reason = "test fixtures must open and run")]
mod tests {
    use super::{probe, validate};
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn program(dir: &Path, name: &str, script: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    #[test]
    fn ac3_minimum_is_numeric_and_the_chosen_path_is_canonical() {
        let dir = tempfile::tempdir().unwrap();
        let minimum = program(
            dir.path(),
            "claude",
            r#"test "$#" = 1 && test "$1" = --version || exit 1
test "$PATH" = /usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin || exit 1
if read -r input; then exit 1; fi
printf 'Claude Code 2.1.295-rc.1\n'"#,
        );
        let alias = dir.path().join("chosen");
        symlink(&minimum, &alias).unwrap();
        let checked = validate(&alias).unwrap();
        assert_eq!(checked.path, minimum.canonicalize().unwrap());
        assert_eq!(checked.version, "2.1.295");

        let old = program(
            dir.path(),
            "claude-2.1.294",
            "printf '2.1.294 (Claude Code)\\n'",
        );
        let error = validate(&old).unwrap_err();
        assert_eq!(error.code, "bad_input");
        assert_eq!(
            error.message,
            "Claude Code 2.1.294 is too old; watane needs 2.1.295 or later."
        );

        let single_digit = program(dir.path(), "claude-2.1.9", "printf '2.1.9\\n'");
        assert_eq!(
            validate(&single_digit).unwrap_err().message,
            "Claude Code 2.1.9 is too old; watane needs 2.1.295 or later."
        );

        let next_minor = program(dir.path(), "claude-2.2.0", "printf '2.2.0\\n'");
        assert_eq!(validate(&next_minor).unwrap().version, "2.2.0");

        let two_digit = program(
            dir.path(),
            "claude-10.0.0",
            "printf '10.0.0 (Claude Code)\\n'",
        );
        assert_eq!(validate(&two_digit).unwrap().version, "10.0.0");
    }

    #[test]
    fn ac3_rejects_non_program_and_malformed_or_failed_answer() {
        let dir = tempfile::tempdir().unwrap();
        let malformed = program(
            dir.path(),
            "claude-malformed",
            "printf 'unavailable\\n2.1.295\\n'",
        );
        let path = malformed.canonicalize().unwrap();
        assert_eq!(
            validate(&malformed).unwrap_err().message,
            format!("{} did not answer --version.", path.display())
        );
        let failed = program(dir.path(), "claude-failed", "printf '2.1.295\\n'; exit 1");
        assert_eq!(
            validate(&failed).unwrap_err().message,
            format!(
                "{} did not answer --version.",
                failed.canonicalize().unwrap().display()
            )
        );
        std::fs::set_permissions(&malformed, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            validate(&malformed).unwrap_err().message,
            format!("{} is not a program you can run.", path.display())
        );
        assert_eq!(validate(dir.path()).unwrap_err().code, "bad_input");
    }

    #[test]
    fn ac3_probe_kills_a_group_that_leaves_stdout_open() {
        let dir = tempfile::tempdir().unwrap();
        let program = program(
            dir.path(),
            "claude",
            "printf '2.1.295\\n'; (sleep 0.3; touch \"$0.survived\") & wait",
        );
        let _ = probe(&program, Duration::from_millis(100)).unwrap_err();
        // The delayed marker would be created by the child if only the shell were killed.
        std::thread::sleep(Duration::from_millis(400));
        assert!(!dir.path().join("claude.survived").exists());
    }
}
