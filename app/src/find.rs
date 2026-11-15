use crate::{
    CmdError,
    words::{self, Parties},
};
use serde_json::{Value, json};
use watane_dist::{Hit, Query, Via};

#[derive(serde::Deserialize)]
pub(crate) struct SearchQuery {
    pub text: String,
    pub publisher: Option<String>,
    pub interface: Option<String>,
    pub origin: Option<String>,
}
impl SearchQuery {
    pub fn parse(self) -> Result<Query, CmdError> {
        Ok(Query {
            text: self.text,
            origin: self
                .origin
                .map(|text| -> Result<_, CmdError> {
                    let bad = || CmdError::new("bad_input", "expected origin <key>/<slug>");
                    let (key, slug) = text.split_once('/').ok_or_else(bad)?;
                    Ok((
                        key.parse().map_err(|_| bad())?,
                        slug.parse().map_err(|_| bad())?,
                    ))
                })
                .transpose()?,
            publisher: self
                .publisher
                .map(|p| {
                    p.parse().map_err(|e: watane_dist::KeyTextError| {
                        CmdError::new("bad_input", e.to_string())
                    })
                })
                .transpose()?,
            interface: self
                .interface
                .map(|i| {
                    i.parse().map_err(|e: watane_dist::DistError| {
                        CmdError::new("bad_input", e.to_string())
                    })
                })
                .transpose()?,
        })
    }
}
fn status(hit: &Hit) -> &'static str {
    if matches!(hit.status, watane_dist::Status::Ok) {
        "ok"
    } else {
        "compromised"
    }
}
fn uses(hit: &Hit) -> Vec<String> {
    hit.info
        .release
        .grants
        .iter()
        .filter_map(|g| match g {
            watane_host::watane_pkg::Grant::Call { interface, .. }
            | watane_host::watane_pkg::Grant::Subscribe { interface, .. } => {
                Some(interface.to_string())
            }
            _ => None,
        })
        .collect()
}
fn via(hit: &Hit, parties: Option<&Parties>) -> Vec<Value> {
    let party = |key: &watane_dist::PublicKey| match parties {
        Some(parties) => json!(words::party(&key.to_string(), parties)),
        None => json!(key.to_string()),
    };
    hit.via.iter().filter_map(|v| match v {
        Via::Own => Some(json!({"kind":"own"})),
        Via::Followed => Some(json!({"kind":"followed"})),
        Via::Index { index, through, note } => Some(json!({"kind":"index", "index":party(index), "through":through.as_ref().map(party), "note":note})),
        // The app does not keep channels yet; their provenance view is stage G.
        Via::Kept => None,
    }).collect()
}
pub(crate) fn api_result(hit: &Hit) -> Value {
    json!({"release":hit.info.reference.to_string(), "name":hit.info.release.name, "summary":hit.info.release.summary,
        "link":hit.link.as_ref().map(ToString::to_string),
        "publisher":{"key":hit.info.reference.publisher.to_string(),"name":hit.publisher_name,"fingerprint":hit.info.channel.fingerprint()},
        "status":status(hit),"checked_at":hit.checked_at,"unchecked_since":hit.unchecked_since,
        "via":via(hit,None),"offers":hit.info.release.offers.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "events":hit.info.release.events.iter().map(ToString::to_string).collect::<Vec<_>>(),"uses":uses(hit)})
}
pub(crate) fn hit_view(
    hit: &Hit,
    installed: &[watane_host::InstalledMod],
    parties: &Parties,
) -> Value {
    let mut view = api_result(hit);
    let row = view.as_object_mut().unwrap();
    row.remove("checked_at");
    row.remove("unchecked_since");
    row.insert("checkedAt".into(), json!(hit.checked_at));
    row.insert("uncheckedSince".into(), json!(hit.unchecked_since));
    row.insert(
        "origin".into(),
        json!(hit.info.release.origin.as_ref().map(ToString::to_string)),
    );
    row.insert("slug".into(), json!(hit.info.reference.slug.to_string()));
    row.insert(
        "version".into(),
        json!(hit.info.reference.version.to_string()),
    );
    row.insert(
        "publisher".into(),
        json!(crate::consent::Party {
            you: words::party(&hit.info.channel.to_string(), parties).you,
            name: hit.publisher_name.clone(),
            fingerprint: hit.info.channel.fingerprint()
        }),
    );
    row.insert(
        "verified".into(),
        json!(matches!(hit.status, watane_dist::Status::Ok)),
    );
    row.insert(
        "via".into(),
        json!(
            via(hit, Some(parties))
                .into_iter()
                .map(|mut v| {
                    let object = v.as_object_mut().unwrap();
                    object.entry("index").or_insert(Value::Null);
                    object.entry("through").or_insert(Value::Null);
                    object.entry("note").or_insert(json!(""));
                    v
                })
                .collect::<Vec<_>>()
        ),
    );
    row.insert(
        "installed".into(),
        json!(
            installed
                .iter()
                .find(
                    |m| m.publisher.as_ref() == Some(&hit.info.reference.publisher)
                        && m.slug == hit.info.reference.slug
                )
                .map(|m| m.local.to_string())
        ),
    );
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ac7_search_rejects_bad_origin() {
        for origin in [
            "",
            "bad",
            "bad/ratings",
            "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy/ratings@0.1.0",
        ] {
            let input = serde_json::json!({"text":"", "origin":origin});
            let query: SearchQuery = serde_json::from_value(input).unwrap();
            assert_eq!(query.parse().unwrap_err().code, "bad_input", "{origin}");
        }
        let key = "47pjoycnsrfmxikm95jh13y88e8qnhzu5kungjpxyepgt7a8krpy";
        let query: SearchQuery = serde_json::from_value(
            serde_json::json!({"text":"", "origin":format!("{key}/ratings")}),
        )
        .unwrap();
        assert_eq!(
            query.parse().unwrap().origin.unwrap().1.to_string(),
            "ratings"
        );
    }
}
