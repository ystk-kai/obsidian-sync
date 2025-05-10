use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::{any, get},
    Router,
};
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::info;

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
        .layer(CorsLayer::permissive())
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
    let method = req.method().as_str();
    let path = req.uri().path();

    info!("DB Proxy handling: {} {}", method, path);

    // http_proxy_handlerを呼び出す
    http_proxy_handler(state, req).await
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
