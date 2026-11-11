use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("home {} is busy", .0.display())]
    Busy(PathBuf),
    #[error("home {} has mode {found}, incompatible with requested mode", path.display())]
    ModeMismatch { path: PathBuf, found: &'static str },
    #[error("bad home {}: {reason}", path.display())]
    BadHome { path: PathBuf, reason: String },
    #[error("{0}")]
    Refused(String),
    #[error("missing providers: {0}")]
    MissingProviders(String),
    #[error("call cycle: {0}")]
    Cycle(String),
    #[error("ambiguous provider: {0}")]
    AmbiguousProvider(String),
    #[error("invalid mod {input}: {reason}")]
    InvalidMod { input: String, reason: String },
    #[error("invalid name {value:?}: {rule}")]
    BadName { value: String, rule: &'static str },
    #[error("no mod {0}")]
    NoSuchMod(String),
    #[error("invalid toolset entry {entry}: {reason}")]
    BadToolsetEntry { entry: String, reason: String },
    #[error("no toolset {0}")]
    NoSuchToolset(String),
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
            Self::Refused(_) => "refused",
            Self::MissingProviders(_) => "missing_providers",
            Self::Cycle(_) => "cycle",
            Self::AmbiguousProvider(_) => "ambiguous_provider",
            Self::ModeMismatch { .. } => "mode_mismatch",
            Self::BadHome { .. } => "bad_home",
            Self::InvalidMod { .. } => "invalid_mod",
            Self::BadName { .. } => "bad_name",
            Self::NoSuchMod(_) => "no_mod",
            Self::BadToolsetEntry { .. } => "bad_toolset_entry",
            Self::NoSuchToolset(_) => "no_toolset",
            Self::PortInUse(_) => "port_in_use",
            Self::AlreadyServing => "already_serving",
            Self::Internal(_) => "internal",
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            Self::Refused(_) => 403,
            Self::BadName { .. } | Self::BadToolsetEntry { .. } => 400,
            Self::NoSuchMod(_) | Self::NoSuchToolset(_) => 404,
            Self::Busy(_) | Self::ModeMismatch { .. } => 409,
            Self::InvalidMod { .. }
            | Self::MissingProviders(_)
            | Self::Cycle(_)
            | Self::AmbiguousProvider(_) => 422,
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
