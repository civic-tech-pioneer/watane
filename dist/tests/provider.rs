mod support;
use support::*;
use watane_dist::*;

fn reference(home: &SearchHome, slug: &str) -> ReleaseRef {
    format!("{}/{slug}@0.1.0", home.dist.identity().unwrap())
        .parse()
        .unwrap()
}
fn records(home: &SearchHome) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(home.dir.path().join("dist/follows.json")).unwrap())
        .unwrap()
}
fn setup() -> (SearchHome, SearchHome, SearchHome, PackageId) {
    let carol = SearchHome::new("Carol");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    let bytes = named_fixture("ratings", "Ratings");
    let id = PackageId::of(&bytes);
    carol.dist.publish(bytes).unwrap();
    dave.dist
        .list(&carol.link(Some("ratings")), "provider")
        .unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    (carol, dave, eve, id)
}
#[test]
fn ac1_provider_from_listing() {
    let (carol, dave, eve, id) = setup();
    let got = eve.dist.fetch(&reference(&carol, "ratings")).unwrap();
    assert_eq!(got.package.id(), id);
    assert_eq!(got.link, Some(carol.link(Some("ratings"))));
    assert!(
        records(&eve)["channels"]
            .get(carol.dist.identity().unwrap().to_string())
            .is_none()
    );
    assert_eq!(
        eve.dist
            .followed()
            .unwrap()
            .iter()
            .map(|v| v.channel)
            .collect::<Vec<_>>(),
        [dave.dist.identity().unwrap()]
    );
    assert!(
        eve.dist
            .search(&Query {
                text: "ratings".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    let host = carol.host.as_ref().unwrap();
    let paths = host.requests.lock().unwrap();
    assert_eq!(
        paths
            .iter()
            .filter(|p| *p == &format!("/packages/{id}"))
            .count(),
        1
    );
}
#[test]
fn ac2_provider_wrong_channel() {
    let carol = SearchHome::new("Carol");
    let mallory = SearchHome::new("Mallory");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    carol
        .dist
        .publish(named_fixture("ratings", "Ratings"))
        .unwrap();
    dave.dist.follow(&carol.link(None)).unwrap();
    let wrong = Link::new(
        carol.dist.identity().unwrap(),
        &mallory.url,
        Some(&"ratings".parse().unwrap()),
    )
    .unwrap();
    dave.dist.list(&wrong, "provider").unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    assert!(
        matches!(eve.dist.fetch(&reference(&carol, "ratings")), Err(DistError::WrongChannel { expected, found, .. }) if expected == carol.dist.identity().unwrap() && found == mallory.dist.identity().unwrap())
    );
    assert_eq!(
        std::fs::read_dir(eve.dir.path().join("dist/packages"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        eve.dist
            .channel(&mallory.dist.identity().unwrap())
            .unwrap()
            .head
            .is_none()
    );
}
#[test]
fn ac3_provider_no_hint() {
    let carol = SearchHome::new("Carol");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    eve.dist.follow(&dave.link(None)).unwrap();
    let error = eve.dist.fetch(&reference(&carol, "ratings")).unwrap_err();
    assert!(
        matches!(error, DistError::NotFollowed { channel, local_only: false } if channel == carol.dist.identity().unwrap())
    );
    let text = error.to_string();
    assert!(text.contains("no followed channel, chosen index or listing names a base"));
    assert!(text.contains("follow the publisher's link"));
    assert_eq!(carol.host.as_ref().unwrap().count(), 0);
    carol
        .dist
        .publish(named_fixture("ratings", "Ratings"))
        .unwrap();
    dave.dist
        .list(&carol.link(Some("ratings")), "provider")
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let restricted = open(dir.path());
    restricted.follow(&dave.link(None)).unwrap();
    let before = carol.host.as_ref().unwrap().count();
    let error = restricted.fetch(&reference(&carol, "ratings")).unwrap_err();
    assert!(
        matches!(error, DistError::NotFollowed { channel, local_only: true } if channel == carol.dist.identity().unwrap())
    );
    let text = error.to_string();
    assert!(text.contains("local address"));
    assert!(!text.contains("no followed channel, chosen index or listing names a base"));
    assert!(text.contains("follow the publisher's link"));
    assert_eq!(carol.host.as_ref().unwrap().count(), before);
}
#[test]
fn ac4_provider_trust_order() {
    let carol = SearchHome::new("Carol");
    let gil = SearchHome::new("Gil");
    let first = SearchHome::new("First");
    let second = SearchHome::new("Second");
    let (dave, zed) = if first.dist.identity().unwrap() > second.dist.identity().unwrap() {
        (first, second)
    } else {
        (second, first)
    };
    dave.dist.set_name("Dave").unwrap();
    zed.dist.set_name("Zed").unwrap();
    let eve = SearchHome::new("Eve");
    gil.dist.publish(named_fixture("tags", "Tags")).unwrap();
    carol
        .dist
        .list(&gil.link(Some("tags")), "nested provider")
        .unwrap();
    for slug in ["ratings", "one", "two", "three"] {
        carol.dist.publish(named_fixture(slug, slug)).unwrap();
    }
    zed.dist.follow(&carol.link(None)).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let dead: Vec<_> = (0..4).map(|_| TestHost::serve(empty.path())).collect();
    for (host, slug) in dead.iter().zip(["ratings", "one", "two", "three"]) {
        zed.dist
            .list(
                &Link::new(
                    carol.dist.identity().unwrap(),
                    &host.url,
                    Some(&slug.parse().unwrap()),
                )
                .unwrap(),
                "dead",
            )
            .unwrap();
    }
    dave.dist
        .list(&carol.link(Some("ratings")), "real")
        .unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    eve.dist.choose_index(&zed.link(None)).unwrap();
    let before = records(&eve)["channels"][carol.dist.identity().unwrap().to_string()].clone();
    assert_eq!(
        before["bases"],
        serde_json::json!(dead.iter().map(|h| &h.url).collect::<Vec<_>>())
    );
    assert!(before["last_attempt"].is_u64());
    assert!(before["last_success"].is_null());
    carol
        .host
        .as_ref()
        .unwrap()
        .request_times
        .lock()
        .unwrap()
        .clear();
    for h in &dead {
        h.request_times.lock().unwrap().clear();
    }
    let got = eve
        .dist
        .fetch_for(
            &reference(&carol, "ratings"),
            Some(&dave.dist.identity().unwrap()),
        )
        .unwrap();
    assert_eq!(got.link, Some(carol.link(Some("ratings"))));
    let first = carol.host.as_ref().unwrap().request_times.lock().unwrap()[0];
    for h in &dead {
        if let Some(time) = h.request_times.lock().unwrap().first() {
            assert!(first < *time);
        }
    }
    assert_eq!(
        dead.iter()
            .filter(|h| !h.request_times.lock().unwrap().is_empty())
            .count(),
        // 5.11 step 3 amendment (DD-32): the first base serves, so no dead base is read.
        0
    );
    let after = records(&eve)["channels"][carol.dist.identity().unwrap().to_string()].clone();
    assert_eq!(before, after);
    let nested = eve
        .dist
        .fetch_for(
            &reference(&gil, "tags"),
            Some(&carol.dist.identity().unwrap()),
        )
        .unwrap();
    assert_eq!(nested.link, Some(gil.link(Some("tags"))));
}
#[test]
fn ac5_provider_cap_four() {
    let carol = SearchHome::new("Carol");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    let slugs = ["ratings", "one", "two", "three", "four"];
    for slug in slugs {
        carol.dist.publish(named_fixture(slug, slug)).unwrap();
    }
    dave.dist.follow(&carol.link(None)).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let hosts: Vec<_> = (0..5).map(|_| TestHost::serve(empty.path())).collect();
    for (host, slug) in hosts.iter().zip(slugs) {
        dave.dist
            .list(
                &Link::new(
                    carol.dist.identity().unwrap(),
                    &host.url,
                    Some(&slug.parse().unwrap()),
                )
                .unwrap(),
                "hint",
            )
            .unwrap();
    }
    eve.dist.follow(&dave.link(None)).unwrap();
    assert!(matches!(
        eve.dist.fetch(&reference(&carol, "ratings")),
        Err(DistError::NotFound { .. })
    ));
    assert!(
        hosts[..4]
            .iter()
            .all(|h| h.requests.lock().unwrap().as_slice() == ["/head"])
    );
    assert_eq!(hosts[4].count(), 0);
}
#[test]
fn ac6_read_link_no_follow() {
    let carol = SearchHome::new("Carol");
    let eve = SearchHome::new("Eve");
    carol
        .dist
        .publish(named_fixture("ratings", "Old ratings"))
        .unwrap();
    let bytes = origin_fixture("ratings", "0.2.0", None);
    let id = PackageId::of(&bytes);
    carol.dist.publish(bytes).unwrap();
    let path = eve.dir.path().join("dist/follows.json");
    let before = std::fs::read(&path).ok();
    let link = carol.link(Some("ratings"));
    let got = eve.dist.read_link(&link).unwrap();
    assert_eq!(got.package.id(), id);
    assert_eq!(got.link, Some(link.clone()));
    assert_eq!(std::fs::read(&path).ok(), before);
    assert!(matches!(
        eve.dist.read_link(&carol.link(None)),
        Err(DistError::BadLink { .. })
    ));
    let dir = tempfile::tempdir().unwrap();
    let restricted = open(dir.path());
    let count = carol.host.as_ref().unwrap().count();
    assert!(matches!(
        restricted.read_link(&link),
        Err(DistError::BadUrl {
            cause: "local address",
            ..
        })
    ));
    assert_eq!(carol.host.as_ref().unwrap().count(), count);
    eve.dist.follow(&carol.link(None)).unwrap();
    let before = records(&eve)["channels"].as_object().unwrap().len();
    assert_eq!(eve.dist.read_link(&link).unwrap().link, Some(link));
    assert_eq!(records(&eve)["channels"].as_object().unwrap().len(), before);
    assert_eq!(eve.dist.followed().unwrap().len(), 1);
}
#[test]
fn ac7_provider_offline() {
    let (mut carol, mut dave, mut eve, id) = setup();
    let reference = reference(&carol, "ratings");
    let link = carol.link(Some("ratings"));
    eve.dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    carol.host.take();
    dave.host.take();
    eve.host.take();
    let got = eve
        .dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    assert_eq!(got.package.id(), id);
    assert_eq!(got.link, Some(link));
}
#[test]
fn ac9_fetch_compatibility() {
    let (carol, _dave, eve, id) = setup();
    eve.dist.follow(&carol.link(None)).unwrap();
    let reference = reference(&carol, "ratings");
    let fetched = eve.dist.fetch(&reference).unwrap();
    let contextual = eve.dist.fetch_for(&reference, None).unwrap();
    assert_eq!(fetched.package.id(), id);
    assert_eq!(fetched.entry.bytes(), contextual.entry.bytes());
    assert_eq!(fetched.link, contextual.link);
    assert_eq!(fetched.link, Some(carol.link(Some("ratings"))));
    assert!(carol.dist.fetch(&reference).unwrap().link.is_none());
}
#[test]
fn provider_cached_without_hint() {
    let (carol, dave, eve, id) = setup();
    let reference = reference(&carol, "ratings");
    eve.dist.fetch(&reference).unwrap();
    eve.dist.unfollow(&dave.dist.identity().unwrap()).unwrap();
    let count = carol.host.as_ref().unwrap().count();
    let got = eve.dist.fetch(&reference).unwrap();
    assert_eq!(got.package.id(), id);
    assert_eq!(got.link, None);
    assert_eq!(carol.host.as_ref().unwrap().count(), count);
}
#[test]
fn read_link_record_uses_record_base() {
    let carol = SearchHome::new("Carol");
    let bytes = named_fixture("ratings", "Ratings");
    let id = PackageId::of(&bytes);
    carol.dist.publish(bytes).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let restricted = open(dir.path());
    restricted.follow(&carol.link(None)).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let other = TestHost::serve(empty.path());
    let link = Link::new(
        carol.dist.identity().unwrap(),
        &other.url,
        Some(&"ratings".parse().unwrap()),
    )
    .unwrap();
    let got = restricted.read_link(&link).unwrap();
    assert_eq!(got.package.id(), id);
    assert_eq!(got.link, Some(carol.link(Some("ratings"))));
    assert_eq!(other.count(), 0);
}
/// S35 acceptance: the second fetch_for of the same release (a cache hit) must keep the
/// trusted base of the consumer's listing, not the dead base of an index's listed record.
#[test]
fn regress_s35_cache_hit_keeps_consumer_base() {
    let carol = SearchHome::new("Carol");
    let first = SearchHome::new("First");
    let second = SearchHome::new("Second");
    let (dave, zed) = if first.dist.identity().unwrap() > second.dist.identity().unwrap() {
        (first, second)
    } else {
        (second, first)
    };
    carol
        .dist
        .publish(named_fixture("ratings", "Ratings"))
        .unwrap();
    zed.dist.follow(&carol.link(None)).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let dead = TestHost::serve(empty.path());
    zed.dist
        .list(
            &Link::new(
                carol.dist.identity().unwrap(),
                &dead.url,
                Some(&"ratings".parse().unwrap()),
            )
            .unwrap(),
            "dead",
        )
        .unwrap();
    dave.dist
        .list(&carol.link(Some("ratings")), "real")
        .unwrap();
    let eve = SearchHome::new("Eve");
    eve.dist.follow(&dave.link(None)).unwrap();
    eve.dist.choose_index(&zed.link(None)).unwrap();
    let reference = reference(&carol, "ratings");
    let once = eve
        .dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    assert_eq!(once.link, Some(carol.link(Some("ratings"))));
    let twice = eve
        .dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    assert_eq!(twice.link, Some(carol.link(Some("ratings"))));
}
/// 5.11 step 3 (DD-32): the first base that serves ends the reads.
#[test]
fn ac3_provider_stops_at_first_serving_base() {
    let carol = SearchHome::new("Carol");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    carol
        .dist
        .publish(named_fixture("ratings", "Ratings"))
        .unwrap();
    let second = TestHost::serve(&carol.dist.site_dir());
    let slug = "ratings".parse().unwrap();
    eve.dist.list(&carol.link(Some("ratings")), "own").unwrap();
    dave.dist
        .list(
            &Link::new(carol.dist.identity().unwrap(), &second.url, Some(&slug)).unwrap(),
            "followed",
        )
        .unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    // The follow reads the listed channel; only the fetch must not touch the second base.
    let before = second.count();
    let got = eve
        .dist
        .fetch_for(&reference(&carol, "ratings"), None)
        .unwrap();
    assert_eq!(got.link, Some(carol.link(Some("ratings"))));
    assert_eq!(second.count(), before);
}
/// 5.7 step 8 as amended in S35: a cache hit with no record links the first base in trust order.
#[test]
fn ac4_cache_hit_no_record_links_first_base() {
    let (carol, dave, eve, _id) = setup();
    let reference = reference(&carol, "ratings");
    eve.dist.fetch(&reference).unwrap();
    let empty = tempfile::tempdir().unwrap();
    let dead = TestHost::serve(empty.path());
    let dead_link = Link::new(
        carol.dist.identity().unwrap(),
        &dead.url,
        Some(&"ratings".parse().unwrap()),
    )
    .unwrap();
    eve.dist
        .list(&dead_link, "own, first in trust order")
        .unwrap();
    let count = carol.host.as_ref().unwrap().count();
    let got = eve
        .dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    assert_eq!(got.link, Some(dead_link));
    assert_eq!(carol.host.as_ref().unwrap().count(), count);
    assert_eq!(dead.count(), 0);
}
fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn stale_setup() -> (
    SearchHome,
    SearchHome,
    SearchHome,
    TestHost,
    tempfile::TempDir,
) {
    let carol = SearchHome::new("Carol");
    let dave = SearchHome::new("Dave");
    let eve = SearchHome::new("Eve");
    carol.dist.publish(named_fixture("one", "One")).unwrap();
    let old = tempfile::tempdir().unwrap();
    copy_dir(&carol.dist.site_dir(), old.path());
    let mirror = TestHost::serve(old.path());
    carol
        .dist
        .publish(named_fixture("ratings", "Ratings"))
        .unwrap();
    let stale = Link::new(
        carol.dist.identity().unwrap(),
        &mirror.url,
        Some(&"one".parse().unwrap()),
    )
    .unwrap();
    eve.dist.list(&stale, "stale mirror").unwrap();
    (carol, dave, eve, mirror, old)
}
#[test]
fn ac3_stale_first_base_falls_through() {
    let (carol, dave, eve, mirror, _old) = stale_setup();
    dave.dist
        .list(&carol.link(Some("ratings")), "provider")
        .unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    let reference = reference(&carol, "ratings");
    let before = mirror.count();
    let got = eve
        .dist
        .fetch_for(&reference, Some(&dave.dist.identity().unwrap()))
        .unwrap();
    assert_eq!(got.link, Some(carol.link(Some("ratings"))));
    let id = got.package.id();
    assert!(
        carol
            .host
            .as_ref()
            .unwrap()
            .requests
            .lock()
            .unwrap()
            .contains(&format!("/packages/{id}"))
    );
    // The stale mirror is first in trust order (Eve's own listing) and was read.
    assert!(mirror.count() > before);
    assert!(
        !mirror
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.starts_with("/packages/"))
    );
}
#[test]
fn ac3_all_bases_stale_is_stale_bases() {
    let (carol, dave, eve, mirror, old) = stale_setup();
    // A second stale mirror of the same old head, named by Dave's listing.
    let second = TestHost::serve(old.path());
    let link = Link::new(
        carol.dist.identity().unwrap(),
        &second.url,
        Some(&"one".parse().unwrap()),
    )
    .unwrap();
    dave.dist.list(&link, "second stale mirror").unwrap();
    eve.dist.follow(&dave.link(None)).unwrap();
    let before = [mirror.count(), second.count()];
    let error = eve
        .dist
        .fetch_for(
            &reference(&carol, "ratings"),
            Some(&dave.dist.identity().unwrap()),
        )
        .unwrap_err();
    assert!(
        matches!(error, DistError::StaleBases { bases: 2, .. }),
        "{error}"
    );
    let text = error.to_string();
    assert!(text.contains("older copy of the channel"), "{text}");
    assert!(text.contains("follow the publisher's own link"), "{text}");
    // DD-34: both stale bases were read, neither got a package request.
    for (host, before) in [&mirror, &second].into_iter().zip(before) {
        assert!(host.count() > before);
        let paths = host.requests.lock().unwrap();
        assert!(!paths.iter().any(|p| p.starts_with("/packages/")));
    }
}
