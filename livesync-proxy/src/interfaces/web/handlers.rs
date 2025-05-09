use std::sync::Arc;
use std::time::Instant;

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, Request, StatusCode, Uri},
    response::Response,
};
use base64::engine::{general_purpose, Engine};
use futures::Stream;
use http_body_util::BodyExt;
use hyper_tls::HttpsConnector;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use tracing::{error, info};
use uuid::Uuid;

use crate::interfaces::web::server::AppState;

/// HTTP proxy handler for Obsidian LiveSync
/// This handler proxies HTTP requests to the CouchDB server
pub async fn http_proxy_handler(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
) -> Response<Body> {
    // Record the start time for metrics
    let start = Instant::now();

    // Generate a client ID for tracking
    let client_id = Uuid::new_v4().to_string();
    info!("New HTTP proxy request from client {}", client_id);

    // Get the original URI
    let original_uri = req.uri().clone();
    let path = original_uri.path();
    let query = original_uri.query();
    let method = req.method().clone();
    let method_str = method.as_str().to_string(); // デバッグログ用に文字列として保存

    info!(
        "Request details - Method: {}, Path: {}, Query: {:?}",
        method_str, path, query
    );

    // CouchDBのステータスをチェック
    let couchdb_status = state.health_state.couchdb_status.read().await.clone();
    info!(
        "CouchDB status - Available: {}, Error: {:?}",
        couchdb_status.available, couchdb_status.error_message
    );

    // CouchDBサービスのURLとユーザー情報を常に取得 (エラー時にも情報を表示するため)
    let couchdb_url = state.livesync_service.get_couchdb_url();
    let couchdb_auth = state.livesync_service.get_couchdb_auth();
    info!(
        "CouchDB configuration - URL: {}, Auth available: {}",
        couchdb_url,
        couchdb_auth.is_some()
    );

    // ヘルスチェックでエラーの場合でもリクエストを試行する
    if !couchdb_status.available {
        error!(
            "CouchDB connection is not available: {:?} - Will try direct request anyway",
            couchdb_status.error_message
        );
        // エラーを返さずに続行
    }

    // CouchDBのURLを構築
    let db_path = if let Some(stripped) = path.strip_prefix("/db") {
        // /db プレフィックスを除去
        if stripped.is_empty() {
            // ルートパスの場合は空文字列に
            "".to_string()
        } else {
            stripped.to_string() // "/db" を削除済み
        }
    } else {
        path.to_string()
    };

    let target_uri = match query {
        Some(q) => format!("{}{}", couchdb_url, db_path + "?" + q),
        None => format!("{}{}", couchdb_url, db_path),
    };

    info!(
        "Proxying {} request to CouchDB: {} -> {}",
        method_str, path, target_uri
    );

    // リクエストのURIとホストヘッダーを書き換え
    *req.uri_mut() = Uri::try_from(&target_uri).unwrap_or_else(|_| {
        error!("Failed to parse target URI: {}", target_uri);
        Uri::from_static("http://localhost")
    });

    // リクエストヘッダーのログ
    info!("Original request headers: {:?}", req.headers());

    // ホストヘッダーを更新
    if let Some(host) = req.uri().authority().map(|a| a.as_str().to_string()) {
        info!("Using authority from URI for host header: {}", host);
        if let Ok(host_value) = header::HeaderValue::from_str(&host) {
            req.headers_mut().insert(header::HOST, host_value);
        }
    } else {
        // ターゲットURIからホスト部分を抽出
        let host_str = target_uri
            .replace("http://", "")
            .replace("https://", "")
            .split('/')
            .next()
            .unwrap_or("localhost")
            .to_string(); // 所有権を持つString値に変換

        info!("Extracted host from target URI: {}", host_str);

        if let Ok(host_value) = header::HeaderValue::from_str(&host_str) {
            req.headers_mut().insert(header::HOST, host_value);
        }
    }

    // 認証情報があれば追加
    if let Some((username, password)) = couchdb_auth {
        info!("Adding basic auth header for user: {}", username);
        let auth_value = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", username, password))
        );
        if let Ok(auth_header) = header::HeaderValue::from_str(&auth_value) {
            req.headers_mut().insert(header::AUTHORIZATION, auth_header);
        }
    } else {
        info!("No authentication credentials provided");
    }

    info!("Modified request headers: {:?}", req.headers());

    // HTTPSクライアントを作成
    let https = HttpsConnector::new();
    let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build(https);
    info!("Created HTTPS client for request");

    // リクエスト本文をストリームとして取得しhyperリクエストに変換
    let (parts, body) = req.into_parts();
    let hyper_req = Request::from_parts(parts, body);
    info!("Prepared request for sending to CouchDB");

    // リクエストを送信
    info!("Sending request to CouchDB...");
    match client.request(hyper_req).await {
        Ok(res) => {
            // レスポンスを返す
            let duration = start.elapsed();
            let status = res.status();
            info!(
                "Received response from CouchDB - Status: {}, Duration: {:?}",
                status, duration
            );

            // Hyper ResponseをAxum Responseに変換
            let (parts, body) = res.into_parts();
            info!("Response headers from CouchDB: {:?}", parts.headers);

            // 応答ヘッダーとステータスコードを設定
            let mut response_builder = Response::builder().status(parts.status);

            // ヘッダーをコピー
            for (key, value) in parts.headers.iter() {
                response_builder = response_builder.header(key.as_str(), value);
            }

            info!("Returning proxied response to client");
            // レスポンスボディを変換して返す
            response_builder
                .body(Body::from_stream(BodyStream {
                    inner: body.into_data_stream(),
                }))
                .unwrap_or_else(|_| {
                    error!("Failed to build response from CouchDB response");
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to build response"))
                        .unwrap()
                })
        }
        Err(e) => {
            error!("Failed to forward request to CouchDB: {}", e);

            // 開発用のデモ応答
            if path == "/db" {
                info!("Returning demo data for /db endpoint");
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{
                        "couchdb": "Welcome to LiveSync Proxy",
                        "version": "3.5.0",
                        "status": "Development Mode",
                        "note": "This is a demo response for development purposes only",
                        "databases": ["_users", "_replicator", "obsidian_sync"]
                    }"#,
                    ))
                    .unwrap()
            } else {
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        "{{\"error\":\"bad_gateway\",\"reason\":\"Failed to forward request to CouchDB: {}\"}}", 
                        e
                    )))
                    .unwrap()
            }
        }
    }
}

// レスポンスボディをストリームとして扱うためのラッパー構造体
struct BodyStream<S> {
    inner: S,
}

impl<S> Stream for BodyStream<S>
where
    S: Stream<Item = Result<Bytes, hyper::Error>> + Unpin,
{
    type Item = Result<Bytes, axum::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match std::pin::Pin::new(&mut self.inner).poll_next(cx) {
            std::task::Poll::Ready(Some(Ok(chunk))) => std::task::Poll::Ready(Some(Ok(chunk))),
            std::task::Poll::Ready(Some(Err(e))) => {
                std::task::Poll::Ready(Some(Err(axum::Error::new(e))))
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}
