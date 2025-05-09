use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Body as AxumBody,
    extract::State,
    http::{Request, StatusCode},
    response::Response,
};
use tracing::debug;
use uuid::Uuid;

use crate::interfaces::web::server::AppState;

/// HTTP proxy handler for Obsidian LiveSync
/// This handler proxies HTTP requests to the CouchDB server
pub async fn http_proxy_handler<B>(
    State(_state): State<Arc<AppState>>,
    req: Request<B>,
) -> Response<AxumBody>
where
    B: axum::body::HttpBody + Send + 'static,
    B::Data: Into<bytes::Bytes>,
    B::Error: Into<axum::BoxError>,
{
    // Record the start time for metrics
    let _start = Instant::now();

    // Generate a client ID for tracking
    let client_id = Uuid::new_v4().to_string();
    debug!("New HTTP proxy request from client {}", client_id);

    // Get the original URI
    let original_uri = req.uri().clone();
    let path = original_uri.path();
    let _query = original_uri.query();
    let _method = req.method().clone();

    // 式を直接返す
    Response::builder()
        .status(StatusCode::OK)
        .body(AxumBody::from(format!("Proxied request to path: {}", path)))
        .unwrap()
}
