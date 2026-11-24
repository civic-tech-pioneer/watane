//! Launch the S111 Responses fake for the lifetime of a dev preview (TECHNICAL 19.9).

use serde::Deserialize;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use watane_host::{Host, HostError, Mode};

#[derive(Default)]
pub(super) struct Options {
    program: Option<PathBuf>,
    script: Option<PathBuf>,
}

impl Options {
    pub(super) fn parse(
        &mut self,
        option: &str,
        value: Option<OsString>,
    ) -> Result<(), &'static str> {
        let target = if option == "--scripted-openai" {
            &mut self.program
        } else {
            &mut self.script
        };
        if target.is_some() {
            return Err("scripted OpenAI options can only be given once");
        }
        let value = value
            .filter(|p| !p.is_empty() && !p.to_str().is_some_and(|s| s.starts_with("--")))
            .ok_or("scripted OpenAI option needs a path")?;
        *target = Some(value.into());
        Ok(())
    }

    pub(super) fn validate(&self, mode: &Mode) -> Result<(), &'static str> {
        if !matches!(mode, Mode::Preview) && (self.program.is_some() || self.script.is_some()) {
            return Err("scripted OpenAI requires --preview");
        }
        if self.script.is_some() && self.program.is_none() {
            return Err("--openai-script requires --scripted-openai");
        }
        Ok(())
    }

    pub(super) fn start(&self, host: &Host) -> Result<Option<ScriptedChild>, HostError> {
        let Some(program) = &self.program else {
            return Ok(None);
        };
        let token = script_token(self.script.as_deref())?;
        let mut command = Command::new(program);
        if let Some(script) = &self.script {
            command.arg("--script").arg(script);
        }
        let mut child = ScriptedChild(
            command
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|err| invalid(format!("cannot start {}: {err}", program.display())))?,
        );
        let stdout = child.0.stdout.take().expect("piped stdout");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = BufReader::new(stdout).take(64 * 1024).read_line(&mut line);
            let _ = send.send(result.map(|_| line));
        });
        let line = receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|err| invalid(format!("waiting for the ready line: {err}")))?
            .map_err(|err| invalid(format!("reading the ready line: {err}")))?;
        #[derive(Deserialize)]
        struct Ready {
            scripted: u32,
            responses_addr: SocketAddrV4,
            ca_pem: String,
        }
        let ready: Ready = serde_json::from_str(&line).map_err(|_| {
            invalid("no valid ready line; see the program's error in the preview log")
        })?;
        if ready.scripted != 1 || !line.ends_with('\n') {
            return Err(invalid("invalid ready line"));
        }
        host.set_scripted_openai(ready.responses_addr, &ready.ca_pem, token)?;
        Ok(Some(child))
    }
}

fn invalid(message: impl std::fmt::Display) -> HostError {
    HostError::BadRequest(format!("scripted OpenAI: {message}"))
}

fn script_token(script: Option<&Path>) -> Result<String, HostError> {
    #[derive(Deserialize)]
    struct Script {
        token: Option<String>,
    }
    let token = if let Some(path) = script {
        let file = std::fs::File::open(path)
            .map_err(|err| invalid(format!("cannot open {}: {err}", path.display())))?;
        let script: Script = serde_json::from_reader(BufReader::new(file))
            .map_err(|err| invalid(format!("invalid script {}: {err}", path.display())))?;
        script.token
    } else {
        None
    };
    Ok(token.unwrap_or_else(|| "watane-preview".into()))
}

pub(super) struct ScriptedChild(Child);

impl Drop for ScriptedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
