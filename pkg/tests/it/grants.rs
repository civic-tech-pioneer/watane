use watane_pkg::*;

#[test]
fn ac3_grants_json_order_and_authority() {
    let texts = [
        r#"{"kind":"storage"}"#,
        r#"{"kind":"http","host":"api.openlibrary.org"}"#,
        r#"{"kind":"call","interface":"mods:reading-list/books@0.1.0","provider":{"slug":"reading-list","publisher":null}}"#,
        r#"{"kind":"subscribe","interface":"mods:reading-list/events@0.1.0","provider":{"slug":"reading-list","publisher":"yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo"}}"#,
    ];
    let grants: Vec<Grant> = texts
        .iter()
        .map(|t| serde_json::from_str(t).unwrap())
        .collect();
    for (grant, text) in grants.iter().zip(texts) {
        assert_eq!(serde_json::to_string(grant).unwrap(), text);
    }
    assert!(grants.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        grants.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "storage",
            "http api.openlibrary.org",
            "call mods:reading-list/books@0.1.0 from local/reading-list",
            "subscribe mods:reading-list/events@0.1.0 from yryonyebyryonyebyryonyebyryonyebyryonyebyryonyebyryo/reading-list"
        ]
    );
    for bad in [
        r#"{"kind":"call","interface":"wasi:reading-list/books@0.1.0","provider":{"slug":"reading-list","publisher":null}}"#,
        r#"{"kind":"subscribe","interface":"mods:tags/events@0.1.0","provider":{"slug":"reading-list","publisher":null}}"#,
    ] {
        serde_json::from_str::<Grant>(bad).unwrap_err();
    }
    let local = Provider {
        slug: "reading-list".parse().unwrap(),
        publisher: None,
    };
    let published = Provider {
        slug: "reading-list".parse().unwrap(),
        publisher: Some(PublisherKey::from_bytes([1; 32])),
    };
    let call = |version: &str, provider: Provider| Grant::Call {
        interface: format!("mods:reading-list/books@{version}")
            .parse()
            .unwrap(),
        provider,
    };
    assert!(call("0.1.0", local.clone()) < call("0.1.0", published.clone()));
    assert!(call("0.1.0", local.clone()).same_authority(&call("2.0.0", local.clone())));
    assert!(!call("0.1.0", local.clone()).same_authority(&call("0.1.0", published.clone())));
    assert!(!grants[2].same_authority(&grants[3]));
    assert!(grants[0].same_authority(&Grant::Storage));
    assert!(!grants[1].same_authority(&Grant::Http {
        host: "b.example".parse().unwrap()
    }));
    let requirement = Requirement {
        provider: local.clone(),
        version: "0.1.0".parse().unwrap(),
    };
    assert!(requirement.satisfied_by(&local, &"0.1.7".parse().unwrap()));
    assert!(!requirement.satisfied_by(&published, &"0.1.7".parse().unwrap()));
    assert!(!requirement.satisfied_by(&local, &"0.2.0".parse().unwrap()));
}
