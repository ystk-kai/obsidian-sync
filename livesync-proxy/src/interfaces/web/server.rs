use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{debug, error, info};

use super::handlers::{debug_handler, http_proxy_handler, status_handler};
use super::setup::setup_uri_handler;
use crate::application::services::LiveSyncService;
use crate::interfaces::web::health::HealthState;
use crate::interfaces::web::metrics::MetricsState;

/// アプリケーションの状態を管理する構造体
pub struct AppState {
    pub livesync_service: Arc<LiveSyncService>,
    pub health_state: Arc<HealthState>,
    pub metrics_state: Arc<MetricsState>,
    pub static_dir: String,
}

impl AppState {
    pub fn new(service: Arc<LiveSyncService>, health_state: Arc<HealthState>) -> Self {
        Self {
            livesync_service: service,
            health_state,
            metrics_state: Arc::new(MetricsState::new()),
            static_dir: "/app/static".to_string(),
        }
    }
}

/// Webサーバーを起動する関数
pub async fn start_web_server(
    addr: SocketAddr,
    service: Arc<LiveSyncService>,
    health_state: Arc<HealthState>,
) -> Result<()> {
    // アプリケーション状態の作成
    let app_state = Arc::new(AppState::new(service, health_state.clone()));

    info!("Serving static files from {}", app_state.static_dir);

    // 静的ファイルハンドリング
    // ServeDir サービスを使用
    let static_service = ServeDir::new(&app_state.static_dir);

    // 許可するオリジンの明示的なリスト
    let allowed_origins = AllowOrigin::list([
        "app://obsidian.md".parse().unwrap(),
        "capacitor://localhost".parse().unwrap(),
        "http://localhost".parse().unwrap(),
    ]);

    // 許可するメソッドの明示的なリスト
    let allowed_methods = vec![
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::DELETE,
        Method::HEAD,
        Method::OPTIONS,
    ];

    // 許可するヘッダーの明示的なリスト - CORSの制約に対応するため
    let allowed_headers = vec![
        HeaderName::from_static("accept"),
        HeaderName::from_static("authorization"),
        HeaderName::from_static("content-type"),
        HeaderName::from_static("origin"),
        HeaderName::from_static("referer"),
        HeaderName::from_static("x-csrf-token"),
        HeaderName::from_static("if-match"),
        HeaderName::from_static("destination"),
        HeaderName::from_static("x-requested-with"),
        HeaderName::from_static("x-pouchdb-read-quorum"),
        HeaderName::from_static("x-pouchdb-write-quorum"),
        HeaderName::from_static("content-length"),
        HeaderName::from_static("cache-control"),
        HeaderName::from_static("pragma"),
    ];

    // 公開するレスポンスヘッダーのリスト
    let expose_headers = vec![
        HeaderName::from_static("content-type"),
        HeaderName::from_static("cache-control"),
        HeaderName::from_static("accept-ranges"),
        HeaderName::from_static("etag"),
        HeaderName::from_static("server"),
        HeaderName::from_static("x-couch-request-id"),
        HeaderName::from_static("x-couch-update-newrev"),
        HeaderName::from_static("x-couch-update-newseq"),
    ];

    // カスタムCORS設定 - credential=trueの場合はワイルドカードを使用不可
    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods(allowed_methods)
        .allow_headers(allowed_headers)
        .expose_headers(expose_headers)
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600));

    info!(
        "Serving static files from {} and index.html from {}/index.html",
        app_state.static_dir, app_state.static_dir
    );

    // すべてのルートを直接定義したルーター
    let app = Router::new()
        // APIエンドポイント
        .route("/api/status", get(status_handler))
        .route("/api/setup", get(setup_uri_handler))
        .route("/debug", get(debug_handler))
        // ヘルスチェック
        .route(
            "/health",
            get(super::health::health_handler).with_state(health_state),
        )
        // メトリクス
        .route(
            "/metrics",
            get(super::metrics::metrics_handler).with_state(app_state.metrics_state.clone()),
        )
        // 静的ファイル
        .nest_service("/static", static_service)
        // CouchDBプロキシエンドポイント - すべてのパターンを明示的に定義
        .route("/db", any(db_proxy_handler))
        .route("/db/", any(db_proxy_handler))
        .route("/db/{*path}", any(db_proxy_handler))
        // ルートパス
        .route("/", get(index_handler))
        // フォールバック
        .fallback(fallback_handler)
        // ミドルウェア
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(app_state);

    // サーバーの起動
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;

    info!("Server shutdown gracefully");
    Ok(())
}

/// DBプロキシハンドラーラッパー - パスの確実なマッピングを行う
async fn db_proxy_handler(
    state: State<Arc<AppState>>,
    req: axum::http::Request<Body>,
) -> impl IntoResponse {
    // デバッグ用にリクエスト情報を出力
    let _uri = req.uri().to_string();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let query = req.uri().query();

    info!("DB Proxy handling: {} {}", method, path);

    // _changesエンドポイントのlongpoll検出
    let is_longpoll =
        path.contains("/_changes") && query.is_some_and(|q| q.contains("feed=longpoll"));

    // bulk_docsリクエストの検出（大きなデータ転送が予想される）
    let is_bulk_docs = path.contains("/_bulk_docs");

    if is_longpoll {
        info!(
            "Detected _changes longpoll request: {} {} with query: {:?}",
            method, path, query
        );
    }

    if is_bulk_docs {
        info!(
            "Detected _bulk_docs request: {} {} - expecting larger payload",
            method, path
        );
    }

    // リクエストタイプに基づいて適切なバッファサイズを選択
    let buffer_size = match (is_longpoll, is_bulk_docs) {
        (true, _) => 1024 * 1024,      // longpoll: 1MB
        (_, true) => 20 * 1024 * 1024, // bulk_docs: 20MB
        _ => 5 * 1024 * 1024,          // その他: 5MB
    };

    info!(
        "Using buffer size of {} bytes for {} {}",
        buffer_size, method, path
    );

    // リクエストをハンドラに渡す
    let orig_response = http_proxy_handler(state, req).await;

    // 詳細なロギングのためにレスポンスを展開
    let (parts, body) = orig_response.into_response().into_parts();
    let status = parts.status;
    let headers = parts.headers;

    info!("DB Proxy got initial response with status: {}", status);
    debug!("Response headers before processing: {:?}", headers);

    // longpollリクエストの場合は特別な処理（AbortErrorが発生しやすい）
    if is_longpoll && status == StatusCode::NO_CONTENT {
        info!("Returning early for longpoll request with 204 status");
        return Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"results":[],"last_seq":"0"}"#))
            .unwrap()
            .into_response();
    }

    match to_bytes(body, buffer_size).await {
        // 10MB制限
        Ok(bytes) => {
            info!("Successfully buffered response body: {} bytes", bytes.len());
            if bytes.len() < 1000 {
                // 小さいレスポンスはデバッグのために表示
                debug!("Response body content: {}", String::from_utf8_lossy(&bytes));
            }

            // マニュアルでレスポンスを構築 - HTTP/1.1互換の方法で
            let mut response_headers = HeaderMap::new();

            // 必要なヘッダーを転送（transfer-encodingを除く）
            for (key, value) in headers.iter() {
                if key.as_str().to_lowercase() != "transfer-encoding" {
                    response_headers.insert(key.clone(), value.clone());
                }
            }

            // content-lengthを設定して、chunkedエンコーディングを確実に防ぐ
            response_headers.insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&bytes.len().to_string()).unwrap(),
            );

            // content-typeヘッダーが確実に設定されるようにする
            if !response_headers.contains_key(header::CONTENT_TYPE) {
                response_headers.insert(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                );
            }

            info!("Built final response headers: {:?}", response_headers);

            // 新しいレスポンスを構築
            let mut http_response = Response::builder().status(status);

            // ヘッダーを設定
            for (name, value) in response_headers.iter() {
                http_response = http_response.header(name, value);
            }

            match http_response.body(Body::from(bytes)) {
                Ok(response) => {
                    info!("Successfully built final response");
                    response
                }
                Err(e) => {
                    error!("Failed to build response: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from("Failed to build response"))
                        .unwrap()
                }
            }
        }
        Err(e) => {
            error!("Failed to buffer response body: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Failed to process response: {}", e)))
                .unwrap()
        }
    }
}

/// インデックスページを提供するハンドラー
async fn index_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let index_path = format!("{}/index.html", state.static_dir);
    serve_file(index_path).await
}

/// ファイルを提供する共通関数
async fn serve_file(path: String) -> impl IntoResponse {
    match tokio::fs::read(&path).await {
        Ok(content) => {
            // MIME型を推測する
            let content_type = match Path::new(&path).extension().and_then(|ext| ext.to_str()) {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                Some("json") => "application/json",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(content))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("File not found: {}", path)))
            .unwrap(),
    }
}

/// フォールバックハンドラー
async fn fallback_handler(uri: Uri) -> impl IntoResponse {
    info!("404 Not Found: {}", uri);
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from(format!("404 - Not Found: {}", uri)))
        .unwrap()
}
