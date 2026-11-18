#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use support::*;
use watane_dist::*;

fn setup() -> (SearchHome, SearchHome, SearchHome) {
    let alice = SearchHome::new("Alice Reader");
    let ivy = SearchHome::new("Ivy");
    let carol = SearchHome::new("Carol");
    alice.dist.publish(fixture("0.1.0", false)).unwrap();
    alice.dist.publish(fixture("0.2.0", false)).unwrap();
    ivy.dist
        .list(&alice.link(Some("reading-list")), "good for books")
        .unwrap();
    carol.dist.choose_index(&ivy.link(None)).unwrap();
    (alice, ivy, carol)
}
fn refs(hits: &[Hit]) -> Vec<String> {
    hits.iter().map(|h| h.info.reference.to_string()).collect()
}
/// `last_success` of `key` in `home`'s follows.json, read without `watane_dist`.
fn last_success(home: &SearchHome, key: PublicKey) -> Option<u64> {
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.dir.path().join("dist/follows.json")).unwrap())
            .unwrap();
    file["channels"][key.to_string()]["last_success"].as_u64()
}
fn reference(home: &SearchHome, slug: &str, version: &str) -> String {
    format!("{}/{slug}@{version}", home.dist.identity().unwrap())
}

#[test]
fn ac1_search_finds() {
    let (alice, ivy, carol) = setup();
    for text in [
        "reading",
        "READING",
        "ReAdInG BOOKS alice",
        "",
        "good reader",
    ] {
        let hits = carol
            .dist
            .search(&Query {
                text: text.into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(refs(&hits), [reference(&alice, "reading-list", "0.2.0")]);
        let hit = &hits[0];
        assert_eq!(hit.publisher_name.as_deref(), Some("Alice Reader"));
        assert_eq!(hit.link, Some(alice.link(Some("reading-list"))));
        assert_eq!(
            hit.via,
            [Via::Index {
                index: ivy.dist.identity().unwrap(),
                through: None,
                note: "good for books".into()
            }]
        );
        assert_eq!(hit.status, Status::Ok);
        assert_eq!(
            hit.checked_at,
            last_success(&carol, alice.dist.identity().unwrap())
        );
        assert!(hit.checked_at.is_some());
        assert_eq!(hit.unchecked_since, None);
    }
    assert!(
        carol
            .dist
            .search(&Query {
                text: "reading impossible".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
}
#[test]
fn ac2_search_withdrawn() {
    let (alice, ivy, carol) = setup();
    alice
        .dist
        .withdraw(
            &reference(&alice, "reading-list", "0.2.0").parse().unwrap(),
            "bug",
        )
        .unwrap();
    carol.dist.refresh();
    assert_eq!(
        refs(&carol.dist.search(&Query::default()).unwrap()),
        [reference(&alice, "reading-list", "0.1.0")]
    );
    alice
        .dist
        .withdraw(
            &reference(&alice, "reading-list", "0.1.0").parse().unwrap(),
            "gone",
        )
        .unwrap();
    carol.dist.refresh();
    assert!(carol.dist.search(&Query::default()).unwrap().is_empty());
    ivy.dist.publish(fixture("0.1.0", false)).unwrap();
    carol.dist.refresh();
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(refs(&hits), [reference(&ivy, "reading-list", "0.1.0")]);
    assert_eq!(
        hits[0].via,
        [Via::Index {
            index: ivy.dist.identity().unwrap(),
            through: None,
            note: "".into()
        }]
    );
}
#[test]
fn ac2_search_listing_without_stored_publisher() {
    let mut alice = SearchHome::new("Alice");
    let ivy = SearchHome::new("Ivy");
    let carol = SearchHome::new("Carol");
    alice.dist.publish(fixture("0.1.0", false)).unwrap();
    ivy.dist
        .list(&alice.link(Some("reading-list")), "good")
        .unwrap();
    alice.host.take();
    carol.dist.choose_index(&ivy.link(None)).unwrap();
    assert!(
        carol
            .dist
            .channel(&alice.dist.identity().unwrap())
            .unwrap()
            .releases
            .is_empty()
    );
    assert!(carol.dist.search(&Query::default()).unwrap().is_empty());
}
#[test]
fn ac3_search_mark() {
    let (mut alice, _ivy, carol) = setup();
    let before = last_success(&carol, alice.dist.identity().unwrap());
    assert!(before.is_some());
    alice.host.take();
    let reports = carol.dist.refresh();
    assert!(
        reports
            .iter()
            .any(|r| r.channel == alice.dist.identity().unwrap()
                && matches!(r.outcome, Outcome::Failed { .. }))
    );
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(refs(&hits), [reference(&alice, "reading-list", "0.2.0")]);
    assert_eq!(hits[0].checked_at, before);
    assert_eq!(hits[0].unchecked_since, before);
    assert_eq!(last_success(&carol, alice.dist.identity().unwrap()), before);
}
#[test]
fn ac4_search_bad_query() {
    assert!(matches!(
        "x".parse::<InterfaceQuery>(),
        Err(DistError::BadQuery { .. })
    ));
    let home = SearchHome::offline("Carol");
    assert!(matches!(
        home.dist.search(&Query {
            text: "é".repeat(129),
            ..Default::default()
        }),
        Err(DistError::BadQuery { .. })
    ));
    home.dist
        .search(&Query {
            text: "é".repeat(128),
            ..Default::default()
        })
        .unwrap();
}
#[test]
fn ac5_search_offline() {
    let (mut alice, mut ivy, mut carol) = setup();
    let expected = reference(&alice, "reading-list", "0.2.0");
    alice.host.take();
    ivy.host.take();
    carol.host.take();
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(refs(&hits), std::slice::from_ref(&expected));
    assert_eq!(hits[0].unchecked_since, None);
    drop(carol.dist);
    // Reopen with the original listed_local setting.
    let reopened = Dist::open(
        carol.dir.path(),
        std::sync::Arc::new(RecordingConfirm::new([])),
        Options {
            listed_local: true,
            ..fast_options()
        },
    )
    .unwrap();
    assert_eq!(
        refs(&reopened.search(&Query::default()).unwrap()),
        [expected]
    );
}
#[test]
fn ac6_search_order() {
    let alice = SearchHome::new("Alice");
    let ivy = SearchHome::new("Ivy");
    let jay = SearchHome::new("Jay");
    let carol = SearchHome::new("Carol");
    alice
        .dist
        .publish(named_fixture("listed", "zebra"))
        .unwrap();
    carol
        .dist
        .publish(named_fixture("personal", "Alpha"))
        .unwrap();
    ivy.dist
        .publish(named_fixture("index-mod", "Zebra index"))
        .unwrap();
    jay.dist
        .publish(named_fixture("jay-mod", "aardvark"))
        .unwrap();
    ivy.dist.list(&alice.link(Some("listed")), "one").unwrap();
    jay.dist.list(&alice.link(Some("listed")), "two").unwrap();
    carol.dist.choose_index(&ivy.link(None)).unwrap();
    carol.dist.choose_index(&jay.link(None)).unwrap();
    carol.dist.follow(&alice.link(None)).unwrap();
    carol.dist.follow(&ivy.link(None)).unwrap();
    carol
        .dist
        .list(&alice.link(Some("listed")), "mine")
        .unwrap();
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(
        refs(&hits),
        [
            reference(&alice, "listed", "0.1.0"),
            reference(&ivy, "index-mod", "0.1.0"),
            reference(&jay, "jay-mod", "0.1.0"),
            reference(&carol, "personal", "0.1.0")
        ]
    );
    let mut indexes = vec![
        carol.dist.identity().unwrap(),
        ivy.dist.identity().unwrap(),
        jay.dist.identity().unwrap(),
    ];
    indexes.sort();
    assert_eq!(hits[0].via[0], Via::Followed);
    assert_eq!(
        hits[0]
            .via
            .iter()
            .filter_map(|v| match v {
                Via::Index { index, .. } => Some(*index),
                _ => None,
            })
            .collect::<Vec<_>>(),
        indexes
    );
    assert_eq!(hits[3].via, [Via::Own]);
    assert!(hits[3].link.is_none());
    assert!(hits[3].checked_at.is_none());
    assert!(hits[3].unchecked_since.is_none());
}
#[test]
fn ac6_search_name_reference_order() {
    let home = SearchHome::offline("Carol");
    let mut expected = Vec::new();
    for n in (0..4).rev() {
        let slug = format!(
            "mod-{}{}",
            (b'a' + (n / 26) as u8) as char,
            (b'a' + (n % 26) as u8) as char
        );
        let name = if n % 2 == 0 { "Alpha" } else { "alpha" };
        home.dist.publish(named_fixture(&slug, name)).unwrap();
        expected.push(reference(&home, &slug, "0.1.0"));
    }
    expected.sort();
    assert_eq!(
        refs(&home.dist.search(&Query::default()).unwrap()),
        expected
    );
}

#[test]
fn ac4_search_filters() {
    let alice = SearchHome::new("Alice");
    let bob = SearchHome::new("Bob");
    let carol = SearchHome::new("Carol");
    alice
        .dist
        .publish(interface_fixture("offer", alice.dist.identity().unwrap()))
        .unwrap();
    alice
        .dist
        .publish(interface_fixture("call", alice.dist.identity().unwrap()))
        .unwrap();
    bob.dist
        .publish(interface_fixture(
            "subscribe",
            alice.dist.identity().unwrap(),
        ))
        .unwrap();
    bob.dist
        .publish(named_fixture("unrelated", "Unrelated"))
        .unwrap();
    carol.dist.follow(&alice.link(None)).unwrap();
    carol.dist.follow(&bob.link(None)).unwrap();
    let mut expected = vec![
        reference(&alice, "reading-list", "0.1.0"),
        reference(&alice, "caller", "0.1.0"),
        reference(&bob, "subscriber", "0.1.0"),
    ];
    expected.sort();
    for interface in ["mods:reading-list/books@0.1.0", "mods:reading-list/books"] {
        let query = Query {
            interface: Some(interface.parse().unwrap()),
            ..Default::default()
        };
        let mut found = refs(&carol.dist.search(&query).unwrap());
        found.sort();
        assert_eq!(found, expected);
        let mut found = refs(
            &carol
                .dist
                .search(&Query {
                    publisher: Some(alice.dist.identity().unwrap()),
                    ..query
                })
                .unwrap(),
        );
        found.sort();
        let mut alice_expected = vec![
            reference(&alice, "reading-list", "0.1.0"),
            reference(&alice, "caller", "0.1.0"),
        ];
        alice_expected.sort();
        assert_eq!(found, alice_expected);
    }
    assert!(
        carol
            .dist
            .search(&Query {
                interface: Some("mods:reading-list/books@0.2.0".parse().unwrap()),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    let mut found = refs(
        &carol
            .dist
            .search(&Query {
                text: "mods:reading-list/books".into(),
                ..Default::default()
            })
            .unwrap(),
    );
    found.sort();
    assert_eq!(found, expected);
}

#[test]
fn ac7_search_compromised() {
    let (alice, _ivy, carol) = setup();
    // Independent log-only A15 fork; the publisher's original release stays stored.
    let dir = tempfile::tempdir().unwrap();
    let fork = watane_log::Log::open(&dir.path().join("log")).unwrap();
    fork.restore_identity(&alice.dist.backup("passphrase").unwrap(), "passphrase")
        .unwrap();
    let zero =
        Entry::decode(&std::fs::read(alice.dist.site_dir().join("entries/0")).unwrap()).unwrap();
    fork.ingest(&alice.dist.identity().unwrap(), &[zero])
        .unwrap();
    fork.confirm_own_head().unwrap();
    let other = fork.append("profile", b"\x81\x65Other").unwrap();
    std::fs::write(alice.dist.site_dir().join("head"), other.bytes()).unwrap();
    std::fs::write(alice.dist.site_dir().join("entries/1"), other.bytes()).unwrap();
    carol.dist.refresh();
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(refs(&hits), [reference(&alice, "reading-list", "0.2.0")]);
    assert_eq!(hits[0].status, Status::Compromised { seq: 1 });
}

#[test]
fn ac6_search_distinct_indexes_and_included_provenance() {
    let alice = SearchHome::new("Alice");
    let bob = SearchHome::new("Bob");
    let ivy = SearchHome::new("Ivy");
    let jay = SearchHome::new("Jay");
    let carol = SearchHome::new("Carol");
    alice.dist.publish(named_fixture("books", "Zulu")).unwrap();
    bob.dist.publish(named_fixture("books", "alpha")).unwrap();
    ivy.dist.list(&alice.link(Some("books")), "direct").unwrap();
    ivy.dist.list(&bob.link(Some("books")), "other").unwrap();
    ivy.dist.include(&alice.link(None), "included").unwrap();
    carol.dist.choose_index(&ivy.link(None)).unwrap();
    let hits = carol.dist.search(&Query::default()).unwrap();
    assert_eq!(
        refs(&hits),
        [
            reference(&bob, "books", "0.1.0"),
            reference(&alice, "books", "0.1.0")
        ]
    );
    assert_eq!(
        hits[1].via,
        [
            Via::Index {
                index: ivy.dist.identity().unwrap(),
                through: None,
                note: "direct".into()
            },
            Via::Index {
                index: ivy.dist.identity().unwrap(),
                through: Some(alice.dist.identity().unwrap()),
                note: "".into()
            },
        ]
    );
    jay.dist
        .list(&alice.link(Some("books")), "second index")
        .unwrap();
    carol.dist.choose_index(&jay.link(None)).unwrap();
    assert_eq!(
        refs(&carol.dist.search(&Query::default()).unwrap()),
        [
            reference(&alice, "books", "0.1.0"),
            reference(&bob, "books", "0.1.0")
        ]
    );
}

#[test]
fn ac8_search_origin() {
    let alice = SearchHome::new("Alice");
    let carol = SearchHome::new("Carol");
    let eve = SearchHome::new("Eve");
    for version in ["0.1.0", "0.2.0"] {
        alice
            .dist
            .publish(origin_fixture("reading-list", version, None))
            .unwrap();
    }
    let origin = reference(&alice, "reading-list", "0.1.0").parse().unwrap();
    carol
        .dist
        .publish(origin_fixture("ratings", "0.1.0", Some(&origin)))
        .unwrap();
    eve.dist.follow(&alice.link(None)).unwrap();
    eve.dist.follow(&carol.link(None)).unwrap();
    let query = Query {
        origin: Some((
            alice.dist.identity().unwrap(),
            "reading-list".parse().unwrap(),
        )),
        ..Default::default()
    };
    assert_eq!(
        refs(&eve.dist.search(&query).unwrap()),
        [reference(&carol, "ratings", "0.1.0")]
    );
    assert!(
        eve.dist
            .search(&Query {
                origin: Some((alice.dist.identity().unwrap(), "other".parse().unwrap())),
                ..query
            })
            .unwrap()
            .is_empty()
    );
}
