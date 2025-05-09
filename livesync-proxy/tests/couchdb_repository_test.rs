use async_trait::async_trait;
use livesync_proxy::domain::models::{CouchDbDocument, DomainError};
use livesync_proxy::domain::services::CouchDbRepository;
use mockall::mock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

// モックCouchDBリポジトリの作成
mock! {
    pub CouchDbMock {}

    #[async_trait]
    impl CouchDbRepository for CouchDbMock {
        async fn get_document(&self, db_name: &str, doc_id: &str) -> Result<CouchDbDocument, DomainError>;
        async fn save_document(&self, db_name: &str, doc: CouchDbDocument) -> Result<CouchDbDocument, DomainError>;
        async fn delete_document(&self, db_name: &str, doc_id: &str, rev: &str) -> Result<(), DomainError>;
        async fn query_view(&self, db_name: &str, design_doc: &str, view_name: &str, options: Value)
            -> Result<Vec<CouchDbDocument>, DomainError>;
        async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError>;
        async fn replicate(&self, source: &str, target: &str, options: Value) -> Result<Value, DomainError>;
    }
}

// インメモリCouchDBリポジトリの実装
struct InMemoryCouchDb {
    databases: Mutex<HashMap<String, HashMap<String, CouchDbDocument>>>,
}

impl InMemoryCouchDb {
    fn new() -> Self {
        Self {
            databases: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl CouchDbRepository for InMemoryCouchDb {
    async fn get_document(
        &self,
        db_name: &str,
        doc_id: &str,
    ) -> Result<CouchDbDocument, DomainError> {
        let databases = self.databases.lock().unwrap();

        if let Some(db) = databases.get(db_name) {
            if let Some(doc) = db.get(doc_id) {
                return Ok(doc.clone());
            }
        }

        Err(DomainError::CouchDbError(format!(
            "Document {} not found in database {}",
            doc_id, db_name
        )))
    }

    async fn save_document(
        &self,
        db_name: &str,
        doc: CouchDbDocument,
    ) -> Result<CouchDbDocument, DomainError> {
        let mut databases = self.databases.lock().unwrap();

        // データベースが存在しない場合は作成
        let db = databases
            .entry(db_name.to_string())
            .or_insert_with(HashMap::new);

        // ドキュメントのIDが空の場合はUUIDを生成
        let id = if doc.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            doc.id.clone()
        };

        // リビジョンの生成
        let rev = format!("1-{}", uuid::Uuid::new_v4().to_string());

        // 新しいドキュメントを作成
        let mut new_doc = doc.clone();
        new_doc.id = id;
        new_doc.rev = Some(rev);

        // ドキュメントを保存
        db.insert(new_doc.id.clone(), new_doc.clone());

        Ok(new_doc)
    }

    async fn delete_document(
        &self,
        db_name: &str,
        doc_id: &str,
        _rev: &str,
    ) -> Result<(), DomainError> {
        let mut databases = self.databases.lock().unwrap();

        if let Some(db) = databases.get_mut(db_name) {
            if db.remove(doc_id).is_some() {
                return Ok(());
            }
        }

        Err(DomainError::CouchDbError(format!(
            "Document {} not found in database {}",
            doc_id, db_name
        )))
    }

    async fn query_view(
        &self,
        db_name: &str,
        _design_doc: &str,
        _view_name: &str,
        _options: Value,
    ) -> Result<Vec<CouchDbDocument>, DomainError> {
        let databases = self.databases.lock().unwrap();

        if let Some(db) = databases.get(db_name) {
            let docs: Vec<CouchDbDocument> = db.values().cloned().collect();
            return Ok(docs);
        }

        Ok(vec![])
    }

    async fn ensure_database(&self, db_name: &str) -> Result<(), DomainError> {
        let mut databases = self.databases.lock().unwrap();

        databases
            .entry(db_name.to_string())
            .or_insert_with(HashMap::new);

        Ok(())
    }

    async fn replicate(
        &self,
        source: &str,
        target: &str,
        _options: Value,
    ) -> Result<Value, DomainError> {
        let mut databases = self.databases.lock().unwrap();

        // ソースデータベースからドキュメントを取得し、すべてコピーする
        let docs_count = if let Some(db) = databases.get(source) {
            let source_docs: Vec<CouchDbDocument> = db.values().cloned().collect();
            let count = source_docs.len();

            // ターゲットデータベースを取得または作成
            let target_db = databases
                .entry(target.to_string())
                .or_insert_with(HashMap::new);

            // ドキュメントをコピー
            for doc in source_docs {
                target_db.insert(doc.id.clone(), doc);
            }

            count
        } else {
            return Err(DomainError::CouchDbError(format!(
                "Source database {} not found",
                source
            )));
        };

        Ok(serde_json::json!({
            "ok": true,
            "docs_read": docs_count,
            "docs_written": docs_count,
            "docs_failed": 0
        }))
    }
}

#[tokio::test]
async fn test_save_and_get_document() {
    // インメモリCouchDBリポジトリを作成
    let repo = Arc::new(InMemoryCouchDb::new());

    // テスト用のドキュメントを作成
    let doc = CouchDbDocument {
        id: "test-doc".to_string(),
        rev: None,
        data: serde_json::json!({
            "name": "Test Document",
            "content": "This is a test"
        }),
    };

    // ドキュメントを保存
    let saved_doc = repo.save_document("test-db", doc).await.unwrap();

    // ドキュメントを取得して検証
    let retrieved_doc = repo.get_document("test-db", &saved_doc.id).await.unwrap();

    assert_eq!(saved_doc.id, retrieved_doc.id);
    assert_eq!(saved_doc.rev, retrieved_doc.rev);
    assert_eq!(saved_doc.data, retrieved_doc.data);
}

#[tokio::test]
async fn test_delete_document() {
    // インメモリCouchDBリポジトリを作成
    let repo = Arc::new(InMemoryCouchDb::new());

    // テスト用のドキュメントを作成して保存
    let doc = CouchDbDocument {
        id: "test-doc".to_string(),
        rev: None,
        data: serde_json::json!({"name": "Test Document"}),
    };

    let saved_doc = repo.save_document("test-db", doc).await.unwrap();

    // ドキュメントが存在することを確認
    let _ = repo.get_document("test-db", &saved_doc.id).await.unwrap();

    // ドキュメントを削除
    repo.delete_document("test-db", &saved_doc.id, saved_doc.rev.as_ref().unwrap())
        .await
        .unwrap();

    // ドキュメントが削除されたことを確認
    let result = repo.get_document("test-db", &saved_doc.id).await;
    assert!(result.is_err());
}
