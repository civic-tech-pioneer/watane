use crate::{Caller, Host, LocalName, api};
use axum::{
    Json,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, path::Path, sync::Arc};

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; media-src 'self' blob:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; form-action 'self'; base-uri 'none'; frame-src 'none'; frame-ancestors 'none'; object-src 'none'";

pub(crate) fn plain(status: u16, text: &str) -> Response {
    (
        StatusCode::from_u16(status).expect("valid status"),
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        text.to_owned(),
    )
        .into_response()
}
pub(crate) fn not_found() -> Response {
    secure(plain(404, "not found"))
}
fn secure(mut response: Response) -> Response {
    for (name, value) in [
        ("content-security-policy", CSP),
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        ("cache-control", "no-store"),
    ] {
        response.headers_mut().insert(
            header::HeaderName::from_static(name),
            header::HeaderValue::from_static(value),
        );
    }
    response
}

pub(crate) fn respond(host: &Host, local: &LocalName, method: &Method, encoded: &str) -> Response {
    let mut response = file(host, local, method, encoded);
    if method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    secure(response)
}

pub(crate) async fn call(
    host: &Arc<Host>,
    local: LocalName,
    port: u16,
    request: Request<Body>,
) -> Response {
    if !available(host, &local) {
        return not_found();
    }
    secure(call_response(host, local, port, request).await)
}

async fn call_response(
    host: &Arc<Host>,
    local: LocalName,
    port: u16,
    request: Request<Body>,
) -> Response {
    if request.method() != Method::POST {
        let mut response = api::error(405, "bad_request", "use POST");
        response
            .headers_mut()
            .insert(header::ALLOW, header::HeaderValue::from_static("POST"));
        return response;
    }
    let origin = format!("http://{local}.localhost:{port}");
    let mut origins = request.headers().get_all(header::ORIGIN).iter();
    let Some(supplied_origin) = origins.next() else {
        return api::error(
            403,
            "forbidden",
            format!("the Origin header is missing; it must be {origin}"),
        );
    };
    if supplied_origin.to_str().ok() != Some(origin.as_str()) || origins.next().is_some() {
        return api::error(403, "forbidden", "Origin must match the mod origin");
    }
    if !request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    {
        return api::error(415, "bad_request", "use application/json");
    }
    let bytes = match to_bytes(request.into_body(), 1 << 20).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let mut response = api::error(413, "bad_request", e);
            // The unread body must not leave a reusable HTTP/1 connection.
            response.headers_mut().insert(
                header::CONNECTION,
                header::HeaderValue::from_static("close"),
            );
            return response;
        }
    };
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Call {
        #[serde(rename = "fn")]
        function: String,
        args: Value,
    }
    let body: serde_json::Map<String, Value> = match serde_json::from_slice(&bytes) {
        Ok(body) => body,
        Err(e) => return api::error(400, "bad_request", e),
    };
    let call: Call = match serde_json::from_value(Value::Object(body)) {
        Ok(call) => call,
        Err(e) => return api::error(400, "bad_request", e),
    };
    if !matches!(call.args, Value::Object(_) | Value::Array(_) | Value::Null) {
        return api::error(400, "bad_request", "args must be an object, array or null");
    }
    match host
        .call_async(Caller::Page, local, call.function, call.args)
        .await
    {
        Ok(value) => Json(json!({"value":value})).into_response(),
        Err(error) => (
            StatusCode::from_u16(error.status()).expect("valid call status"),
            Json(json!({"error":error})),
        )
            .into_response(),
    }
}

fn available(host: &Host, local: &LocalName) -> bool {
    host.catalog
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .mods
        .iter()
        .any(|m| &m.local == local && m.enabled && m.error.is_none())
}

pub(crate) fn events(
    host: &Host,
    local: LocalName,
    port: u16,
    request: Request<Body>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Response {
    if !available(host, &local) {
        return not_found();
    }
    let response = if request.method() != Method::GET {
        let mut response = api::error(405, "bad_request", "use GET");
        response
            .headers_mut()
            .insert(header::ALLOW, header::HeaderValue::from_static("GET"));
        response
    } else {
        let origin = format!("http://{local}.localhost:{port}");
        let mut origins = request.headers().get_all(header::ORIGIN).iter();
        if origins
            .next()
            .is_some_and(|v| v.to_str().ok() != Some(origin.as_str()))
            || origins.next().is_some()
        {
            api::error(403, "forbidden", "Origin must match the mod origin")
        } else {
            host.bus
                .response(local, shutdown, std::time::Duration::from_secs(15))
        }
    };
    secure(response)
}

fn file(host: &Host, local: &LocalName, method: &Method, encoded: &str) -> Response {
    if !available(host, local) {
        return not_found();
    }
    let Some(path) = decode_path(encoded) else {
        return not_found();
    };
    if path == "/watane.js" {
        if method != Method::GET && method != Method::HEAD {
            return file_method_not_allowed();
        }
        let mut response = (
            [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
            crate::WATANE_JS,
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, crate::WATANE_JS.len().into());
        return response;
    }
    if matches!(path.as_str(), "/call" | "/events") {
        return not_found();
    }
    let Some(relative) = path.strip_prefix('/') else {
        return not_found();
    };
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative.split('/').any(|segment| {
        segment.is_empty() || segment.starts_with('.') || segment.contains(['\\', '\0'])
    }) {
        return not_found();
    }
    let root = host.home().join("mods").join(local.as_ref()).join("ui");
    let Ok(root) = root.canonicalize() else {
        return not_found();
    };
    let Ok(file) = root.join(relative).canonicalize() else {
        return not_found();
    };
    if !file.starts_with(&root) || !file.is_file() {
        return not_found();
    }
    if method != Method::GET && method != Method::HEAD {
        return file_method_not_allowed();
    }
    let Ok(bytes) = fs::read(&file) else {
        return not_found();
    };
    let length = bytes.len();
    let mut response = Body::from(bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static(content_type(&file)),
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, length.into());
    response
}

fn file_method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "GET, HEAD")],
    )
        .into_response()
}

fn decode_path(path: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(path.len());
    let mut input = path.bytes();
    while let Some(b) = input.next() {
        if b == b'%' {
            let high = (input.next()? as char).to_digit(16)?;
            let low = (input.next()? as char).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).ok()
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
