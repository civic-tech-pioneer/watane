#![allow(clippy::unwrap_used, reason = "tests may panic")]
//! `host-info` (TECHNICAL 19.1) and the linking of the claude interfaces (19.1, Linking).

use crate::claude::Rig;
use crate::streams::{call_as, fixture};
use serde_json::{Value, json};
use watane_host::{CallCode, Caller, Harness};
use watane_pkg::Grant;

#[test]
fn ac1_generation_is_1_at_the_first_open_and_2_after_a_close_and_a_second_open() {
    let mut rig = Rig::new(&[]);
    for _ in 0..2 {
        assert_eq!(rig.call("api.generation", Value::Null), json!(1));
        let b = rig
            .call_as(Caller::Page, "claude-b", "api.generation", Value::Null)
            .unwrap();
        assert_eq!(b, json!(1));
    }
    rig.close();
    // The `host` table holds the number.
    assert_eq!(crate::claude::stored_generation(&rig), 1);
    rig.open();
    assert_eq!(rig.call("api.generation", Value::Null), json!(2));
    let b = rig
        .call_as(Caller::Page, "claude-b", "api.generation", Value::Null)
        .unwrap();
    assert_eq!(b, json!(2));
}

#[test]
fn ac1_the_harness_fake_returns_1() {
    let harness = Harness::open(&fixture("claude"), Default::default()).unwrap();
    assert_eq!(
        harness.call("api.generation", Value::Null).unwrap().value,
        json!(1)
    );
}

#[test]
fn the_harness_fakes_return_s1_and_ok_and_record_the_claude_requests() {
    // 19.1, Harness: `start` returns `s<n>`, a session function ok, each one a request.
    let harness = Harness::open(&fixture("claude"), Default::default()).unwrap();
    let start = harness
        .call("api.start", json!({"launch": crate::claude::LAUNCH}))
        .unwrap();
    assert_eq!(start.value, json!("s1"));
    let say = harness
        .call("api.say", json!({"s": "s1", "text": "hello"}))
        .unwrap();
    assert_eq!(say.value, json!("ok"));
    let record = |r: &watane_host::RequestRecord| json!([r.function, r.args]);
    assert_eq!(
        start.requests.iter().map(record).collect::<Vec<_>>(),
        [json!(["claude-code.start", {"launch": crate::claude::LAUNCH}])]
    );
    assert_eq!(
        say.requests.iter().map(record).collect::<Vec<_>>(),
        [json!(["claude-session.say", {"s": "s1", "content": "hello"}])]
    );
}

#[test]
fn ac3_claude_code_without_its_grant_traps_not_granted() {
    let rig = Rig::new(&[]);
    let local = crate::streams::local("claude");
    rig.host().revoke(&local, &Grant::ClaudeCode).unwrap();
    let e = rig
        .call_as(
            Caller::Page,
            "claude",
            "api.start",
            json!({"launch": crate::claude::LAUNCH}),
        )
        .unwrap_err();
    assert_eq!(e.code, CallCode::NotGranted);
    assert!(
        e.message
            .contains("claude has no grant for watane:host/claude-code"),
        "{}",
        e.message
    );
    rig.host().revoke(&local, &Grant::ClaudeCodeShell).unwrap();
    let e = rig
        .call_as(
            Caller::Page,
            "claude",
            "api.start-shell",
            json!({"launch": crate::claude::LAUNCH}),
        )
        .unwrap_err();
    assert_eq!(e.code, CallCode::NotGranted);
    assert!(
        e.message.contains("watane:host/claude-code-shell"),
        "{}",
        e.message
    );
    // `claude-session`, `claude-types` and `host-info` link with no grant: `claude-b` holds none.
    let b = call_as(
        rig.host(),
        Caller::Page,
        "claude-b",
        "api.say",
        json!({"s": "0".repeat(32), "text": "x"}),
    )
    .unwrap();
    assert_eq!(b, json!("unknown-stream"));
}
