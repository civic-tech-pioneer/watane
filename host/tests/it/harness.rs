use serde_json::json;
use std::path::{Path, PathBuf};
use watane_host::{EventRecord, Fakes, Harness, Outcome};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/mods")
        .join(name)
}

#[test]
fn ac1_counter_returns_values_and_exact_events_for_each_call() {
    let harness = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    assert_eq!(
        harness.call("api.get", json!({})).unwrap(),
        Outcome {
            value: json!(0),
            events: vec![],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.add", json!({})).unwrap(),
        Outcome {
            value: json!(1),
            events: vec![EventRecord {
                interface: "mods:counter/changes@0.1.0".parse().unwrap(),
                function: "counted".into(),
                args: json!({"count":1}),
            }],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.add", json!({})).unwrap(),
        Outcome {
            value: json!(2),
            events: vec![EventRecord {
                interface: "mods:counter/changes@0.1.0".parse().unwrap(),
                function: "counted".into(),
                args: json!({"count":2}),
            }],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.who", json!({})).unwrap(),
        Outcome {
            value: json!("page"),
            events: vec![],
            manages: vec![],
            requests: vec![],
        }
    );
}

#[test]
fn ac2_fresh_and_simultaneous_harnesses_have_independent_state() {
    let first = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    assert_eq!(first.call("api.add", json!({})).unwrap().value, json!(1));
    assert_eq!(first.call("api.add", json!({})).unwrap().value, json!(2));
    let second = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    assert_eq!(second.call("api.get", json!({})).unwrap().value, json!(0));
    assert_eq!(second.call("api.add", json!({})).unwrap().value, json!(1));
    assert_eq!(first.call("api.get", json!({})).unwrap().value, json!(2));
    drop(first);
    let third = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    assert_eq!(third.call("api.get", json!({})).unwrap().value, json!(0));
    assert_eq!(second.call("api.get", json!({})).unwrap().value, json!(1));
}

#[test]
fn ac3_failed_call_rolls_back_state_and_events_and_unknown_function_fails() {
    let harness = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    assert_eq!(harness.call("api.add", json!({})).unwrap().value, json!(1));
    let error = harness.call("api.add-then-fail", json!({})).unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap()["code"], "trap");
    assert_eq!(
        harness.call("api.get", json!({})).unwrap(),
        Outcome {
            value: json!(1),
            events: vec![],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.add", json!({})).unwrap(),
        Outcome {
            value: json!(2),
            events: vec![EventRecord {
                interface: "mods:counter/changes@0.1.0".parse().unwrap(),
                function: "counted".into(),
                args: json!({"count":2}),
            }],
            manages: vec![],
            requests: vec![],
        }
    );
    let error = harness.call("api.unknown", json!({})).unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap()["code"], "no_function");
}

#[test]
fn ac4_echo_accepts_named_and_positional_arguments() {
    let harness = Harness::open(&fixture("echo"), Fakes::new()).unwrap();
    for args in [json!({"x":"hello"}), json!(["hello"])] {
        assert_eq!(
            harness.call("api.id-string", args).unwrap(),
            Outcome {
                value: json!("hello"),
                events: vec![],
                manages: vec![],
                requests: vec![],
            }
        );
    }
    // Two parameters also prove positional order.
    harness.call("api.put", json!(["key", [65, 66]])).unwrap();
    assert_eq!(
        harness
            .call("api.read", json!({"key":"key"}))
            .unwrap()
            .value,
        json!([65, 66])
    );
}

#[test]
fn ac5_drop_removes_the_temporary_home_after_use_on_another_thread() {
    let harness = Harness::open(&fixture("counter"), Fakes::new()).unwrap();
    let home = harness.home().to_path_buf();
    assert!(home.is_dir());
    assert!(!home.join("run.json").exists());
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            assert_eq!(harness.call("api.add", json!({})).unwrap().value, json!(1));
            assert!(home.join("state/state.redb").is_file());
            assert!(!home.join("run.json").exists());
            drop(harness);
            assert!(!home.exists());
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn ac6_invalid_mod_returns_a_host_error_naming_the_problem() {
    let folder = tempfile::tempdir().unwrap();
    let error = Harness::open(folder.path(), Fakes::new()).unwrap_err();
    assert_eq!(error.code(), "invalid_mod");
    let message = error.to_string();
    assert!(message.contains("mod.toml"), "{message}");
    assert!(message.contains("wit/world.wit"), "{message}");
    assert!(
        message.contains(&folder.path().display().to_string()),
        "{message}"
    );
}

#[test]
fn ac9_s11_fake_import_returns_typed_value_and_records_emission() {
    let fakes = serde_json::from_value(json!({"mods:shelf/items":{"all":["Dune"]}})).unwrap();
    let harness = Harness::open(&fixture("recs"), fakes).unwrap();
    assert_eq!(
        harness.call("api.suggest", json!(null)).unwrap(),
        Outcome {
            value: json!("Dune"),
            events: vec![EventRecord {
                interface: "mods:recs/events@0.1.0".parse().unwrap(),
                function: "suggested".into(),
                args: json!({"title":"Dune"})
            }],
            manages: vec![],
            requests: vec![],
        }
    );
    let harness = Harness::open(&fixture("recs"), Fakes::new()).unwrap();
    let error = harness.call("api.suggest", json!(null)).unwrap_err();
    assert_eq!(error.code, watane_host::CallCode::Trap);
    assert!(
        error.message.contains("no fake for mods:shelf/items.all"),
        "{error}"
    );
    let fakes = serde_json::from_value(json!({"mods:shelf/items":{"all":42}})).unwrap();
    let harness = Harness::open(&fixture("recs"), fakes).unwrap();
    let error = harness.call("api.suggest", json!(null)).unwrap_err();
    assert_eq!(error.code, watane_host::CallCode::Trap);
    assert!(error.message.contains("mods:shelf/items.all"), "{error}");
}

#[test]
fn ac10_s11_deliver_accepts_only_subscription_functions() {
    let harness = Harness::open(&fixture("recs"), Fakes::new()).unwrap();
    assert_eq!(
        harness
            .deliver("events.added", json!({"title":"Emma"}))
            .unwrap(),
        Outcome {
            value: json!(null),
            events: vec![],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.received", json!(null)).unwrap().value,
        json!(["Emma"])
    );
    for name in ["api.ping", "events.nope"] {
        assert_eq!(
            harness.deliver(name, json!(null)).unwrap_err().code,
            watane_host::CallCode::NoFunction
        );
    }
    assert_eq!(
        harness
            .call("events.added", json!({"title":"lost"}))
            .unwrap_err()
            .code,
        watane_host::CallCode::NoFunction
    );
    assert_eq!(
        harness
            .deliver("events.added", json!({"title":42}))
            .unwrap_err()
            .code,
        watane_host::CallCode::BadRequest
    );
    assert_eq!(
        harness.call("api.received", json!(null)).unwrap().value,
        json!(["Emma"])
    );
}

#[test]
fn ac10_s11_delivery_records_its_outbox_and_commits_state() {
    let harness = Harness::open(&fixture("loop-a"), Fakes::new()).unwrap();
    assert_eq!(
        harness.deliver("events.ping", json!({"n":1})).unwrap(),
        Outcome {
            value: json!(null),
            events: vec![EventRecord {
                interface: "mods:loop-a/events@0.1.0".parse().unwrap(),
                function: "ping".into(),
                args: json!({"n":2})
            }],
            manages: vec![],
            requests: vec![],
        }
    );
    assert_eq!(
        harness.call("api.count", json!(null)).unwrap().value,
        json!(1)
    );
}
