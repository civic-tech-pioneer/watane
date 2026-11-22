//! The control protocol, lines in and out (TECHNICAL 19.3, The control protocol): what each
//! stdout line of `claude` becomes, and the only lines the host writes to stdin.

use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde_json::{Value, json};
use std::collections::HashSet;

/// The top-level `type` of a watane item: a `claude` line that says one is wrapped (Q36).
const HOST_ITEM_TYPES: [&str; 8] = [
    "ask",
    "check",
    "ask-resolved",
    "initialized",
    "wrapped",
    "consent-pending",
    "verdict",
    "checking",
];

/// What the host does with one stdout line.
#[derive(Debug, PartialEq)]
pub(crate) enum Line {
    /// Push these bytes as one item.
    Item(Vec<u8>),
    /// Answer the control request deny at once and push nothing (asks and checks wait for S100).
    Deny { request_id: String, hook: bool },
    /// A `system/init` line: checked, then pushed unchanged.
    Init(Value, Vec<u8>),
    /// The answer to a control request of the host.
    Response { request_id: String, ok: bool },
}

/// Whether the top level of a JSON object text has a key twice.
fn has_duplicate_key(text: &str) -> bool {
    struct Keys;
    impl<'de> Visitor<'de> for Keys {
        type Value = bool;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<bool, A::Error> {
            let mut seen = HashSet::new();
            let mut twice = false;
            while let Some(key) = map.next_key::<String>()? {
                twice |= !seen.insert(key);
                map.next_value::<IgnoredAny>()?;
            }
            Ok(twice)
        }
    }
    let mut de = serde_json::Deserializer::from_str(text);
    serde::Deserializer::deserialize_map(&mut de, Keys).unwrap_or(false)
}

/// `{"type":"wrapped","line":"<the line as a JSON string>"}`: bytes that are not UTF-8 become U+FFFD.
fn wrapped(raw: &[u8]) -> Line {
    let text = String::from_utf8_lossy(raw);
    // Written by hand: the key order is part of the item.
    let line = Value::String(text.into_owned());
    Line::Item(format!(r#"{{"type":"wrapped","line":{line}}}"#).into_bytes())
}

/// Decides what one line (without its newline) becomes (19.3, step 4).
pub(crate) fn classify(raw: &[u8]) -> Line {
    let Ok(text) = std::str::from_utf8(raw) else {
        return wrapped(raw);
    };
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return wrapped(raw);
    };
    if !value.is_object() || has_duplicate_key(text) {
        return wrapped(raw);
    }
    let kind = value.get("type").and_then(Value::as_str);
    if kind.is_some_and(|t| HOST_ITEM_TYPES.contains(&t)) {
        return wrapped(raw);
    }
    match kind {
        Some("control_request") => {
            let subtype = value.pointer("/request/subtype").and_then(Value::as_str);
            let id = value.get("request_id").and_then(Value::as_str);
            match (subtype, id) {
                (Some(s @ ("can_use_tool" | "hook_callback")), Some(id)) => Line::Deny {
                    request_id: id.to_owned(),
                    hook: s == "hook_callback",
                },
                _ => Line::Item(raw.to_vec()),
            }
        }
        Some("control_response") => {
            let id = value
                .pointer("/response/request_id")
                .and_then(Value::as_str);
            match id {
                Some(id) => Line::Response {
                    request_id: id.to_owned(),
                    ok: value.pointer("/response/subtype").and_then(Value::as_str)
                        == Some("success"),
                },
                None => Line::Item(raw.to_vec()),
            }
        }
        Some("system") if value.get("subtype").and_then(Value::as_str) == Some("init") => {
            Line::Init(value, raw.to_vec())
        }
        _ => Line::Item(raw.to_vec()),
    }
}

/// Whether a line, as `classify` made it, is a control line that `claude` wrote: a
/// `control_request` or `control_response` object (a wrapped line has the type `wrapped`).
pub(crate) fn is_control(line: &Line) -> bool {
    match line {
        Line::Deny { .. } | Line::Response { .. } => true,
        Line::Item(body) => serde_json::from_slice::<Value>(body).is_ok_and(|v| {
            matches!(
                v.get("type").and_then(Value::as_str),
                Some("control_request" | "control_response")
            )
        }),
        Line::Init(..) => false,
    }
}

// ---- what the host writes (19.3, step 5 and Wire shapes) ---------------------------------

pub(crate) fn initialize(request_id: &str) -> Vec<u8> {
    json!({"type":"control_request","request_id":request_id,"request":{"subtype":"initialize"}})
        .to_string()
        .into_bytes()
}
/// A `user` message: `say` and `compact`.
pub(crate) fn user(text: &str) -> Vec<u8> {
    json!({"type":"user","message":{"role":"user","content":text}})
        .to_string()
        .into_bytes()
}
pub(crate) fn interrupt(request_id: &str) -> Vec<u8> {
    json!({"type":"control_request","request_id":request_id,"request":{"subtype":"interrupt"}})
        .to_string()
        .into_bytes()
}
/// The deny that answers a `can_use_tool` (or, as a hook answer, a `hook_callback`) at once.
pub(crate) fn deny(request_id: &str, hook: bool) -> Vec<u8> {
    let reason = "the host answers deny: asks and checks are not built yet";
    let body = if hook {
        json!({"hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }})
    } else {
        json!({"behavior": "deny", "message": reason})
    };
    json!({"type":"control_response","response":{
        "subtype":"success","request_id":request_id,"response":body}})
    .to_string()
    .into_bytes()
}

/// Characters that `say` skips before it looks for a `/` (19.3, step 5): what JavaScript's
/// `trim` removes, and every character of the Unicode categories Z, Cc and Cf.
fn skipped(c: char) -> bool {
    c.is_whitespace()
        || c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061C}'
                | '\u{06DD}'
                | '\u{070F}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08E2}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{110BD}'
                | '\u{110CD}'
                | '\u{13430}'..='\u{1343F}'
                | '\u{1BCA0}'..='\u{1BCA3}'
                | '\u{1D173}'..='\u{1D17A}'
                | '\u{E0001}'
                | '\u{E0020}'..='\u{E007F}'
        )
}
/// A text that `claude` would read as a slash command (S64).
pub(crate) fn is_slash_command(text: &str) -> bool {
    text.chars().find(|c| !skipped(*c)) == Some('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(raw: &[u8]) -> String {
        match classify(raw) {
            Line::Item(b) => String::from_utf8(b).unwrap(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_plain_line_goes_through_unchanged() {
        let line = br#"{"type":"assistant","message":{"role":"assistant"}}"#;
        assert_eq!(item(line), String::from_utf8_lossy(line));
    }

    #[test]
    fn a_host_item_type_is_wrapped() {
        assert_eq!(
            item(br#"{"type":"ask","id":"1"}"#),
            r#"{"type":"wrapped","line":"{\"type\":\"ask\",\"id\":\"1\"}"}"#
        );
    }

    #[test]
    fn every_host_item_type_is_wrapped() {
        for kind in [
            "ask",
            "check",
            "ask-resolved",
            "initialized",
            "wrapped",
            "consent-pending",
            "verdict",
            "checking",
        ] {
            let line = format!(r#"{{"type":"{kind}"}}"#);
            let v: Value = serde_json::from_str(&item(line.as_bytes())).unwrap();
            assert_eq!(v, json!({"type": "wrapped", "line": line}), "{kind}");
        }
    }

    #[test]
    fn invalid_utf8_a_second_object_and_a_duplicate_key_are_wrapped() {
        assert!(
            item(b"{\"a\":\"\xff\"}").contains("\\ufffd")
                || item(b"{\"a\":\"\xff\"}").contains('\u{fffd}')
        );
        assert!(item(br#"{"a":1} {"b":2}"#).contains("wrapped"));
        assert!(item(b"[1]").contains("wrapped"));
        assert!(item(br#"{"type":"x","type":"y"}"#).contains("wrapped"));
        assert!(item(b"").contains("wrapped"));
    }

    #[test]
    fn control_lines_are_control_and_wrapped_or_other_lines_are_not() {
        for raw in [
            &br#"{"type":"control_request","request_id":"1","request":{"subtype":"can_use_tool"}}"#
                [..],
            br#"{"type":"control_request","request_id":"1","request":{"subtype":"other"}}"#,
            br#"{"type":"control_response","response":{"subtype":"success","request_id":"1"}}"#,
            br#"{"type":"control_response","response":{}}"#,
        ] {
            assert!(
                is_control(&classify(raw)),
                "{}",
                String::from_utf8_lossy(raw)
            );
        }
        for raw in [
            &br#"{"type":"control_request","type":"control_request"}"#[..],
            br#"{"type":"assistant"}"#,
            br#"{"type":"system","subtype":"init"}"#,
            b"{\"type\":\"control_response\",\"x\":\"\xff\"}",
        ] {
            assert!(
                !is_control(&classify(raw)),
                "{}",
                String::from_utf8_lossy(raw)
            );
        }
    }

    #[test]
    fn slash_commands_are_found_after_white_space_and_invisible_characters() {
        for text in [
            "/x",
            " /x",
            "\u{feff}/x",
            "\u{200b}/x",
            "\t\n/x",
            "\u{00a0}\u{2028}/x",
        ] {
            assert!(is_slash_command(text), "{text:?}");
        }
        for text in ["hello", "a /x", "", "  ", "x/"] {
            assert!(!is_slash_command(text), "{text:?}");
        }
    }
}
