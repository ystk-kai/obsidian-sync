use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use livesync_proxy::application::services::LiveSyncService;
use livesync_proxy::infrastructure::config::AppConfig;
use livesync_proxy::infrastructure::couchdb::CouchDbClient;
use livesync_proxy::interfaces::web::health::HealthState;
use livesync_proxy::interfaces::web::server::start_web_server;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting LiveSync proxy server");

    // Load configuration
    let config = AppConfig::from_env();
    info!("Configuration loaded");

    // Initialize CouchDB client
    let couchdb_client = Arc::new(CouchDbClient::new(
        &config.couchdb.url,
        &config.couchdb.username,
        &config.couchdb.password,
    ));
    info!("CouchDB client initialized at {}", config.couchdb.url);

    // Initialize LiveSync service with just the CouchDB client
    let livesync_service = Arc::new(LiveSyncService::new(couchdb_client.clone()));
    info!("LiveSync service initialized");

    // ヘルスステートの作成
    let health_state = Arc::new(HealthState::new());

    // CouchDBの健全性をチェック
    let couchdb_check_result = couchdb_client.ping().await;
    match &couchdb_check_result {
        Ok(_) => {
            info!("CouchDB connection verified");
            health_state.update_couchdb_status(true, None).await;
        }
        Err(e) => {
            info!("CouchDB connection check failed: {}", e);
            health_state
                .update_couchdb_status(false, Some(format!("接続エラー: {}", e)))
                .await;
        }
    }

    // CouchDB接続を定期的にチェックするバックグラウンドタスクを開始
    let check_client = couchdb_client.clone();
    let check_health = health_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            match check_client.ping().await {
                Ok(_) => {
                    check_health.update_couchdb_status(true, None).await;
                }
                Err(e) => {
                    check_health
                        .update_couchdb_status(false, Some(format!("接続エラー: {}", e)))
                        .await;
                }
            }
        }
    });

    // Start web server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Starting web server on {}", addr);

    // Start web server with updated service and health state
    start_web_server(addr, livesync_service, health_state).await?;

    info!("Server shutdown gracefully");
    Ok(())
}
