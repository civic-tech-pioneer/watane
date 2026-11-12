use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
struct Answer(bool);
impl watane_dist::Confirm for Answer {
    fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
        if self.0 {
            watane_dist::Decision::Approve
        } else {
            watane_dist::Decision::Refuse
        }
    }
}
fn package() -> Vec<u8> {
    watane_host::watane_pkg::pack(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list"),
        false,
    )
    .unwrap()
    .as_bytes()
    .to_vec()
}
async fn post(router: Router, content_type: &str, bytes: Vec<u8>) -> (u16, Value) {
    let response = router
        .oneshot(
            Request::post("/api/publish")
                .header("content-type", content_type)
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    (
        response.status().as_u16(),
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap(),
    )
}
#[tokio::test]
async fn ac6_publish_http_rows_without_window() {
    let home = tempfile::tempdir().unwrap();
    let dist = Arc::new(
        watane_dist::Dist::open(home.path(), Arc::new(Answer(true)), Default::default()).unwrap(),
    );
    dist.set_site_url("http://127.0.0.1:1/").unwrap();
    let router = watane_app::api::router(Some(dist.clone()));
    for (content, bytes, status, code) in [
        ("text/plain", vec![1], 400, "bad_request"),
        ("application/octet-stream", vec![], 400, "bad_request"),
        (
            "application/octet-stream",
            vec![0; 67_108_865],
            413,
            "too_large",
        ),
        (
            "application/octet-stream",
            vec![1, 2, 3],
            422,
            "invalid_package",
        ),
    ] {
        let (actual, body) = post(router.clone(), content, bytes).await;
        assert_eq!(
            (actual, body["error"]["code"].as_str()),
            (status, Some(code)),
            "{body}"
        );
    }
    let bytes = package();
    assert!(bytes.len() > 65_536 && bytes.len() < 67_108_864);
    let (status, body) = post(router.clone(), "application/octet-stream", bytes.clone()).await;
    assert_eq!(status, 200, "{body}");
    assert!(
        body["release"]
            .as_str()
            .unwrap()
            .ends_with("/reading-list@0.1.0")
    );
    assert_eq!(body["site"], json!(dist.site_dir()));
    assert!(body["link"].as_str().unwrap().contains("#reading-list"));
    assert!(dist.site_dir().join("head").is_file());
    let (status, body) = post(router, "application/octet-stream", bytes.clone()).await;
    assert_eq!(status, 409);
    assert_eq!(body["error"]["code"], "publish_refused");
    let denied_home = tempfile::tempdir().unwrap();
    let denied = Arc::new(
        watane_dist::Dist::open(
            denied_home.path(),
            Arc::new(Answer(false)),
            Default::default(),
        )
        .unwrap(),
    );
    let before = tree(&denied.site_dir());
    let (status, body) = post(
        watane_app::api::router(Some(denied.clone())),
        "application/octet-stream",
        bytes.clone(),
    )
    .await;
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "refused");
    assert_eq!(tree(&denied.site_dir()), before);
    let (status, body) = post(
        watane_app::api::router(None),
        "application/octet-stream",
        bytes.clone(),
    )
    .await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "unavailable");
    let broken_home = tempfile::tempdir().unwrap();
    let broken = Arc::new(
        watane_dist::Dist::open(
            broken_home.path(),
            Arc::new(Answer(true)),
            Default::default(),
        )
        .unwrap(),
    );
    std::fs::write(broken.site_dir().join("head"), b"conflict").unwrap();
    let (status, body) = post(
        watane_app::api::router(Some(broken)),
        "application/octet-stream",
        bytes,
    )
    .await;
    assert_eq!(status, 500, "{body}");
    assert_eq!(body["error"]["code"], "site_not_written");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("stored")
    );
}
fn tree(path: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &std::path::Path,
        p: &std::path::Path,
        out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for e in std::fs::read_dir(p).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out)
            } else {
                out.insert(
                    p.strip_prefix(root).unwrap().into(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = Default::default();
    walk(path, path, &mut out);
    out
}
#[cfg(feature = "driver")]
mod support;
#[cfg(feature = "driver")]
#[test]
fn ac6_publish_through_consent_window_and_host_guards() {
    use support::{driver::App, journey::*, publish::*};
    let home = tempfile::tempdir().unwrap();
    let app = App::start(home.path(), &[]);
    invoke(&app, "site_set_url", json!({"url":"http://127.0.0.1:1/"}));
    let bytes = package();
    let before = tree(&home.path().join("dist/site"));
    let pending = publish(&app, bytes.clone());
    consent(&app);
    app.eval("consent", "document.querySelector('#deny').click(); null");
    let (status, body) = pending.join().unwrap();
    assert_eq!(status, 403);
    assert_eq!(body["error"]["code"], "refused");
    assert_eq!(tree(&home.path().join("dist/site")), before);
    let pending = publish(&app, bytes.clone());
    consent(&app);
    approve(&app);
    let (status, body) = pending.join().unwrap();
    assert_eq!(status, 200, "{body}");
    assert!(body["release"].is_string());
    assert!(body["site"].is_string());
    assert!(body["link"].is_string());
    assert_ne!(tree(&home.path().join("dist/site")), before);
    for (content, body, status, code) in [
        ("text/plain", vec![1], 400, "bad_request"),
        ("application/octet-stream", vec![], 400, "bad_request"),
        (
            "application/octet-stream",
            vec![0; 67_108_865],
            413,
            "too_large",
        ),
        ("application/octet-stream", bytes, 409, "publish_refused"),
        ("application/octet-stream", vec![1], 422, "invalid_package"),
    ] {
        let (actual, body) = http(app.port, content, body);
        assert_eq!(actual, status, "{body}");
        assert_eq!(body["error"]["code"], code);
    }
    assert!(app.quit().success());
}
