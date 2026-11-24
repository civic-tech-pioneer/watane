//! Selects and snapshots the maker's scripted OpenAI responses (TECHNICAL 18.4).

use crate::outcome::{Code, Problem};
use crate::toolchain::executable;
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use watane_pkg::{Grant, Mod};

const SCRIPT: &str = "scripted-responses.json";
const MARKER: &str = "scripted-responses.ready";

struct Script {
    path: PathBuf,
    bytes: Vec<u8>,
}

pub(super) struct OpenAi {
    program: PathBuf,
    script: Option<Script>,
    fingerprint: blake3::Hash,
}

impl OpenAi {
    pub(super) fn select(
        dir: &Path,
        root: &Mod,
        folders: &[PathBuf],
        explicit: Option<&Path>,
    ) -> Result<Option<Self>, Problem> {
        let path = explicit.map_or_else(|| dir.join(SCRIPT), Path::to_path_buf);
        let script = match fs::read(&path) {
            Ok(bytes) => {
                serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&bytes)
                    .map_err(|e| script_problem(&path, e))?;
                Some(Script { path, bytes })
            }
            Err(e) if explicit.is_none() && e.kind() == ErrorKind::NotFound => None,
            Err(e) => return Err(script_problem(&path, e)),
        };
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
        let program = program()?;
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

    pub(super) fn configure(&self, command: &mut Command, folder: &Path) -> Result<(), Problem> {
        command.arg("--scripted-openai").arg(&self.program);
        if let Some(script) = &self.script {
            let snapshot = folder.join(SCRIPT);
            match fs::remove_file(&snapshot) {
                Ok(()) => (),
                Err(e) if e.kind() == ErrorKind::NotFound => (),
                Err(e) => return Err(Problem::internal(e)),
            }
            fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&snapshot)
                .and_then(|mut file| file.write_all(&script.bytes))
                .map_err(Problem::internal)?;
            command.arg("--openai-script").arg(snapshot);
        }
        Ok(())
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
        problem.fix = Some(format!(
            "Use a development build of watane for scripted OpenAI, and check the script against the scripted-responses format. {log}"
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
            Some(setting) => fs::write(marker, setting.marker(port)).map_err(Problem::internal),
            None => match fs::remove_file(marker) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(Problem::internal(e)),
            },
        }
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
    .fix("Use a readable JSON object in the scripted-responses format")
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
