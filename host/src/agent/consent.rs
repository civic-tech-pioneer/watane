//! Consents (TECHNICAL 19.7): the types, the key text, the question to the app, and the
//! withdrawal that ends what depends on a consent.

use crate::agent::streams::Hub;
use crate::{Host, HostError, LocalName, Mode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// One consent the participant gave, kept on the record of the origin client (3.3, 19.7).
///
/// JSON: `{"kind":"workspace","via":"…","folder":"…"}`, `{"kind":"account","via":"…","account":"…"}`,
/// `{"kind":"toolset","toolset":"…","functions":[…]}`. `Display` and `parse` use the `what` key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Consent {
    Workspace {
        via: String,
        folder: String,
    },
    Account {
        via: String,
        account: String,
    },
    Toolset {
        toolset: String,
        functions: Vec<String>,
    },
}

impl std::fmt::Display for Consent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Workspace { via, folder } => write!(f, "workspace:{via}:{folder}"),
            Self::Account { via, account } => write!(f, "account:{via}:{account}"),
            Self::Toolset { toolset, .. } => write!(f, "toolset:{toolset}"),
        }
    }
}

/// An identity as `via`: no `:`, no white space or control character, and not an absolute path.
fn is_via(via: &str) -> bool {
    !via.is_empty()
        && !via.starts_with('/')
        && !via
            .chars()
            .any(|c| c == ':' || c.is_whitespace() || c.is_control())
}

impl Consent {
    /// Reads a `what` key back (19.7). `functions` of a toolset is empty: it is no part of the key.
    /// Refused: a workspace or account key with no `via`, an empty or relative folder, an account
    /// other than `openai`, an empty toolset name, any other kind.
    pub fn parse(what: &str) -> Result<Self, HostError> {
        let bad = |why: &str| HostError::BadRequest(format!("consent {what:?}: {why}"));
        if let Some(rest) = what.strip_prefix("workspace:") {
            let (via, folder) = rest
                .split_once(':')
                .ok_or_else(|| bad("a workspace consent is `workspace:<via>:<folder>`"))?;
            if !is_via(via) {
                return Err(bad("`via` is the identity of the mod whose request waits"));
            }
            if !folder.starts_with('/') || folder.chars().any(char::is_control) {
                return Err(bad("the folder is an absolute path"));
            }
            return Ok(Self::Workspace {
                via: via.into(),
                folder: folder.into(),
            });
        }
        if let Some(rest) = what.strip_prefix("account:") {
            let (via, account) = rest
                .split_once(':')
                .ok_or_else(|| bad("an account consent is `account:<via>:openai`"))?;
            if !is_via(via) {
                return Err(bad("`via` is the identity of the mod whose request waits"));
            }
            if account != "openai" {
                return Err(bad("the only account with a consent is `openai`"));
            }
            return Ok(Self::Account {
                via: via.into(),
                account: account.into(),
            });
        }
        if let Some(name) = what.strip_prefix("toolset:") {
            if name.is_empty() || name.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(bad("a toolset consent is `toolset:<name>`"));
            }
            return Ok(Self::Toolset {
                toolset: name.into(),
                functions: vec![],
            });
        }
        Err(bad("not a workspace, account or toolset consent"))
    }
}

/// What the app needs to ask one consent (19.7). `profile` and `folder` for a workspace,
/// `functions` for a toolset.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsentRequest {
    /// The origin client: {origin client}.
    pub origin: LocalName,
    /// The mod whose request waits (a provider, the kernel): {provider}.
    pub via: LocalName,
    pub consent: Consent,
    pub profile: Option<String>,
    pub functions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentAnswer {
    Granted,
    Refused,
}

/// How long a question waits for the app (19.7, Asking).
const WAIT: Duration = Duration::from_secs(600);
/// How often a waiting question looks at its stream and the clock.
const POLL: Duration = Duration::from_millis(10);

/// What became of a question.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Granted,
    /// `Refused`, no app, a dropped answer or 600 s.
    Refused,
    /// The stream ended while the question waited: the future is dropped.
    Stopped,
}

/// Asks the app one consent and waits for the answer on the calling thread. A preview and a home
/// with no app refuse at once. The future is dropped when `stopped` turns true, or when 600 s
/// have passed on the hub's clock (the one a test moves, 19.9). Fail closed on any error.
pub(crate) fn ask(
    mode: &Mode,
    hub: &Hub,
    stopped: impl Fn() -> bool,
    request: ConsentRequest,
) -> Asked {
    let Mode::Home { app: Some(app) } = mode else {
        return Asked::Refused;
    };
    let mut future = app.consent(request);
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    else {
        return Asked::Refused;
    };
    let deadline = hub.now() + WAIT;
    runtime.block_on(async {
        loop {
            tokio::select! {
                answer = &mut future => {
                    return match answer {
                        ConsentAnswer::Granted => Asked::Granted,
                        ConsentAnswer::Refused => Asked::Refused,
                    };
                }
                () = tokio::time::sleep(POLL) => {
                    if stopped() {
                        return Asked::Stopped;
                    }
                    if hub.now() >= deadline {
                        return Asked::Refused;
                    }
                }
            }
        }
    })
}

/// A consent was withdrawn from the origin client with this identity: ends what depends on it
/// (19.7, Withdrawal). A workspace consent ends the `claude` streams of that origin client through
/// that `via` whose launch names the folder, and kills their process groups. A toolset consent
/// closes the handles of that origin client that hold the toolset, and their jobs' streams end
/// `gone` (19.5). An account consent ends the live `openai` streams of that origin client through that `via` (19.4).
pub(crate) fn withdrawn(host: &Host, consent: &Consent, origin: &str) {
    match consent {
        Consent::Workspace { via, folder } => {
            let ids = host.sessions.depending(origin, via, folder);
            host.hub.end_gone(&ids);
        }
        Consent::Toolset { toolset, .. } => host.close_handles_of_toolset(origin, toolset),
        Consent::Account { via, .. } => {
            host.hub.end_gone(&host.https.of_account(origin, via));
        }
    }
}

impl Host {
    /// Takes one consent back from the record of `local`, like `revoke` for a grant (4.4, 19.7):
    /// it matches by its `what` key, so `Consent::parse("toolset:t")` withdraws the stored
    /// toolset consent `t` whatever its functions. The streams that depend on it end `gone` at
    /// once, also in the middle of a turn. A consent the mod does not hold is `bad_request`.
    pub fn withdraw_consent(&self, local: &LocalName, consent: &Consent) -> Result<(), HostError> {
        let key = consent.to_string();
        let identity = self
            .get(local)
            .ok_or_else(|| HostError::NoSuchMod(local.to_string()))?
            .identity;
        let mut taken = None;
        self.change_mod(local, |m| {
            let at = m
                .consents
                .iter()
                .position(|c| c.to_string() == key)
                .ok_or_else(|| {
                    HostError::BadRequest(format!("{local} holds no consent {key:?}"))
                })?;
            taken = Some(m.consents.remove(at));
            Ok(())
        })?;
        if let Some(taken) = taken {
            withdrawn(self, &taken, &identity);
        }
        self.catalog_written();
        Ok(())
    }

    /// Whether the mod with this identity holds the consent (by its `what` key).
    #[cfg_attr(
        windows,
        allow(dead_code, reason = "only the Unix Claude runner reads it")
    )]
    pub(crate) fn consent_held(&self, identity: &str, consent: &Consent) -> bool {
        let key = consent.to_string();
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .find(|m| m.identity == identity)
            .is_some_and(|m| m.consents.iter().any(|c| c.to_string() == key))
    }

    /// The local name of the installed mod with this identity.
    pub(crate) fn local_of(&self, identity: &str) -> Option<LocalName> {
        self.catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it")
            .mods
            .iter()
            .find(|m| m.identity == identity)
            .map(|m| m.local.clone())
    }

    /// Stores a granted consent on the record of the origin client, if that client is still the
    /// install (identity and install generation) that asked, enabled and not in error. `Err`
    /// when it is not: the request ends `gone`. A toolset list built at the drop mark `since`
    /// loses, under the same lock, the names of each mod dropped after it (19.5, Limits).
    pub(crate) fn store_consent(
        &self,
        identity: &str,
        generation: Option<u64>,
        mut consent: Consent,
        since: Option<u64>,
    ) -> Result<(), ()> {
        let mut catalog = self
            .catalog
            .lock()
            .expect("lock poisoned: another thread panicked while holding it");
        if let (Some(mark), Consent::Toolset { functions, .. }) = (since, &mut consent) {
            *functions = catalog.without_dropped(mark, functions);
        }
        let Some(m) = catalog.mods.iter().find(|m| {
            m.identity == identity
                && m.enabled
                && m.error.is_none()
                && generation.is_some_and(|g| catalog.install_of(&m.local) == Some(g))
        }) else {
            return Err(());
        };
        let key = consent.to_string();
        // A toolset consent given again replaces the stored list (19.5, S65); any other kind
        // is kept as it was.
        let held = m.consents.iter().position(|c| c.to_string() == key);
        if held.is_some() && !matches!(consent, Consent::Toolset { .. }) {
            return Ok(());
        }
        let local = m.local.clone();
        let mut next = catalog.clone();
        if let Some(m) = next.mods.iter_mut().find(|m| m.local == local) {
            match held {
                Some(at) => m.consents[at] = consent,
                None => m.consents.push(consent),
            }
        }
        next.save(self.home()).map_err(|_| ())?;
        *catalog = next;
        drop(catalog);
        self.catalog_written();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC1 (19.7): the JSON of each consent, by literal.
    #[test]
    fn ac1_consent_json_is_exact_and_reads_back() {
        let cases = [
            (
                Consent::Workspace {
                    via: "local/p".into(),
                    folder: "/x".into(),
                },
                r#"{"kind":"workspace","via":"local/p","folder":"/x"}"#,
                "workspace:local/p:/x",
            ),
            (
                Consent::Account {
                    via: "local/p".into(),
                    account: "openai".into(),
                },
                r#"{"kind":"account","via":"local/p","account":"openai"}"#,
                "account:local/p:openai",
            ),
            (
                Consent::Toolset {
                    toolset: "t".into(),
                    functions: vec!["a_b_c".into()],
                },
                r#"{"kind":"toolset","toolset":"t","functions":["a_b_c"]}"#,
                "toolset:t",
            ),
        ];
        for (consent, text, what) in cases {
            assert_eq!(serde_json::to_string(&consent).unwrap(), text);
            assert_eq!(consent.to_string(), what);
            assert_eq!(serde_json::from_str::<Consent>(text).unwrap(), consent);
            let parsed = Consent::parse(what).unwrap();
            match (&consent, &parsed) {
                (Consent::Toolset { toolset, .. }, Consent::Toolset { functions, .. }) => {
                    assert_eq!(toolset, "t");
                    assert!(functions.is_empty());
                }
                _ => assert_eq!(parsed, consent),
            }
        }
        // The folder is the rest of the text: it may hold `:`.
        assert_eq!(
            Consent::parse("workspace:local/p:/a:b").unwrap(),
            Consent::Workspace {
                via: "local/p".into(),
                folder: "/a:b".into()
            }
        );
    }

    #[test]
    fn ac1_parse_refuses_a_key_without_via_an_empty_or_relative_folder_and_other_kinds() {
        for what in [
            "workspace:/x",
            "workspace::/x",
            "workspace:local/p:",
            "workspace:local/p:x",
            "workspace:local/p:./x",
            "account:openai",
            "account:local/p:other",
            "account:local/p:",
            "toolset:",
            "other:x",
            "",
            "workspace",
        ] {
            let e = Consent::parse(what).unwrap_err();
            assert_eq!(e.code(), "bad_request", "{what}");
        }
    }
}
