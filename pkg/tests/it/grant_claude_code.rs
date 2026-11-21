#![allow(clippy::unwrap_used, reason = "tests may panic")]

use std::fs;
use std::path::Path;
use watane_pkg::*;

const HOST: &str = "package watane:host@0.1.0;
interface state { get: func(key: string) -> option<list<u8>>; }
interface claude-types { type id = u32; }
interface claude-code { start: func() -> u32; }
interface claude-code-shell { start: func() -> u32; }
interface claude-session { say: func(text: string); }";

fn put(dir: &Path, file: &str, bytes: &str) {
    let path = dir.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn world(imports: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    put(
        dir.path(),
        "mod.toml",
        "name = 'Hello'\nsummary = 'Agent.'\n",
    );
    put(dir.path(), "ui/index.html", "<p>hi</p>\n");
    put(dir.path(), "logic/Cargo.toml", "");
    let imports: String = imports
        .iter()
        .map(|i| format!("import watane:host/{i}@0.1.0; "))
        .collect();
    put(
        dir.path(),
        "wit/world.wit",
        &format!(
            "package mods:hello@0.1.0;\ninterface api {{ count: func() -> u32; }}\nworld hello {{ {imports}export api; }}"
        ),
    );
    put(dir.path(), "wit/deps/watane-host/host.wit", HOST);
    dir
}
fn grants(imports: &[&str]) -> Vec<Grant> {
    Mod::read(world(imports).path()).unwrap().grants()
}

#[test]
fn ac1_worlds_give_the_claude_code_grants() {
    assert_eq!(
        grants(&["state", "claude-code", "claude-session"]),
        vec![Grant::Storage, Grant::ClaudeCode]
    );
    assert_eq!(
        grants(&["state", "claude-code-shell", "claude-types"]),
        vec![Grant::Storage, Grant::ClaudeCodeShell]
    );
    assert_eq!(
        grants(&["state", "claude-code", "claude-code-shell"]),
        vec![Grant::Storage, Grant::ClaudeCode, Grant::ClaudeCodeShell]
    );
}

#[test]
fn ac1_json_text_order_authority() {
    for (grant, json, text) in [
        (
            Grant::ClaudeCode,
            r#"{"kind":"claude-code"}"#,
            "claude-code",
        ),
        (
            Grant::ClaudeCodeShell,
            r#"{"kind":"claude-code-shell"}"#,
            "claude-code-shell",
        ),
    ] {
        assert_eq!(serde_json::to_string(&grant).unwrap(), json);
        assert_eq!(serde_json::from_str::<Grant>(json).unwrap(), grant);
        assert_eq!(grant.to_string(), text);
        assert!(grant.same_authority(&grant));
    }
    assert!(Grant::Manage < Grant::ClaudeCode);
    assert!(Grant::ClaudeCode < Grant::ClaudeCodeShell);
    assert!(!Grant::ClaudeCode.same_authority(&Grant::ClaudeCodeShell));
    assert!(!Grant::ClaudeCodeShell.same_authority(&Grant::ClaudeCode));
}
