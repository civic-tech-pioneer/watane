//! The local API (TECHNICAL 11): JSON routes on the API host that install,
//! manage and inspect mods, with errors mapped to their HTTP status.

use crate::HostError;
use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct ApiState {
    pub host: Arc<crate::server::ServingHost>,
    pub port: u16,
}
use axum::Json;
use axum::body::{Body, Bytes, HttpBody, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

fn json_response(status: u16, value: Value) -> Response {
    let mut response = (
        StatusCode::from_u16(status).expect("valid status"),
        Json(value),
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}
pub(crate) fn error(status: u16, code: &str, message: impl ToString) -> Response {
    json_response(
        status,
        json!({"error":{"code":code,"message":message.to_string()}}),
    )
}
pub(crate) fn not_found() -> Response {
    error(404, "not_found", "unknown API route")
}
/// Read a built-in route's body within its limit, rejecting a known oversize before polling.
pub(crate) async fn read_body(body: Body, limit: usize) -> Result<Bytes, Response> {
    let mut response = if body.size_hint().lower() > limit as u64 {
        error(413, "bad_request", "length limit exceeded")
    } else {
        match to_bytes(body, limit).await {
            Ok(bytes) => return Ok(bytes),
            Err(e) => error(413, "bad_request", e),
        }
    };
    // The unread body must not leave a reusable HTTP/1 connection.
    response.headers_mut().insert(
        header::CONNECTION,
        header::HeaderValue::from_static("close"),
    );
    Err(response)
}

fn host_error(error: HostError) -> Response {
    json_response(error.status(), json!({"error":error.json()}))
}

pub(crate) async fn respond(state: ApiState, request: Request<Body>) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return error(403, "forbidden", "Origin is forbidden on the local API");
    }
    let path = request.uri().path();
    let allowed = match path {
        "/api/status" | "/api/mods" => Method::GET,
        "/api/plan" => Method::POST,
        "/api/install" => Method::POST,
        _ => return not_found(),
    };
    if request.method() != allowed {
        let mut response = error(405, "bad_request", format!("use {allowed}"));
        response.headers_mut().insert(
            header::ALLOW,
            allowed.as_str().parse().expect("method is a header"),
        );
        return response;
    }
    let route = path.to_owned();
    let status_route = path == "/api/status" || path == "/api/mods";
    let content_type = request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !status_route
        && !content_type
            .split(';')
            .next()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    {
        return error(415, "bad_request", "use application/json");
    }
    let bytes = match read_body(request.into_body(), 65_536).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    if route == "/api/mods" {
        return json_response(200, json!({"mods":state.host.mods()}));
    }
    if status_route {
        return json_response(
            200,
            json!({"watane":1,"mode":state.host.home.mode.as_str(),"home":state.host.home(),"port":state.port,"app":state.host.home.mode.has_app()}),
        );
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Install {
        folders: Vec<PathBuf>,
    }
    let install: Install = match serde_json::from_slice(&bytes) {
        Ok(install) => install,
        Err(e) => return error(400, "bad_request", e),
    };
    if install.folders.iter().any(|path| !path.is_absolute()) {
        return error(400, "bad_request", "folder paths must be absolute");
    }
    let host = state.host;
    // Section 11: /api/install plans and installs under the install mutex.
    let _install = if route == "/api/install" {
        Some(host.installing.lock().await)
    } else {
        None
    };
    let planner = Arc::clone(&host);
    let plan = match tokio::task::spawn_blocking(move || {
        planner.plan(
            install
                .folders
                .into_iter()
                .map(crate::Source::Folder)
                .collect(),
        )
    })
    .await
    {
        Ok(Ok(plan)) => plan,
        Ok(Err(e)) => return host_error(e),
        Err(e) => return host_error(HostError::Internal(e.to_string())),
    };
    if route == "/api/plan" {
        return json_response(200, serde_json::to_value(&plan).expect("plan serializes"));
    }
    match host.install_locked(plan, crate::Requester::LocalApi).await {
        Ok(installed) => json_response(200, json!({"installed":installed})),
        Err(e) => host_error(e),
    }
}
