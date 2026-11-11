use crate::{
    consent,
    words::{self, Parties},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock, Weak},
};
use watane_host::{Answer, Change, Confirm, InstallRequest, Requester};

pub(crate) struct HostConfirm {
    pub broker: Arc<consent::Broker>,
    pub parties: Arc<Parties>,
    pub host: OnceLock<Weak<watane_host::Host>>,
}

fn request_view(
    request: &InstallRequest,
    parties: &Parties,
    host: Option<&watane_host::Host>,
) -> consent::Request {
    consent::Request::Install {
        by_program: matches!(request.requester, Requester::LocalApi),
        mods: request
            .entries
            .iter()
            .map(|e| consent::PlanMod {
                local: e.local.to_string(),
                name: e.name.clone(),
                slug: e.slug.to_string(),
                version: e.version.to_string(),
                release: e.release.as_ref().map(ToString::to_string),
                publisher: e
                    .publisher
                    .as_ref()
                    .map(|p| words::party(&p.to_string(), parties)),
                replaces: match &e.change {
                    Change::Update { from_version } => Some(from_version.to_string()),
                    _ => None,
                },
                grants: e
                    .grants
                    .iter()
                    .map(|g| words::grant_line(g, parties, true, e.new_grants.contains(g)))
                    .collect(),
                uses: e
                    .providers
                    .iter()
                    .map(|local| {
                        // The host resolves provider identities. Look in the plan first;
                        // an already-installed provider is also named by its grant.
                        let provider =
                            request.entries.iter().find(|p| &p.local == local).map(|p| {
                                watane_host::watane_pkg::Provider {
                                    slug: p.slug.clone(),
                                    publisher: p.publisher.clone(),
                                }
                            });
                        let provider = provider.or_else(|| {
                            host.and_then(|h| h.get(local)).map(|m| {
                                watane_host::watane_pkg::Provider {
                                    slug: m.slug,
                                    publisher: m.publisher,
                                }
                            })
                        });
                        provider
                            .map(|p| words::provider(&p, parties))
                            .unwrap_or_else(|| local.to_string())
                    })
                    .collect(),
            })
            .collect(),
    }
}
fn host_answer(answer: consent::Answer, request: &InstallRequest) -> Answer {
    match answer {
        consent::Answer::Deny => Answer::Deny,
        consent::Answer::Approve { withheld } => {
            let mut withhold = Vec::new();
            for w in withheld {
                let Some((local, grant)) = request
                    .entries
                    .iter()
                    .find(|e| e.local.as_ref() == w.local)
                    .and_then(|e| {
                        e.grants
                            .iter()
                            .find(|g| g.to_string() == w.key)
                            .map(|g| (e.local.clone(), g.clone()))
                    })
                else {
                    return Answer::Deny;
                };
                withhold.push((local, grant));
            }
            Answer::Approve { withhold }
        }
    }
}
impl Confirm for HostConfirm {
    fn install(&self, request: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        let broker = self.broker.clone();
        let host = self.host.get().and_then(Weak::upgrade);
        let view = request_view(&request, &self.parties, host.as_deref());
        Box::pin(async move { host_answer(broker.ask(view).await, &request) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watane_host::{
        LocalName, PlanEntry,
        watane_pkg::{Grant, Provider},
    };
    #[test]
    fn ac2_install_request_and_withheld_answer() {
        let storage = Grant::Storage;
        let call = Grant::Call {
            interface: "mods:reading-list/books@0.1.0".parse().unwrap(),
            provider: Provider {
                slug: "reading-list".parse().unwrap(),
                publisher: None,
            },
        };
        let entry = |local: &str, grants: Vec<Grant>| PlanEntry {
            local: LocalName::parse(local).unwrap(),
            identity: format!("local/{local}"),
            slug: local.parse().unwrap(),
            name: local.into(),
            summary: String::new(),
            version: "0.2.0".parse().unwrap(),
            publisher: None,
            package: "00".repeat(32).parse().unwrap(),
            release: None,
            origin: None,
            change: Change::New,
            grants: grants.clone(),
            new_grants: grants,
            providers: vec![],
        };
        let mut first = entry("reading-list", vec![storage.clone()]);
        first.change = Change::Update {
            from_version: "0.1.0".parse().unwrap(),
        };
        first.new_grants.clear();
        let mut second = entry("recommendations", vec![call.clone()]);
        second.providers.push(first.local.clone());
        let request = InstallRequest {
            entries: vec![first, second],
            requester: Requester::LocalApi,
        };
        let consent::Request::Install { mods, by_program } =
            request_view(&request, &Parties::new(), None)
        else {
            panic!("install expected")
        };
        assert!(by_program);
        assert_eq!(
            mods.iter().map(|m| m.local.as_str()).collect::<Vec<_>>(),
            ["reading-list", "recommendations"]
        );
        assert_eq!(mods[0].replaces.as_deref(), Some("0.1.0"));
        assert!(!mods[0].grants[0].new);
        assert!(mods[1].grants[0].new);
        assert_eq!(mods[1].uses, ["reading-list (local)"]);
        let answer = host_answer(
            consent::Answer::Approve {
                withheld: vec![consent::Withheld {
                    local: "recommendations".into(),
                    key: call.to_string(),
                }],
            },
            &request,
        );
        let Answer::Approve { withhold } = answer else {
            panic!("approve expected")
        };
        assert_eq!(
            withhold,
            vec![(LocalName::parse("recommendations").unwrap(), call)]
        );
        assert!(matches!(
            host_answer(consent::Answer::Deny, &request),
            Answer::Deny
        ));
    }
    #[tokio::test]
    async fn ac2_uses_resolves_an_already_installed_provider() {
        let home = tempfile::tempdir().unwrap();
        let host = watane_host::Host::open(
            home.path(),
            watane_host::Mode::Preview,
            watane_host::Limits::default(),
        )
        .unwrap();
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let plan = host
            .plan(vec![watane_host::Source::Folder(
                fixtures.join("reading-list"),
            )])
            .unwrap();
        host.install(plan, Requester::App).await.unwrap();
        let plan = host
            .plan(vec![watane_host::Source::Folder(
                fixtures.join("recommendations"),
            )])
            .unwrap();
        let request = InstallRequest {
            entries: plan.entries().to_vec(),
            requester: Requester::App,
        };
        let consent::Request::Install { mods, by_program } =
            request_view(&request, &Parties::new(), Some(&host))
        else {
            panic!("install expected")
        };
        assert!(!by_program);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].uses, ["reading-list (local)"]);
    }
}
