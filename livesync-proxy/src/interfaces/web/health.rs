use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::application::services::LiveSyncService;
use crate::infrastructure::couchdb::CouchDbClient;

// ヘルスチェックの状態
pub struct HealthState {
    pub livesync_service: Arc<LiveSyncService>,
    pub start_time: SystemTime,
    pub couchdb_status: RwLock<CouchDbStatus>,
    pub check_interval: Duration,
    // バックオフ戦略のための状態追加
    consecutive_failures: AtomicU32,
    max_check_interval: Duration,
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
#[derive(Debug, Serialize, Clone)]
pub struct ServiceStatus {
    pub couchdb: CouchDbStatus,
}

impl HealthState {
    pub fn new(service: Arc<LiveSyncService>, check_interval: Duration) -> Self {
        Self {
            livesync_service: service,
            start_time: SystemTime::now(),
            couchdb_status: RwLock::new(CouchDbStatus {
                available: false,
                last_checked: SystemTime::now(),
                error_message: None,
            }),
            check_interval,
            // 初期値の設定
            consecutive_failures: AtomicU32::new(0),
            max_check_interval: Duration::from_secs(300), // 最大5分まで伸ばす
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

            // 初期間隔を設定
            let mut current_interval = health_state.check_interval;

            loop {
                tokio::time::sleep(current_interval).await;
                debug!("Performing CouchDB health check");

                let couchdb_url = health_state.livesync_service.get_couchdb_url();
                let couchdb_auth = health_state.livesync_service.get_couchdb_auth();

                if let Some((username, password)) = couchdb_auth {
                    let couchdb_client = CouchDbClient::new(&couchdb_url, &username, &password);

                    // タイムアウト付きPing
                    let ping_result = tokio::time::timeout(
                        Duration::from_secs(5), // 5秒タイムアウト
                        couchdb_client.ping(),
                    )
                    .await;

                    // エラーケースを適切に処理
                    match ping_result {
                        // 正常応答
                        Ok(Ok(_)) => {
                            // 成功したので連続失敗カウンターをリセット
                            health_state.consecutive_failures.store(0, Ordering::SeqCst);
                            // 通常の間隔に戻す
                            current_interval = health_state.check_interval;
                            health_state.update_couchdb_status(true, None).await;
                        }
                        // エラー（CouchDBエラーまたはタイムアウト）
                        _ => {
                            let error_msg = match &ping_result {
                                Ok(Err(e)) => format!("CouchDB connection error: {}", e),
                                Err(_) => "CouchDB connection timed out".to_string(),
                                _ => "Unknown error".to_string(),
                            };

                            // 連続失敗カウンターを増加
                            let failures = health_state
                                .consecutive_failures
                                .fetch_add(1, Ordering::SeqCst)
                                + 1;

                            // バックオフ戦略: 2^n秒（最大max_check_intervalまで）
                            let backoff_secs = std::cmp::min(
                                2u64.pow(failures),
                                health_state.max_check_interval.as_secs(),
                            );

                            warn!(
                                "CouchDB health check failed {} times in a row. Next check in {} seconds. Error: {}", 
                                failures, backoff_secs, error_msg
                            );

                            // 次回のチェック間隔を計算
                            current_interval = Duration::from_secs(backoff_secs);

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
