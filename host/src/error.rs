//! Errors (TECHNICAL 2.2): `HostError` for host operations and `CallError` for
//! calls, with their codes, HTTP statuses and JSON form.

use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyCheck {
    Kind,
    ReleaseBody,
    PackageHash,
    Mismatch,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("{0}")]
    BadRequest(String),
    #[error("home {} is busy", .0.display())]
    Busy(PathBuf),
    #[error("home {} has mode {found}, incompatible with requested mode", path.display())]
    ModeMismatch { path: PathBuf, found: &'static str },
    #[error("bad home {}: {reason}", path.display())]
    BadHome { path: PathBuf, reason: String },
    #[error("{0}")]
    Refused(String),
    #[error("missing providers: {}", missing_message(.0))]
    MissingProviders(Vec<crate::MissingProvider>),
    #[error("call cycle: {0}")]
    Cycle(String),
    #[error(
        "{consumer} needs {} and {} installed or planned mods fit: {}; pick one to settle it",
        provider_name(provider),
        candidates.len(),
        join_names(candidates)
    )]
    AmbiguousProvider {
        consumer: crate::LocalName,
        provider: watane_pkg::Provider,
        candidates: Vec<crate::LocalName>,
    },
    #[error("release verification {check:?}: {reason}")]
    Verify { check: VerifyCheck, reason: String },
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
    #[error("plan no longer matches the catalog")]
    StalePlan,
    #[error("{local} does not declare {grant}")]
    NotDeclared {
        local: crate::LocalName,
        grant: watane_pkg::Grant,
    },
    #[error("{0}")]
    Internal(String),
}

impl HostError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::Verify { .. } => "verify",
            Self::Busy(_) => "busy",
            Self::StalePlan => "stale_plan",
            Self::NotDeclared { .. } => "not_declared",
            Self::Refused(_) => "refused",
            Self::MissingProviders(_) => "missing_providers",
            Self::Cycle(_) => "cycle",
            Self::AmbiguousProvider { .. } => "ambiguous_provider",
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
            Self::BadRequest(_) => 400,
            Self::BadName { .. } | Self::BadToolsetEntry { .. } | Self::NotDeclared { .. } => 400,
            Self::NoSuchMod(_) | Self::NoSuchToolset(_) => 404,
            Self::Busy(_)
            | Self::ModeMismatch { .. }
            | Self::StalePlan
            | Self::MissingProviders(_)
            | Self::Cycle(_)
            | Self::AmbiguousProvider { .. } => 409,
            Self::InvalidMod { .. } | Self::Verify { .. } => 422,
            _ => 500,
        }
    }

    pub(crate) fn json(&self) -> Value {
        let mut value = json!({"code":self.code(), "message":self.to_string()});
        if let Self::MissingProviders(missing) = self {
            value["missing"] = json!(missing);
        }
        if let Self::Cycle(path) = self {
            value["cycle"] = json!(path.split(" -> ").collect::<Vec<_>>());
        }
        if let Self::Verify { check, .. } = self {
            value["check"] = json!(check);
        }
        value
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

fn missing_message(missing: &[crate::MissingProvider]) -> String {
    missing
        .iter()
        .map(|m| {
            format!(
                "{} at {} needed by {}",
                provider_name(&m.provider),
                m.version,
                m.needed_by
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl serde::Serialize for HostError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.json().serialize(serializer)
    }
}

fn join_names(names: &[crate::LocalName]) -> String {
    names
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn provider_name(provider: &watane_pkg::Provider) -> String {
    match &provider.publisher {
        Some(key) => format!("{}/{}", &key.to_string()[..10], provider.slug),
        None => provider.to_string(),
    }
}

/// Natural order: digit runs compare by number, other runs by bytes, then fewer runs
/// first. Equal numbers with different leading zeros fall back to the plain byte order
/// of the whole text.
pub(crate) fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn runs(text: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut digit = None;
        for (i, c) in text.char_indices() {
            let now = c.is_ascii_digit();
            if digit.is_some_and(|d| d != now) {
                out.push(&text[start..i]);
                start = i;
            }
            digit = Some(now);
        }
        if start < text.len() {
            out.push(&text[start..]);
        }
        out
    }
    let (ra, rb) = (runs(a), runs(b));
    for (x, y) in ra.iter().zip(&rb) {
        let both_digits = x.starts_with(|c: char| c.is_ascii_digit())
            && y.starts_with(|c: char| c.is_ascii_digit());
        let order = if both_digits {
            let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
            x.len().cmp(&y.len()).then_with(|| x.cmp(y))
        } else {
            x.cmp(y)
        };
        if order.is_ne() {
            return order;
        }
    }
    ra.len().cmp(&rb.len()).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo";

    fn provider(key: Option<&str>) -> watane_pkg::Provider {
        watane_pkg::Provider {
            slug: "shelf".parse().unwrap(),
            publisher: key.map(|k| k.parse().unwrap()),
        }
    }

    fn name(s: &str) -> crate::LocalName {
        crate::LocalName::parse(s).unwrap()
    }

    #[test]
    fn ac1_ambiguous_provider_names_publisher_by_fingerprint() {
        let make = |key| HostError::AmbiguousProvider {
            consumer: name("recs"),
            provider: provider(key),
            candidates: vec![name("shelf"), name("shelf-2")],
        };
        assert_eq!(
            make(Some(KEY)).to_string(),
            "recs needs yryonyebyr/shelf and 2 installed or planned mods fit: shelf, shelf-2; pick one to settle it"
        );
        assert_eq!(
            make(None).to_string(),
            "recs needs local/shelf and 2 installed or planned mods fit: shelf, shelf-2; pick one to settle it"
        );
    }

    #[test]
    fn ac2_missing_providers_names_publisher_by_fingerprint_and_json_keeps_full_key() {
        let error = HostError::MissingProviders(vec![crate::MissingProvider {
            provider: provider(Some(KEY)),
            version: "1.2.0".parse().unwrap(),
            release: None,
            needed_by: "recs".into(),
        }]);
        assert_eq!(
            error.to_string(),
            "missing providers: yryonyebyr/shelf at 1.2.0 needed by recs"
        );
        assert_eq!(
            serde_json::to_value(&error).unwrap()["missing"][0]["provider"]["publisher"],
            KEY
        );
    }

    #[test]
    fn ac3_natural_order_of_candidates() {
        let names = ["shelf-10", "shelf-2", "shelf", "shelf-02"];
        let mut sorted: Vec<&str> = names.to_vec();
        sorted.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(sorted, ["shelf", "shelf-02", "shelf-2", "shelf-10"]);
        // A total order: a shorter run list sorts first, so no cycle a01c < a1 < a1b < a01c.
        let mut sorted = vec!["a1b", "a01c", "a1"];
        sorted.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(sorted, ["a1", "a1b", "a01c"]);
        assert!(natural_cmp("a1", "a01c").is_lt());
    }
}
