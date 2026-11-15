//! Conversions between net's and dist's types (TECHNICAL 4.3).
use std::str::FromStr;
use watane_dist::Grant;
use watane_dist::{InterfaceQuery, PackageId, Release, Slug};
use watane_net::BlobId;

pub(crate) fn package_id(id: &BlobId) -> PackageId {
    PackageId::from_bytes(id.0)
}

pub(crate) fn blob_id(id: &PackageId) -> BlobId {
    BlobId(*id.as_bytes())
}

/// A net query as dist's query and the interface `(mode, name)` that dist cannot
/// express. `None`: the query names a bad origin slug, interface or mode, so it has no hits.
pub(crate) fn query(
    q: &watane_net::Query,
) -> Option<(watane_dist::Query, Option<(String, String)>)> {
    let origin = match &q.origin {
        Some((key, slug)) => Some((*key, Slug::from_str(slug).ok()?)),
        None => None,
    };
    let interface = match &q.interface {
        Some((mode, name)) => {
            if !matches!(mode.as_str(), "offers" | "uses" | "sends") {
                return None;
            }
            Some((
                InterfaceQuery::from_str(name).ok()?,
                (mode.clone(), name.clone()),
            ))
        }
        None => None,
    };
    let (iq, mode) = match interface {
        Some((iq, mode)) => (Some(iq), Some(mode)),
        None => (None, None),
    };
    Some((
        watane_dist::Query {
            text: q.text.clone().unwrap_or_default(),
            publisher: q.publisher,
            origin,
            interface: iq,
        },
        mode,
    ))
}

fn named(interface: &str, name: &str) -> bool {
    if name.contains('@') {
        interface == name
    } else {
        interface.split_once('@').is_some_and(|(n, _)| n == name)
    }
}

/// Whether the release has the interface `name` in `mode` (`offers`, `sends`, `uses`).
pub(crate) fn has_in_mode(release: &Release, mode: &str, name: &str) -> bool {
    match mode {
        "offers" => release.offers.iter().any(|i| named(&i.to_string(), name)),
        "sends" => release.events.iter().any(|i| named(&i.to_string(), name)),
        "uses" => release.grants.iter().any(|g| match g {
            Grant::Call { interface, .. } | Grant::Subscribe { interface, .. } => {
                named(&interface.to_string(), name)
            }
            _ => false,
        }),
        _ => false,
    }
}

/// Dist's query as one net query for a remote machine. The interface is left out:
/// `InterfaceQuery` has no accessor, and `search_stored` applies it again (5.4).
pub(crate) fn net_query(q: &watane_dist::Query) -> watane_net::Query {
    watane_net::Query {
        text: Some(q.text.clone()),
        publisher: q.publisher,
        interface: None,
        origin: q.origin.as_ref().map(|(k, s)| (*k, s.to_string())),
        limit: 200,
    }
}

pub(crate) fn hit(h: &watane_dist::Hit) -> watane_net::Hit {
    watane_net::Hit {
        channel: h.info.channel,
        seq: h.info.seq,
        entry: h.info.entry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use watane_dist::PublicKey;
    use watane_dist::{InterfaceName, Provider};

    fn key() -> PublicKey {
        (1..=255u8)
            .find_map(|n| PublicKey::from_bytes(&[n; 32]).ok())
            .unwrap()
    }

    fn net_query() -> watane_net::Query {
        watane_net::Query {
            text: None,
            publisher: None,
            interface: None,
            origin: None,
            limit: 10,
        }
    }

    fn release() -> Release {
        let iface = |s: &str| s.parse::<InterfaceName>().unwrap();
        Release {
            slug: "app".parse().unwrap(),
            version: "1.0.0".parse().unwrap(),
            name: "App".into(),
            summary: String::new(),
            package: PackageId::of(b"x"),
            size: 1,
            source: false,
            offers: vec![iface("mods:a/books@0.1.0")],
            events: vec![iface("mods:a/ping@0.1.0")],
            grants: vec![
                Grant::Storage,
                Grant::Call {
                    interface: iface("mods:b/calls@0.2.0"),
                    provider: Provider {
                        slug: "prov".parse().unwrap(),
                        publisher: None,
                    },
                },
                Grant::Subscribe {
                    interface: iface("mods:c/news@0.3.0"),
                    provider: Provider {
                        slug: "prov".parse().unwrap(),
                        publisher: None,
                    },
                },
            ],
            origin: None,
        }
    }

    #[test]
    fn ac5_blob_and_package_ids_round_trip() {
        let id = PackageId::of(b"package");
        let blob = blob_id(&id);
        assert_eq!(blob.0, *id.as_bytes());
        assert_eq!(package_id(&blob), id);
        assert_eq!(blob_id(&package_id(&BlobId([9; 32]))), BlobId([9; 32]));
    }

    #[test]
    fn ac5_query_text_null_is_empty_and_fields_carry_over() {
        let (q, mode) = query(&net_query()).unwrap();
        assert_eq!(q.text, "");
        assert!(q.publisher.is_none() && q.origin.is_none() && q.interface.is_none());
        assert!(mode.is_none());
        let full = watane_net::Query {
            text: Some("books".into()),
            publisher: Some(key()),
            interface: Some(("offers".into(), "mods:a/books".into())),
            origin: Some((key(), "app".into())),
            limit: 5,
        };
        let (q, mode) = query(&full).unwrap();
        assert_eq!(q.text, "books");
        assert_eq!(q.publisher, Some(key()));
        assert_eq!(q.origin, Some((key(), "app".parse().unwrap())));
        assert_eq!(q.interface, Some("mods:a/books".parse().unwrap()));
        assert_eq!(mode, Some(("offers".into(), "mods:a/books".into())));
    }

    #[test]
    fn ac5_bad_origin_slug_interface_or_mode_gives_no_hits() {
        let mut q = net_query();
        q.origin = Some((key(), "Not A Slug".into()));
        assert!(query(&q).is_none());
        let mut q = net_query();
        q.interface = Some(("offers".into(), "not an interface".into()));
        assert!(query(&q).is_none());
        let mut q = net_query();
        q.interface = Some(("hears".into(), "mods:a/books".into()));
        assert!(query(&q).is_none());
    }

    #[test]
    fn ac5_mode_filter() {
        let r = release();
        assert!(has_in_mode(&r, "offers", "mods:a/books"));
        assert!(has_in_mode(&r, "offers", "mods:a/books@0.1.0"));
        assert!(!has_in_mode(&r, "offers", "mods:a/books@0.2.0"));
        assert!(!has_in_mode(&r, "sends", "mods:a/books"));
        assert!(has_in_mode(&r, "sends", "mods:a/ping"));
        assert!(has_in_mode(&r, "uses", "mods:b/calls"));
        assert!(has_in_mode(&r, "uses", "mods:c/news"));
        assert!(!has_in_mode(&r, "uses", "mods:a/books"));
        assert!(!has_in_mode(&r, "hears", "mods:a/books"));
    }
}
