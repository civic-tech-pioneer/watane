//! A maker's OpenAI stream through the serve command (TECHNICAL 18.4).
#![allow(clippy::unwrap_used, reason = "tests may panic")]

use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

struct Preview(tempfile::TempDir);

impl Preview {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../host/tests/fixtures/mods/https");
        for file in [
            "mod.toml",
            "logic.wasm",
            "wit/world.wit",
            "wit/deps/watane-host/host.wit",
        ] {
            let dest = temp.path().join("https").join(file);
            fs::create_dir_all(dest.parent().unwrap()).unwrap();
            fs::copy(fixture.join(file), dest).unwrap();
        }
        let tests = temp.path().join("https/tests");
        fs::create_dir(&tests).unwrap();
        fs::write(
            tests.join("initial-state.json"),
            br#"{"steps":[{"call":"api.ended","returns":[]}]}"#,
        )
        .unwrap();
        Self(temp)
    }

    fn mod_dir(&self) -> std::path::PathBuf {
        self.0.path().join("https")
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_watane-dev"));
        command
            .current_dir(self.0.path())
            .env("WATANE_DEV_CACHE", self.0.path().join("cache"))
            .env(
                "WATANE_SCRIPTED_RESPONSES",
                env!("CARGO_BIN_EXE_scripted-responses"),
            )
            .args(["serve", "--json", "--dir", "https"])
            .args(args);
        command
    }

    fn serve(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    fn host_stub(&self, body: &str) -> std::path::PathBuf {
        let path = self.0.path().join("watane-stub");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }

    fn started(&self, args: &[&str], reused: bool) -> Value {
        let out = self.serve(args);
        let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert!(
            out.status.success(),
            "preview failed: {}",
            envelope["errors"]
        );
        assert_eq!(envelope["result"]["reused"], reused);
        envelope["result"].clone()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        let _ = self.serve(&["--stop"]);
    }
}

fn delta(result: &Value) -> String {
    let url = reqwest::Url::parse(result["api"].as_str().unwrap()).unwrap();
    let port = url.port().unwrap();
    let host = format!("https.localhost:{port}");
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap();
    let call = |function: &str, args: Value| {
        let response = client
            .post(format!("http://127.0.0.1:{port}/call"))
            .header("Host", &host)
            .header("Origin", format!("http://{host}"))
            .header("Content-Type", "application/json")
            .body(json!({"fn": function, "args": args}).to_string())
            .send()
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().unwrap()
        );
        let body: Value = serde_json::from_str(&response.text().unwrap()).unwrap();
        body["value"].clone()
    };
    let id = call(
        "api.start",
        json!({"request": json!({
            "url": "https://api.openai.com/v1/responses",
            "method": "POST",
            "account": "openai",
            "headers": {"content-type": "application/json"},
            "body": "{\"model\":\"gpt-5\",\"input\":\"hi\",\"stream\":true}"
        }).to_string()}),
    );
    let id = id.as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let ended = loop {
        let ended: Vec<String> = serde_json::from_value(call("api.ended", Value::Null)).unwrap();
        if let Some(end) = ended.iter().find(|end| end.starts_with(&format!("{id} "))) {
            break end.clone();
        }
        assert!(Instant::now() < deadline, "scripted stream did not end");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(ended, format!("{id} closed"));
    let items: Vec<String> = serde_json::from_value(call("api.items", json!({"s": id}))).unwrap();
    let head: Value = serde_json::from_str(&items[0]).unwrap();
    assert_eq!(head["status"], 200);
    let delta: Value = serde_json::from_str(&items[1]).unwrap();
    assert_eq!(delta["type"], "response.output_text.delta");
    delta["delta"].as_str().unwrap().to_owned()
}

#[test]
fn script_error_names_original_file_and_stop_conflicts() {
    let preview = Preview::new();
    let path = preview.0.path().join("bad.json");
    fs::write(&path, b"{").unwrap();
    let out = preview.serve(&["--script", "bad.json"]);
    assert!(!out.status.success());
    let error: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(error["errors"][0]["code"], "env.preview_start");
    let reported = Path::new(error["errors"][0]["file"].as_str().unwrap());
    assert_eq!(
        reported.canonicalize().unwrap(),
        path.canonicalize().unwrap()
    );
    assert!(error["errors"][0]["fix"].is_string());

    let out = preview.serve(&["--stop", "--script", "bad.json"]);
    assert!(!out.status.success());
}

#[test]
fn regress_s114_bad_script_shape_error_is_clear() {
    let preview = Preview::new();
    let path = preview.0.path().join("shape.json");
    fs::write(&path, br#"{"replies":[{"nope":1}]}"#).unwrap();
    // If validation happens after test_local, its contract error masks the script error.
    fs::write(preview.mod_dir().join("tests/initial-state.json"), b"{").unwrap();
    let out = preview.serve(&["--script", "shape.json"]);
    assert_eq!(out.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
    let error = &envelope["errors"][0];
    assert_eq!(error["code"], "env.preview_start");
    let reported = Path::new(error["file"].as_str().unwrap());
    assert_eq!(
        reported.canonicalize().unwrap(),
        path.canonicalize().unwrap()
    );
    let message = error["message"].as_str().unwrap();
    let fix = error["fix"].as_str().unwrap();
    assert!(message.contains("nope"), "cause not named: {message}");
    assert!(!message.contains("missing field `mode`"));
    assert!(!message.contains(".scripted-responses-"));
    assert!(!fix.contains("development build"));
    assert_ne!(envelope["next"], "watane-dev doctor --json");
}

#[test]
fn regress_s114_reports_unrelated_host_error_without_blame() {
    let preview = Preview::new();
    fs::write(preview.0.path().join("script.json"), b"{}").unwrap();
    let host = preview.host_stub(
        r#"printf '%s\n' '{"watane":1,"error":{"code":"internal","message":"Database already open."}}'
exit 1"#,
    );
    let out = preview
        .command(&["--script", "script.json"])
        .env("WATANE_BIN", host)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
    let error = &envelope["errors"][0];
    assert_eq!(error["code"], "env.preview_start");
    assert_eq!(
        error["message"],
        "the preview did not start: Database already open."
    );
    assert!(error["file"].is_null());
    let fix = error["fix"].as_str().unwrap();
    assert!(fix.starts_with("Read "));
    assert!(!fix.contains("development build"));
}

#[test]
#[ignore = "slow: needs WATANE_BIN for ordinary preview startup"]
fn regress_s114_missing_program_warns_unless_script_supplied() {
    unavailable_scripted_openai_warns_unless_script_supplied(true);
}

#[test]
#[ignore = "slow: needs WATANE_BIN for ordinary preview startup"]
fn regress_s114_refused_setting_warns_unless_script_supplied() {
    unavailable_scripted_openai_warns_unless_script_supplied(false);
}

fn unavailable_scripted_openai_warns_unless_script_supplied(missing_program: bool) {
    let host = std::env::var_os("WATANE_BIN").expect("set WATANE_BIN to a host");
    let preview = Preview::new();
    let stub = preview.host_stub(
            r#"for arg in "$@"; do
    if [ "$arg" = --scripted-openai ]; then
        printf '%s\n' '{"watane":1,"error":{"code":"usage","message":"fixture host rejects the setting"}}'
        exit 2
    fi
done
exec "$TEST_PREVIEW_HOST" "$@""#,
        );
    let output = |args: &[&str]| {
        let mut command = preview.command(args);
        command
            .env("WATANE_BIN", &stub)
            .env("TEST_PREVIEW_HOST", &host);
        if missing_program {
            command.env(
                "WATANE_SCRIPTED_RESPONSES",
                preview.0.path().join("missing"),
            );
        }
        command.output().unwrap()
    };
    let out = output(&[]);
    let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", envelope["errors"]);
    assert!(
        envelope["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| { w["code"] == "preview.scripted_openai_missing" })
    );
    assert_eq!(envelope["result"]["pages"][0]["slug"], "https");

    for supplied in [true, false] {
        let (path, args): (_, &[_]) = if supplied {
            (
                preview.0.path().join("explicit.json"),
                &["--script", "explicit.json"],
            )
        } else {
            (preview.mod_dir().join("scripted-responses.json"), &[])
        };
        fs::write(&path, b"{}").unwrap();
        let out = output(args);
        let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.code(), Some(3));
        assert_eq!(envelope["errors"][0]["code"], "env.preview_start");
        if !missing_program {
            let message = envelope["errors"][0]["message"].as_str().unwrap();
            assert!(message.contains("fixture host rejects the setting"));
            assert!(!message.contains("missing field `mode`"));
        }
        fs::remove_file(path).unwrap();
    }
}

#[test]
#[ignore = "slow: needs development WATANE_BIN; real OpenAI preview lifecycle"]
fn serve_delivers_default_folder_and_explicit_script_responses() {
    assert!(
        std::env::var_os("WATANE_BIN").is_some(),
        "set WATANE_BIN to a development host"
    );
    let preview = Preview::new();
    let default = preview.started(&[], false);
    assert_eq!(delta(&default), "ok");

    let mod_script = preview.mod_dir().join("scripted-responses.json");
    fs::write(
        &mod_script,
        br#"{"replies":[{"sse":[{"delta":"folder"}]}]}"#,
    )
    .unwrap();
    let folder = preview.started(&[], false);
    assert_eq!(delta(&folder), "folder");
    let same = preview.started(&[], true);
    assert_eq!(same["api"], folder["api"]);

    let explicit = preview.0.path().join("explicit.json");
    fs::write(
        &explicit,
        br#"{"token":"fixture-token","replies":[{"sse":[{"delta":"explicit"}]}]}"#,
    )
    .unwrap();
    let explicit = preview.started(&["--script", "explicit.json"], false);
    assert_eq!(delta(&explicit), "explicit");
    assert_eq!(explicit["home"], default["home"]);
    let folder = Path::new(explicit["home"].as_str().unwrap())
        .parent()
        .unwrap();
    assert!(!fs::read_dir(folder).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".scripted-responses-")
    }));

    let stopped = preview.serve(&["--stop"]);
    assert!(stopped.status.success());
    let stopped: Value = serde_json::from_slice(&stopped.stdout).unwrap();
    assert_eq!(stopped["result"]["stopped"], true);
    assert!(!Path::new(explicit["home"].as_str().unwrap()).exists());
}
