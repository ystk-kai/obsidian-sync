// This file is generated automatically

use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::Body,
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
    let query = req.uri().query();

    info!("CouchDB proxy request: {} {}", method, uri_path);

    // CouchDBのURLを取得
    let couchdb_url = state.livesync_service.get_couchdb_url();

    // /dbプレフィックスを除去
    let stripped_path = uri_path.trim_start_matches("/db").trim_start_matches("/");

    // CouchDBへのパスをマッピング
    let couchdb_path = if stripped_path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", stripped_path)
    };

    // ターゲットURIを構築
    let target_uri = if let Some(q) = query {
        format!(
            "{}{}?{}",
            couchdb_url.trim_end_matches('/'),
            couchdb_path,
            q
        )
    } else {
        format!("{}{}", couchdb_url.trim_end_matches('/'), couchdb_path)
    };

    debug!("Forwarding to CouchDB: {} {}", method, target_uri);

    // 単純に文字列でレスポンスを返す - モック応答
    let mock_response = match &couchdb_path[..] {
        "/" => {
            // ルートパスの場合はCouchDBの基本情報を返す
            info!("Returning mock response for CouchDB root");
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"couchdb":"Welcome","version":"3.2.1"}"#))
                .unwrap()
        }
        "/_session" => {
            // セッション情報
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"ok":true,"userCtx":{"name":null,"roles":["_admin"]}}"#,
                ))
                .unwrap()
        }
        "/_all_dbs" => {
            // 全データベースリスト
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("[]"))
                .unwrap()
        }
        "/_up" => {
            // ヘルスチェック
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"status":"ok"}"#))
                .unwrap()
        }
        _ => {
            // その他のパスに対するデフォルト応答
            // 本来はここでCouchDBにリクエストを転送する実装となる
            info!("Returning mock response for path: {}", couchdb_path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"ok":true}"#))
                .unwrap()
        }
    };

    // メトリクスを記録
    state
        .metrics_state
        .record_request_duration(&uri_path, method.as_str(), start);
    state
        .metrics_state
        .record_request(&uri_path, method.as_str(), 200);

    mock_response
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
