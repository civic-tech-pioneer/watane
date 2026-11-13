use crate::CmdError;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, post},
};
use std::sync::Arc;
use watane_dist::{Dist, DistError};
fn error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(serde_json::json!({"error":CmdError::new(code,message)})),
    )
        .into_response()
}
pub fn router(dist: Option<Arc<Dist>>) -> Router {
    Router::new()
        .route(
            "/api/publish",
            post(publish).layer(DefaultBodyLimit::max(67_108_864)),
        )
        .route("/api/releases", any(releases))
        .route("/api/search", post(search))
        .route(
            "/api/fetch",
            post(fetch).layer(DefaultBodyLimit::max(65_536)),
        )
        .with_state(dist)
}
async fn releases(State(dist): State<Option<Arc<Dist>>>, method: Method) -> Response {
    if method != Method::GET {
        let mut response = error(StatusCode::METHOD_NOT_ALLOWED, "bad_request", "use GET");
        response
            .headers_mut()
            .insert("allow", "GET".parse().expect("static method"));
        return response;
    }
    let Some(dist) = dist else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "dist did not open",
        );
    };
    match tauri::async_runtime::spawn_blocking(move || {
        dist.own()?
            .releases
            .into_iter()
            .map(|info| {
                let link = match dist.own_link(Some(&info.release.slug)) {
                    Ok(link) => Some(link.to_string()),
                    Err(DistError::NoSiteUrl) => None,
                    Err(error) => return Err(error),
                };
                Ok(serde_json::json!({
                    "slug": info.release.slug.to_string(),
                    "version": info.release.version.to_string(),
                    "release": info.reference.to_string(),
                    "seq": info.seq,
                    "link": link,
                }))
            })
            .collect::<Result<Vec<_>, DistError>>()
    })
    .await
    {
        Ok(Ok(releases)) => Json(serde_json::json!({"releases": releases})).into_response(),
        Ok(Err(e)) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            CmdError::from(e).message,
        ),
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            e.to_string(),
        ),
    }
}

async fn publish(
    State(dist): State<Option<Arc<Dist>>>,
    headers: HeaderMap,
    bytes: Result<Bytes, BytesRejection>,
) -> Response {
    let Some(dist) = dist else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "dist did not open",
        );
    };
    if headers.get("content-type").and_then(|v| v.to_str().ok()) != Some("application/octet-stream")
    {
        return error(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "expected application/octet-stream",
        );
    }
    let bytes = match bytes {
        Ok(b) if !b.is_empty() => b,
        Ok(_) => return error(StatusCode::BAD_REQUEST, "bad_request", "empty package body"),
        Err(e) => {
            return error(
                e.status(),
                if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    "too_large"
                } else {
                    "bad_request"
                },
                crate::cause_chain(&e),
            );
        }
    };
    let links = match provider_links(&headers) {
        Ok(links) => links,
        Err(cause) => return error(StatusCode::BAD_REQUEST, "bad_request", cause),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        let p = dist.publish(bytes.to_vec())?;
        let providers = publish_providers(&dist, &p.info.release, &links);
        Ok::<_, DistError>(serde_json::json!({
            "release": p.info.reference.to_string(),
            "site": p.site.dir,
            "link": p.link.map(|l| l.to_string()),
            "providers": providers,
        }))
    })
    .await
    {
        Ok(Ok(body)) => Json(body).into_response(),
        Ok(Err(e)) => {
            let (status, code) = match &e {
                DistError::Refused => (StatusCode::FORBIDDEN, "refused"),
                DistError::VersionExists { .. } => (StatusCode::CONFLICT, "version_exists"),
                DistError::Package(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_package"),
                DistError::SiteNotWritten { .. } => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "site_not_written")
                }
                _ => (StatusCode::CONFLICT, "publish_refused"),
            };
            let message = match e {
                DistError::Package(watane_host::watane_pkg::PackageError::Mod(
                    watane_host::watane_pkg::ModError::Invalid(mut violations),
                )) => {
                    // pkg names the file in both the location and some diagnostic texts.
                    for violation in &mut violations {
                        if let Some(message) = violation
                            .message
                            .strip_prefix(&format!("{} ", violation.file))
                        {
                            violation.message = message.to_owned();
                        }
                    }
                    watane_host::watane_pkg::ModError::Invalid(violations).to_string()
                }
                other => CmdError::from(other).message,
            };
            error(status, code, message)
        }
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            e.to_string(),
        ),
    }
}

fn provider_links(headers: &HeaderMap) -> Result<Vec<watane_dist::Link>, String> {
    let mut values = headers.get_all("Watane-Provider-Links").iter();
    let Some(value) = values.next() else {
        return Ok(Vec::new());
    };
    if values.next().is_some() {
        return Err("give one Watane-Provider-Links header".into());
    }
    let value = value.to_str().map_err(|e| e.to_string())?;
    let texts: Vec<_> = value.split(' ').filter(|s| !s.is_empty()).collect();
    if texts.len() > 32 {
        return Err("at most 32 provider links".into());
    }
    texts
        .into_iter()
        .map(|text| {
            let link: watane_dist::Link = text.parse().map_err(|e: DistError| e.to_string())?;
            if link.slug().is_none() {
                return Err("a provider link needs a slug".into());
            }
            Ok(link)
        })
        .collect()
}

fn publish_providers(
    dist: &Dist,
    release: &watane_dist::Release,
    links: &[watane_dist::Link],
) -> Vec<serde_json::Value> {
    use watane_host::watane_pkg::Grant;
    let own = dist.own().ok();
    let followed = dist.followed().unwrap_or_default();
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for grant in &release.grants {
        let (Grant::Call { provider, .. } | Grant::Subscribe { provider, .. }) = grant else {
            continue;
        };
        let Some(publisher) = &provider.publisher else {
            continue;
        };
        if publisher.as_bytes() == dist.identity().as_bytes() || !seen.insert(provider.clone()) {
            continue;
        }
        let matches = |link: &watane_dist::Link| {
            link.channel().as_bytes() == publisher.as_bytes() && link.slug() == Some(&provider.slug)
        };
        let already = own.as_ref().is_some_and(|own| {
            own.listings
                .iter()
                .any(|l| !l.include && l.unlisted.is_none() && matches(&l.listing.link))
        });
        let link = links
            .iter()
            .find(|link| matches(link))
            .cloned()
            .or_else(|| {
                followed
                    .iter()
                    .find(|c| c.followed && c.channel.as_bytes() == publisher.as_bytes())
                    .and_then(|c| {
                        c.base.as_ref().and_then(|base| {
                            watane_dist::Link::new(c.channel, base, Some(&provider.slug)).ok()
                        })
                    })
            });
        let listed = already
            || link.is_some_and(|link| {
                dist.list(&link, &format!("used by {}", release.slug))
                    .is_ok()
            });
        result.push(serde_json::json!({"provider":provider.to_string(), "listed":listed}));
    }
    result
}

#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum FetchQuery {
    Link { link: String },
    Release { release: String },
}
enum FetchTarget {
    Link(watane_dist::Link),
    Release(watane_dist::ReleaseRef),
}
async fn fetch(
    State(dist): State<Option<Arc<Dist>>>,
    query: Result<Json<FetchQuery>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(dist) = dist else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "dist did not open",
        );
    };
    let target = (|| -> Result<_, String> {
        let Json(query) = query.map_err(|e| e.to_string())?;
        match query {
            FetchQuery::Link { link } => {
                let link: watane_dist::Link = link.parse().map_err(|e: DistError| e.to_string())?;
                if link.slug().is_none() {
                    return Err("a release link needs a slug".into());
                }
                Ok(FetchTarget::Link(link))
            }
            FetchQuery::Release { release } => {
                let release: watane_dist::ReleaseRef = release
                    .parse()
                    .map_err(|e: watane_host::watane_pkg::NameError| e.to_string())?;
                Ok(FetchTarget::Release(release))
            }
        }
    })();
    let target = match target {
        Ok(target) => target,
        Err(cause) => return error(StatusCode::BAD_REQUEST, "bad_request", cause),
    };
    match tauri::async_runtime::spawn_blocking(move || match target {
        FetchTarget::Link(link) => dist.read_link(&link),
        FetchTarget::Release(release) => dist.fetch(&release),
    })
    .await
    {
        Ok(Ok(fetched)) => (
            [
                ("content-type", "application/octet-stream".to_owned()),
                ("Watane-Release", fetched.info.reference.to_string()),
                ("Watane-Package", fetched.package.id().to_string()),
            ],
            fetched.package.into_bytes(),
        )
            .into_response(),
        Ok(Err(e)) => {
            let (status, code) = match &e {
                DistError::NoRelease { .. }
                | DistError::Withdrawn { .. }
                | DistError::Compromised { .. } => (StatusCode::CONFLICT, "fetch_refused"),
                _ => (StatusCode::BAD_GATEWAY, "fetch_failed"),
            };
            error(status, code, crate::cause_chain(&e))
        }
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            e.to_string(),
        ),
    }
}

async fn search(
    State(dist): State<Option<Arc<Dist>>>,
    query: Result<Json<crate::find::SearchQuery>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(dist) = dist else {
        return error(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "dist did not open",
        );
    };
    let query = match query
        .map_err(|e| e.to_string())
        .and_then(|Json(q)| q.parse().map_err(|e| e.message))
    {
        Ok(query) => query,
        Err(cause) => return error(StatusCode::BAD_REQUEST, "bad_request", cause),
    };
    match tauri::async_runtime::spawn_blocking(move || {
        let hits = dist.search(&query)?;
        Ok::<_, DistError>(serde_json::json!({"indexes":dist.indexes()?.len(), "results":hits.iter().map(crate::find::api_result).collect::<Vec<_>>()}))
    }).await {
        Ok(Ok(body)) => Json(body).into_response(),
        Ok(Err(e @ DistError::BadQuery { .. })) => {
            error(StatusCode::BAD_REQUEST, "bad_request", crate::cause_chain(&e))
        }
        Ok(Err(e)) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            crate::cause_chain(&e),
        ),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, "unavailable", e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    struct Approve;
    impl watane_dist::Confirm for Approve {
        fn confirm(&self, _: &watane_dist::SignRequest) -> watane_dist::Decision {
            watane_dist::Decision::Approve
        }
    }
    fn open(home: &tempfile::TempDir) -> Arc<Dist> {
        Arc::new(Dist::open(home.path(), Arc::new(Approve), Default::default()).unwrap())
    }
    fn package() -> Vec<u8> {
        watane_host::watane_pkg::pack(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-list"),
            false,
        )
        .unwrap()
        .into_bytes()
    }
    async fn request(
        app: Router,
        method: Method,
        path: &str,
        bytes: Vec<u8>,
    ) -> (StatusCode, HeaderMap, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/octet-stream")
                    .body(Body::from(bytes))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, body)
    }
    #[tokio::test]
    async fn ac1_releases_empty_unavailable_and_wrong_methods() {
        let home = tempfile::tempdir().unwrap();
        let app = router(Some(open(&home)));
        let (status, _, body) = request(app.clone(), Method::GET, "/api/releases", vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({"releases":[]}));
        let (status, _, body) = request(router(None), Method::GET, "/api/releases", vec![]).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            body,
            json!({"error":{"code":"unavailable","message":"dist did not open"}})
        );
        for method in [
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
            Method::HEAD,
        ] {
            let (status, headers, body) =
                request(app.clone(), method.clone(), "/api/releases", vec![]).await;
            assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "{method}");
            assert_eq!(headers["allow"], "GET");
            if method != Method::HEAD {
                assert_eq!(
                    body,
                    json!({"error":{"code":"bad_request","message":"use GET"}})
                );
            }
        }
    }
    #[tokio::test]
    async fn ac1_releases_in_seq_order_with_current_links() {
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        dist.set_name("Bob").unwrap();
        let first = dist.publish(package()).unwrap();
        let page = watane_host::watane_pkg::pack(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hostile"),
            false,
        )
        .unwrap();
        let second = dist.publish(page.into_bytes()).unwrap();
        let app = router(Some(dist.clone()));
        let (_, _, body) = request(app.clone(), Method::GET, "/api/releases", vec![]).await;
        assert_eq!(
            body,
            json!({"releases":[
                {"slug":"reading-list","version":"0.1.0","release":first.info.reference.to_string(),"seq":1,"link":null},
                {"slug":"hostile","version":"0.1.0","release":second.info.reference.to_string(),"seq":2,"link":null}
            ]})
        );
        dist.set_site_url("https://example.com/bob/").unwrap();
        let (status, _, body) = request(app, Method::GET, "/api/releases", vec![]).await;
        assert_eq!(status, StatusCode::OK);
        let key = dist.identity();
        assert_eq!(
            body["releases"][0]["link"],
            format!("watane:{key}@https://example.com/bob/#reading-list")
        );
        assert_eq!(
            body["releases"][1]["link"],
            format!(
                "watane:{key}@https://example.com/bob/#{}",
                second.info.release.slug
            )
        );
    }
    #[tokio::test]
    async fn ac2_duplicate_version_has_its_own_code_and_dist_message() {
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        let app = router(Some(dist.clone()));
        let bytes = package();
        let (status, _, first) =
            request(app.clone(), Method::POST, "/api/publish", bytes.clone()).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, body) = request(app, Method::POST, "/api/publish", bytes).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body,
            json!({"error":{"code":"version_exists","message":format!("release {} already exists at seq 0", first["release"].as_str().unwrap())}})
        );

        // A distinct dist refusal still uses the generic publish code.
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        let bytes = watane_host::watane_pkg::pack(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/recommendations"),
            false,
        )
        .unwrap()
        .into_bytes();
        let (status, _, body) =
            request(router(Some(dist)), Method::POST, "/api/publish", bytes).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body,
            json!({"error":{"code":"publish_refused","message":"publish provider reading-list first"}})
        );
    }
    #[tokio::test]
    async fn ac3_corrupt_body_names_the_cause_once_without_temporary_paths() {
        let home = tempfile::tempdir().unwrap();
        let dist = open(&home);
        let mut bytes = package();
        let offset = bytes
            .windows(8)
            .position(|w| w == b"\0asm\x0d\0\x01\0")
            .unwrap();
        bytes[offset] ^= 1;
        let package = watane_host::watane_pkg::Package::from_bytes(bytes.clone()).unwrap();
        let dest = tempfile::tempdir().unwrap();
        assert!(package.unpack(&dest.path().join("mod")).is_err());
        assert!(matches!(
            dist.publish(bytes.clone()),
            Err(DistError::Package(_))
        ));
        let (status, _, body) =
            request(router(Some(dist)), Method::POST, "/api/publish", bytes).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "invalid_package");
        let message = body["error"]["message"].as_str().unwrap();
        assert_eq!(
            message,
            "the mod breaks 1 rule(s): [component] logic.wasm: is not a WebAssembly component"
        );
        assert_eq!(message.matches("logic.wasm").count(), 1);
        assert!(!message.contains(".tmp"));
        for segment in std::env::temp_dir().components() {
            if let std::path::Component::Normal(segment) = segment {
                assert!(!message.contains(&format!("/{}/", segment.to_string_lossy())));
            }
        }
    }

    #[test]
    fn ac4_specs_describe_release_discovery_and_duplicate_code() {
        let app = include_str!("../TECHNICAL.md");
        let dev = include_str!("../../dev/TECHNICAL.md")
            .split("## 14.")
            .nth(1)
            .unwrap()
            .split("## 15.")
            .next()
            .unwrap();
        for spec in [app, dev] {
            assert!(spec.contains("GET /api/releases"));
            assert!(spec.contains("version_exists"));
        }
    }
}
