//! Developer executable discovery and preview platform selection (TECHNICAL 9.1, 9.3, 9.4).

use crate::outcome::{Code, Problem};
use crate::toolchain::{executable, program_name};
use std::path::PathBuf;

#[cfg(unix)] // Preview uses Unix process groups to manage its child tree.
mod unix;
#[cfg(unix)]
pub use unix::stop;
#[cfg(unix)]
pub(crate) use unix::{previous_warning, read_script, serve_with_script};

#[cfg(unix)]
pub(crate) fn require_available() -> Result<(), Problem> {
    Ok(())
}

#[cfg(windows)]
pub(crate) fn require_available() -> Result<(), Problem> {
    Err(unavailable())
}

#[cfg(windows)]
fn unavailable() -> Problem {
    Problem::new(
        Code::EnvPreviewStart,
        "preview is unavailable on Windows because it requires Unix process-group controls",
    )
    .fix("Run watane-dev serve on Linux or macOS; on Windows, watane-dev test still builds and tests the mod")
}

pub fn serve(
    dir: &std::path::Path,
    env: &crate::env::Env,
    rerun: &str,
) -> Result<serde_json::Value, Vec<Problem>> {
    let script = read_script(dir, None).map_err(|p| vec![p])?;
    serve_with_script(dir, env, rerun, script, &mut Vec::new())
}

#[cfg(windows)]
pub(crate) struct ScriptInput;

#[cfg(windows)]
pub(crate) fn read_script(
    _: &std::path::Path,
    _: Option<&std::path::Path>,
) -> Result<Option<ScriptInput>, Problem> {
    Err(unavailable())
}

#[cfg(windows)]
pub(crate) fn serve_with_script(
    _: &std::path::Path,
    _: &crate::env::Env,
    _: &str,
    _: Option<ScriptInput>,
    _: &mut Vec<Problem>,
) -> Result<serde_json::Value, Vec<Problem>> {
    Err(vec![unavailable()])
}

#[cfg(windows)]
pub fn stop(_: &std::path::Path, _: &crate::env::Env) -> Result<serde_json::Value, Vec<Problem>> {
    Err(vec![unavailable()])
}

#[cfg(windows)]
pub(crate) fn previous_warning(
    _: &std::path::Path,
    _: &crate::env::Env,
    _: &str,
) -> Option<Problem> {
    None
}

pub fn find_watane() -> Result<PathBuf, Problem> {
    let missing = |where_: String| {
        Problem::new(
            Code::EnvWataneMissing,
            format!("the program watane was not found ({where_})"),
        )
        .fix("Install watane, or set WATANE_BIN to the program")
    };
    if let Some(p) = crate::env::var_os("WATANE_BIN") {
        let p = PathBuf::from(p);
        return if executable(&p) {
            p.canonicalize().map_err(Problem::internal)
        } else {
            Err(missing(p.display().to_string()))
        };
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.with_file_name(program_name("watane"));
        if executable(&p) {
            return Ok(p);
        }
    }
    if let Some(paths) = crate::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let p = path.join(program_name("watane"));
            if executable(&p) {
                return p.canonicalize().map_err(Problem::internal);
            }
        }
    }
    Err(missing("beside watane-dev or on PATH".into()))
}
/// The program `fake-claude` (TECHNICAL 9.1); a missing one is the warning, never an error.
pub fn find_fake_claude() -> Result<PathBuf, Problem> {
    let missing = |where_: String| {
        Problem::new(
            Code::PreviewFakeClaudeMissing,
            format!("the program fake-claude was not found ({where_}); a preview cannot start Claude Code"),
        )
        .fix("Install watane-dev with its agent layer fakes, or set WATANE_FAKE_CLAUDE to the program")
    };
    if let Some(p) = crate::env::var_os("WATANE_FAKE_CLAUDE") {
        let p = PathBuf::from(p);
        return if executable(&p) {
            p.canonicalize().map_err(Problem::internal)
        } else {
            Err(missing(p.display().to_string()))
        };
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.with_file_name(program_name("fake-claude"));
        if executable(&p) {
            return Ok(p);
        }
    }
    if let Some(paths) = crate::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let p = path.join(program_name("fake-claude"));
            if executable(&p) {
                return p.canonicalize().map_err(Problem::internal);
            }
        }
    }
    Err(missing("beside watane-dev or on PATH".into()))
}
