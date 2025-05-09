use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tracing::{debug, info};
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
    info!("Loaded configuration: {:#?}", config);

    // CouchDBクライアントの作成
    let couchdb_client = CouchDbClient::new(
        &config.couchdb.url,
        &config.couchdb.username,
        &config.couchdb.password,
    );

    // Test connection but continue even if it fails
    info!("Testing connection to CouchDB at {}", config.couchdb.url);
    let couchdb_available = match couchdb_client.ping().await {
        Ok(_) => {
            info!("Successfully connected to CouchDB");
            true
        }
        Err(e) => {
            info!(
                "Failed to connect to CouchDB: {} - will continue anyway for development",
                e
            );
            false
        }
    };

    // Create application service
    let livesync_service = Arc::new(LiveSyncService::new(Arc::new(couchdb_client)));
    debug!("Created LiveSync service");

    // Get and log CouchDB URL and auth for verification
    let service_couchdb_url = livesync_service.get_couchdb_url();
    let service_couchdb_auth = livesync_service.get_couchdb_auth();
    info!("LiveSync service CouchDB URL: {}", service_couchdb_url);
    info!(
        "LiveSync service CouchDB auth available: {}",
        service_couchdb_auth.is_some()
    );

    // Create health check state
    let health_state = Arc::new(HealthState::new(
        Arc::clone(&livesync_service),
        Duration::from_secs(30), // 30秒間隔でヘルスチェック
    ));

    // 初期状態を設定（開発モードでは接続が失敗してもサーバーが起動するように）
    if couchdb_available {
        health_state.update_couchdb_status(true, None).await;
    } else {
        health_state
            .update_couchdb_status(false, Some("Initial connection failed".to_string()))
            .await;
    }

    debug!("Created health check state");

    // Start health check background task
    health_state.start_background_health_check();
    debug!("Started background health check");

    // サーバーアドレスの設定
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("Starting server on {}", addr);

    // 実際のサーバーを起動
    start_web_server(addr, livesync_service, health_state).await?;

    info!("Server shutdown gracefully");
    Ok(())
}
