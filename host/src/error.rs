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
