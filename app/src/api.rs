use crate::CmdError;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
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
        .with_state(dist)
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
    match tauri::async_runtime::spawn_blocking(move || dist.publish(bytes.to_vec())).await {
        Ok(Ok(p)) => Json(serde_json::json!({
            "release": p.info.reference.to_string(),
            "site": p.site.dir,
            "link": p.link.map(|l| l.to_string()),
        }))
        .into_response(),
        Ok(Err(e)) => {
            let (status, code) = match &e {
                DistError::Refused => (StatusCode::FORBIDDEN, "refused"),
                DistError::Package(_) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid_package"),
                DistError::SiteNotWritten { .. } => {
                    (StatusCode::INTERNAL_SERVER_ERROR, "site_not_written")
                }
                _ => (StatusCode::CONFLICT, "publish_refused"),
            };
            error(status, code, CmdError::from(e).message)
        }
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "unavailable",
            e.to_string(),
        ),
    }
}
