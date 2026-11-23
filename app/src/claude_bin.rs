//! Checks the participant's Claude Code program before it is stored (TECHNICAL 19.3).

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
    let version = version(&path).ok_or_else(|| {
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
fn version(_: &Path) -> Option<(u64, u64, u64)> {
    None
}

#[cfg(unix)]
fn version(path: &Path) -> Option<(u64, u64, u64)> {
    let line = probe(path, Duration::from_secs(5))?;
    let line = std::str::from_utf8(&line).ok()?;
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
}

#[cfg(unix)]
fn probe(path: &Path, timeout: Duration) -> Option<Vec<u8>> {
    let env = std::env::vars_os().filter(|(name, _)| {
        name.to_str().is_some_and(|name| {
            matches!(
                name,
                "HOME" | "USER" | "LOGNAME" | "LANG" | "TMPDIR" | "SHELL"
            ) || name.starts_with("LC_")
        })
    });
    // A local socket lets the reader time out without a thread that could stay blocked.
    let (stdout, output) = UnixStream::pair().ok()?;
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
        .ok()?;
    if let Some(line) = read_first_line(stdout, deadline) {
        while Instant::now() < deadline {
            match child.try_wait() {
                Ok(Some(status)) => return status.success().then_some(line),
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break,
            }
        }
    }
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
    None
}

#[cfg(unix)]
fn read_first_line(mut stdout: UnixStream, deadline: Instant) -> Option<Vec<u8>> {
    const LINE_LIMIT: usize = 4096;
    let mut buf = [0; LINE_LIMIT];
    let mut line = Vec::new();
    let mut complete = false;
    loop {
        let time_left = deadline.checked_duration_since(Instant::now())?;
        stdout.set_read_timeout(Some(time_left)).ok()?;
        let count = match stdout.read(&mut buf) {
            Ok(0) => return Some(line),
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        // Drain later output so a verbose program can finish normally.
        if !complete {
            let end = buf[..count].iter().position(|&byte| byte == b'\n');
            let bytes = &buf[..end.unwrap_or(count)];
            if line.len() + bytes.len() > LINE_LIMIT {
                return None;
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
        let program = program(
            dir.path(),
            "claude",
            r#"test "$#" = 1 && test "$1" = --version || exit 1
test "$PATH" = /usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin || exit 1
if read -r input; then exit 1; fi
printf 'Claude Code 2.1.295-rc.1\n'"#,
        );
        let alias = dir.path().join("chosen");
        symlink(&program, &alias).unwrap();
        let checked = validate(&alias).unwrap();
        assert_eq!(checked.path, program.canonicalize().unwrap());
        assert_eq!(checked.version, "2.1.295");

        std::fs::write(&program, "#!/bin/sh\nprintf '2.1.294 (Claude Code)\\n'\n").unwrap();
        let error = validate(&alias).unwrap_err();
        assert_eq!(error.code, "bad_input");
        assert_eq!(
            error.message,
            "Claude Code 2.1.294 is too old; watane needs 2.1.295 or later."
        );

        std::fs::write(&program, "#!/bin/sh\nprintf '2.1.9\\n'\n").unwrap();
        assert_eq!(
            validate(&alias).unwrap_err().message,
            "Claude Code 2.1.9 is too old; watane needs 2.1.295 or later."
        );

        std::fs::write(&program, "#!/bin/sh\nprintf '2.2.0\\n'\n").unwrap();
        assert_eq!(validate(&alias).unwrap().version, "2.2.0");

        std::fs::write(&program, "#!/bin/sh\nprintf '10.0.0 (Claude Code)\\n'\n").unwrap();
        assert_eq!(validate(&alias).unwrap().version, "10.0.0");
    }

    #[test]
    fn ac3_rejects_non_program_and_malformed_or_failed_answer() {
        let dir = tempfile::tempdir().unwrap();
        let program = program(dir.path(), "claude", "printf 'unavailable\\n2.1.295\\n'");
        let path = program.canonicalize().unwrap();
        assert_eq!(
            validate(&program).unwrap_err().message,
            format!("{} did not answer --version.", path.display())
        );
        std::fs::write(&program, "#!/bin/sh\nprintf '2.1.295\\n'\nexit 1\n").unwrap();
        assert_eq!(
            validate(&program).unwrap_err().message,
            format!("{} did not answer --version.", path.display())
        );
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            validate(&program).unwrap_err().message,
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
        assert!(probe(&program, Duration::from_millis(100)).is_none());
        // The delayed marker would be created by the child if only the shell were killed.
        std::thread::sleep(Duration::from_millis(400));
        assert!(!dir.path().join("claude.survived").exists());
    }
}
