// This file is generated automatically

use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::Value;
use tracing::{debug, info};

use crate::interfaces::web::server::AppState;

/// HTTP proxy handler for Obsidian LiveSync
/// This handler proxies HTTP requests to the CouchDB server
pub async fn http_proxy_handler(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
) -> impl IntoResponse {
    // 開始時間の記録
    let start = Instant::now();

    // リクエスト情報を抽出
    let method = req.method().clone();
    let uri_path = req.uri().path().to_string();
    let query = req.uri().query().map(String::from);

    info!("CouchDB proxy request: {} {}", method, uri_path);

    // /dbプレフィックスを除去
    let stripped_path = uri_path.trim_start_matches("/db").trim_start_matches("/");

    // CouchDBへのパスをマッピング
    let couchdb_path = if stripped_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", stripped_path)
    };

    // リクエストのヘッダーとボディを抽出
    let (parts, body) = req.into_parts();
    let headers = parts.headers;

    // ボディをバイト列に変換
    let body_bytes = match to_bytes(body, 1024 * 1024 * 10).await {
        // 10MB制限
        Ok(bytes) => bytes,
        Err(e) => {
            debug!("Failed to read request body: {}", e);
            let response = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"error":"Failed to read request body: {}"}}"#,
                    e
                )))
                .unwrap();

            // メトリクスを記録
            state
                .metrics_state
                .record_request_duration(&uri_path, method.as_str(), start);
            state
                .metrics_state
                .record_request(&uri_path, method.as_str(), 500);

            return response;
        }
    };

    // リクエストをCouchDBに転送
    let response = match state
        .livesync_service
        .forward_request(
            method.as_str(),
            &couchdb_path,
            query.clone(),
            headers,
            body_bytes,
        )
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            debug!("Failed to forward request to CouchDB: {}", e);
            let response = Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(format!(
                    r#"{{"error":"Failed to forward request to CouchDB: {}"}}"#,
                    e
                )))
                .unwrap();

            // メトリクスを記録
            state
                .metrics_state
                .record_request_duration(&uri_path, method.as_str(), start);
            state
                .metrics_state
                .record_request(&uri_path, method.as_str(), 502);

            return response;
        }
    };

    // レスポンスのステータスコードを取得
    let status_code = response.status().as_u16();

    // メトリクスを記録
    state
        .metrics_state
        .record_request_duration(&uri_path, method.as_str(), start);
    state
        .metrics_state
        .record_request(&uri_path, method.as_str(), status_code);

    response
}

/// サーバーのステータス情報を返すハンドラー
pub async fn status_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    // CouchDBの状態を取得
    let couchdb_status = state.health_state.couchdb_status.read().await;

    Json(serde_json::json!({
        "status": if couchdb_status.available { "ok" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "services": {
            "couchdb": {
                "available": couchdb_status.available,
                "last_checked": couchdb_status.last_checked.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                "error": couchdb_status.error_message
            }
        }
    }))
}

/// デバッグ用の単純なハンドラー
pub async fn debug_handler() -> impl IntoResponse {
    info!("Debug handler called");
    Json(serde_json::json!({
        "status": "ok",
        "message": "Debug endpoint is working"
    }))
}
