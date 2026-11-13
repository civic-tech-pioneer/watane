use watane_dist::{Hit, Status};
use watane_host::watane_pkg::{Grant, Provider};

pub(crate) fn candidates<'a>(
    hits: &'a [Hit],
    provider: &Provider,
    needed: &[Grant],
) -> Vec<&'a Hit> {
    hits.iter()
        .filter(|h| {
            matches!(h.status, Status::Ok)
                && h.info.reference.slug == provider.slug
                && Some(&h.info.reference.publisher) != provider.publisher.as_ref()
                && needed.iter().all(|g| match g {
                    Grant::Call { interface, .. } => h
                        .info
                        .release
                        .offers
                        .iter()
                        .any(|i| interface.satisfied_by(i)),
                    Grant::Subscribe { interface, .. } => h
                        .info
                        .release
                        .events
                        .iter()
                        .any(|i| interface.satisfied_by(i)),
                    _ => true,
                })
        })
        .take(20)
        .collect()
}

pub(crate) fn needed(entries: &[watane_host::PlanEntry], provider: &Provider) -> Vec<Grant> {
    entries.iter().flat_map(|e| &e.grants).filter(|g| matches!(g,
        Grant::Call { provider: p, .. } | Grant::Subscribe { provider: p, .. } if p == provider
    )).cloned().collect()
}
pub(crate) fn bindings(
    entries: &[watane_host::PlanEntry],
    provider: &Provider,
    to: &str,
) -> Vec<watane_host::Bind> {
    entries
        .iter()
        .filter(|e| !needed(std::slice::from_ref(e), provider).is_empty())
        .map(|e| watane_host::Bind {
            consumer: e.identity.clone(),
            provider: provider.clone(),
            to: to.into(),
        })
        .collect()
}
pub(crate) fn no_candidates(provider: &str, cause: &str) -> crate::CmdError {
    crate::CmdError::new(
        "missing_providers",
        format!(
            "{provider} is not available ({cause}), and no mod in your indexes fits in its place"
        ),
    )
}
pub(crate) fn picked(answer: crate::consent::Answer) -> Result<String, crate::CmdError> {
    match answer {
        crate::consent::Answer::Pick { choice } => Ok(choice),
        _ => Err(crate::CmdError::new("refused", "provider pick was refused")),
    }
}

pub(crate) fn fetch_or_pick<S, B>(
    fetched: Result<(Option<S>, Vec<B>), crate::CmdError>,
    pick: Option<impl FnOnce(&crate::CmdError) -> Result<(Option<S>, Vec<B>), crate::CmdError>>,
) -> Result<(Option<S>, Vec<B>), crate::CmdError> {
    match fetched {
        Ok(value) => Ok(value),
        Err(error) => match pick {
            Some(pick) => pick(&error),
            None => Err(error),
        },
    }
}
pub(crate) fn follow_links(
    links: impl IntoIterator<Item = watane_dist::Link>,
    followed: impl IntoIterator<Item = watane_dist::PublicKey>,
) -> Vec<watane_dist::Link> {
    let mut seen: std::collections::BTreeSet<_> = followed.into_iter().collect();
    links
        .into_iter()
        .filter(|link| seen.insert(link.channel()))
        .collect()
}
pub(crate) fn follow_all(
    links: Vec<watane_dist::Link>,
    mut follow: impl FnMut(&watane_dist::Link) -> Result<(), crate::CmdError>,
    failures: &mut std::collections::BTreeMap<String, String>,
) {
    for link in links {
        match follow(&link) {
            Ok(()) => {
                failures.remove(&link.channel().to_string());
            }
            Err(error) => {
                failures.insert(link.channel().to_string(), error.message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac3_follow_once_skip_followed_or_absent_keep_failures() {
        let mut bytes = [0x66; 32];
        bytes[0] = 0x58;
        let key = watane_dist::PublicKey::from_bytes(&bytes)
            .unwrap()
            .to_string();
        let other = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        let a: watane_dist::Link = format!("watane:{key}@https://a.example/#ratings")
            .parse()
            .unwrap();
        let b: watane_dist::Link = format!("watane:{other}@https://b.example/#stats")
            .parse()
            .unwrap();
        let fetched = [Some(a.clone()), None, Some(a.clone()), Some(b.clone())];
        let links = follow_links(fetched.into_iter().flatten(), [b.channel()]);
        assert_eq!(
            links.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [a.to_string()]
        );
        let mut failures = Default::default();
        let mut calls = 0;
        follow_all(
            links,
            |_| {
                calls += 1;
                Err(crate::CmdError::new("dist", "offline"))
            },
            &mut failures,
        );
        assert_eq!(calls, 1);
        assert_eq!(failures.get(&key).map(String::as_str), Some("offline"));
    }
    #[test]
    fn ac8_background_resolver_has_no_pick() {
        let error = fetch_or_pick::<(), ()>(
            Err(crate::CmdError::new("dist", "provider offline")),
            None::<fn(&crate::CmdError) -> Result<(Option<()>, Vec<()>), crate::CmdError>>,
        )
        .unwrap_err();
        assert_eq!(error.message, "provider offline");
    }
    #[test]
    fn ac2_pick_binds_every_pinned_consumer_and_denial_and_no_fit() {
        let home = tempfile::tempdir().unwrap();
        let host =
            watane_host::Host::open(home.path(), watane_host::Mode::Preview, Default::default())
                .unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Folder(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/recommendations"),
            )])
            .unwrap();
        let mut first = plan.entries()[0].clone();
        first.identity = "local/first".into();
        let provider = Provider {
            slug: "ratings".parse().unwrap(),
            publisher: Some("y".repeat(52).parse().unwrap()),
        };
        first.grants = vec![Grant::Call {
            provider: provider.clone(),
            interface: "mods:ratings/api@0.1.0".parse().unwrap(),
        }];
        let mut second = first.clone();
        second.identity = "local/second".into();
        let mut other = first.clone();
        if let Grant::Call { provider, .. } = &mut other.grants[0] {
            provider.publisher = None;
        }
        let entries = vec![first, second, other];
        let binds = bindings(&entries, &provider, "choice/ratings");
        assert_eq!(binds.len(), 2);
        assert_eq!(
            binds
                .iter()
                .map(|b| b.consumer.as_str())
                .collect::<Vec<_>>(),
            ["local/first", "local/second"]
        );
        assert!(
            binds
                .iter()
                .all(|b| b.provider == provider && b.to == "choice/ratings")
        );
        let error = no_candidates("ratings by Carol", "connection refused");
        assert_eq!(error.code, "missing_providers");
        assert_eq!(
            error.message,
            "ratings by Carol is not available (connection refused), and no mod in your indexes fits in its place"
        );
        assert_eq!(
            picked(crate::consent::Answer::Deny).unwrap_err().code,
            "refused"
        );
        assert_eq!(
            picked(crate::consent::Answer::Pick {
                choice: "chosen".into()
            })
            .unwrap(),
            "chosen"
        );
    }
}
