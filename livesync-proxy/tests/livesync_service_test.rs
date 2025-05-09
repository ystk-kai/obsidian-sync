use async_trait::async_trait;
use livesync_proxy::application::services::LiveSyncService;
use livesync_proxy::domain::models::{CouchDbDocument, DomainError, LiveSyncMessage, MessageType};
use livesync_proxy::domain::services::{CouchDbRepository, MessageBroker};
use mockall::mock;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

// Create mocks for the traits
mock! {
    pub CouchDbRepo {}

    #[async_trait]
    impl CouchDbRepository for CouchDbRepo {
        async fn get_document(&self, db_name: &str, doc_id: &str) -> Result<CouchDbDocument, DomainError>;
        async fn save_document(&self, db_name: &str, doc: CouchDbDocument) -> Result<CouchDbDocument, DomainError>;
        async fn delete_document(&self, db_name: &str, doc_id: &str, rev: &str) -> Result<(), DomainError>;
        async fn query_view(&self, db_name: &str, design_doc: &str, view_name: &str, options: Value)
            -> Result<Vec<CouchDbDocument>, DomainError>;
        async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError>;
        async fn replicate(&self, source: &str, target: &str, options: Value) -> Result<Value, DomainError>;
    }
}

mock! {
    pub MessageBrokerMock {}

    #[async_trait]
    impl MessageBroker for MessageBrokerMock {
        async fn send_message(&self, client_id: &str, message: Value) -> Result<(), DomainError>;
        async fn broadcast_message(&self, message: Value) -> Result<(), DomainError>;
        async fn register_client(&self, client_id: &str) -> Result<(), DomainError>;
        async fn unregister_client(&self, client_id: &str) -> Result<(), DomainError>;
    }
}

// テスト用のヘルパー関数
fn create_test_message(message_type: MessageType, payload: Value) -> LiveSyncMessage {
    LiveSyncMessage {
        id: Uuid::new_v4(),
        message_type,
        payload,
    }
}

#[tokio::test]
async fn test_handle_connection() {
    // モックの作成
    let mock_couchdb = MockCouchDbRepo::new();
    let mut mock_broker = MockMessageBrokerMock::new();

    // モックの期待値を設定
    mock_broker
        .expect_register_client()
        .with(mockall::predicate::eq("test-client"))
        .times(1)
        .returning(|_| Ok(()));

    mock_broker
        .expect_send_message()
        .withf(|client_id, _| client_id == "test-client")
        .times(1)
        .returning(|_, _| Ok(()));

    // LiveSyncサービスを作成
    let service = LiveSyncService::new(Arc::new(mock_couchdb), Arc::new(mock_broker));

    // 接続メッセージを作成
    let message = create_test_message(
        MessageType::Connection,
        serde_json::json!({"client": "test-client"}),
    );

    // メッセージを処理
    let result = service.handle_message("test-client", message).await;

    // 結果を検証
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_sync() {
    // モックの作成
    let mut mock_couchdb = MockCouchDbRepo::new();
    let mut mock_broker = MockMessageBrokerMock::new();

    // テスト用ドキュメント
    let _doc = CouchDbDocument {
        id: "test-id".to_string(),
        rev: None,
        data: serde_json::json!({"content": "test content"}),
    };

    let saved_doc = CouchDbDocument {
        id: "test-id".to_string(),
        rev: Some("1-abc123".to_string()),
        data: serde_json::json!({"content": "test content"}),
    };

    // モックの期待値を設定
    mock_couchdb
        .expect_ensure_database()
        .with(mockall::predicate::eq("test-db"))
        .times(1)
        .returning(|_| Ok(()));

    mock_couchdb
        .expect_save_document()
        .withf(|db_name, doc| {
            db_name == "test-db" && doc.id == "test-id" && doc.data.get("content").is_some()
        })
        .times(1)
        .returning(move |_, _| Ok(saved_doc.clone()));

    mock_broker
        .expect_send_message()
        .withf(|client_id, _| client_id == "test-client")
        .times(1)
        .returning(|_, _| Ok(()));

    // LiveSyncサービスを作成
    let service = LiveSyncService::new(Arc::new(mock_couchdb), Arc::new(mock_broker));

    // 同期メッセージを作成
    let message = LiveSyncMessage {
        id: Uuid::new_v4(),
        message_type: MessageType::Sync,
        payload: serde_json::json!({
            "database": "test-db",
            "document": {
                "_id": "test-id",
                "_rev": null,
                "content": "test content"
            }
        }),
    };

    // メッセージを処理
    let result = service.handle_message("test-client", message).await;

    // エラーの場合はエラー内容を表示
    if let Err(ref e) = result {
        eprintln!("Error in handle_sync test: {:?}", e);
    }

    // 結果を検証
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_replication() {
    // モックの作成
    let mut mock_couchdb = MockCouchDbRepo::new();
    let mut mock_broker = MockMessageBrokerMock::new();

    // レプリケーション結果
    let replication_result = serde_json::json!({
        "ok": true,
        "docs_read": 10,
        "docs_written": 10,
        "docs_failed": 0
    });

    // モックの期待値を設定
    mock_couchdb
        .expect_replicate()
        .withf(|source, target, _| source == "source-db" && target == "target-db")
        .times(1)
        .returning(move |_, _, _| Ok(replication_result.clone()));

    mock_broker
        .expect_send_message()
        .withf(|client_id, _| client_id == "test-client")
        .times(1)
        .returning(|_, _| Ok(()));

    // LiveSyncサービスを作成
    let service = LiveSyncService::new(Arc::new(mock_couchdb), Arc::new(mock_broker));

    // レプリケーションメッセージを作成
    let message = create_test_message(
        MessageType::Replicate,
        serde_json::json!({
            "source": "source-db",
            "target": "target-db",
            "options": {}
        }),
    );

    // メッセージを処理
    let result = service.handle_message("test-client", message).await;

    // 結果を検証
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handle_disconnection() {
    // モックの作成
    let mock_couchdb = MockCouchDbRepo::new();
    let mut mock_broker = MockMessageBrokerMock::new();

    // モックの期待値を設定
    mock_broker
        .expect_unregister_client()
        .with(mockall::predicate::eq("test-client"))
        .times(1)
        .returning(|_| Ok(()));

    // LiveSyncサービスを作成
    let service = LiveSyncService::new(Arc::new(mock_couchdb), Arc::new(mock_broker));

    // 切断処理を実行
    let result = service.handle_disconnection("test-client").await;

    // 成功することを確認
    assert!(result.is_ok());
}
