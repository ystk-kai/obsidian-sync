use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{debug, error, info};
use url::Url;

use crate::interfaces::web::server::AppState;

/// Setup URIのパラメータ
#[derive(Debug, Deserialize)]
pub struct SetupUriQuery {
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
}

/// Setup URIのレスポンス形式
#[derive(Debug, Serialize)]
pub struct SetupUriResponse {
    pub username: String,
    pub password: String,
    pub remote_uri: String,
    pub setup_uri: String,
}

/// Setup URIハンドラー - ObsidianのLiveSyncプラグイン用のSetup URLを生成
pub async fn setup_uri_handler(
    Query(params): Query<SetupUriQuery>,
    State(state): State<Arc<AppState>>,
) -> Response<axum::body::Body> {
    debug!("Setup URI handler called");

    // CouchDBの認証情報を取得
    let auth_credentials = state.livesync_service.get_couchdb_auth();

    let (username, password) = match auth_credentials {
        Some((u, p)) => (u, p),
        None => {
            error!("No authentication credentials available for setup URI");
            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(axum::body::Body::from(
                    "Authentication credentials not available",
                ))
                .unwrap();
        }
    };

    // CouchDBのURLを取得
    let couchdb_url = state.livesync_service.get_couchdb_url();

    // リモートURIとして使用するCouchDBのURL
    let remote_uri = couchdb_url.clone();

    // 環境変数からデフォルトの外部ポートを取得（デフォルトは3000, .envのHOST_PROXY_PORTから）
    let default_external_port = env::var("HOST_PROXY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    // ホストとポートを取得
    let host = params.host.unwrap_or_else(|| "localhost".to_string());
    let port = params.port.unwrap_or(default_external_port);

    // Setup URIを作成 - これはObsidianのLiveSyncプラグインが理解できるフォーマット
    let setup_uri = format!("http://{}:{}@{}:{}/db", username, password, host, port);

    // 有効なURIかチェック
    if Url::parse(&setup_uri).is_err() {
        error!("Generated setup URI is invalid");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(axum::body::Body::from("Failed to generate valid setup URI"))
            .unwrap();
    }

    // レスポンスを構築
    let response = SetupUriResponse {
        username: username.clone(),
        password: password.clone(),
        remote_uri,
        setup_uri,
    };

    info!(
        "Setup URI generated successfully for host={}, port={}",
        host, port
    );

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&response).unwrap(),
        ))
        .unwrap()
}
