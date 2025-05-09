use axum::{Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

// ヘルスチェックの状態
pub struct HealthState {
    pub start_time: SystemTime,
    pub couchdb_status: RwLock<CouchDbStatus>,
}

// CouchDBの状態
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouchDbStatus {
    pub available: bool,
    pub last_checked: SystemTime,
    pub error_message: Option<String>,
}

// ヘルスチェックのレスポンス
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub version: String,
    pub services: ServiceStatus,
}

// サービスの状態
#[derive(Debug, Serialize)]
pub struct ServiceStatus {
    pub couchdb: CouchDbStatus,
}

impl Default for HealthState {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthState {
    pub fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            couchdb_status: RwLock::new(CouchDbStatus {
                available: false,
                last_checked: SystemTime::now(),
                error_message: None,
            }),
        }
    }

    // CouchDBの状態を更新する
    pub async fn update_couchdb_status(&self, available: bool, error_message: Option<String>) {
        let mut status = self.couchdb_status.write().await;
        status.available = available;
        status.last_checked = SystemTime::now();
        status.error_message = error_message;
    }
}

// ヘルスチェックのハンドラー
pub async fn health_handler(State(state): State<Arc<HealthState>>) -> Json<HealthResponse> {
    let uptime = SystemTime::now()
        .duration_since(state.start_time)
        .unwrap_or_default()
        .as_secs();

    let couchdb_status = state.couchdb_status.read().await.clone();

    let status = if couchdb_status.available {
        "healthy"
    } else {
        "degraded"
    };

    Json(HealthResponse {
        status: status.to_string(),
        uptime_seconds: uptime,
        version: env!("CARGO_PKG_VERSION").to_string(),
        services: ServiceStatus {
            couchdb: couchdb_status,
        },
    })
}

// ヘルスチェックのルーターを作成
pub fn create_health_router<S>(state: Arc<HealthState>) -> Router<S> {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(state)
}
