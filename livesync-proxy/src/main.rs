use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use livesync_proxy::application::services::LiveSyncService;
use livesync_proxy::infrastructure::config::AppConfig;
use livesync_proxy::infrastructure::couchdb::CouchDbClient;
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

    // CouchDBの健全性をチェック
    match couchdb_client.ping().await {
        Ok(_) => info!("CouchDB connection verified"),
        Err(e) => info!("CouchDB connection check failed: {}", e),
    }

    // Start web server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    info!("Starting web server on {}", addr);

    // Start web server with updated service
    start_web_server(addr, livesync_service).await?;

    info!("Server shutdown gracefully");
    Ok(())
}
