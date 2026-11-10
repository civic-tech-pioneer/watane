use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("home {} is busy", .0.display())]
    Busy(PathBuf),
    #[error("home {} has mode {found}, expected preview", path.display())]
    ModeMismatch { path: PathBuf, found: &'static str },
    #[error("bad home {}: {reason}", path.display())]
    BadHome { path: PathBuf, reason: String },
    #[error("invalid mod {input}: {reason}")]
    InvalidMod { input: String, reason: String },
    #[error("invalid name {value:?}: {rule}")]
    BadName { value: String, rule: &'static str },
    #[error("cannot bind loopback port {0}")]
    PortInUse(u16),
    #[error("host is already serving")]
    AlreadyServing,
    #[error("{0}")]
    Internal(String),
}

impl HostError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Busy(_) => "busy",
            Self::ModeMismatch { .. } => "mode_mismatch",
            Self::BadHome { .. } => "bad_home",
            Self::InvalidMod { .. } => "invalid_mod",
            Self::BadName { .. } => "bad_name",
            Self::PortInUse(_) => "port_in_use",
            Self::AlreadyServing => "already_serving",
            Self::Internal(_) => "internal",
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::BadName { .. } => 400,
            Self::Busy(_) | Self::ModeMismatch { .. } => 409,
            Self::InvalidMod { .. } => 422,
            _ => 500,
        }
    }

    pub(crate) fn json(&self) -> Value {
        json!({"code":self.code(), "message":self.to_string()})
    }
}

impl From<std::io::Error> for HostError {
    fn from(error: std::io::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<serde_json::Error> for HostError {
    fn from(error: serde_json::Error) -> Self {
        Self::Internal(error.to_string())
    }
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct CallError {
    pub code: CallCode,
    pub message: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallCode {
    BadRequest,
    NoMod,
    NoFunction,
    Disabled,
    NotGranted,
    Trap,
    Limit(Limit),
    Internal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Limit {
    Fuel,
    Memory,
    Time,
    Depth,
    StateSize,
    StateQuota,
}
impl CallError {
    pub(crate) fn new(code: CallCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::new(CallCode::Internal, error.to_string())
    }
    pub(crate) fn limit(limit: Limit, message: impl Into<String>) -> Self {
        Self::new(CallCode::Limit(limit), message)
    }
    pub fn status(&self) -> u16 {
        match self.code {
            CallCode::BadRequest => 400,
            CallCode::NoMod | CallCode::NoFunction => 404,
            CallCode::Disabled => 409,
            CallCode::NotGranted | CallCode::Trap | CallCode::Limit(_) => 422,
            CallCode::Internal => 500,
        }
    }
}
impl serde::Serialize for CallError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let code = match self.code {
            CallCode::BadRequest => "bad_request",
            CallCode::NoMod => "no_mod",
            CallCode::NoFunction => "no_function",
            CallCode::Disabled => "disabled",
            CallCode::NotGranted => "not_granted",
            CallCode::Trap => "trap",
            CallCode::Limit(_) => "limit",
            CallCode::Internal => "internal",
        };
        let mut out = serializer.serialize_struct(
            "CallError",
            if matches!(self.code, CallCode::Limit(_)) {
                3
            } else {
                2
            },
        )?;
        out.serialize_field("code", code)?;
        if let CallCode::Limit(limit) = self.code {
            out.serialize_field("limit", &limit)?;
        }
        out.serialize_field("message", &self.message)?;
        out.end()
    }
}
