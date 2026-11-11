use crate::{HostError, server::ServerState};
use axum::{
    Json,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::{IntoResponse, Response},
};
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
fn host_error(error: HostError) -> Response {
    json_response(error.status(), json!({"error":error.json()}))
}

pub(crate) async fn respond(state: ServerState, request: Request<Body>) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return error(403, "forbidden", "Origin is forbidden on the local API");
    }
    let path = request.uri().path();
    let allowed = match path {
        "/api/status" => Method::GET,
        "/api/install" => Method::POST,
        _ => return error(404, "bad_request", "unknown API route"),
    };
    if request.method() != allowed {
        let mut response = error(405, "bad_request", format!("use {allowed}"));
        response.headers_mut().insert(
            header::ALLOW,
            allowed.as_str().parse().expect("method is a header"),
        );
        return response;
    }
    let status_route = path == "/api/status";
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
    let bytes = match to_bytes(request.into_body(), 65_536).await {
        Ok(bytes) => bytes,
        Err(e) => return error(413, "bad_request", e),
    };
    if status_route {
        return json_response(
            200,
            json!({"watane":1,"mode":state.host.home.mode.as_str(),"home":state.host.home(),"port":state.port,"app":false}),
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
    match tokio::task::spawn_blocking(move || {
        state.host.install_folders(&install.folders, state.port)
    })
    .await
    {
        Ok(Ok(installed)) => json_response(200, json!({"installed":installed})),
        Ok(Err(e)) => host_error(e),
        Err(e) => host_error(HostError::Internal(e.to_string())),
    }
}
