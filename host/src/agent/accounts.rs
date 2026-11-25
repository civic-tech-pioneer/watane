//! `AccountSource`: the in-process trait through which the host reaches tokens, keys and the
//! `claude` program path (TECHNICAL 19.4). The app implements it and hands it over through
//! `Confirm::accounts`. Stage A4 adds `openai_token`; `Account` names an entry that changed.

use std::path::PathBuf;
use std::pin::Pin;

pub trait AccountSource: Send + Sync + 'static {
    /// A fresh access token for the account `openai`, refreshed when needed (app 19.1).
    fn openai_token(&self) -> Pin<Box<dyn Future<Output = Result<String, AccountError>> + Send>>;
    /// The key and origin entered for the provider with this identity (`<publisher>/<slug>`).
    fn judge(&self, provider: &str) -> Result<JudgeAccount, AccountError>;
    /// The path of the `claude` program the participant picked (app 19.3).
    fn claude_binary(&self) -> Result<PathBuf, AccountError>;
}

/// The key and the origin of a judge. `Debug` prints no key.
pub struct JudgeAccount {
    pub key: String,
    pub origin: String,
}
impl std::fmt::Debug for JudgeAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JudgeAccount")
            .field("key", &"<hidden>")
            .field("origin", &self.origin)
            .finish()
    }
}

/// `NoAccount`: no sign-in, no key, no program chosen. `Unavailable`: the entry is there but
/// cannot be used now (a token refresh failed on the network); never read as "not signed in".
#[derive(Debug, Clone, PartialEq)]
pub enum AccountError {
    NoAccount,
    Unavailable(String),
}

/// An account whose entry changed (app 19.1 sign-out, 19.4 key removed or replaced, uninstall).
/// `Judge.provider` is the identity of the provider (`<publisher>/<slug>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Account {
    OpenAi,
    Judge { provider: String },
}

/// Canonicalize the HTTPS origin the participant entered (TECHNICAL 19.4).
pub fn judge_origin(text: &str) -> Result<String, crate::HostError> {
    fn bad_origin() -> crate::HostError {
        crate::HostError::BadRequest("not an https origin".into())
    }

    let (scheme, authority) = text.split_once("://").ok_or_else(bad_origin)?;
    let authority = authority.strip_suffix('/').unwrap_or(authority);
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, _)| host);
    if !scheme.eq_ignore_ascii_case("https")
        || authority.is_empty()
        || authority.ends_with(':')
        || host.ends_with('.')
        || authority.contains(['/', '\\', '?', '#', '@'])
        || authority.chars().any(char::is_whitespace)
        || authority.chars().any(char::is_control)
    {
        return Err(bad_origin());
    }

    let url = url::Url::parse(text).map_err(|_| bad_origin())?;
    let name = match url.host() {
        Some(url::Host::Domain(name)) if !name.ends_with('.') => name.to_owned(),
        Some(url::Host::Ipv4(address)) => address.to_string(),
        _ => return Err(bad_origin()),
    };
    match url.port() {
        Some(0) => Err(bad_origin()),
        Some(port) => Ok(format!("https://{name}:{port}")),
        None => Ok(format!("https://{name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::judge_origin;
    use crate::HostError;

    #[test]
    fn ac3_judge_origin_is_canonical_and_has_no_path_or_userinfo() {
        assert_eq!(
            judge_origin("HTTPS://Judge.Example:443/").unwrap(),
            "https://judge.example"
        );
        assert_eq!(
            judge_origin("https://Judge.Example:8443").unwrap(),
            "https://judge.example:8443"
        );
        for text in [
            "http://judge.example",
            "https://judge.example.",
            "https://127.0.0.1.:443",
            "https://judge.example:",
            "https://judge.example:0",
            "https://judge.example/v1",
            "https://judge.example/.",
            "https://judge.example/?",
            "https://judge.example/#",
            "https://@judge.example",
            "https://name@judge.example",
        ] {
            assert!(
                matches!(judge_origin(text), Err(HostError::BadRequest(_))),
                "{text}"
            );
        }
    }
}
