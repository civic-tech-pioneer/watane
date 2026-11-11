use crate::consent::{GrantKind, GrantLine, Party};
use std::collections::BTreeMap;
use watane_host::watane_pkg::{Grant, Provider};
pub type Parties = BTreeMap<String, String>;

pub fn party(publisher: &str, parties: &Parties) -> Party {
    Party {
        name: parties.get(publisher).cloned(),
        fingerprint: publisher.chars().take(10).collect(),
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
pub fn grant_line(grant: &Grant, parties: &Parties, granted: bool, new: bool) -> GrantLine {
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
            format!("Call {interface} of {}", provider(p, parties)),
        ),
        Grant::Subscribe {
            interface,
            provider: p,
        } => (
            GrantKind::Subscribe,
            format!("Receive the {interface} events of {}", provider(p, parties)),
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut parties = Parties::new();
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
