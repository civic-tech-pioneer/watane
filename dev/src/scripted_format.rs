//! The scripted Responses file format shared by the server and dev previews (TECHNICAL 18.3).

use serde::{Deserialize, Serialize};

/// What the Responses origin answers to `POST /v1/responses`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Reply {
    Sse(Vec<SseEvent>),
    Status {
        code: u16,
        body: String,
    },
    UsageLimit {
        resets_in_s: u64,
    },
    Redirect(String),
    Reset,
    Stall,
    Raw {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

/// One event of an event stream, sent as one write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum SseEvent {
    Delta(String),
    ToolCalls(usize),
    Completed {
        input_tokens: u64,
        output_tokens: u64,
    },
    Failed {
        code: String,
    },
    Raw(String),
    Cut,
}

/// What the judge origin does with a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum Play {
    Safe(Vec<String>),
    Unsafe(Vec<String>),
    Unsure(Vec<String>),
    Down,
    Slow {
        ms: u64,
        then: Box<Play>,
    },
    Unreadable,
    Redirect(String),
    Raw {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
}

/// The standalone program and maker preview share the same script schema.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub token: Option<String>,
    pub models: Option<Vec<String>>,
    pub replies: Option<Vec<Reply>>,
    pub judge_key: Option<String>,
    pub plays: Option<Vec<Play>>,
}

impl std::fmt::Debug for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Script").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::Script;

    #[test]
    fn ac3_unknown_fields_are_refused_at_every_level() {
        for (script, field) in [
            (r#"{"replise":[]}"#, "replise"),
            (r#"{"_comment":"x"}"#, "_comment"),
            (
                r#"{"replies":[{"usage-limit":{"resets_in_s":60,"extra":0}}]}"#,
                "extra",
            ),
            (
                r#"{"replies":[{"sse":[{"completed":{"input_tokens":1,"output_tokens":1,"extra":0}}]}]}"#,
                "extra",
            ),
            (
                r#"{"plays":[{"slow":{"ms":1,"then":"down","extra":0}}]}"#,
                "extra",
            ),
        ] {
            let error = serde_json::from_str::<Script>(script)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(&format!("unknown field `{field}`")),
                "{script}: {error}"
            );
        }
    }
}
