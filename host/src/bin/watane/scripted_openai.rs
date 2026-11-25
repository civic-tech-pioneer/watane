//! Launch the S111 Responses fake for the lifetime of a dev preview (TECHNICAL 19.9).

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::net::SocketAddrV4;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use watane_host::{Host, HostError, JudgeAccount, Mode};

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
        let script = read_script(self.script.as_deref())?;
        let mut command = Command::new(program);
        if let Some(script) = &self.script {
            command.arg("--script").arg(script);
        }
        let mut child = ScriptedChild {
            process: command
                .env_clear()
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(|err| invalid(format!("cannot start {}: {err}", program.display())))?,
            judge_origin: None,
        };
        let stdout = child.process.stdout.take().expect("piped stdout");
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = send.send(read_ready(stdout));
        });
        let ready = receive
            .recv_timeout(Duration::from_secs(10))
            .map_err(|err| invalid(format!("waiting for the ready line: {err}")))??;
        let judge = if script.plays.is_some_and(|plays| !plays.is_empty()) {
            let address = ready
                .judge_addr
                .as_str()
                .and_then(|address| address.parse().ok())
                .ok_or_else(|| invalid("ready line needs an IPv4 judge address"))?;
            let origin = ready
                .judge_origin
                .as_str()
                .ok_or_else(|| invalid("ready line needs a judge origin"))?
                .to_owned();
            let key = script
                .judge_key
                .unwrap_or_else(|| "watane-preview-judge".into());
            Some((address, JudgeAccount { origin, key }))
        } else {
            None
        };
        let token = script.token.unwrap_or_else(|| "watane-preview".into());
        child.judge_origin = host
            .set_scripted_responses(ready.responses_addr, &ready.ca_pem, token, judge)
            .map_err(invalid)?;
        Ok(Some(child))
    }
}

#[derive(Deserialize)]
struct Ready {
    scripted: u32,
    responses_addr: SocketAddrV4,
    ca_pem: String,
    // An OpenAI-only script does not depend on the judge fields.
    #[serde(default)]
    judge_addr: serde_json::Value,
    #[serde(default)]
    judge_origin: serde_json::Value,
}

fn read_ready(reader: impl Read) -> Result<Ready, HostError> {
    let mut line = String::new();
    BufReader::new(reader)
        .take(64 * 1024)
        .read_line(&mut line)
        .map_err(|err| invalid(format!("reading the ready line: {err}")))?;
    let ready: Ready = serde_json::from_str(&line)
        .map_err(|_| invalid("no valid ready line; see the program's error in the preview log"))?;
    if ready.scripted != 1 || !line.ends_with('\n') {
        return Err(invalid("invalid ready line"));
    }
    Ok(ready)
}

fn invalid(message: impl std::fmt::Display) -> HostError {
    HostError::BadRequest(format!("scripted OpenAI: {message}"))
}

#[derive(Default, Deserialize)]
struct Script {
    token: Option<String>,
    judge_key: Option<String>,
    plays: Option<Vec<serde::de::IgnoredAny>>,
}

fn read_script(script: Option<&Path>) -> Result<Script, HostError> {
    let Some(path) = script else {
        return Ok(Script::default());
    };
    let file = std::fs::File::open(path)
        .map_err(|err| invalid(format!("cannot open {}: {err}", path.display())))?;
    serde_json::from_reader(BufReader::new(file))
        .map_err(|err| invalid(format!("invalid script {}: {err}", path.display())))
}

#[derive(Serialize)]
pub(super) struct PreviewReady<'a> {
    #[serde(flatten)]
    pub(super) ready: watane_host::Ready,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) judge_origin: Option<&'a str>,
}

pub(super) struct ScriptedChild {
    process: Child,
    pub(super) judge_origin: Option<String>,
}

impl Drop for ScriptedChild {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::{Options, read_ready, read_script};

    #[test]
    fn an_option_is_not_a_scripted_program_path() {
        let mut options = Options::default();
        assert_eq!(
            options.parse("--scripted-openai", Some("--openai-script".into())),
            Err("scripted OpenAI option needs a path")
        );
        assert!(options.program.is_none());
    }

    #[test]
    fn the_ready_line_requires_version_one_a_newline_and_a_bounded_line() {
        let ready = r#"{"scripted":1,"responses_addr":"127.0.0.1:1","ca_pem":"fixture"}"#;
        let parsed = read_ready(format!("{ready}\n").as_bytes()).expect("valid ready line");
        assert_eq!(parsed.responses_addr.to_string(), "127.0.0.1:1");
        let unused_judge = ready.replace(
            "\"ca_pem\"",
            "\"judge_addr\":\"unused\",\"judge_origin\":{},\"ca_pem\"",
        );
        assert!(read_ready(format!("{unused_judge}\n").as_bytes()).is_ok());
        assert!(read_ready(ready.as_bytes()).is_err());
        assert!(read_ready(format!("{}\n", ready.replace(":1,", ":2,")).as_bytes()).is_err());
        assert!(read_ready(format!("{}{ready}\n", " ".repeat(64 * 1024)).as_bytes()).is_err());
    }

    #[test]
    fn no_script_supplies_no_token_or_judge_plays() {
        let script = read_script(None).expect("default script");
        assert!(script.token.is_none());
        assert!(script.plays.is_none());
    }
}
