use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{any, get},
    Json, Router,
};
use serde_json::Value;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
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
    pub fn new(service: Arc<LiveSyncService>) -> Self {
        Self {
            livesync_service: service,
            health_state: Arc::new(HealthState::new()),
            metrics_state: Arc::new(MetricsState::new()),
        }
    }
}

/// Webサーバーを起動する関数
pub async fn start_web_server(addr: String, service: Arc<LiveSyncService>) -> Result<()> {
    // サーバーアドレスをパース
    let addr: SocketAddr = addr.parse()?;

    // アプリケーション状態の作成
    let app_state = Arc::new(AppState::new(service));

    // ルーターの設定
    let health_router = create_health_router(app_state.health_state.clone());
    let metrics_router = create_metrics_router(app_state.metrics_state.clone());

    let app = Router::new()
        .route("/db", any(http_proxy_handler))
        .route("/db/:path", any(http_proxy_handler))
        .route("/db/:path/*rest", any(http_proxy_handler))
        .route("/api/status", get(status_handler))
        .route("/api/setup", get(setup_uri_handler))
        .route("/debug", get(debug_handler))
        // ヘルスチェックとメトリクスのルーターを追加
        .merge(health_router)
        .merge(metrics_router)
        // 静的ファイルを提供するためのSPAルーター
        .nest_service("/static", ServeDir::new("static"))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    // サーバーの起動
    info!("Starting server on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
        .await
        .map_err(Into::into)
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
