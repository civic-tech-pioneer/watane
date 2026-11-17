#![allow(clippy::unwrap_used, reason = "tests may panic")]

mod support;
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
        "base": a, "followed_at": 1, "last_attempt": 1, "last_success": 1}}});
    std::fs::write(home.join("dist/follows.json"), old.to_string()).unwrap();
    let bob = open(&home);
    let reports = bob.refresh();
    assert!(matches!(reports[0].outcome, Outcome::Current { head: 1 }));
    assert!(bases(&home, key).is_empty());
    assert_eq!(bob.channel(&key).unwrap().base, Some(a));
    assert!(bob.followed().unwrap()[0].followed);
}
