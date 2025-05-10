use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::{any, get},
    Json, Router,
};
use serde_json::Value;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::info;

use super::handlers::http_proxy_handler;
use crate::application::services::LiveSyncService;
use crate::interfaces::web::health::{create_health_router, HealthState};
use crate::interfaces::web::metrics::{create_metrics_router, MetricsState};
use crate::interfaces::web::setup::setup_uri_handler;

/// アプリケーションの状態を管理する構造体
pub struct AppState {
    pub livesync_service: Arc<LiveSyncService>,
    pub health_state: Arc<HealthState>,
    pub metrics_state: Arc<MetricsState>,
}

impl AppState {
    pub fn new(service: Arc<LiveSyncService>, health_state: Arc<HealthState>) -> Self {
        Self {
            livesync_service: service,
            health_state,
            metrics_state: Arc::new(MetricsState::new()),
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

    // ルーターの設定
    let health_router = create_health_router(health_state);
    let metrics_router = create_metrics_router(app_state.metrics_state.clone());

    // 静的ファイルサービスを設定（ベースディレクトリを指定）
    let static_dir = "/app/static";
    let index_path = format!("{}/index.html", static_dir);

    info!(
        "Serving static files from {} and index.html from {}",
        static_dir, index_path
    );

    let static_service = ServeDir::new(static_dir);

    let app = Router::new()
        // APIエンドポイント
        .route("/api/status", get(status_handler))
        .route("/api/setup", get(setup_uri_handler))
        .route("/debug", get(debug_handler))
        // ヘルスチェックとメトリクスのルーターを追加
        .merge(health_router)
        .merge(metrics_router)
        // /db のすべてのパスを同じハンドラで処理
        .route("/db", any(http_proxy_handler))
        .route("/db/:path", any(http_proxy_handler))
        .route("/db/{path}", any(http_proxy_handler))
        // 静的ファイルを提供する
        .nest_service("/static", static_service.clone())
        // ルートパスはindex.htmlを提供
        .route_service("/", ServeFile::new(index_path))
        // その他のパスは404を返す
        .fallback(fallback_handler)
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

/// サーバーのステータス情報を返すハンドラー
async fn status_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
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
async fn debug_handler() -> impl IntoResponse {
    info!("Debug handler called");
    Json(serde_json::json!({
        "status": "ok",
        "message": "Debug endpoint is working"
    }))
}

/// フォールバックハンドラー
async fn fallback_handler() -> impl IntoResponse {
    info!("404 Not Found");
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("404 - Not Found"))
        .unwrap()
}
