use crate::{Host, LocalName};
use axum::{
    body::Body,
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::{fs, path::Path};

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

fn file(host: &Host, local: &LocalName, method: &Method, encoded: &str) -> Response {
    let catalog = host
        .catalog
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !catalog
        .mods
        .iter()
        .any(|m| &m.local == local && m.enabled && m.error.is_none() && m.page.is_some())
    {
        return not_found();
    }
    if method != Method::GET && method != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, "GET, HEAD")],
        )
            .into_response();
    }
    let Some(path) = decode_path(encoded) else {
        return not_found();
    };
    if matches!(path.as_str(), "/call" | "/events" | "/watane.js") {
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
