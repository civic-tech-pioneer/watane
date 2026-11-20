//! Regressions found by the S55 user-sim on the two-machine journey (TECHNICAL 11).

#![cfg(feature = "driver")]

use crate::support::driver::App;
use crate::support::journey::{approve, fixture, invoke};
use crate::support::publish::{Site, begin, consent, finish, publish};
use serde_json::json;

/// When Bob cannot reach Alice's web host (a VM guest blocked by the host
/// firewall, or a stopped server), the journey must fail with the cause Bob's
/// app gave, not with `assertion failed: app.wait("consent", TIMEOUT)`.
/// `vm_two_machines.sh` showed only that line when the web host vanished.
#[test]
fn regress_s55_unreachable_site_names_the_cause() {
    let work = tempfile::tempdir().unwrap();
    let alice_home = work.path().join("alice");
    let site = Site::serve(alice_home.join("dist/site"));
    let base = site.base.clone();

    let alice = App::start(&alice_home, &[]);
    invoke(&alice, "site_set_url", json!({"url":base}));
    begin(&alice, "identity_set_name", json!({"name":"Alice"}));
    consent(&alice);
    approve(&alice);
    assert_eq!(finish(&alice), json!({"ok":null}));
    let package = watane_host::watane_pkg::pack(&fixture("reading-list"), false).unwrap();
    let pending = publish(&alice, package.as_bytes().to_vec());
    consent(&alice);
    approve(&alice);
    let (status, published) = pending.join().unwrap();
    assert_eq!(status, 200, "{published}");
    let link = published["link"].as_str().unwrap().to_owned();
    // The web host goes away before Bob follows the link.
    drop(site);

    let bob = App::start(&work.path().join("bob"), &[]);
    // The same two steps as two_machines::ac1_j3_two_machines.
    begin(&bob, "install_link", json!({"link":link}));
    // A scoped thread keeps the journey's panic message without catch_unwind.
    let shown = std::thread::scope(|s| s.spawn(|| consent(&bob)).join());
    let error = finish(&bob);
    let cause = error["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!cause.is_empty(), "Bob's install should fail: {error}");
    let panic = shown.expect_err("no consent without a reachable site");
    let said = panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default();
    assert!(
        said.contains(&cause),
        "the journey failed with {said:?}, hiding Bob's cause {cause:?}"
    );
    assert!(bob.quit().success());
    assert!(alice.quit().success());
}
