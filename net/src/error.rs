//! NetError and the wire's error codes (TECHNICAL 3.2, 4.2).

use crate::key::NodeKey;
use std::fmt;
use std::path::PathBuf;
use watane_log::PublicKey;

/// The error codes of the wire (TECHNICAL 4.2).
#[expect(
    clippy::enum_variant_names,
    reason = "BadCode is a bad pairing code, not a bad Code; its wire name is bad_code"
)]
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
    /// A full machine answered and did not serve the package (TECHNICAL 5.9).
    #[error("node {} did not serve {cause}", node.short())]
    NotServed { node: NodeKey, cause: String },
    /// The holder of this machine did not take a page `need` pulled.
    #[error("this machine did not take channel {channel}: {cause}")]
    NotTaken { channel: PublicKey, cause: String },
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
    #[error("the pairing code was used or has expired; make a new one on the anchor")]
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

impl NetError {
    /// What this error says once the node is named by an outer message: the
    /// text without its `node <short>` (NET-F-32), to nest in a cause.
    pub(crate) fn cause_text(&self) -> String {
        match self {
            NetError::Unreachable { cause, .. } => cause.clone(),
            NetError::Refused { code, .. } => format!("refused: {code}"),
            NetError::Timeout { .. } => "did not answer in time".to_owned(),
            NetError::NotServed { cause, .. } => format!("did not serve {cause}"),
            NetError::TooLarge { .. } => "a message is over its byte limit".to_owned(),
            NetError::BadObject { cause, .. } => format!("sent a bad object: {cause}"),
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC4: a nested error names no node.
    #[test]
    fn ac4_cause_text_has_no_node() {
        let node = NodeKey::from_bytes([3; 32]);
        let short = node.short();
        for e in [
            NetError::Unreachable {
                node,
                cause: "x".into(),
            },
            NetError::Refused {
                node,
                code: Code::NotAllowed,
            },
            NetError::Timeout { node },
            NetError::TooLarge { node },
            NetError::BadObject {
                node,
                cause: "y".into(),
            },
            NetError::NotServed {
                node,
                cause: "package ab12cd34: z".into(),
            },
        ] {
            assert!(!e.cause_text().contains(&short), "{e:?}");
        }
        assert_eq!(
            NetError::Refused {
                node,
                code: Code::NotAllowed
            }
            .cause_text(),
            "refused: not_allowed"
        );
    }

    /// AC2, AC3, AC5: the texts of the two variants.
    #[test]
    fn ac5_not_served_and_not_taken_texts() {
        let node = NodeKey::from_bytes([3; 32]);
        let served = NetError::NotServed {
            node,
            cause: "package ab12cd34: it does not have it".into(),
        };
        assert_eq!(
            served.to_string(),
            format!(
                "node {} did not serve package ab12cd34: it does not have it",
                node.short()
            )
        );
        assert_eq!(
            served.cause_text(),
            "did not serve package ab12cd34: it does not have it"
        );
        assert!(!served.to_string().contains("unreachable"));
        let channel = PublicKey::from_bytes(
            &ed25519_dalek::SigningKey::from_bytes(&[4; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let taken = NetError::NotTaken {
            channel,
            cause: "not wanted".into(),
        };
        assert_eq!(
            taken.to_string(),
            format!("this machine did not take channel {channel}: not wanted")
        );
        assert_eq!(
            NetError::NoFullMachine.to_string(),
            "no full machine is reachable"
        );
    }
}
