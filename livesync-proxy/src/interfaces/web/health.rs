use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::application::services::LiveSyncService;
use crate::infrastructure::couchdb::CouchDbClient;

// ヘルスチェックの状態
pub struct HealthState {
    pub start_time: SystemTime,
    pub couchdb_status: RwLock<CouchDbStatus>,
    livesync_service: Arc<LiveSyncService>,
    check_interval: Duration,
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

impl HealthState {
    pub fn new(livesync_service: Arc<LiveSyncService>, check_interval: Duration) -> Self {
        Self {
            start_time: SystemTime::now(),
            couchdb_status: RwLock::new(CouchDbStatus {
                available: false,
                last_checked: SystemTime::now(),
                error_message: None,
            }),
            livesync_service,
            check_interval,
        }
    }

    // CouchDBの状態を更新する
    pub async fn update_couchdb_status(&self, available: bool, error_message: Option<String>) {
        let mut status = self.couchdb_status.write().await;
        status.available = available;
        status.last_checked = SystemTime::now();
        let error_msg_copy = error_message.clone();
        status.error_message = error_message;

        if available {
            debug!("CouchDB connection is available");
        } else {
            error!("CouchDB connection is not available: {:?}", error_msg_copy);
        }
    }

    // バックグラウンドでヘルスチェックを開始する
    pub fn start_background_health_check(self: &Arc<Self>) {
        let health_state = Arc::clone(self);

        tokio::spawn(async move {
            info!(
                "Starting background health check with interval {:?}",
                health_state.check_interval
            );
            let mut interval = tokio::time::interval(health_state.check_interval);

            loop {
                interval.tick().await;
                debug!("Performing CouchDB health check");

                let couchdb_url = health_state.livesync_service.get_couchdb_url();
                let couchdb_auth = health_state.livesync_service.get_couchdb_auth();

                if let Some((username, password)) = couchdb_auth {
                    let couchdb_client = CouchDbClient::new(&couchdb_url, &username, &password);
                    match couchdb_client.ping().await {
                        Ok(_) => {
                            health_state.update_couchdb_status(true, None).await;
                        }
                        Err(e) => {
                            let error_msg = format!("CouchDB connection error: {}", e);
                            health_state
                                .update_couchdb_status(false, Some(error_msg))
                                .await;
                        }
                    }
                } else {
                    let error_msg = "No CouchDB authentication credentials available".to_string();
                    health_state
                        .update_couchdb_status(false, Some(error_msg))
                        .await;
                }
            }
        });
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
