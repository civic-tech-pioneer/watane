#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! The launch grammar at the call (TECHNICAL 19.3): `bad-launch {field, reason}`, no stream and
//! no process. Handles, skills and the sandbox name the stage that builds them.

use crate::claude::Rig;
use serde_json::{Value, json};
use std::time::Duration;

fn launch(extra: Value) -> String {
    let mut doc = json!({"version": 1, "workspace": "scratch"});
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    doc.to_string()
}
fn bad(rig: &Rig, function: &str, text: &str, field: &str) {
    let r = rig.call(function, json!({"launch": text}));
    let r = r.as_str().unwrap();
    assert!(
        r.starts_with(&format!("bad-launch:{field}:")),
        "{text}\n  gave {r}, wanted field {field}"
    );
}

#[test]
fn ac4_each_field_outside_its_grammar_is_bad_launch_with_no_stream_and_no_process() {
    let rig = Rig::new(&[]);
    let long = "x".repeat(65 * 1024);
    let agent = |extra: Value| {
        let mut a =
            json!({"name":"rev","description":"d","prompt":"p","tools":["Read"],"model":"sonnet"});
        for (k, v) in extra.as_object().unwrap() {
            a[k] = v.clone();
        }
        launch(json!({"tools":["Read"],"subagents":[a]}))
    };
    let cases: Vec<(String, &str)> = vec![
        (
            r#"{"version":1,"workspace":"scratch","bogus":1}"#.into(),
            "bogus",
        ),
        (r#"{"version":2,"workspace":"scratch"}"#.into(), "version"),
        (r#"{"workspace":"scratch"}"#.into(), "version"),
        (launch(json!({"model":"bad model"})), "model"),
        (launch(json!({"model":""})), "model"),
        (
            launch(json!({"fallback-models":["a","b","c","d","e"]})),
            "fallback-models",
        ),
        (launch(json!({"effort":"extreme"})), "effort"),
        (launch(json!({"tools":["Bash"]})), "tools"),
        (launch(json!({"tools":["Nope"]})), "tools"),
        (launch(json!({"allow":["Read"]})), "allow"),
        (launch(json!({"allow":["Write"]})), "allow"),
        (launch(json!({"allow":["Read(/etc/**)"]})), "allow"),
        (launch(json!({"allow":["Read(~/x)"]})), "allow"),
        (launch(json!({"allow":["Read(//x)"]})), "allow"),
        (launch(json!({"allow":["Glob(../x)"]})), "allow"),
        (launch(json!({"allow":["Bash(ls)"]})), "allow"),
        (launch(json!({"allow":["Read(a\nb)"]})), "allow"),
        (launch(json!({"allow":["Read(a(b))"]})), "allow"),
        (launch(json!({"deny":["Bad Name"]})), "deny"),
        (launch(json!({"mode":"auto"})), "mode"),
        (launch(json!({"mode":"bypassPermissions"})), "mode"),
        (agent(json!({"permissionMode":"auto"})), "subagents"),
        (agent(json!({"tools":["Write"]})), "subagents"),
        (agent(json!({"name":"Bad Name"})), "subagents"),
        (launch(json!({"subagent-depth":4})), "subagent-depth"),
        (launch(json!({"max-turns":0})), "max-turns"),
        (launch(json!({"max-turns":1001})), "max-turns"),
        (launch(json!({"max-budget-usd":0})), "max-budget-usd"),
        (launch(json!({"session":{"new":"x"}})), "session"),
        (
            launch(
                json!({"session":{"new":"7b1f1b7e-8c4a-4b9e-9d0a-2f5d6c1a3e4f","at":"7b1f1b7e-8c4a-4b9e-9d0a-2f5d6c1a3e4f"}}),
            ),
            "session",
        ),
        (launch(json!({"profile":"a b"})), "profile"),
        (
            launch(json!({"system-prompt":{"replace":long}})),
            "system-prompt",
        ),
        (
            launch(json!({"system-prompt":{"replace":"a","append":"b"}})),
            "system-prompt",
        ),
        (
            launch(json!({"json-schema":{"type":"nonsense"}})),
            "json-schema",
        ),
        // What later stages build (15, "A2 in two slices"): each names its stage.
        (launch(json!({"ask":"yes"})), "ask"),
        (
            launch(json!({"pre-tool-check":{"timeout-s":4}})),
            "pre-tool-check",
        ),
        (
            launch(json!({"pre-tool-check":{"timeout-s":121}})),
            "pre-tool-check",
        ),
        (launch(json!({"pre-tool-check":{}})), "pre-tool-check"),
        (
            launch(json!({"pre-tool-check":{"timeout-s":5,"x":1}})),
            "pre-tool-check",
        ),
        // Built by S103 (19.5): a value that is no handle, and a handle the origin client does
        // not hold, are `bad-launch {field: "agent-handle"}`; neither names a stage.
        (launch(json!({"agent-handle":"h"})), "agent-handle"),
        (launch(json!({"agent-handle":7})), "agent-handle"),
        (
            launch(json!({"agent-handle":"A".repeat(32)})),
            "agent-handle",
        ),
        (
            launch(json!({"agent-handle":"a".repeat(32)})),
            "agent-handle",
        ),
        (
            launch(json!({"skills":[{"name":"s","files":{}}]})),
            "skills",
        ),
        (launch(json!({"sandbox":{"enabled":true}})), "sandbox"),
        (launch(json!({"workspace":"/tmp/x"})), "workspace"),
        (launch(json!({"workspace":"scratch:xyz"})), "workspace"),
        (
            launch(json!({"workspace":format!("scratch:{}", "a".repeat(32))})),
            "workspace",
        ),
        (
            launch(json!({"extra-workspaces":["scratch"]})),
            "extra-workspaces",
        ),
        (
            launch(json!({"extra-workspaces":[format!("scratch:{}", "b".repeat(32))]})),
            "extra-workspaces",
        ),
    ];
    for (text, field) in &cases {
        bad(&rig, "api.start", text, field);
    }
    let stages = ["skills", "sandbox"];
    for (text, field) in cases.iter().filter(|(_, f)| stages.contains(f)) {
        let r = rig.call("api.start", json!({"launch": text}));
        assert!(
            r.as_str().unwrap().contains("stage")
                || r.as_str().unwrap().contains("slice")
                || r.as_str().unwrap().contains("section 18"),
            "{field}: {r}"
        );
    }
    std::thread::sleep(Duration::from_millis(300));
    assert!(rig.ended().is_empty());
    assert!(rig.log().is_empty(), "a process ran");
    assert!(!rig.home.path().join("runs").exists());
    assert!(!rig.home.path().join("scratch").exists());
}

#[test]
fn ac4_a_scratch_id_that_another_identity_owns_is_bad_launch() {
    let mut rig = Rig::new(&[]);
    rig.close();
    let id = "c".repeat(32);
    crate::claude::add_scratch(&rig, &id, "local/other");
    rig.open();
    let text = launch(json!({"workspace": format!("scratch:{id}")}));
    bad(&rig, "api.start", &text, "workspace");
    let text = launch(json!({"extra-workspaces": [format!("scratch:{id}")]}));
    bad(&rig, "api.start", &text, "extra-workspaces");
}

#[test]
fn ac4_the_bash_grammar_refuses_what_runs_other_commands_and_allows_plain_ones() {
    let rig = Rig::new(&[]);
    let shell = |rule: &str| {
        let text = launch(json!({"sandbox":{"enabled":true},"allow":[rule]}));
        rig.call("api.start-shell", json!({"launch": text}))
            .as_str()
            .unwrap()
            .to_owned()
    };
    for rule in [
        "Bash",
        "Bash()",
        "Bash(:*)",
        "Bash(python3 x)",
        "Bash(python3.12:*)",
        "Bash(node x)",
        "Bash(sh -c x)",
        "Bash(ls; rm x)",
        "Bash(ls | wc)",
        "Bash(ls $HOME)",
        "Bash(/bin/ls)",
        "Bash(ls:x)",
        "Bash(~/x)",
        "Bash(find . -exec rm {} +)",
        "Bash(find . -ok x)",
        "Bash(find * -name x)",
        "Bash(npm run x)",
        "Bash(npm test:*)",
        "Bash(npm install -g x)",
        "Bash(npm)",
        "Bash(yarn ls)",
        "Bash(env x)",
        "Bash(xargs rm)",
        "Bash(ls é)",
    ] {
        let r = shell(rule);
        assert!(r.starts_with("bad-launch:allow:"), "{rule} gave {r}");
    }
    for rule in [
        "Bash(ls:*)",
        "Bash(npm test)",
        "Bash(git status)",
        "Bash(find . -name x)",
        "Bash(uv sync)",
    ] {
        let r = shell(rule);
        assert_eq!(r.len(), 32, "{rule} gave {r}");
    }
    // Deny rules are not held to the grammar.
    let text = launch(json!({"sandbox":{"enabled":true},"deny":["Bash(python3:*)"]}));
    assert_eq!(
        rig.call("api.start-shell", json!({"launch": text}))
            .as_str()
            .unwrap()
            .len(),
        32
    );
}

/// Adversarial (S99): Claude Code 2.1.330's rule parser (`Ur`) drops a content of `""` or `"*"`
/// and keeps the bare tool name, so `Write(*)` and `Read()` are the bare `Write` and `Read`,
/// which match every path; 19.3 (Protected reads) refuses a bare file-tool allow.
#[test]
fn adv_s99_file_allow_with_an_empty_or_star_argument_is_a_bare_name() {
    let rig = Rig::new(&[]);
    for rule in [
        "Write(*)",
        "Read(*)",
        "Edit(*)",
        "NotebookEdit(*)",
        "Glob(*)",
        "Grep(*)",
        "Read()",
        "Write()",
    ] {
        let text = launch(json!({
            "tools": ["Read", "Write", "Edit", "Glob", "Grep", "NotebookEdit"],
            "allow": [rule],
        }));
        bad(&rig, "api.start", &text, "allow");
    }
}

#[test]
fn ac1_s99_a_file_allow_with_a_relative_pattern_that_holds_a_star_is_kept() {
    let rig = Rig::new(&[]);
    for rule in [
        "Read(**)",
        "Write(*.rs)",
        "Edit(src/*)",
        "Glob(**/*)",
        "Grep(**)",
    ] {
        let text = launch(json!({
            "tools": ["Read", "Write", "Edit", "Glob", "Grep"],
            "allow": [rule],
        }));
        let r = rig.call("api.start", json!({"launch": text}));
        assert_eq!(r.as_str().unwrap().len(), 32, "{rule} gave {r}");
    }
}

// ---- S100 AC1: ask and pre-tool-check ----------------------------------------------------

#[test]
fn s100_ac1_ask_true_gives_the_host_prompt_flags_in_the_place_of_none() {
    let rig = Rig::new(&[]);
    let s = rig.start_ok(&launch(json!({"ask":true,"mode":"manual"})));
    rig.wait_items(&s, 2);
    let args = rig.entries("ARG");
    let at = |a: &str| {
        args.iter()
            .position(|x| x == a)
            .unwrap_or_else(|| panic!("{a}"))
    };
    assert_eq!(
        at("--permission-prompts=host"),
        at("--permission-mode=default") + 1
    );
    assert_eq!(
        at("--permission-prompt-tool=stdio"),
        at("--permission-prompts=host") + 1
    );
    assert!(!args.contains(&"--permission-prompts=none".to_owned()));
}

#[test]
fn s100_ac1_pre_tool_check_puts_the_hook_in_initialize_for_5_to_120_seconds() {
    let rig = Rig::new(&[]);
    for t in [5, 120] {
        let s = rig.start_ok(&launch(json!({"pre-tool-check":{"timeout-s":t}})));
        rig.wait_items(&s, 2);
    }
    let inits: Vec<Value> = rig
        .entries("IN")
        .iter()
        .filter(|l| l.contains(r#""subtype":"initialize""#))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(inits.len(), 2);
    let mut timeouts: Vec<u64> = vec![];
    for init in &inits {
        let hook = &init["request"]["hooks"]["PreToolUse"];
        assert_eq!(hook.as_array().unwrap().len(), 1);
        assert_eq!(hook[0]["matcher"], "*");
        assert_eq!(hook[0]["onFailure"], "block");
        assert_eq!(hook[0]["hookCallbackIds"].as_array().unwrap().len(), 1);
        timeouts.push(hook[0]["timeout"].as_u64().unwrap());
    }
    timeouts.sort_unstable();
    assert_eq!(timeouts, [5, 120]);
}

#[test]
fn s100_ac1_a_callback_needs_hook_support_in_the_answer_to_initialize() {
    for reply in [
        r#"{"hooks_applied":true}"#,
        r#"{"hook_on_failure_supported":true,"hooks_applied":false}"#,
        r#"{"hook_on_failure_supported":false}"#,
    ] {
        let rig = Rig::new(&[("reply", reply.as_bytes())]);
        let s = rig.start_ok(&launch(json!({"pre-tool-check":{"timeout-s":5}})));
        let ended = rig.wait_ended(1);
        assert_eq!(ended, [format!("{s} mismatch(hooks)")], "{reply}");
        assert!(rig.items(&s).is_empty(), "{reply}");
    }
    // Without a callback the answer is not read for it.
    let rig = Rig::new(&[("reply", br#"{}"#)]);
    let s = rig.start_ok(LAUNCH_PLAIN);
    rig.wait_items(&s, 2);
}

const LAUNCH_PLAIN: &str = r#"{"version":1,"workspace":"scratch"}"#;
