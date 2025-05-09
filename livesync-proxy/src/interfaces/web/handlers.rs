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
use tracing::{debug, error};
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
    debug!("New HTTP proxy request from client {}", client_id);

    // Get the original URI
    let original_uri = req.uri().clone();
    let path = original_uri.path();
    let query = original_uri.query();
    let method = req.method().clone();
    let method_str = method.as_str().to_string(); // デバッグログ用に文字列として保存

    // CouchDBのステータスをチェック
    let couchdb_status = state.health_state.couchdb_status.read().await.clone();
    if !couchdb_status.available {
        error!(
            "CouchDB connection is not available: {:?}",
            couchdb_status.error_message
        );
        return Response::builder()
            .status(StatusCode::SERVICE_UNAVAILABLE)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(format!(
                "{{\"error\":\"service_unavailable\",\"reason\":\"CouchDB connection is not available: {}\"}}", 
                couchdb_status.error_message.unwrap_or_else(|| "Unknown error".to_string())
            )))
            .unwrap();
    }

    // CouchDBサービスのURLを取得
    let couchdb_url = state.livesync_service.get_couchdb_url();
    let couchdb_auth = state.livesync_service.get_couchdb_auth();

    // CouchDBのURLを構築
    let db_path = path.replace("/db", "");
    let target_uri = match query {
        Some(q) => format!("{}{}", couchdb_url, db_path + "?" + q),
        None => format!("{}{}", couchdb_url, db_path),
    };

    debug!("Proxying {} request to CouchDB: {}", method_str, target_uri);

    // リクエストのURIとホストヘッダーを書き換え
    *req.uri_mut() = Uri::try_from(&target_uri).unwrap_or_else(|_| {
        error!("Failed to parse target URI: {}", target_uri);
        Uri::from_static("http://localhost")
    });

    // ホストヘッダーを更新
    if let Some(host) = req.uri().authority().map(|a| a.as_str().to_string()) {
        if let Ok(host_value) = header::HeaderValue::from_str(&host) {
            req.headers_mut().insert(header::HOST, host_value);
        }
    } else if let Ok(host_value) = header::HeaderValue::from_str(
        target_uri
            .replace("http://", "")
            .replace("https://", "")
            .split('/')
            .next()
            .unwrap_or("localhost"),
    ) {
        req.headers_mut().insert(header::HOST, host_value);
    }

    // 認証情報があれば追加
    if let Some((username, password)) = couchdb_auth {
        let auth_value = format!(
            "Basic {}",
            general_purpose::STANDARD.encode(format!("{}:{}", username, password))
        );
        if let Ok(auth_header) = header::HeaderValue::from_str(&auth_value) {
            req.headers_mut().insert(header::AUTHORIZATION, auth_header);
        }
    }

    // HTTPSクライアントを作成
    let https = HttpsConnector::new();
    let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build(https);

    // リクエスト本文をストリームとして取得しhyperリクエストに変換
    let (parts, body) = req.into_parts();
    let hyper_req = Request::from_parts(parts, body);

    // リクエストを送信
    match client.request(hyper_req).await {
        Ok(res) => {
            // レスポンスを返す
            let duration = start.elapsed();
            debug!(
                "Proxied {} request to path: {}, took: {:?}, status: {}",
                method_str,
                path,
                duration,
                res.status()
            );

            // Hyper ResponseをAxum Responseに変換
            let (parts, body) = res.into_parts();

            // 応答ヘッダーとステータスコードを設定
            let mut response_builder = Response::builder().status(parts.status);

            // ヘッダーをコピー
            for (key, value) in parts.headers.iter() {
                response_builder = response_builder.header(key.as_str(), value);
            }

            // レスポンスボディを変換して返す
            response_builder
                .body(Body::from_stream(BodyStream {
                    inner: body.into_data_stream(),
                }))
                .unwrap_or_else(|_| {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to build response"))
                        .unwrap()
                })
        }
        Err(e) => {
            error!("Failed to forward request to CouchDB: {}", e);
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
