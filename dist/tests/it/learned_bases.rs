#![allow(clippy::unwrap_used, reason = "tests may panic")]

use crate::support;
use support::*;
use watane_dist::*;

fn snapshot(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            snapshot(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}
fn record(home: &std::path::Path, key: PublicKey) -> serde_json::Value {
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join("dist/follows.json")).unwrap()).unwrap();
    file["channels"][key.to_string()].clone()
}
fn bases(home: &std::path::Path, key: PublicKey) -> Vec<String> {
    record(home, key)["bases"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|b| b.as_str().unwrap().to_owned())
        .collect()
}

/// Alice has two entries, mirrored at A (a snapshot), then a third that only B serves.
struct World {
    _a: tempfile::TempDir,
    alice: Dist,
    mirror: tempfile::TempDir,
    host_a: Option<TestHost>,
    host_b: Option<TestHost>,
    bob_home: tempfile::TempDir,
    bob: Dist,
    key: PublicKey,
}
fn world() -> World {
    let a = tempfile::tempdir().unwrap();
    let alice = open(a.path());
    alice.set_name("Alice").unwrap();
    alice.publish(fixture("1.0.0", false)).unwrap();
    let mirror = tempfile::tempdir().unwrap();
    snapshot(&alice.site_dir(), mirror.path());
    let host_a = TestHost::serve(mirror.path());
    alice.publish(fixture("1.1.0", false)).unwrap();
    let host_b = TestHost::serve(&alice.site_dir());
    let bob_home = tempfile::tempdir().unwrap();
    let bob = open(bob_home.path());
    let key = alice.identity().unwrap();
    World {
        _a: a,
        alice,
        mirror,
        host_a: Some(host_a),
        host_b: Some(host_b),
        bob_home,
        bob,
        key,
    }
}
impl World {
    fn a(&self) -> String {
        self.host_a.as_ref().unwrap().url.clone()
    }
    fn b(&self) -> String {
        self.host_b.as_ref().unwrap().url.clone()
    }
    fn link(&self, base: &str) -> Link {
        Link::new(self.key, base, None).unwrap()
    }
    fn report(&self) -> RefreshReport {
        self.bob
            .refresh()
            .into_iter()
            .find(|r| r.channel == self.key)
            .unwrap()
    }
}

#[test]
fn ac5_follow_with_a_new_base_appends_it_and_the_newer_base_leads() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    assert_eq!(w.bob.channel(&w.key).unwrap().head, Some(1));
    w.bob.follow(&w.link(&w.b())).unwrap();
    assert_eq!(w.bob.channel(&w.key).unwrap().head, Some(2));
    assert_eq!(bases(w.bob_home.path(), w.key), [w.b(), w.a()]);
    assert_eq!(record(w.bob_home.path(), w.key)["base"], w.b());
    assert_eq!(w.bob.channel(&w.key).unwrap().base, Some(w.b()));
}

#[test]
fn ac5_open_link_of_a_known_channel_learns_the_link_base() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    let slug = w.alice.channel(&w.key).unwrap().releases[0]
        .release
        .slug
        .clone();
    let link = Link::new(w.key, &w.b(), Some(&slug)).unwrap();
    w.bob.open_link(&link).unwrap();
    assert_eq!(bases(w.bob_home.path(), w.key), [w.b(), w.a()]);
    assert_eq!(w.bob.channel(&w.key).unwrap().head, Some(2));
}

fn ac6(drop_a: bool) {
    let mut w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    w.bob.follow(&w.link(&w.b())).unwrap();
    // Mirror A catches up so that either base alone is current.
    snapshot(&w.alice.site_dir(), w.mirror.path());
    if drop_a {
        w.host_a = None;
    } else {
        w.host_b = None;
    }
    assert!(
        matches!(w.report().outcome, Outcome::Current { head: 2 }),
        "{:?}",
        w.report().outcome
    );
    assert_eq!(bases(w.bob_home.path(), w.key).len(), 2);
}
#[test]
fn ac6_refresh_reads_every_base_a_down() {
    ac6(true);
}
#[test]
fn ac6_refresh_reads_every_base_b_down() {
    ac6(false);
}

#[test]
fn ac7_a_known_base_changes_nothing_and_five_bases_keep_four() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    w.bob.follow(&w.link(&w.a())).unwrap();
    assert!(bases(w.bob_home.path(), w.key).is_empty());
    w.bob.follow(&w.link(&w.b())).unwrap();
    let before = bases(w.bob_home.path(), w.key);
    w.bob.follow(&w.link(&w.a())).unwrap();
    w.bob.follow(&w.link(&w.b())).unwrap();
    assert_eq!(bases(w.bob_home.path(), w.key), before);

    let hosts: Vec<_> = (0..3)
        .map(|_| TestHost::serve(&w.alice.site_dir()))
        .collect();
    for h in &hosts {
        w.bob.follow(&w.link(&h.url)).unwrap();
    }
    let all = bases(w.bob_home.path(), w.key);
    assert_eq!(all.len(), 4);
    assert_eq!(all[0], w.b());
    // The oldest learned base after the front one (A) went.
    assert!(!all.contains(&w.a()));
    for h in &hosts {
        assert!(all.contains(&h.url));
    }
}

#[test]
fn ac8_choose_index_learns_a_new_base_of_a_chosen_or_followed_channel() {
    let w = world();
    w.bob.choose_index(&w.link(&w.a())).unwrap();
    w.bob.choose_index(&w.link(&w.b())).unwrap();
    assert_eq!(bases(w.bob_home.path(), w.key), [w.b(), w.a()]);
    assert_eq!(w.bob.channel(&w.key).unwrap().head, Some(2));

    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    w.bob.choose_index(&w.link(&w.b())).unwrap();
    let r = record(w.bob_home.path(), w.key);
    assert_eq!(bases(w.bob_home.path(), w.key), [w.b(), w.a()]);
    assert!(r["followed"] == true && r["index"] == true);
}

#[test]
fn ac9_a_step_3_record_reads_from_base_only() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    let home = w.bob_home.path().to_owned();
    let (key, a) = (w.key, w.a());
    drop(w.bob);
    let old = serde_json::json!({"version": 1, "channels": {key.to_string(): {
        "base": a, "given": [a], "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    let reports = bob.refresh();
    assert!(matches!(reports[0].outcome, Outcome::Current { head: 1 }));
    assert!(bases(&home, key).is_empty());
    assert_eq!(bob.channel(&key).unwrap().base, Some(a));
    assert!(bob.followed().unwrap()[0].followed);
}

fn given(home: &std::path::Path, key: PublicKey) -> Vec<String> {
    record(home, key)["given"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|b| b.as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn ac1_follow_choose_and_a_learned_link_mark_the_base_given() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    assert_eq!(given(w.bob_home.path(), w.key), [w.a()]);
    w.bob.choose_index(&w.link(&w.b())).unwrap();
    assert_eq!(given(w.bob_home.path(), w.key), [w.a(), w.b()]);

    let w = world();
    w.bob.choose_index(&w.link(&w.a())).unwrap();
    assert_eq!(given(w.bob_home.path(), w.key), [w.a()]);

    // open_link of a link at a base the record holds but was never given gives nothing.
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    let home = w.bob_home.path().to_owned();
    let (key, a, b) = (w.key, w.a(), w.b());
    drop(w.bob);
    let old = serde_json::json!({"version": 1, "channels": {key.to_string(): {
        "base": a, "bases": [a, b], "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    assert!(given(&home, key).is_empty());
    let link = Link::new(key, &b, Some(&"reading-list".parse().unwrap())).unwrap();
    let _ = bob.open_link(&link);
    assert!(given(&home, key).is_empty(), "a held base stays unknown");

    // open_link at a base the record does not hold still learns and gives it.
    drop(bob);
    let dead = "http://127.0.0.1:9/";
    let old = serde_json::json!({"version": 1, "channels": {key.to_string(): {
        "base": a, "bases": [a, dead], "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    let _ = bob.open_link(&link);
    assert_eq!(given(&home, key), [b]);
}

/// AC6 (DD-48): a file without `seen` reads unchanged and its held bases become seen, so
/// a link at one of them gives nothing; the top level gets no `seen` without a tombstone.
#[test]
fn ac6_a_file_without_seen_marks_its_held_bases_seen() {
    let w = world();
    w.bob.follow(&w.link(&w.a())).unwrap();
    let home = w.bob_home.path().to_owned();
    let (key, a, b) = (w.key, w.a(), w.b());
    drop(w.bob);
    let old = serde_json::json!({"version": 1, "channels": {key.to_string(): {
        "base": a, "bases": [a, b], "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    assert_eq!(bob.channel(&key).unwrap().base, Some(a.clone()));
    bob.refresh();
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join("dist/follows.json")).unwrap()).unwrap();
    assert_eq!(file["version"], 1);
    assert!(
        file.get("seen").is_none(),
        "no tombstone, no top-level seen"
    );
    let r = &file["channels"][key.to_string()];
    assert_eq!(r["seen"], serde_json::json!([a, b]));
    assert!(r.get("given").is_none(), "an empty given is not written");
    assert_eq!(bases(&home, key), [a, b]);
}

/// A fresh home whose `follows.json` holds only `tombs` (top-level `seen`), then a follow of `base`.
fn follow_after_tombstone(w: World, tombs: serde_json::Value, base: &str) -> serde_json::Value {
    let home = w.bob_home.path().to_owned();
    let key = w.key;
    let link = w.link(base);
    drop(w.bob);
    let file = serde_json::json!({"version": 1, "channels": {}, "seen": {key.to_string(): tombs}});
    std::fs::create_dir_all(home.join("dist")).unwrap();
    std::fs::write(home.join("dist/follows.json"), file.to_string()).unwrap();
    // The test hosts are local: `listed_local` lets the listed first read reach them (F2.1).
    let bob = Dist::open(
        &home,
        std::sync::Arc::new(RecordingConfirm::new(std::iter::repeat_n(
            Decision::Approve,
            20,
        ))),
        Options {
            listed_local: true,
            ..fast_options()
        },
    )
    .unwrap();
    let _ = bob.follow(&link);
    record(&home, key)
}

/// AC4 (DD-48): a tombstone's bases are normalised on load as a record's are, so one
/// spelled without the trailing slash still blocks a link at `a`.
#[test]
fn ac4_a_tombstone_base_is_normalised_on_load() {
    let w = world();
    let (a, spelled) = (w.a(), w.a().trim_end_matches('/').to_owned());
    assert_ne!(spelled, a);
    let r = follow_after_tombstone(
        w,
        serde_json::json!({"seen": [spelled], "given": [], "at": 1}),
        &a,
    );
    assert!(
        r.get("given").is_none(),
        "a link at a tombstoned base gave it: {r}"
    );
    assert_eq!(r["seen"], serde_json::json!([a]));
}

/// AC3 (DD-48 r5): a tombstone carries `seen_full` and a new record takes it back, so a
/// link at an unseen base gives nothing and the record writes `seen_full`.
#[test]
fn ac3_a_new_record_takes_seen_full_back_from_its_tombstone() {
    let w = world();
    let a = w.a();
    let seen: Vec<String> = (0..16).map(|n| format!("http://h{n}.invalid/")).collect();
    let r = follow_after_tombstone(
        w,
        serde_json::json!({"seen": seen, "given": [], "seen_full": true, "at": 1}),
        &a,
    );
    assert!(
        r.get("given").is_none(),
        "an unseen base of a full seen was given: {r}"
    );
    assert_eq!(r["seen_full"], true);
    assert_eq!(r["seen"], serde_json::json!(seen));
}

/// AC3 (DD-48 r5): `prune_listed` writes a removed record's `seen_full` into its tombstone.
#[test]
fn ac3_a_pruned_record_leaves_seen_full_in_its_tombstone() {
    let w = world();
    let home = w.bob_home.path().to_owned();
    let (key, a) = (w.key, w.a());
    drop(w.bob);
    let old = serde_json::json!({"version": 1, "channels": {key.to_string(): {
        "base": a, "followed": true, "seen": [a], "seen_full": true,
        "followed_at": 1, "last_attempt": 1}}});
    std::fs::create_dir_all(home.join("dist")).unwrap();
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    bob.unfollow(&key).unwrap();
    let file: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join("dist/follows.json")).unwrap()).unwrap();
    assert!(file["channels"].get(key.to_string()).is_none(), "pruned");
    assert_eq!(file["seen"][key.to_string()]["seen_full"], true, "{file}");
}
