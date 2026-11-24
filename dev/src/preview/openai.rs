//! Selects and snapshots the maker's scripted OpenAI responses (TECHNICAL 18.4).

use crate::outcome::{Code, Problem};
use crate::toolchain::executable;
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use watane_pkg::{Grant, Mod};

const SCRIPT: &str = "scripted-responses.json";
const MARKER: &str = "scripted-responses.ready";

pub(crate) struct ScriptInput {
    path: PathBuf,
    bytes: Vec<u8>,
}

/// Validate and snapshot the source before running any mod tests.
pub(crate) fn read_script(
    dir: &Path,
    explicit: Option<&Path>,
) -> Result<Option<ScriptInput>, Problem> {
    let path = explicit.map_or_else(|| dir.join(SCRIPT), Path::to_path_buf);
    match fs::read(&path) {
        Ok(bytes) => {
            serde_json::from_slice::<crate::scripted_format::Script>(&bytes)
                .map_err(|e| script_problem(&path, e))?;
            Ok(Some(ScriptInput { path, bytes }))
        }
        Err(e) if explicit.is_none() && e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(script_problem(&path, e)),
    }
}

pub(super) fn unavailable_warning(mut problem: Problem) -> Problem {
    problem.code = Code::PreviewScriptedOpenaiMissing;
    problem.message = format!("serving without scripted OpenAI: {}", problem.message);
    problem
}

pub(super) struct OpenAi {
    program: PathBuf,
    script: Option<ScriptInput>,
    fingerprint: blake3::Hash,
}

impl OpenAi {
    pub(super) fn select(
        dir: &Path,
        root: &Mod,
        folders: &[PathBuf],
        script: Option<ScriptInput>,
        warnings: &mut Vec<Problem>,
    ) -> Result<Option<Self>, Problem> {
        if script.is_none() && !declares_openai(root) {
            let mut enabled = false;
            for provider in folders.iter().filter(|provider| provider.as_path() != dir) {
                let provider = Mod::read(provider).map_err(Problem::internal)?;
                if declares_openai(&provider) {
                    enabled = true;
                    break;
                }
            }
            if !enabled {
                return Ok(None);
            }
        }
        let program = match program() {
            Ok(program) => program,
            Err(problem) if script.is_none() => {
                warnings.push(unavailable_warning(problem));
                return Ok(None);
            }
            Err(mut problem) => {
                problem.file = script
                    .as_ref()
                    .map(|script| script.path.to_string_lossy().into_owned());
                return Err(problem);
            }
        };
        let mut hash = blake3::Hasher::new();
        hash.update(program.as_os_str().as_encoded_bytes());
        if let Some(script) = &script {
            hash.update(b"\0");
            hash.update(&script.bytes);
        }
        Ok(Some(Self {
            program,
            script,
            fingerprint: hash.finalize(),
        }))
    }

    pub(super) fn configure(
        &self,
        command: &mut Command,
        folder: &Path,
    ) -> Result<Option<PrivateFile>, Problem> {
        command.arg("--scripted-openai").arg(&self.program);
        let Some(script) = &self.script else {
            return Ok(None);
        };
        let snapshot = PrivateFile::create(folder, &script.bytes)?;
        command.arg("--openai-script").arg(&snapshot.0);
        Ok(Some(snapshot))
    }

    pub(super) fn automatic(&self) -> bool {
        self.script.is_none()
    }

    pub(super) fn start_failure(&self, mut problem: Problem) -> Problem {
        if let Some(script) = &self.script {
            problem.message = format!(
                "could not start scripted OpenAI with {}: {}",
                script.path.display(),
                problem.message
            );
            problem.file = Some(script.path.to_string_lossy().into_owned());
        }
        let log = problem.fix.unwrap_or_default();
        let check_script = if self.automatic() {
            ""
        } else {
            " Check the script against the scripted-responses format."
        };
        problem.fix = Some(format!(
            "Use a development build of watane for scripted OpenAI.{check_script} {log}"
        ));
        problem
    }

    fn marker(&self, port: u16) -> String {
        format!("{port}:{}", self.fingerprint)
    }

    pub(super) fn matches(setting: Option<&Self>, folder: &Path, port: u16) -> bool {
        match fs::read_to_string(folder.join(MARKER)) {
            Ok(marker) => setting.is_some_and(|setting| setting.marker(port) == marker),
            Err(e) => e.kind() == ErrorKind::NotFound && setting.is_none(),
        }
    }

    pub(super) fn remember(
        setting: Option<&Self>,
        folder: &Path,
        port: u16,
    ) -> Result<(), Problem> {
        let marker = folder.join(MARKER);
        match setting {
            Some(setting) => {
                let temp = PrivateFile::create(folder, setting.marker(port).as_bytes())?;
                fs::rename(&temp.0, marker).map_err(Problem::internal)
            }
            None => match fs::remove_file(marker) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(Problem::internal(e)),
            },
        }
    }
}

/// A private file unique to this serve invocation, removed when the host finishes starting.
pub(super) struct PrivateFile(PathBuf);

impl PrivateFile {
    fn create(folder: &Path, bytes: &[u8]) -> Result<Self, Problem> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = folder.join(format!(
                ".scripted-responses-{}-{sequence}",
                std::process::id()
            ));
            let mut file = match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(Problem::internal(e)),
            };
            let temp = Self(path);
            file.write_all(bytes).map_err(Problem::internal)?;
            return Ok(temp);
        }
    }
}

impl Drop for PrivateFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn declares_openai(mod_: &Mod) -> bool {
    mod_.grants()
        .iter()
        .any(|grant| matches!(grant, Grant::Http { host } if host.as_str() == "api.openai.com"))
}

fn script_problem(path: &Path, error: impl std::fmt::Display) -> Problem {
    Problem::new(
        Code::EnvPreviewStart,
        format!(
            "could not read scripted OpenAI responses from {}: {error}",
            path.display()
        ),
    )
    .at(path.to_string_lossy(), None, None)
    .fix("Use the scripted-responses JSON format from dev TECHNICAL 18.3")
}

fn program() -> Result<PathBuf, Problem> {
    let missing = |where_: String| {
        Problem::new(
            Code::EnvPreviewStart,
            format!("the program scripted-responses was not found ({where_})"),
        )
        .fix("Install scripted-responses beside watane-dev or set WATANE_SCRIPTED_RESPONSES to the program")
    };
    if let Some(path) = crate::env::var_os("WATANE_SCRIPTED_RESPONSES") {
        let path = PathBuf::from(path);
        return if executable(&path) {
            path.canonicalize().map_err(Problem::internal)
        } else {
            Err(missing(path.display().to_string()))
        };
    }
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.with_file_name("scripted-responses");
        if executable(&path) {
            return Ok(path);
        }
    }
    if let Some(paths) = crate::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            let path = path.join("scripted-responses");
            if executable(&path) {
                return path.canonicalize().map_err(Problem::internal);
            }
        }
    }
    Err(missing("beside watane-dev or on PATH".into()))
}

#[cfg(test)]
mod tests {
    use super::{MARKER, OpenAi, ScriptInput, read_script};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn absent_default_is_optional_and_explicit_script_takes_precedence() {
        let dir = tempfile::tempdir().expect("mod folder");
        let path = dir.path().join("explicit.json");
        assert!(
            read_script(dir.path(), None)
                .expect("optional default")
                .is_none()
        );
        let missing = read_script(dir.path(), Some(&path))
            .err()
            .expect("missing explicit script");
        assert_eq!(missing.file.as_deref().map(Path::new), Some(path.as_path()));
        fs::write(dir.path().join(super::SCRIPT), b"{").expect("invalid default");
        fs::write(&path, b"{}").expect("explicit script");
        let actual = read_script(dir.path(), Some(&path))
            .expect("script parses")
            .expect("explicit script");
        assert_eq!(actual.path, path);
        assert_eq!(actual.bytes, b"{}");
    }

    #[test]
    fn preview_marker_binds_setting_and_port_and_is_cleared_for_ordinary_preview() {
        let dir = tempfile::tempdir().expect("preview folder");
        let folder = dir.path();
        let setting = |script| OpenAi {
            program: "/fixture".into(),
            script: None,
            fingerprint: blake3::hash(script),
        };
        let openai = setting(b"default");
        let changed = setting(b"changed");
        assert!(OpenAi::matches(None, folder, 100));
        assert!(!OpenAi::matches(Some(&openai), folder, 100));
        OpenAi::remember(Some(&openai), folder, 100).expect("save scripted marker");
        assert!(!OpenAi::matches(None, folder, 100));
        assert!(OpenAi::matches(Some(&openai), folder, 100));
        assert!(!OpenAi::matches(Some(&openai), folder, 101));
        assert!(!OpenAi::matches(Some(&changed), folder, 100));
        OpenAi::remember(None, folder, 101).expect("clear scripted marker");
        assert!(!folder.join(MARKER).exists());
        assert!(OpenAi::matches(None, folder, 101));
    }

    #[test]
    fn command_gets_a_private_snapshot_and_drop_removes_it() {
        let dir = tempfile::tempdir().expect("preview folder");
        let original = dir.path().join("script.json");
        let openai = OpenAi {
            program: "/fixture".into(),
            script: Some(ScriptInput {
                path: original.clone(),
                bytes: b"{}".to_vec(),
            }),
            fingerprint: blake3::hash(b"script"),
        };
        let mut command = Command::new("/fixture-host");
        let snapshot = openai
            .configure(&mut command, dir.path())
            .expect("configure")
            .expect("snapshot");
        let path = snapshot.0.clone();
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                "--scripted-openai",
                "/fixture",
                "--openai-script",
                path.to_str().expect("path")
            ]
        );
        assert_ne!(path, original);
        assert_eq!(fs::read(&path).expect("script bytes"), b"{}");
        assert_eq!(
            fs::metadata(&path)
                .expect("snapshot metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(snapshot);
        assert!(!path.exists());
    }
}
