use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{ws::{Message, WebSocket}, WebSocketUpgrade, State},
    response::IntoResponse,
};
use futures_util::StreamExt;
use uuid::Uuid;
use tracing::{info, error, debug};

use crate::{
    domain::models::{LiveSyncMessage, MessageType},
    interfaces::web::server::AppState,
};

/// WebSocket接続をアップグレードし、LiveSyncの接続を処理するハンドラー
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // 処理開始時間を記録
    let start = Instant::now();
    
    // Clone state to measure metrics before it moves into the closure
    let metrics_state = state.metrics_state.clone();
    
    // 処理完了時に時間を計測するようにする
    let response = ws.on_upgrade(move |socket| {
        handle_socket(socket, state)
    });
    
    // メトリクスに記録
    metrics_state.record_request_duration("/db", "GET", start);
    
    response
}

/// WebSocket接続を処理する関数
async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    // クライアントIDを生成
    let client_id = Uuid::new_v4().to_string();
    info!("New WebSocket connection established: {}", client_id);

    // ソケットを送信/受信に分離
    let (_sender, mut receiver) = socket.split();

    // WebSocket接続カウンターを更新
    let connection_count = 1; // TODO: 実際の接続数を取得
    state.metrics_state.update_websocket_connections(connection_count);

    // 接続メッセージを送信
    let connect_msg = LiveSyncMessage {
        id: Uuid::new_v4(),
        message_type: MessageType::Connection,
        payload: serde_json::json!({
            "status": "connected",
            "client_id": client_id,
        }),
    };

    // サービスの接続ハンドラーを呼び出す
    if let Err(err) = state.livesync_service.handle_message(&client_id, connect_msg).await {
        error!("Failed to handle connection message: {:?}", err);
    }

    // メッセージの受信ループ
    while let Some(Ok(message)) = receiver.next().await {
        match message {
            Message::Text(text) => {
                debug!("Received text message: {}", text);
                let msg_start = Instant::now();
                
                match serde_json::from_str::<LiveSyncMessage>(&text) {
                    Ok(msg) => {
                        // メッセージタイプに応じてメトリクスを記録
                        match msg.message_type {
                            MessageType::Sync => {
                                let db_name = msg.payload.get("database")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                    
                                if let Err(err) = state.livesync_service.handle_message(&client_id, msg.clone()).await {
                                    error!("Failed to handle sync message: {:?}", err);
                                    state.metrics_state.record_document_sync(&db_name, false);
                                } else {
                                    state.metrics_state.record_document_sync(&db_name, true);
                                }
                            },
                            MessageType::Replicate => {
                                let source = msg.payload.get("source")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                    
                                let target = msg.payload.get("target")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                    
                                if let Err(err) = state.livesync_service.handle_message(&client_id, msg.clone()).await {
                                    error!("Failed to handle replication message: {:?}", err);
                                    state.metrics_state.record_replication(&source, &target, false);
                                } else {
                                    state.metrics_state.record_replication(&source, &target, true);
                                }
                            },
                            _ => {
                                if let Err(err) = state.livesync_service.handle_message(&client_id, msg.clone()).await {
                                    error!("Failed to handle message: {:?}", err);
                                }
                            }
                        }
                        
                        // メッセージ処理時間を記録
                        let msg_type = format!("{:?}", msg.message_type).to_lowercase();
                        state.metrics_state.record_request_duration(
                            &format!("/db/{}", msg_type), 
                            "WS", 
                            msg_start
                        );
                    }
                    Err(err) => {
                        error!("Failed to parse message: {:?}", err);
                    }
                }
            }
            Message::Binary(data) => {
                debug!("Received binary message: {} bytes", data.len());
                // バイナリメッセージの処理（必要に応じて）
            }
            Message::Close(_) => {
                info!("WebSocket connection closed: {}", client_id);
                break;
            }
            _ => {}
        }
    }

    // WebSocket接続カウンターを更新
    state.metrics_state.update_websocket_connections(connection_count - 1);

    // クライアントの切断を処理
    if let Err(err) = state.livesync_service.handle_disconnection(&client_id).await {
        error!("Failed to handle disconnection: {:?}", err);
    }
}
