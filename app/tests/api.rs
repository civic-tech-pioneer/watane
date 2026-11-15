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
        if status == 422 {
            let message = body["error"]["message"].as_str().unwrap();
            assert!(!message.contains(['{', '(', '"']), "{message}");
            assert_eq!(message, "bad package format: short header");
            assert!(
                !message.contains("Format") && !message.contains("Package"),
                "{message}"
            );
        }
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
    assert_eq!(body["error"]["code"], "version_exists");
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
    deny(&app);
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
        ("application/octet-stream", bytes, 409, "version_exists"),
        ("application/octet-stream", vec![1], 422, "invalid_package"),
    ] {
        let (actual, body) = http(app.port, content, body);
        assert_eq!(actual, status, "{body}");
        assert_eq!(body["error"]["code"], code);
    }
    assert!(app.quit().success());
}

#[tokio::test]
async fn ac2_search_http_rows_and_own_release_shape() {
    async fn search(router: Router, bytes: Vec<u8>) -> (u16, Value) {
        let response = router
            .oneshot(
                Request::post("/api/search")
                    .header("content-type", "application/json")
                    .body(Body::from(bytes))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status().as_u16();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, body)
    }
    let home = tempfile::tempdir().unwrap();
    let dist = Arc::new(
        watane_dist::Dist::open(home.path(), Arc::new(Answer(true)), Default::default()).unwrap(),
    );
    dist.set_name("Alice").unwrap();
    let published = dist.publish(package()).unwrap();
    let router = watane_app::api::router(Some(dist.clone()));
    for query in [
        json!({"text":"reading"}),
        json!({"text":"", "publisher":dist.identity().unwrap().to_string(), "interface":"mods:reading-list/books"}),
    ] {
        let (status, body) = search(router.clone(), serde_json::to_vec(&query).unwrap()).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["indexes"], 0);
        let result = &body["results"][0];
        assert_eq!(result["release"], published.info.reference.to_string());
        assert_eq!(
            result["publisher"],
            json!({"key":dist.identity().unwrap().to_string(),"name":"Alice","fingerprint":dist.fingerprint().unwrap()})
        );
        assert_eq!(result["link"], Value::Null);
        assert_eq!(result["via"], json!([{"kind":"own"}]));
        assert_eq!(result["status"], "ok");
        assert_eq!(result["checked_at"], Value::Null);
        assert_eq!(result["unchecked_since"], Value::Null);
        for key in ["name", "summary", "offers", "events", "uses"] {
            assert!(result.get(key).is_some(), "{key}: {result}");
        }
        for key in ["slug", "version", "verified", "installed", "checkedAt"] {
            assert!(result.get(key).is_none());
        }
    }
    for bytes in [
        b"{".to_vec(),
        br#"{"text":"", "publisher":"bad"}"#.to_vec(),
        br#"{"text":"", "interface":"bad"}"#.to_vec(),
        serde_json::to_vec(&json!({"text":"x".repeat(257)})).unwrap(),
    ] {
        let (status, body) = search(router.clone(), bytes).await;
        assert_eq!(status, 400, "{body}");
        assert_eq!(body["error"]["code"], "bad_request");
    }
    let (status, body) = search(watane_app::api::router(None), br#"{"text":""}"#.to_vec()).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "unavailable");
}

#[path = "support/api.rs"]
mod api_support;
use api_support::{Recording, Site, open};
async fn api_request(
    dist: Option<Arc<watane_dist::Dist>>,
    path: &str,
    bytes: Vec<u8>,
    links: Option<&str>,
) -> (u16, axum::http::HeaderMap, Vec<u8>) {
    let mut request = Request::post(path).header(
        "content-type",
        if path == "/api/fetch" {
            "application/json"
        } else {
            "application/octet-stream"
        },
    );
    if let Some(links) = links {
        request = request.header("Watane-Provider-Links", links);
    }
    let response = watane_app::api::router(dist)
        .oneshot(request.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    (
        response.status().as_u16(),
        response.headers().clone(),
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}
async fn fetch(
    dist: Option<Arc<watane_dist::Dist>>,
    query: Value,
) -> (u16, axum::http::HeaderMap, Vec<u8>) {
    api_request(
        dist,
        "/api/fetch",
        serde_json::to_vec(&query).unwrap(),
        None,
    )
    .await
}
#[tokio::test(flavor = "multi_thread")]
async fn ac1_fetch_link_verified_without_follow_or_consent() {
    let carol_home = tempfile::tempdir().unwrap();
    let carol = open(carol_home.path(), Recording::new(true), true);
    let site = Site::serve(carol.site_dir()).await;
    carol.set_site_url(&site.base).unwrap();
    let published = carol.publish(package()).unwrap();
    let home = tempfile::tempdir().unwrap();
    let confirm = Recording::new(true);
    let dist = open(home.path(), confirm.clone(), true);
    let follows = home.path().join("dist/follows.json");
    let before = std::fs::read(&follows).ok();
    let (status, headers, bytes) = fetch(
        Some(dist.clone()),
        json!({"link":published.link.unwrap().to_string()}),
    )
    .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    assert_eq!(headers["content-type"], "application/octet-stream");
    assert_eq!(
        headers["Watane-Release"],
        published.info.reference.to_string()
    );
    assert_eq!(
        headers["Watane-Package"],
        watane_dist::PackageId::of(&bytes).to_string()
    );
    assert_eq!(bytes, package());
    assert_eq!(std::fs::read(follows).ok(), before);
    assert!(dist.followed().unwrap().is_empty());
    assert!(dist.own().unwrap().releases.is_empty());
    assert!(confirm.requests.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn ac2_fetch_release_followed_and_unknown() {
    let carol_home = tempfile::tempdir().unwrap();
    let carol = open(carol_home.path(), Recording::new(true), true);
    let site = Site::serve(carol.site_dir()).await;
    carol.set_site_url(&site.base).unwrap();
    let published = carol.publish(package()).unwrap();
    let home = tempfile::tempdir().unwrap();
    let confirm = Recording::new(true);
    let dist = open(home.path(), confirm.clone(), true);
    let query = json!({"release":published.info.reference.to_string()});
    let (status, _, bytes) = fetch(Some(dist.clone()), query.clone()).await;
    assert_eq!(status, 502);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
        "fetch_failed"
    );
    dist.follow(published.link.as_ref().unwrap()).unwrap();
    let before = std::fs::read(home.path().join("dist/follows.json")).unwrap();
    let (status, headers, bytes) = fetch(Some(dist.clone()), query).await;
    assert_eq!(status, 200);
    assert_eq!(
        headers["Watane-Release"],
        published.info.reference.to_string()
    );
    assert_eq!(
        headers["Watane-Package"],
        watane_dist::PackageId::of(&bytes).to_string()
    );
    assert_eq!(headers["content-type"], "application/octet-stream");
    assert_eq!(bytes, package());
    assert_eq!(
        std::fs::read(home.path().join("dist/follows.json")).unwrap(),
        before
    );
    assert!(confirm.requests.lock().unwrap().is_empty());
}
#[tokio::test(flavor = "multi_thread")]
async fn ac3_fetch_errors_loopback_and_host_origin_guard() {
    let carol_home = tempfile::tempdir().unwrap();
    let carol = open(carol_home.path(), Recording::new(true), true);
    let site = Site::serve(carol.site_dir()).await;
    carol.set_site_url(&site.base).unwrap();
    let published = carol.publish(package()).unwrap();
    let link = published.link.unwrap();
    let home = tempfile::tempdir().unwrap();
    let dist = open(home.path(), Recording::new(true), false);
    for (body, expected) in [
        ("{", "EOF while parsing an object at line 1 column 1"),
        ("[]", "data did not match any variant"),
        ("{}", "give link or release"),
        (r#"{"link":4,"release":[]}"#, "give link or release"),
        (r#"{"link":"a","release":"b"}"#, "give link or release"),
    ] {
        let (status, _, bytes) = api_request(
            Some(dist.clone()),
            "/api/fetch",
            body.as_bytes().to_vec(),
            None,
        )
        .await;
        assert_eq!(status, 400);
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let message = value["error"]["message"].as_str().unwrap();
        if body == "{" || body == "[]" {
            assert!(message.starts_with("give link or release: "), "{message}");
            assert!(message.contains(expected), "{message}");
        } else {
            assert_eq!(message, expected);
        }
    }
    for bytes in [
        b"{".to_vec(),
        serde_json::to_vec(
            &json!({"link":link.to_string(),"release":published.info.reference.to_string()}),
        )
        .unwrap(),
        b"{}".to_vec(),
        serde_json::to_vec(&json!({"link":link.to_string(), "release":null})).unwrap(),
        serde_json::to_vec(&json!({"link":null, "release":published.info.reference.to_string()}))
            .unwrap(),
        br#"{"link":"bad"}"#.to_vec(),
        br#"{"release":"bad"}"#.to_vec(),
        serde_json::to_vec(&json!({"link":carol.own_link(None).unwrap().to_string()})).unwrap(),
    ] {
        let (status, _, bytes) = api_request(Some(dist.clone()), "/api/fetch", bytes, None).await;
        assert_eq!(status, 400);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
            "bad_request"
        );
    }
    let (status, _, bytes) = fetch(Some(dist.clone()), json!({"link":link.to_string()})).await;
    assert_eq!(status, 502);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
        "fetch_failed"
    );
    assert_eq!(site.hits.load(std::sync::atomic::Ordering::SeqCst), 0);
    let (status, _, bytes) = fetch(None, json!({"link":link.to_string()})).await;
    assert_eq!(status, 503);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
        "unavailable"
    );
    let allowed_home = tempfile::tempdir().unwrap();
    let allowed = open(allowed_home.path(), Recording::new(true), true);
    allowed.follow(&link).unwrap();
    carol.withdraw(&published.info.reference, "broken").unwrap();
    allowed.refresh_channel(&carol.identity().unwrap()).unwrap();
    let (status, _, bytes) = fetch(
        Some(allowed),
        json!({"release":published.info.reference.to_string()}),
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
        "fetch_refused"
    );
    let host_home = tempfile::tempdir().unwrap();
    let host = watane_host::Host::open(
        host_home.path(),
        watane_host::Mode::Home { app: None },
        Default::default(),
    )
    .unwrap();
    let served = host
        .serve(0, watane_app::api::router(Some(dist)))
        .await
        .unwrap();
    let response = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/api/fetch", served.port))
        .header("Origin", "http://example.com")
        .json(&json!({"link":link.to_string()}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    served.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ac4_publish_lists_provider_after_release_and_denial_keeps_release() {
    let carol_home = tempfile::tempdir().unwrap();
    let carol = open(carol_home.path(), Recording::new(true), true);
    let site = Site::serve(carol.site_dir()).await;
    carol.set_site_url(&site.base).unwrap();
    let published = carol.publish(package()).unwrap();
    let link = published.link.unwrap();
    for approve in [true, false] {
        let home = tempfile::tempdir().unwrap();
        let confirm = Recording::new(approve);
        let dave = open(home.path(), confirm.clone(), true);
        dave.follow(&link).unwrap();
        let (status, _, bytes) = api_request(
            Some(dave.clone()),
            "/api/publish",
            api_support::consumer(&carol.identity().unwrap()),
            Some(&link.to_string()),
        )
        .await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            body["providers"],
            json!([{"provider":format!("{}/reading-list", carol.identity().unwrap()), "listed":approve}])
        );
        let requests = confirm.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .map(|r| r.statement.kind())
                .collect::<Vec<_>>(),
            ["release", "list"]
        );
        match &requests[1].statement {
            watane_dist::Statement::List(listing) => {
                assert_eq!(listing.note, "used by recommendations");
                assert_eq!(listing.link, link);
            }
            other => panic!("{other:?}"),
        }
        let own = dave.own().unwrap();
        assert_eq!(own.releases.len(), 1);
        assert_eq!(own.releases[0].reference.to_string(), body["release"]);
        assert_eq!(
            own.listings
                .iter()
                .filter(|l| !l.include && l.unlisted.is_none() && l.listing.link == link)
                .count(),
            usize::from(approve)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ac5_publish_provider_header_validation_and_resolution() {
    let carol_home = tempfile::tempdir().unwrap();
    let carol = open(carol_home.path(), Recording::new(true), true);
    let site = Site::serve(carol.site_dir()).await;
    carol.set_site_url(&site.base).unwrap();
    let link = carol.publish(package()).unwrap().link.unwrap();
    let consumer = api_support::consumer(&carol.identity().unwrap());
    for header in [
        "bad".to_owned(),
        carol.own_link(None).unwrap().to_string(),
        vec![link.to_string(); 33].join(" "),
    ] {
        let home = tempfile::tempdir().unwrap();
        let confirm = Recording::new(true);
        let dave = open(home.path(), confirm.clone(), true);
        let (status, _, bytes) = api_request(
            Some(dave.clone()),
            "/api/publish",
            consumer.clone(),
            Some(&header),
        )
        .await;
        assert_eq!(status, 400);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["error"]["code"],
            "bad_request"
        );
        assert!(confirm.requests.lock().unwrap().is_empty());
        assert!(dave.own().unwrap().releases.is_empty());
    }
    // Header, followed fallback, existing listing, no base, and an unrelated header.
    for case in [
        "header",
        "followed",
        "listed",
        "missing",
        "unmatched",
        "listed_record",
    ] {
        let home = tempfile::tempdir().unwrap();
        let confirm = Recording::new(true);
        let dave = open(home.path(), confirm.clone(), true);
        if case == "followed" {
            dave.follow(&link).unwrap();
        }
        if case == "listed" {
            dave.list(&link, "already listed").unwrap();
        }
        if case == "listed_record" {
            let ivy_home = tempfile::tempdir().unwrap();
            let ivy = open(ivy_home.path(), Recording::new(true), true);
            let ivy_site = Site::serve(ivy.site_dir()).await;
            ivy.set_site_url(&ivy_site.base).unwrap();
            ivy.list(&link, "third party").unwrap();
            dave.choose_index(&ivy.own_link(None).unwrap()).unwrap();
            assert!(!dave.channel(&carol.identity().unwrap()).unwrap().followed);
        }
        confirm.requests.lock().unwrap().clear();
        let unrelated = watane_dist::Link::new(
            carol.identity().unwrap(),
            &site.base,
            Some(&"other".parse().unwrap()),
        )
        .unwrap()
        .to_string();
        let header = match case {
            "header" => Some(vec![link.to_string(); 32].join(" ")),
            "unmatched" => Some(unrelated),
            _ => None,
        };
        let (status, _, bytes) = api_request(
            Some(dave.clone()),
            "/api/publish",
            consumer.clone(),
            header.as_deref(),
        )
        .await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
        let listed = matches!(case, "header" | "followed" | "listed");
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["providers"],
            json!([{"provider":format!("{}/reading-list", carol.identity().unwrap()), "listed":listed}])
        );
        let requests = confirm.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r.statement.kind() == "list")
                .count(),
            usize::from(matches!(case, "header" | "followed"))
        );
    }
    let home = tempfile::tempdir().unwrap();
    let confirm = Recording::new(true);
    let dave = open(home.path(), confirm.clone(), true);
    let (status, _, bytes) = api_request(
        Some(dave),
        "/api/publish",
        package(),
        Some(&link.to_string()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["providers"],
        json!([])
    );
    assert_eq!(confirm.requests.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn ac5_publish_own_provider_needs_no_listing() {
    let home = tempfile::tempdir().unwrap();
    let confirm = Recording::new(true);
    let dist = open(home.path(), confirm.clone(), true);
    dist.publish(package()).unwrap();
    confirm.requests.lock().unwrap().clear();
    let (status, _, bytes) = api_request(
        Some(dist.clone()),
        "/api/publish",
        api_support::consumer(&dist.identity().unwrap()),
        None,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["providers"],
        json!([])
    );
    assert_eq!(
        confirm
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.statement.kind())
            .collect::<Vec<_>>(),
        ["release"]
    );
}

#[tokio::test]
async fn ac4_fetch_shape_and_wrong_methods_have_plain_json_errors() {
    let home = tempfile::tempdir().unwrap();
    let dist = Arc::new(
        watane_dist::Dist::open(home.path(), Arc::new(Answer(true)), Default::default()).unwrap(),
    );
    for body in [
        json!({}),
        json!({"link":null,"release":null}),
        json!({"link":"bad","release":"bad"}),
    ] {
        let (status, _, bytes) = fetch(Some(dist.clone()), body).await;
        assert_eq!(status, 400);
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["error"],
            json!({"code":"bad_request","message":"give link or release"})
        );
    }
    for (path, method) in [
        ("/api/fetch", "GET"),
        ("/api/publish", "GET"),
        ("/api/search", "GET"),
        ("/api/releases", "POST"),
    ] {
        let response = watane_app::api::router(Some(dist.clone()))
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 405);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: Value = serde_json::from_slice(&bytes).expect("JSON method error");
        assert_eq!(body["error"]["code"], "method_not_allowed");
        assert!(!body["error"]["message"].as_str().unwrap().is_empty());
    }
}
