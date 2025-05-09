use livesync_proxy::domain::models::DomainError;
use livesync_proxy::domain::services::MessageBroker;
use livesync_proxy::infrastructure::websocket::WebSocketBroker;
use std::sync::Arc;

#[tokio::test]
#[ignore = "This test needs a proper WebSocket setup to receive messages"]
async fn test_websocket_broker() {
    // WebSocketブローカーを作成
    let broker = Arc::new(WebSocketBroker::new(100));

    // クライアントを登録
    broker.register_client("test-client-1").await.unwrap();
    broker.register_client("test-client-2").await.unwrap();

    // test-client-1へのメッセージ送信
    let message = serde_json::json!({
        "type": "test",
        "content": "Hello from test"
    });

    // 存在するクライアントへの送信が成功することを確認
    let result = broker.send_message("test-client-1", message.clone()).await;
    assert!(result.is_ok());

    // 存在しないクライアントへの送信が失敗することを確認
    let result = broker
        .send_message("non-existent-client", message.clone())
        .await;
    assert!(result.is_err());

    // クライアントの登録解除
    broker.unregister_client("test-client-1").await.unwrap();

    // 登録解除されたクライアントへの送信が失敗することを確認
    let result = broker.send_message("test-client-1", message.clone()).await;
    assert!(result.is_err());

    // 存在するクライアントへの送信が引き続き成功することを確認
    let result = broker.send_message("test-client-2", message.clone()).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_register_duplicate_client() {
    // WebSocketブローカーを作成
    let broker = Arc::new(WebSocketBroker::new(100));

    // クライアントを登録
    broker.register_client("test-client").await.unwrap();

    // 同じクライアントIDでの再登録が失敗することを確認
    let result = broker.register_client("test-client").await;
    assert!(result.is_err());

    // エラーが正しい型であることを確認
    if let Err(DomainError::WebSocketError(error_msg)) = result {
        assert!(error_msg.contains("already registered"));
    } else {
        panic!("Expected WebSocketError but got different error or success");
    }
}

#[tokio::test]
async fn test_unregister_nonexistent_client() {
    // WebSocketブローカーを作成
    let broker = Arc::new(WebSocketBroker::new(100));

    // 存在しないクライアントの登録解除が失敗することを確認
    let result = broker.unregister_client("non-existent-client").await;
    assert!(result.is_err());

    // エラーが正しい型であることを確認
    if let Err(DomainError::WebSocketError(error_msg)) = result {
        assert!(error_msg.contains("not found"));
    } else {
        panic!("Expected WebSocketError but got different error or success");
    }
}
