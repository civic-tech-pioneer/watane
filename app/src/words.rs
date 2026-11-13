use crate::consent::{GrantKind, GrantLine, Party};
use std::collections::BTreeMap;
use watane_host::watane_pkg::{Grant, Provider};
#[derive(Default)]
pub struct Parties(std::sync::RwLock<BTreeMap<String, String>>);
impl Parties {
    pub fn new() -> Self {
        Self::default()
    }
    #[cfg(test)]
    pub fn insert(&self, key: String, name: String) {
        self.0.write().unwrap().insert(key, name);
    }
    pub fn replace(&self, views: &[watane_dist::ChannelView]) {
        *self.0.write().unwrap() = views
            .iter()
            .filter_map(|v| v.name.clone().map(|n| (v.channel.to_string(), n)))
            .collect();
    }
}

pub fn party(publisher: &str, parties: &Parties) -> Party {
    Party {
        name: parties.0.read().unwrap().get(publisher).cloned(),
        fingerprint: publisher.chars().take(10).collect(), // PublicKey::fingerprint
    }
}
pub(crate) fn provider(provider: &Provider, parties: &Parties) -> String {
    match &provider.publisher {
        None => format!("{} (local)", provider.slug),
        Some(key) => {
            let p = party(&key.to_string(), parties);
            let label = p
                .name
                .map(|n| format!("{n} ({})", p.fingerprint))
                .unwrap_or(p.fingerprint);
            format!("{} by {label}", provider.slug)
        }
    }
}
fn bound_target(bind: &watane_host::Bind, parties: &Parties) -> String {
    let Some((publisher, slug)) = bind.to.split_once('/') else {
        return bind.to.clone();
    };
    if publisher == "local" {
        return format!("{slug} (local)");
    }
    let p = party(publisher, parties);
    let label = p
        .name
        .map(|n| format!("{n} ({})", p.fingerprint))
        .unwrap_or(p.fingerprint);
    format!("{slug} by {label}")
}
pub(crate) fn bind_line(bind: &watane_host::Bind, parties: &Parties) -> String {
    format!(
        "{} in place of {} (your pick)",
        bound_target(bind, parties),
        provider(&bind.provider, parties)
    )
}
pub(crate) fn provider_bound(
    p: &Provider,
    binds: &[watane_host::Bind],
    parties: &Parties,
) -> String {
    match binds.iter().find(|b| b.provider == *p) {
        Some(bind) => format!(
            "{} (in place of {})",
            bound_target(bind, parties),
            provider(p, parties)
        ),
        None => provider(p, parties),
    }
}
pub fn grant_line(grant: &Grant, parties: &Parties, granted: bool, new: bool) -> GrantLine {
    grant_line_bound(grant, &[], parties, granted, new)
}
pub fn grant_line_bound(
    grant: &Grant,
    binds: &[watane_host::Bind],
    parties: &Parties,
    granted: bool,
    new: bool,
) -> GrantLine {
    let (kind, text) = match grant {
        Grant::Storage => (
            GrantKind::Storage,
            "Keep its own data on this device".into(),
        ),
        Grant::Http { host } => (GrantKind::Http, format!("Connect to {host}")),
        Grant::Call {
            interface,
            provider: p,
        } => (
            GrantKind::Call,
            format!("Call {interface} of {}", provider_bound(p, binds, parties)),
        ),
        Grant::Subscribe {
            interface,
            provider: p,
        } => (
            GrantKind::Subscribe,
            format!(
                "Receive the {interface} events of {}",
                provider_bound(p, binds, parties)
            ),
        ),
    };
    GrantLine {
        key: grant.to_string(),
        kind,
        text,
        granted,
        new,
    }
}

pub(crate) const COMPROMISED: &str = "The publisher's channel is compromised";
pub(crate) fn withdrawn(reason: &str) -> String {
    format!("Withdrawn by the publisher: {reason}")
}
pub(crate) fn update_failed(version: &str, cause: &str) -> String {
    format!("Update to {version} failed: {cause}")
}

pub(crate) fn port_in_use(port: u16) -> String {
    format!(
        "Port {port} is in use, maybe by another watane app. Start this one with --port <n> or WATANE_PORT=<n> to use another port, or --port 0 for any free port."
    )
}
pub(crate) fn bad_reason(cause: &str) -> String {
    if cause == "invalid character count" {
        "Give a reason of 1 to 280 characters.".into()
    } else {
        format!("This reason cannot be used: {cause}.")
    }
}
pub(crate) fn update_declined(version: &str) -> String {
    format!("You declined the update to {version}.")
}

pub(crate) const OWN_INDEX: &str = "Your own channel is always your index.";
pub(crate) fn bad_note(cause: &str) -> String {
    if cause == "invalid character count" {
        "Give a note of at most 280 characters.".into()
    } else {
        format!("This note cannot be used: {cause}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac3_bound_grants_and_bind_lines_name_both_parties() {
        let carol = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        let frank = "59dc6y8s5c95d6jiotx5czp8b7sb5o7e4cquc54i8yo81xdubf6y";
        let p = Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some(carol.parse().unwrap()),
        };
        let bind = watane_host::Bind {
            consumer: "local/stats".into(),
            provider: p.clone(),
            to: format!("{frank}/ratings"),
        };
        let parties = Parties::new();
        for known in [false, true] {
            if known {
                parties.insert(carol.into(), "Carol".into());
                parties.insert(frank.into(), "Frank".into());
            }
            let from = if known {
                "Carol (47pjoycnsr)"
            } else {
                "47pjoycnsr"
            };
            let to = if known {
                "Frank (59dc6y8s5c)"
            } else {
                "59dc6y8s5c"
            };
            let target = format!("ratings by {to} (in place of ratings by {from})");
            assert_eq!(
                provider_bound(&p, std::slice::from_ref(&bind), &parties),
                target
            );
            assert_eq!(
                bind_line(&bind, &parties),
                format!("ratings by {to} in place of ratings by {from} (your pick)")
            );
            let interface: watane_host::watane_pkg::InterfaceName =
                "mods:ratings/scores@1.0.0".parse().unwrap();
            for (grant, expected) in [
                (
                    Grant::Call {
                        interface: interface.clone(),
                        provider: p.clone(),
                    },
                    format!("Call {interface} of {target}"),
                ),
                (
                    Grant::Subscribe {
                        interface: interface.clone(),
                        provider: p.clone(),
                    },
                    format!("Receive the {interface} events of {target}"),
                ),
                (Grant::Storage, "Keep its own data on this device".into()),
                (
                    Grant::Http {
                        host: "example.org".parse().unwrap(),
                    },
                    "Connect to example.org".into(),
                ),
            ] {
                assert_eq!(
                    grant_line_bound(&grant, std::slice::from_ref(&bind), &parties, false, true)
                        .text,
                    expected
                );
                let plain = grant_line(&grant, &parties, false, true);
                assert_eq!(grant_line_bound(&grant, &[], &parties, false, true), plain);
                let bound =
                    grant_line_bound(&grant, std::slice::from_ref(&bind), &parties, false, true);
                assert_eq!(
                    (bound.key, bound.kind, bound.granted, bound.new),
                    (plain.key, plain.kind, plain.granted, plain.new)
                );
            }
        }
    }
    #[test]
    fn ac1_grant_words_match_section_four() {
        let interface: watane_host::watane_pkg::InterfaceName =
            "mods:reading-list/books@0.1.0".parse().unwrap();
        let provider = Provider {
            slug: "reading-list".parse().unwrap(),
            publisher: None,
        };
        let rows = [
            (
                Grant::Storage,
                "storage",
                "storage",
                "Keep its own data on this device",
            ),
            (
                Grant::Http {
                    host: "example.org".parse().unwrap(),
                },
                "http example.org",
                "http",
                "Connect to example.org",
            ),
            (
                Grant::Call {
                    interface: interface.clone(),
                    provider: provider.clone(),
                },
                "call mods:reading-list/books@0.1.0 from local/reading-list",
                "call",
                "Call mods:reading-list/books@0.1.0 of reading-list (local)",
            ),
            (
                Grant::Subscribe {
                    interface,
                    provider,
                },
                "subscribe mods:reading-list/books@0.1.0 from local/reading-list",
                "subscribe",
                "Receive the mods:reading-list/books@0.1.0 events of reading-list (local)",
            ),
        ];
        for (grant, key, kind, text) in rows {
            let line = grant_line(&grant, &Parties::new(), false, true);
            assert_eq!(line.key, key);
            assert_eq!(serde_json::to_value(line.kind).unwrap(), kind);
            assert_eq!(line.text, text);
            assert!(!line.granted);
            assert!(line.new);
        }
    }
    #[test]
    fn ac1_party_uses_the_first_ten_key_characters_before_stage_c() {
        let key = "y".repeat(52);
        let parties = Parties::new();
        assert_eq!(
            party(&key, &parties),
            Party {
                name: None,
                fingerprint: "yyyyyyyyyy".into()
            }
        );
        parties.insert(key.clone(), "Alice".into());
        let provider_key = Provider {
            slug: "reading-list".parse().unwrap(),
            publisher: Some(key.parse().unwrap()),
        };
        assert_eq!(
            provider(&provider_key, &parties),
            "reading-list by Alice (yyyyyyyyyy)"
        );
    }
}
