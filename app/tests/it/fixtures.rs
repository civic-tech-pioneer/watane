//! Validates the committed components, without compiling mods or starting a GUI.
use serde_json::json;
use std::pin::Pin;
use watane_host::{
    Answer, Confirm, Host, InstallRequest, Limits, LocalName, Mode, Requester, Source,
};
struct Approve;
impl Confirm for Approve {
    fn install(&self, _: InstallRequest) -> Pin<Box<dyn Future<Output = Answer> + Send>> {
        Box::pin(async { Answer::Approve { withhold: vec![] } })
    }
}
#[tokio::test]
async fn ac5_committed_fixtures_call_subscribe_and_keep_state() {
    let home = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let open = || {
        Host::open(
            home.path(),
            Mode::Home {
                app: Some(std::sync::Arc::new(Approve)),
            },
            Limits::default(),
        )
        .unwrap()
    };
    let host = open();
    let plan = host
        .plan(vec![
            Source::Folder(root.join("recommendations")),
            Source::Folder(root.join("reading-list")),
        ])
        .unwrap();
    assert_eq!(
        plan.entries()
            .iter()
            .map(|e| e.local.to_string())
            .collect::<Vec<_>>(),
        ["reading-list", "recommendations"]
    );
    host.install(plan, Requester::App).await.unwrap();
    let reading = LocalName::parse("reading-list").unwrap();
    let recs = LocalName::parse("recommendations").unwrap();
    host.call_async(
        watane_host::Caller::Page,
        reading.clone(),
        "books.add".into(),
        json!({"title":"Dune"}),
    )
    .await
    .unwrap();
    assert_eq!(
        host.call_async(
            watane_host::Caller::Page,
            recs.clone(),
            "api.list".into(),
            json!({})
        )
        .await
        .unwrap(),
        json!(["Dune"])
    );
    // Event delivery is asynchronous; wait on state, with a hang guard.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if host
            .call_async(
                watane_host::Caller::Page,
                recs.clone(),
                "api.received".into(),
                json!({}),
            )
            .await
            .unwrap()
            == json!(["Dune"])
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    let grant = host
        .get(&recs)
        .unwrap()
        .grants
        .into_iter()
        .find(|g| matches!(g.grant, watane_host::watane_pkg::Grant::Call { .. }))
        .unwrap()
        .grant;
    host.revoke(&recs, &grant).unwrap();
    let error = host
        .call_async(
            watane_host::Caller::Page,
            recs.clone(),
            "api.list".into(),
            json!({}),
        )
        .await
        .unwrap_err();
    assert_eq!(serde_json::to_value(error).unwrap()["code"], "not_granted");
    host.grant(&recs, &grant).unwrap();
    assert_eq!(
        host.call_async(
            watane_host::Caller::Page,
            recs.clone(),
            "api.list".into(),
            json!({})
        )
        .await
        .unwrap(),
        json!(["Dune"])
    );
    let mut host = host;
    for _ in 0..20 {
        drop(host);
        host = open();
        assert_eq!(
            host.call_async(
                watane_host::Caller::Page,
                reading.clone(),
                "books.list".into(),
                json!({})
            )
            .await
            .unwrap(),
            json!(["Dune"])
        );
    }
    assert_eq!(
        host.call_async(
            watane_host::Caller::Page,
            recs.clone(),
            "api.received".into(),
            json!({})
        )
        .await
        .unwrap(),
        json!(["Dune"])
    );
    host.remove(&recs, true).unwrap();
    host.install(
        host.plan(vec![Source::Folder(root.join("recommendations"))])
            .unwrap(),
        Requester::App,
    )
    .await
    .unwrap();
    assert_eq!(
        host.call_async(
            watane_host::Caller::Page,
            recs,
            "api.received".into(),
            json!({})
        )
        .await
        .unwrap(),
        json!([])
    );
}
