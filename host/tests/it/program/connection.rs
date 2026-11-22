#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! S99: `--fake-claude` (TECHNICAL 12.2) and the connection-replaced message of `watane.js`
//! (TECHNICAL 8.5).

use super::Preview;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

#[test]
fn ac13_fake_claude_without_preview_is_a_usage_error() {
    for args in [
        vec!["--home", "h", "--fake-claude", "/bin/true"],
        vec!["--fake-claude", "/bin/true"],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_watane"))
            .current_dir(dir.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["error"]["code"], "usage");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ac13_a_preview_with_fake_claude_runs_it_for_a_claude_code_start() {
    let home = tempfile::tempdir().unwrap();
    let fake = tempfile::tempdir().unwrap();
    let program = fake.path().join("claude");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake-claude/claude"),
        &program,
    )
    .unwrap();
    std::fs::write(
        fake.path().join("init"),
        "{\"type\":\"system\",\"subtype\":\"init\",\"tools\":[],\"permissionMode\":\"default\",\"plugins\":[]}\n",
    )
    .unwrap();
    std::fs::write(fake.path().join("exit"), "0").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let claude = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods/claude");
    let mut preview = Preview::start_with(
        home.path(),
        &[
            "--install".as_ref(),
            claude.as_os_str(),
            "--fake-claude".as_ref(),
            program.as_os_str(),
        ],
    );
    let client = preview.client("claude.localhost", "127.0.0.1");
    let url = format!("http://claude.localhost:{}/call", preview.ready.port);
    let call = |function: &str, args: Value| {
        let client = client.clone();
        let url = url.clone();
        let function = function.to_owned();
        async move {
            let r: Value = client
                .post(url)
                .json(&json!({"fn": function, "args": args}))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            r["value"].clone()
        }
    };
    let s = call(
        "api.start",
        json!({"launch": r#"{"version":1,"workspace":"scratch"}"#}),
    )
    .await;
    let s = s.as_str().unwrap().to_owned();
    assert_eq!(s.len(), 32, "{s}");
    let mut ended = Value::Null;
    for _ in 0..200 {
        ended = call("api.ended", Value::Null).await;
        if ended != json!([]) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(ended, json!([format!("{s} closed")]));
    preview.stop();
}

/// Runs `src/watane.js` under node with a stand-in `EventSource`, and prints what a handler saw.
/// Skipped, with a note, where node is not installed.
#[test]
fn ac14_the_second_connection_message_reaches_handlers_with_previous_the_first_does_not() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: node is not installed");
        return;
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/watane.js");
    let script = r#"
const fs = require('fs');
let sources = [];
globalThis.window = globalThis;
globalThis.location = { reload() {} };
globalThis.EventSource = class { constructor(url) { this.url = url; sources.push(this); } };
const calls = [];
globalThis.fetch = async (url, init) => { calls.push(JSON.parse(init.body)); return { ok: true, json: async () => ({ value: 1 }) }; };
(0, eval)(fs.readFileSync(process.argv[1], 'utf8'));
const seen = [];
window.watane.on((m) => { seen.push(m); if (m.type === 'connection') calls.push('handler'); });
const send = (m) => sources[0].onmessage({ data: JSON.stringify(m) });
(async () => {
  send({ type: 'connection', id: 'c1' });
  await window.watane.call('f', null);
  send({ type: 'connection', id: 'c2' });
  await window.watane.call('f', null);
  console.log(JSON.stringify({ seen, calls }));
})();
"#;
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let out: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        out["seen"],
        json!([{"type":"connection","id":"c2","previous":"c1"}])
    );
    assert_eq!(
        out["calls"],
        json!([
            {"fn":"f","args":null,"connection":"c1"},
            "handler",
            {"fn":"f","args":null,"connection":"c2"}
        ])
    );
}
