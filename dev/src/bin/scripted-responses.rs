//! The program form of `ScriptedResponses`: prints one JSON line with its addresses and CA, then serves until killed (TECHNICAL 18.3).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "a program reports on stdout and stderr"
)]
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;
use watane_dev::scripted::{Play, Reply, ScriptedResponses};

#[derive(Deserialize, Default)]
struct Script {
    token: Option<String>,
    models: Option<Vec<String>>,
    replies: Option<Vec<Reply>>,
    judge_key: Option<String>,
    plays: Option<Vec<Play>>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("scripted-responses: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut script_file = None;
    let mut log = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--script" => {
                script_file = Some(PathBuf::from(args.next().ok_or("--script needs a file")?))
            }
            "--log" => log = Some(PathBuf::from(args.next().ok_or("--log needs a folder")?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let script: Script = match script_file {
        Some(f) => {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?
        }
        None => Script::default(),
    };
    let server = ScriptedResponses::start().map_err(|e| e.to_string())?;
    if let Some(t) = &script.token {
        server.expect_token(t);
    }
    if let Some(m) = &script.models {
        let slugs: Vec<&str> = m.iter().map(String::as_str).collect();
        server.models(&slugs);
    }
    if let Some(r) = script.replies {
        server.script(r);
    }
    if let Some(k) = &script.judge_key {
        server.judge().expect_key(k);
    }
    if let Some(p) = script.plays {
        server.judge().script(p);
    }
    if let Some(d) = &log {
        server.log_to(d);
    }
    let line = serde_json::json!({
        "scripted": 1,
        "responses_addr": server.responses_addr().to_string(),
        "judge_addr": server.judge_addr().to_string(),
        "judge_origin": server.judge_origin(),
        "ca_pem": server.ca_pem(),
    });
    println!("{line}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    loop {
        std::thread::park();
    }
}
