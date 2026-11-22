//! `AccountSource`: the in-process trait through which the host reaches tokens, keys and the
//! `claude` program path (TECHNICAL 19.4). The app implements it and hands it over through
//! `Confirm::accounts`. In stage A2 the host calls `claude_binary` only.

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
