use crate::key::NodeKey;
use std::{fmt, path::PathBuf};

/// The error codes of the wire (TECHNICAL 4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    NotAllowed,
    OverBudget,
    TooLarge,
    BadRequest,
    UnknownOp,
    NotFound,
    Busy,
    BadCode,
    Declined,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::NotAllowed => "not_allowed",
            Code::OverBudget => "over_budget",
            Code::TooLarge => "too_large",
            Code::BadRequest => "bad_request",
            Code::UnknownOp => "unknown_op",
            Code::NotFound => "not_found",
            Code::Busy => "busy",
            Code::BadCode => "bad_code",
            Code::Declined => "declined",
        }
    }
    pub(crate) fn parse(text: &str) -> Option<Code> {
        Some(match text {
            "not_allowed" => Code::NotAllowed,
            "over_budget" => Code::OverBudget,
            "too_large" => Code::TooLarge,
            "bad_request" => Code::BadRequest,
            "unknown_op" => Code::UnknownOp,
            "not_found" => Code::NotFound,
            "busy" => Code::Busy,
            "bad_code" => Code::BadCode,
            "declined" => Code::Declined,
            _ => return None,
        })
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every message names the node in its short form (NET-F-32).
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("net is turned off")]
    Off,
    #[error("node {} is unreachable: {cause}", node.short())]
    Unreachable { node: NodeKey, cause: String },
    #[error("node {} refused: {code}", node.short())]
    Refused { node: NodeKey, code: Code },
    #[error("node {} did not answer in time", node.short())]
    Timeout { node: NodeKey },
    #[error("a message of node {} is over its byte limit", node.short())]
    TooLarge { node: NodeKey },
    #[error("node {} sent a bad object: {cause}", node.short())]
    BadObject { node: NodeKey, cause: String },
    #[error("this home does not hold the participant key")]
    NotAnchor,
    #[error("no full machine is reachable")]
    NoFullMachine,
    #[error("not a pairing code")]
    BadCode,
    #[error("the pairing code was used")]
    CodeSpent,
    #[error("the participant declined")]
    Declined,
    #[error("this machine already has an anchor")]
    HasAnchor,
    #[error("not a valid device list: {cause}")]
    BadList { cause: String },
    #[error("{} is corrupt: {cause}", path.display())]
    Corrupt { path: PathBuf, cause: String },
    #[error("{}: {cause}", path.display())]
    Io { path: PathBuf, cause: String },
}
