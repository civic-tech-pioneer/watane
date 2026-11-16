use crate::consent::{GrantKind, GrantLine, Party};
use std::collections::BTreeMap;
use watane_host::watane_pkg::{Grant, Provider};
#[derive(Default)]
pub struct Parties {
    names: std::sync::RwLock<BTreeMap<String, String>>,
    own: std::sync::RwLock<Option<String>>,
}
impl Parties {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_own(&self, key: String) {
        *self.own.write().unwrap() = Some(key);
    }
    #[cfg(test)]
    pub fn insert(&self, key: String, name: String) {
        self.names.write().unwrap().insert(key, name);
    }
    pub fn remember(&self, views: &[watane_dist::ChannelView]) {
        self.names.write().unwrap().extend(
            views
                .iter()
                .filter_map(|v| v.name.clone().map(|n| (v.channel.to_string(), n)))
                .collect::<BTreeMap<_, _>>(),
        );
    }
}

pub fn party(publisher: &str, parties: &Parties) -> Party {
    Party {
        name: parties.names.read().unwrap().get(publisher).cloned(),
        you: parties.own.read().unwrap().as_deref() == Some(publisher),
        fingerprint: publisher.chars().take(10).collect(), // PublicKey::fingerprint
    }
}
pub(crate) fn party_label(p: &Party) -> String {
    if p.you {
        "you".into()
    } else {
        p.name
            .as_ref()
            .map(|n| format!("{n} ({})", p.fingerprint))
            .unwrap_or_else(|| p.fingerprint.clone())
    }
}
pub(crate) fn asker_label(asker: &crate::consent::Asker) -> String {
    match &asker.publisher {
        Some(p) => format!("{} by {}", asker.name, party_label(p)),
        None => format!("{} (local)", asker.name),
    }
}
pub(crate) fn release_label(r: &watane_dist::ReleaseRef, parties: &Parties) -> String {
    format!(
        "{r} by {}",
        party_label(&party(&r.publisher.to_string(), parties))
    )
}
/// "<name> <version> by <party> — <reference>": the readable part first (S58 AC2).
pub(crate) fn origin_line(name: &str, r: &watane_dist::ReleaseRef, parties: &Parties) -> String {
    format!(
        "{name} {} by {} — {r}",
        r.version,
        party_label(&party(&r.publisher.to_string(), parties))
    )
}
pub(crate) fn already_listed(include: bool) -> &'static str {
    if include {
        "This channel is already in the index."
    } else {
        "already listed"
    }
}
pub(crate) fn index_read(index: &str, cause: &str) -> String {
    format!("Could not read {index}: {cause}")
}
pub(crate) fn wrong_channel(url: &str, expected: &str, found: &str, parties: &Parties) -> String {
    format!(
        "{} serves {}, not {}",
        url.escape_debug(),
        party_label(&party(found, parties)),
        party_label(&party(expected, parties))
    )
}
pub(crate) fn channel_read(channel: &str, cause: &str, parties: &Parties) -> String {
    let p = party(channel, parties);
    index_read(p.name.as_deref().unwrap_or(&p.fingerprint), cause)
}
pub(crate) fn provider(provider: &Provider, parties: &Parties) -> String {
    match &provider.publisher {
        None => format!("{} (local)", provider.slug),
        Some(key) => {
            let p = party(&key.to_string(), parties);
            let label = party_label(&p);
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
    let label = party_label(&p);
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
        Grant::Manage => (
            GrantKind::Manage,
            "Manage your mods: see every mod and its grants, turn mods off, withdraw grants, and ask you to add, turn on or remove mods".into(),
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
                "Receive the {interface} events of {} and answer its requests",
                provider_bound(p, binds, parties)
            ),
        ),
    };
    GrantLine {
        short: String::new(),
        key: grant.to_string(),
        kind,
        text,
        granted,
        new,
    }
}

// Names are keyed by provider identity, not slug: different publishers can
// install the same slug, and a bind must name both sides accurately.
pub fn grant_short(grant: &Grant, parties: &Parties, names: &BTreeMap<String, String>) -> String {
    grant_short_bound(grant, &[], parties, names)
}
pub fn grant_short_bound(
    grant: &Grant,
    binds: &[watane_host::Bind],
    parties: &Parties,
    names: &BTreeMap<String, String>,
) -> String {
    let (interface, p) = match grant {
        Grant::Call {
            interface,
            provider,
        }
        | Grant::Subscribe {
            interface,
            provider,
        } => (interface, provider),
        _ => return grant_line(grant, parties, false, false).text,
    };
    let label = |identity: &str| {
        let (publisher, slug) = identity.split_once('/').expect("provider identity");
        let name = names.get(identity).map(String::as_str).unwrap_or(slug);
        if publisher == "local" {
            return format!("{name} (local)");
        }
        let party = party(publisher, parties);
        let author = if party.you {
            "you"
        } else {
            party.name.as_deref().unwrap_or(&party.fingerprint)
        };
        format!("{name} by {author}")
    };
    let pinned = label(&p.to_string());
    let target = match binds.iter().find(|b| b.provider == *p) {
        Some(bind) => format!("{} (in place of {pinned})", label(&bind.to)),
        None => pinned,
    };
    let text = interface.to_string();
    let interface = text.split_once('/').unwrap().1.split('@').next().unwrap();
    match grant {
        Grant::Call { .. } => format!("Call {interface} of {target}"),
        _ if interface == "events" => format!("Receive the events of {target}"),
        _ => format!("Receive the {interface} events of {target} and answer its requests"),
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
    fn ac3_short_grants_name_interfaces_providers_binds_and_you() {
        let key = "y".repeat(52);
        let parties = Parties::new();
        parties.insert(key.clone(), "Alice".into());
        let p = Provider {
            slug: "reading-list".parse().unwrap(),
            publisher: Some(key.parse().unwrap()),
        };
        let mut names = BTreeMap::from([
            (format!("{key}/reading-list"), "Reading list".into()),
            ("local/reading-list".into(), "Reading list".into()),
        ]);
        let call = Grant::Call {
            interface: "mods:reading-list/books@0.1.0".parse().unwrap(),
            provider: p.clone(),
        };
        assert_eq!(
            grant_short(&call, &parties, &names),
            "Call books of Reading list by Alice"
        );
        assert!(grant_line(&call, &parties, true, false).short.is_empty());
        let legacy = serde_json::json!({"key":"storage","kind":"storage","text":"Exact words","granted":true,"new":false});
        assert!(
            serde_json::from_value::<GrantLine>(legacy)
                .unwrap()
                .short
                .is_empty()
        );
        let sub = Grant::Subscribe {
            interface: "mods:reading-list/events@0.1.0".parse().unwrap(),
            provider: Provider {
                publisher: None,
                ..p.clone()
            },
        };
        assert_eq!(
            grant_short(&sub, &parties, &names),
            "Receive the events of Reading list (local)"
        );
        names.insert("local/reading-list".into(), "My reading list".into());
        let bind = watane_host::Bind {
            consumer: "local/consumer".into(),
            provider: p,
            to: "local/reading-list".into(),
        };
        assert_eq!(
            grant_short_bound(&call, &[bind], &parties, &names),
            "Call books of My reading list (local) (in place of Reading list by Alice)"
        );
        parties.set_own(key);
        assert_eq!(
            grant_short(&call, &parties, &names),
            "Call books of Reading list by you"
        );
        names.clear();
        assert_eq!(
            grant_short(&call, &parties, &names),
            "Call books of reading-list by you"
        );
        for grant in [
            Grant::Storage,
            Grant::Http {
                host: "example.org".parse().unwrap(),
            },
            Grant::Manage,
        ] {
            assert_eq!(
                grant_short(&grant, &parties, &names),
                grant_line(&grant, &parties, true, false).text
            );
        }
    }
    #[test]
    fn ac1_manager_words_and_asker_labels() {
        let line = grant_line(&Grant::Manage, &Parties::new(), true, true);
        assert_eq!(line.key, "manage");
        assert_eq!(line.kind, GrantKind::Manage);
        assert_eq!(
            line.text,
            "Manage your mods: see every mod and its grants, turn mods off, withdraw grants, and ask you to add, turn on or remove mods"
        );
        let mut asker = crate::consent::Asker {
            local: "desk".into(),
            name: "Desk".into(),
            publisher: None,
        };
        assert_eq!(asker_label(&asker), "Desk (local)");
        asker.publisher = Some(Party {
            you: false,
            name: Some("Ivy".into()),
            fingerprint: "abcdefghij".into(),
        });
        assert_eq!(asker_label(&asker), "Desk by Ivy (abcdefghij)");
        asker.publisher.as_mut().unwrap().you = true;
        assert_eq!(asker_label(&asker), "Desk by you");
        asker.publisher.as_mut().unwrap().you = false;
        asker.publisher.as_mut().unwrap().name = None;
        assert_eq!(asker_label(&asker), "Desk by abcdefghij");
    }
    #[test]
    fn ac3_wrong_channel_names_both_parties() {
        let named = "a".repeat(52);
        let unnamed = "b".repeat(52);
        let parties = Parties::new();
        parties.insert(named.clone(), "Alice".into());
        assert_eq!(
            wrong_channel("https://site/head", &named, &unnamed, &parties),
            "https://site/head serves bbbbbbbbbb, not Alice (aaaaaaaaaa)"
        );
        parties.set_own(unnamed.clone());
        assert_eq!(
            wrong_channel("https://site/head", &unnamed, &named, &parties),
            "https://site/head serves Alice (aaaaaaaaaa), not you"
        );
    }
    #[test]
    fn ac4_mod_refresh_notice_names_channel_or_fingerprint() {
        let key = "a".repeat(52);
        let parties = Parties::new();
        assert_eq!(
            channel_read(&key, "offline", &parties),
            "Could not read aaaaaaaaaa: offline"
        );
        parties.insert(key.clone(), "Alice".into());
        assert_eq!(
            channel_read(&key, "offline", &parties),
            "Could not read Alice: offline"
        );
    }
    #[test]
    fn ac1_party_cache_names_an_unfollowed_channel_view() {
        struct Approve;
        impl watane_dist::Confirm for Approve {
            fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
                watane_dist::Decision::Approve
            }
        }
        let home = tempfile::tempdir().unwrap();
        let dist = watane_dist::Dist::open(
            home.path(),
            std::sync::Arc::new(Approve),
            Default::default(),
        )
        .unwrap();
        dist.set_name("Frank").unwrap();
        let view = dist.channel(&dist.identity().unwrap()).unwrap();
        assert!(!view.followed);
        let parties = Parties::new();
        parties.remember(&[view]);
        let label = party_label(&party(&dist.identity().unwrap().to_string(), &parties));
        assert_eq!(label, format!("Frank ({})", dist.fingerprint().unwrap()));
    }
    #[test]
    fn ac5_index_errors_are_plain_and_name_the_index() {
        assert_eq!(
            index_read("Ivy", "request timed out"),
            "Could not read Ivy: request timed out"
        );
        assert_eq!(
            index_read("abcd", "offline"),
            "Could not read abcd: offline"
        );
        assert_eq!(already_listed(false), "already listed");
        assert_eq!(
            already_listed(true),
            "This channel is already in the index."
        );
    }
    #[test]
    fn ac2_own_party_provider_and_bind_say_you() {
        let key = "y".repeat(52);
        let parties = Parties::new();
        parties.set_own(key.clone());
        parties.insert(key.clone(), "Carol".into());
        assert!(party(&key, &parties).you);
        let p = Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some(key.parse().unwrap()),
        };
        assert_eq!(provider(&p, &parties), "ratings by you");
        let bind = watane_host::Bind {
            consumer: "local/stats".into(),
            provider: p,
            to: format!("{key}/ratings"),
        };
        assert_eq!(
            bind_line(&bind, &parties),
            "ratings by you in place of ratings by you (your pick)"
        );
    }
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
                    format!("Receive the {interface} events of {target} and answer its requests"),
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
                "Receive the mods:reading-list/books@0.1.0 events of reading-list (local) and answer its requests",
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
                you: false,
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
